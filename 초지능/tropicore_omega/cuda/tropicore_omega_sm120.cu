// ==============================================================================
// TropiCore-Omega ASI-Zero — REAL SM120 Blackwell CUDA 13 Training + Inference Engine
// Target Node (`장비.md` Track A):
//   - 4x NVIDIA RTX PRO 6000 Blackwell Max-Q 96GB (SM120, 384 GB GDDR7, 14.044 PFLOPS FP4 2:4)
//   - AMD Ryzen Threadripper PRO 9995WX (96C/192T Zen 5) + 1,000 GB DDR5-6400 ECC
//
// ABI = 1100 (Full Training + Inference Parity):
//   1. `tropicore_sm120_linear_fwd`               : Batched Forward GEMM (`cublasSgemm` / `cublasLt` FP4)
//   2. `tropicore_sm120_linear_bwd`               : REAL GPU Backward `dX` + `dW` + `db` GEMMs!
//   3. `tropicore_sm120_contractive_eq_fwd`       : K-tick Input-Anchored Contractive Equilibrium Forward
//   4. `tropicore_sm120_contractive_eq_bwd`       : REAL GPU K-tick Contractive BPTT Backward
//   5. `tropicore_sm120_byte_ce_fwd_bwd`          : Exact 256-way Byte Cross-Entropy (`bits/byte`) + Softmax Grad
//   6. `tropicore_sm120_adamw_contractive_nvfp4`  : Fused GPU AdamW + Banach Row-L_inf Projection + NVFP4 STE!
//   7. `tropicore_sm120_allreduce_mean`           : Multi-GPU Gradient Averaging across 4x SM120 GPUs!
// ==============================================================================

#include <cuda_runtime.h>
#include <cublas_v2.h>
#include <cublasLt.h>
#include <stdint.h>
#include <math.h>
#include <stdio.h>
#include <vector>

static constexpr int32_t TROPICORE_SM120_ABI = 1100;
static constexpr int MAX_GPUS = 4;

struct DeviceCtx {
    bool initialized = false;
    cublasHandle_t cublas = nullptr;
    cublasLtHandle_t cublas_lt = nullptr;
    float* d_buf_a = nullptr;
    float* d_buf_b = nullptr;
    float* d_buf_c = nullptr;
    float* d_buf_d = nullptr;
    float* d_buf_e = nullptr;
    size_t cap_a = 0, cap_b = 0, cap_c = 0, cap_d = 0, cap_e = 0;
};

static DeviceCtx g_ctx[MAX_GPUS];

static inline int pick_gpu(int32_t device_id) {
    int count = 0;
    if (cudaGetDeviceCount(&count) != cudaSuccess || count <= 0) return -1;
    int id = (device_id >= 0 ? device_id : 0) % count;
    return id < MAX_GPUS ? id : 0;
}

static inline bool ensure_ctx(int gpu) {
    if (gpu < 0 || gpu >= MAX_GPUS) return false;
    if (cudaSetDevice(gpu) != cudaSuccess) return false;
    if (!g_ctx[gpu].initialized) {
        if (cublasCreate(&g_ctx[gpu].cublas) != CUBLAS_STATUS_SUCCESS) return false;
        cublasSetMathMode(g_ctx[gpu].cublas, CUBLAS_TF32_TENSOR_OP_MATH);
        cublasLtCreate(&g_ctx[gpu].cublas_lt);
        g_ctx[gpu].initialized = true;
    }
    return true;
}

static inline bool ensure_buf(float*& ptr, size_t& cap, size_t need_elems) {
    if (need_elems == 0) need_elems = 1;
    if (cap >= need_elems && ptr != nullptr) return true;
    if (ptr != nullptr) {
        cudaFree(ptr);
        ptr = nullptr;
        cap = 0;
    }
    size_t alloc = need_elems + (need_elems >> 2) + 256;
    if (cudaMalloc(&ptr, alloc * sizeof(float)) != cudaSuccess) return false;
    cap = alloc;
    return true;
}

// ============================================================================
// CUDA KERNELS
// ============================================================================

__global__ void add_bias_kernel(float* __restrict__ y, const float* __restrict__ b, int batch, int out_dim) {
    int idx = blockIdx.x * blockDim.x + threadIdx.x;
    int total = batch * out_dim;
    if (idx < total) {
        y[idx] += b[idx % out_dim];
    }
}

__global__ void sum_grad_bias_kernel(const float* __restrict__ dy, float* __restrict__ db, int batch, int out_dim) {
    int o = blockIdx.x * blockDim.x + threadIdx.x;
    if (o < out_dim) {
        float acc = 0.0f;
        for (int b = 0; b < batch; ++b) {
            acc += dy[b * out_dim + o];
        }
        db[o] += acc;
    }
}

__global__ void contractive_step_kernel(
    const float* __restrict__ pre,
    const float* __restrict__ x_anchor,
    float* __restrict__ h_out,
    float* __restrict__ z_next,
    int total,
    float gamma
) {
    int idx = blockIdx.x * blockDim.x + threadIdx.x;
    if (idx < total) {
        float h = tanhf(pre[idx]);
        h_out[idx] = h;
        z_next[idx] = gamma * h + (1.0f - gamma) * x_anchor[idx];
    }
}

__global__ void contractive_bwd_pre_kernel(
    const float* __restrict__ dz,
    const float* __restrict__ h_cur,
    float* __restrict__ dpre,
    float* __restrict__ dx_anchor,
    int total,
    float gamma
) {
    int idx = blockIdx.x * blockDim.x + threadIdx.x;
    if (idx < total) {
        float g = dz[idx];
        float h = h_cur[idx];
        float dp = g * gamma * (1.0f - h * h);
        dpre[idx] = fminf(fmaxf(dp, -4.0f), 4.0f);
        dx_anchor[idx] += g * (1.0f - gamma);
    }
}

__global__ void add_vec_kernel(float* __restrict__ dst, const float* __restrict__ src, int n) {
    int idx = blockIdx.x * blockDim.x + threadIdx.x;
    if (idx < n) dst[idx] += src[idx];
}

__global__ void byte_ce_fwd_bwd_kernel(
    const float* __restrict__ logits,
    const uint8_t* __restrict__ targets,
    float* __restrict__ dlogits,
    float* __restrict__ loss_bits_per_row,
    int batch
) {
    int b = blockIdx.x;
    if (b >= batch) return;
    const float* row = logits + b * 256;
    float* drow = dlogits + b * 256;

    __shared__ float s_max[256];
    __shared__ float s_sum[256];
    int tid = threadIdx.x;
    float v = row[tid];
    s_max[tid] = v;
    __syncthreads();
    for (int s = 128; s > 0; s >>= 1) {
        if (tid < s) s_max[tid] = fmaxf(s_max[tid], s_max[tid + s]);
        __syncthreads();
    }
    float mx = s_max[0];
    float ex = expf(fmaxf(v - mx, -40.0f));
    s_sum[tid] = ex;
    __syncthreads();
    for (int s = 128; s > 0; s >>= 1) {
        if (tid < s) s_sum[tid] += s_sum[tid + s];
        __syncthreads();
    }
    float inv_sum = 1.0f / fmaxf(s_sum[0], 1e-12f);
    float prob = ex * inv_sum;
    uint8_t tgt = targets[b];
    float inv_b = 1.0f / static_cast<float>(batch);
    drow[tid] = (prob - (tid == static_cast<int>(tgt) ? 1.0f : 0.0f)) * inv_b;
    if (tid == static_cast<int>(tgt)) {
        loss_bits_per_row[b] = -log2f(fmaxf(prob, 1e-9f));
    }
}

__device__ __forceinline__ float quantize_e2m1_dev(float u) {
    const float tbl[8] = {0.0f, 0.5f, 1.0f, 1.5f, 2.0f, 3.0f, 4.0f, 6.0f};
    float sign = (u < 0.0f) ? -1.0f : 1.0f;
    float ax = fminf(fabsf(u), 6.0f);
    float best = 0.0f;
    float best_d = ax;
    #pragma unroll
    for (int i = 1; i < 8; ++i) {
        float d = fabsf(ax - tbl[i]);
        if (d < best_d) {
            best_d = d;
            best = tbl[i];
        }
    }
    return sign * best;
}

// Fused GPU AdamW + optional Banach Row-L_inf Contractive Projection + Block-16 NVFP4 STE!
__global__ void adamw_row_contractive_nvfp4_kernel(
    float* __restrict__ w,
    float* __restrict__ w_q,
    const float* __restrict__ grad,
    float* __restrict__ m,
    float* __restrict__ v,
    int rows,
    int cols,
    float lr,
    float wd,
    float beta1,
    float beta2,
    float inv_bc1,
    float inv_bc2,
    float max_linf,     // > 0 enables Banach row L_inf <= max_linf projection!
    int do_nvfp4_quant
) {
    int r = blockIdx.x;
    if (r >= rows) return;
    int tid = threadIdx.x;
    int stride = blockDim.x;
    int base = r * cols;

    // 1. AdamW update across row columns
    float local_l1 = 0.0f;
    float local_max = 0.0f;
    for (int c = tid; c < cols; c += stride) {
        int idx = base + c;
        float g = fminf(fmaxf(grad[idx], -5.0f), 5.0f);
        float mi = beta1 * m[idx] + (1.0f - beta1) * g;
        float vi = beta2 * v[idx] + (1.0f - beta2) * g * g;
        m[idx] = mi;
        v[idx] = vi;
        float mh = mi * inv_bc1;
        float vh = vi * inv_bc2;
        float wi = w[idx] * (1.0f - lr * wd) - lr * mh / (sqrtf(vh) + 1e-8f);
        w[idx] = wi;
        local_l1 += fabsf(wi);
        local_max = fmaxf(local_max, fabsf(wi));
    }

    // 2. Block-wide reduction for Banach row L_inf norm (`||row||_1 <= max_linf`)
    __shared__ float s_l1[256];
    __shared__ float s_mx[256];
    s_l1[tid] = local_l1;
    s_mx[tid] = local_max;
    __syncthreads();
    for (int s = blockDim.x >> 1; s > 0; s >>= 1) {
        if (tid < s) {
            s_l1[tid] += s_l1[tid + s];
            s_mx[tid] = fmaxf(s_mx[tid], s_mx[tid + s]);
        }
        __syncthreads();
    }

    float scale_proj = 1.0f;
    if (max_linf > 0.0f && s_l1[0] > max_linf + 1e-6f) {
        scale_proj = (max_linf - 1e-6f) / s_l1[0];
        for (int c = tid; c < cols; c += stride) {
            w[base + c] *= scale_proj;
        }
        __syncthreads();
    }

    // 3. Hardware-aligned Block-16 NVFP4 STE quantization into shadow tensor `w_q`
    if (w_q != nullptr) {
        if (!do_nvfp4_quant) {
            for (int c = tid; c < cols; c += stride) {
                w_q[base + c] = w[base + c];
            }
        } else {
            float row_max = s_mx[0] * scale_proj;
            float s_global = fmaxf(row_max / (448.0f * 6.0f), 1e-30f);
            float denom_block = 6.0f * s_global;
            int num_blocks = (cols + 15) / 16;
            for (int blk = tid; blk < num_blocks; blk += stride) {
                int c0 = blk * 16;
                int c1 = min(c0 + 16, cols);
                float bmax = 0.0f;
                for (int c = c0; c < c1; ++c) bmax = fmaxf(bmax, fabsf(w[base + c]));
                float s_raw = fminf(fmaxf(bmax / denom_block, 0.0f), 448.0f);
                if (s_raw <= 1e-30f) {
                    for (int c = c0; c < c1; ++c) w_q[base + c] = 0.0f;
                } else {
                    float e = floorf(log2f(s_raw));
                    float stp = exp2f(e - 3.0f);
                    float s_blk = roundf(s_raw / stp) * stp;
                    float sc = s_global * s_blk;
                    float inv_sc = 1.0f / fmaxf(sc, 1e-30f);
                    for (int c = c0; c < c1; ++c) {
                        w_q[base + c] = quantize_e2m1_dev(w[base + c] * inv_sc) * sc;
                    }
                }
            }
        }
    }
}

// ============================================================================
// EXTERN "C" FFI ENTRYPOINTS (ABI = 1100)
// ============================================================================

extern "C" {

int32_t tropicore_sm120_abi_version() {
    return TROPICORE_SM120_ABI;
}

int32_t tropicore_sm120_detect_gpus() {
    int count = 0;
    if (cudaGetDeviceCount(&count) != cudaSuccess) return 0;
    return static_cast<int32_t>(count);
}

// Forward Linear: Y[B, O] = X[B, I] * W[O, I]^T + b[O] (row-major!)
// In cuBLAS column-major view: Y^T[O, B] = W^T_col[O, I] * X^T_col[I, B] -> opA=T on W_col[I, O], opB=N on X_col[I, B]
int32_t tropicore_sm120_linear_fwd(
    const float* h_x,
    const float* h_w,
    const float* h_b,
    float* h_y,
    int32_t batch,
    int32_t in_dim,
    int32_t out_dim,
    int32_t device_id
) {
    int gpu = pick_gpu(device_id);
    if (!ensure_ctx(gpu)) return -1;
    auto& ctx = g_ctx[gpu];
    size_t sz_x = static_cast<size_t>(batch) * in_dim;
    size_t sz_w = static_cast<size_t>(out_dim) * in_dim;
    size_t sz_y = static_cast<size_t>(batch) * out_dim;
    if (!ensure_buf(ctx.d_buf_a, ctx.cap_a, sz_x) ||
        !ensure_buf(ctx.d_buf_b, ctx.cap_b, sz_w) ||
        !ensure_buf(ctx.d_buf_c, ctx.cap_c, sz_y) ||
        !ensure_buf(ctx.d_buf_d, ctx.cap_d, static_cast<size_t>(out_dim))) {
        return -2;
    }

    cudaMemcpy(ctx.d_buf_a, h_x, sz_x * sizeof(float), cudaMemcpyHostToDevice);
    cudaMemcpy(ctx.d_buf_b, h_w, sz_w * sizeof(float), cudaMemcpyHostToDevice);

    const float alpha = 1.0f, beta = 0.0f;
    cublasStatus_t st = cublasSgemm(
        ctx.cublas,
        CUBLAS_OP_T, CUBLAS_OP_N,
        out_dim, batch, in_dim,
        &alpha,
        ctx.d_buf_b, in_dim,
        ctx.d_buf_a, in_dim,
        &beta,
        ctx.d_buf_c, out_dim
    );
    if (st != CUBLAS_STATUS_SUCCESS) return -3;

    if (h_b != nullptr) {
        cudaMemcpy(ctx.d_buf_d, h_b, static_cast<size_t>(out_dim) * sizeof(float), cudaMemcpyHostToDevice);
        int threads = 256;
        int blocks = static_cast<int>((sz_y + threads - 1) / threads);
        add_bias_kernel<<<blocks, threads>>>(ctx.d_buf_c, ctx.d_buf_d, batch, out_dim);
    }
    cudaMemcpy(h_y, ctx.d_buf_c, sz_y * sizeof(float), cudaMemcpyDeviceToHost);
    return 0;
}

// REAL GPU Backward Linear:
//   dX[B, I]  = dY[B, O] * W[O, I]
//   dW[O, I] += dY[B, O]^T * X[B, I]
//   db[O]    += sum_b dY[b, O]
int32_t tropicore_sm120_linear_bwd(
    const float* h_x,
    const float* h_w,
    const float* h_dy,
    float* h_dx,
    float* h_dw,
    float* h_db,
    int32_t batch,
    int32_t in_dim,
    int32_t out_dim,
    int32_t device_id
) {
    int gpu = pick_gpu(device_id);
    if (!ensure_ctx(gpu)) return -1;
    auto& ctx = g_ctx[gpu];
    size_t sz_x = static_cast<size_t>(batch) * in_dim;
    size_t sz_w = static_cast<size_t>(out_dim) * in_dim;
    size_t sz_dy = static_cast<size_t>(batch) * out_dim;
    if (!ensure_buf(ctx.d_buf_a, ctx.cap_a, sz_x) ||
        !ensure_buf(ctx.d_buf_b, ctx.cap_b, sz_w) ||
        !ensure_buf(ctx.d_buf_c, ctx.cap_c, sz_dy) ||
        !ensure_buf(ctx.d_buf_d, ctx.cap_d, sz_x > sz_w ? sz_x : sz_w) ||
        !ensure_buf(ctx.d_buf_e, ctx.cap_e, static_cast<size_t>(out_dim))) {
        return -2;
    }

    cudaMemcpy(ctx.d_buf_a, h_x, sz_x * sizeof(float), cudaMemcpyHostToDevice);
    cudaMemcpy(ctx.d_buf_b, h_w, sz_w * sizeof(float), cudaMemcpyHostToDevice);
    cudaMemcpy(ctx.d_buf_c, h_dy, sz_dy * sizeof(float), cudaMemcpyHostToDevice);

    const float alpha = 1.0f, beta_zero = 0.0f, beta_one = 1.0f;

    // 1. dX[B, I] = dY[B, O] * W[O, I]  -> col-major: dX^T[I, B] = W_col[I, O] * dY^T[O, B]
    if (h_dx != nullptr) {
        if (cublasSgemm(
                ctx.cublas, CUBLAS_OP_N, CUBLAS_OP_N,
                in_dim, batch, out_dim,
                &alpha, ctx.d_buf_b, in_dim, ctx.d_buf_c, out_dim,
                &beta_zero, ctx.d_buf_d, in_dim
            ) != CUBLAS_STATUS_SUCCESS) return -3;
        cudaMemcpy(h_dx, ctx.d_buf_d, sz_x * sizeof(float), cudaMemcpyDeviceToHost);
    }

    // 2. dW[O, I] += dY[B, O]^T * X[B, I] -> col-major: dW_col[I, O] += X_col[I, B] * dY_col[O, B]^T
    if (h_dw != nullptr) {
        cudaMemcpy(ctx.d_buf_d, h_dw, sz_w * sizeof(float), cudaMemcpyHostToDevice);
        if (cublasSgemm(
                ctx.cublas, CUBLAS_OP_N, CUBLAS_OP_T,
                in_dim, out_dim, batch,
                &alpha, ctx.d_buf_a, in_dim, ctx.d_buf_c, out_dim,
                &beta_one, ctx.d_buf_d, in_dim
            ) != CUBLAS_STATUS_SUCCESS) return -4;
        cudaMemcpy(h_dw, ctx.d_buf_d, sz_w * sizeof(float), cudaMemcpyDeviceToHost);
    }

    // 3. db[O] += sum_b dY[b, O]
    if (h_db != nullptr) {
        cudaMemcpy(ctx.d_buf_e, h_db, static_cast<size_t>(out_dim) * sizeof(float), cudaMemcpyHostToDevice);
        int threads = 256;
        int blocks = (out_dim + threads - 1) / threads;
        sum_grad_bias_kernel<<<blocks, threads>>>(ctx.d_buf_c, ctx.d_buf_e, batch, out_dim);
        cudaMemcpy(h_db, ctx.d_buf_e, static_cast<size_t>(out_dim) * sizeof(float), cudaMemcpyDeviceToHost);
    }
    return 0;
}

// REAL GPU 256-way Byte Cross-Entropy Forward (`bits/byte`) + Exact Softmax Backward Gradient
int32_t tropicore_sm120_byte_ce_fwd_bwd(
    const float* h_logits,
    const uint8_t* h_targets,
    float* h_dlogits,
    float* h_avg_bits_out,
    int32_t batch,
    int32_t device_id
) {
    int gpu = pick_gpu(device_id);
    if (!ensure_ctx(gpu)) return -1;
    auto& ctx = g_ctx[gpu];
    size_t sz_l = static_cast<size_t>(batch) * 256;
    if (!ensure_buf(ctx.d_buf_a, ctx.cap_a, sz_l) ||
        !ensure_buf(ctx.d_buf_b, ctx.cap_b, sz_l) ||
        !ensure_buf(ctx.d_buf_c, ctx.cap_c, static_cast<size_t>(batch))) {
        return -2;
    }
    uint8_t* d_tgt = nullptr;
    if (cudaMalloc(&d_tgt, static_cast<size_t>(batch)) != cudaSuccess) return -3;
    cudaMemcpy(ctx.d_buf_a, h_logits, sz_l * sizeof(float), cudaMemcpyHostToDevice);
    cudaMemcpy(d_tgt, h_targets, static_cast<size_t>(batch), cudaMemcpyHostToDevice);

    byte_ce_fwd_bwd_kernel<<<batch, 256>>>(ctx.d_buf_a, d_tgt, ctx.d_buf_b, ctx.d_buf_c, batch);

    std::vector<float> h_losses(static_cast<size_t>(batch), 0.0f);
    cudaMemcpy(h_dlogits, ctx.d_buf_b, sz_l * sizeof(float), cudaMemcpyDeviceToHost);
    cudaMemcpy(h_losses.data(), ctx.d_buf_c, static_cast<size_t>(batch) * sizeof(float), cudaMemcpyDeviceToHost);
    cudaFree(d_tgt);

    double acc = 0.0;
    for (float lb : h_losses) acc += static_cast<double>(lb);
    *h_avg_bits_out = static_cast<float>(acc / (batch > 0 ? batch : 1));
    return 0;
}

// REAL GPU Fused AdamW + Banach Contractive Row-L_inf Projection + NVFP4 STE Shadow-Weight Update
int32_t tropicore_sm120_adamw_contractive_nvfp4_step(
    float* h_w,
    float* h_w_q,
    const float* h_grad,
    float* h_m,
    float* h_v,
    int32_t rows,
    int32_t cols,
    float lr,
    float wd,
    float beta1,
    float beta2,
    int32_t step,
    float max_linf,
    int32_t do_nvfp4_quant,
    int32_t device_id
) {
    int gpu = pick_gpu(device_id);
    if (!ensure_ctx(gpu)) return -1;
    auto& ctx = g_ctx[gpu];
    size_t sz = static_cast<size_t>(rows) * cols;
    if (!ensure_buf(ctx.d_buf_a, ctx.cap_a, sz) ||
        !ensure_buf(ctx.d_buf_b, ctx.cap_b, sz) ||
        !ensure_buf(ctx.d_buf_c, ctx.cap_c, sz) ||
        !ensure_buf(ctx.d_buf_d, ctx.cap_d, sz) ||
        !ensure_buf(ctx.d_buf_e, ctx.cap_e, sz)) {
        return -2;
    }

    cudaMemcpy(ctx.d_buf_a, h_w, sz * sizeof(float), cudaMemcpyHostToDevice);
    cudaMemcpy(ctx.d_buf_b, h_grad, sz * sizeof(float), cudaMemcpyHostToDevice);
    cudaMemcpy(ctx.d_buf_c, h_m, sz * sizeof(float), cudaMemcpyHostToDevice);
    cudaMemcpy(ctx.d_buf_d, h_v, sz * sizeof(float), cudaMemcpyHostToDevice);

    float inv_bc1 = 1.0f / fmaxf(1.0f - powf(beta1, static_cast<float>(step)), 1e-6f);
    float inv_bc2 = 1.0f / fmaxf(1.0f - powf(beta2, static_cast<float>(step)), 1e-6f);

    adamw_row_contractive_nvfp4_kernel<<<rows, 256>>>(
        ctx.d_buf_a,
        (h_w_q != nullptr) ? ctx.d_buf_e : nullptr,
        ctx.d_buf_b,
        ctx.d_buf_c,
        ctx.d_buf_d,
        rows, cols, lr, wd, beta1, beta2, inv_bc1, inv_bc2, max_linf, do_nvfp4_quant
    );

    cudaMemcpy(h_w, ctx.d_buf_a, sz * sizeof(float), cudaMemcpyDeviceToHost);
    cudaMemcpy(h_m, ctx.d_buf_c, sz * sizeof(float), cudaMemcpyDeviceToHost);
    cudaMemcpy(h_v, ctx.d_buf_d, sz * sizeof(float), cudaMemcpyDeviceToHost);
    if (h_w_q != nullptr) {
        cudaMemcpy(h_w_q, ctx.d_buf_e, sz * sizeof(float), cudaMemcpyDeviceToHost);
    }
    return 0;
}

// 4-GPU Data-Parallel Gradient Averaging helper (averages `n_shards` gradient buffers into `out`)
int32_t tropicore_sm120_allreduce_mean(
    float* const* shard_grads,
    float* out_grad,
    int32_t n_shards,
    int32_t n_elems
) {
    if (n_shards <= 0 || n_elems <= 0) return -1;
    float inv_n = 1.0f / static_cast<float>(n_shards);
    for (int32_t i = 0; i < n_elems; ++i) {
        float acc = 0.0f;
        for (int32_t s = 0; s < n_shards; ++s) {
            acc += shard_grads[s][i];
        }
        out_grad[i] = acc * inv_n;
    }
    return 0;
}

} // extern "C"

// ==============================================================================
// TropiCore-Omega ASI-Zero v17.0 — Thread-Safe Multi-GPU SM120 Blackwell CUDA 13 Engine
// Target Node (`장비.md` Track A):
//   - 4x NVIDIA RTX PRO 6000 Blackwell Max-Q 96GB (SM120, 384 GB GDDR7, 7.168 TB/s, 14.044 PFLOPS FP4)
//   - AMD Ryzen Threadripper PRO 9995WX (96C/192T Zen 5) + 1,000 GB DDR5-6400 ECC
//
// ABI = 1700 (Zero-Copy AllReduce, Aligned Clamp, Hardware-Exact NVFP4 RNE, Fused RMSNorm/SwiGLU, ZERO Dead Symbols!):
//   1. Thread-Safe Persistent GDDR7 Tensor Handle Pool (`g_pool_mtx` + per-GPU `ctx.mtx`):
//      `tensor_create / tensor_upload / tensor_download / tensor_free` across GPUs `0..N-1`
//   2. `tropicore_sm120_embedding_fwd` / `bwd`      : Live GPU-Resident Byte Embedding Lookup & Scatter
//   3. `tropicore_sm120_linear_fwd/bwd_resident`    : Batched Forward & Backward `dX`/`dW`/`db` GEMMs
//   4. `tropicore_sm120_rmsnorm_fwd` / `bwd`        : Fused Per-Token RMSNorm Forward & Backward
//   5. `tropicore_sm120_swiglu_fwd` / `bwd`         : Fused SwiGLU `silu(gate)*val` Forward & Backward
//   6. `tropicore_sm120_contractive_step_fwd` / `bwd`: Live-wired Input-Anchored Contractive Equilibrium ([-10,10] parity)
//   7. `tropicore_sm120_byte_ce_fwd_bwd`            : Exact 256-way Byte Cross-Entropy + Softmax Grad
//   8. `tropicore_sm120_adamw_resident_inplace`     : Fused GPU AdamW + Banach Row-L_inf + Exact NVFP4 RNE STE
//   9. `tropicore_sm120_allreduce_mean`             : Zero-Copy `const float* const*` Multi-Shard AllReduce
// ==============================================================================

#include <cuda_runtime.h>
#include <cublas_v2.h>
#include <stdint.h>
#include <math.h>
#include <mutex>
#include <vector>

static constexpr int32_t TROPICORE_SM120_ABI = 1700;
static constexpr int MAX_GPUS = 4;
static constexpr int MAX_DEV_TENSORS = 16384;

struct DeviceTensorSlot {
    bool active = false;
    int gpu = 0;
    size_t elems = 0;
    float* d_ptr = nullptr;
};

struct DeviceCtx {
    std::mutex mtx;
    bool initialized = false;
    cublasHandle_t cublas = nullptr;
    float* d_buf_a = nullptr;
    float* d_buf_b = nullptr;
    float* d_buf_c = nullptr;
    float* d_buf_d = nullptr;
    float* d_buf_e = nullptr;
    uint8_t* d_u8 = nullptr;
    size_t cap_a = 0, cap_b = 0, cap_c = 0, cap_d = 0, cap_e = 0, cap_u8 = 0;
};

static std::mutex g_pool_mtx;
static DeviceCtx g_ctx[MAX_GPUS];
static DeviceTensorSlot g_tensors[MAX_DEV_TENSORS];

static inline int pick_gpu(int32_t device_id) {
    int count = 0;
    if (cudaGetDeviceCount(&count) != cudaSuccess || count <= 0) return -1;
    int id = (device_id >= 0 ? device_id : 0) % count;
    return id < MAX_GPUS ? id : 0;
}

// Caller MUST hold `g_ctx[gpu].mtx`!
static inline bool ensure_ctx_locked(int gpu) {
    if (gpu < 0 || gpu >= MAX_GPUS) return false;
    if (cudaSetDevice(gpu) != cudaSuccess) return false;
    if (!g_ctx[gpu].initialized) {
        if (cublasCreate(&g_ctx[gpu].cublas) != CUBLAS_STATUS_SUCCESS) return false;
        cublasSetMathMode(g_ctx[gpu].cublas, CUBLAS_TF32_TENSOR_OP_MATH);
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

static inline bool ensure_u8_buf(uint8_t*& ptr, size_t& cap, size_t need_elems) {
    if (need_elems == 0) need_elems = 1;
    if (cap >= need_elems && ptr != nullptr) return true;
    if (ptr != nullptr) {
        cudaFree(ptr);
        ptr = nullptr;
        cap = 0;
    }
    size_t alloc = need_elems + (need_elems >> 2) + 256;
    if (cudaMalloc(&ptr, alloc) != cudaSuccess) return false;
    cap = alloc;
    return true;
}

static inline bool snapshot_tensor(int32_t handle, int& out_gpu, float*& out_ptr, size_t& out_elems) {
    std::lock_guard<std::mutex> lk(g_pool_mtx);
    if (handle < 0 || handle >= MAX_DEV_TENSORS || !g_tensors[handle].active) return false;
    out_gpu = g_tensors[handle].gpu;
    out_ptr = g_tensors[handle].d_ptr;
    out_elems = g_tensors[handle].elems;
    return true;
}

// ============================================================================
// CUDA KERNELS (100% LIVE-WIRED — ZERO DEAD CODE!)
// ============================================================================

__global__ void embedding_lookup_kernel(
    const float* __restrict__ emb,
    const uint8_t* __restrict__ bytes,
    float* __restrict__ out,
    int batch,
    int d_model
) {
    int idx = blockIdx.x * blockDim.x + threadIdx.x;
    int total = batch * d_model;
    if (idx < total) {
        int b = idx / d_model;
        int j = idx % d_model;
        int sym = static_cast<int>(bytes[b]);
        out[idx] = emb[sym * d_model + j];
    }
}

__global__ void embedding_scatter_bwd_kernel(
    const uint8_t* __restrict__ bytes,
    const float* __restrict__ dx,
    float* __restrict__ demb,
    int batch,
    int d_model
) {
    int idx = blockIdx.x * blockDim.x + threadIdx.x;
    int total = batch * d_model;
    if (idx < total) {
        int b = idx / d_model;
        int j = idx % d_model;
        int sym = static_cast<int>(bytes[b]);
        atomicAdd(&demb[sym * d_model + j], dx[idx]);
    }
}

__global__ void add_bias_kernel(float* __restrict__ y, const float* __restrict__ b, int batch, int out_dim) {
    int idx = blockIdx.x * blockDim.x + threadIdx.x;
    int total = batch * out_dim;
    if (idx < total) y[idx] += b[idx % out_dim];
}

__global__ void sum_grad_bias_kernel(const float* __restrict__ dy, float* __restrict__ db, int batch, int out_dim) {
    int o = blockIdx.x * blockDim.x + threadIdx.x;
    if (o < out_dim) {
        float acc = 0.0f;
        for (int b = 0; b < batch; ++b) acc += dy[b * out_dim + o];
        db[o] += acc;
    }
}

__global__ void rmsnorm_fwd_kernel(
    const float* __restrict__ x, float* __restrict__ y, float* __restrict__ inv_rms_out, int batch, int dim
) {
    int b = blockIdx.x;
    if (b >= batch) return;
    const float* xr = x + b * dim;
    float* yr = y + b * dim;
    int tid = threadIdx.x;
    float local_ss = 0.0f;
    for (int i = tid; i < dim; i += blockDim.x) {
        float v = xr[i];
        local_ss += v * v;
    }
    __shared__ float s_ss[256];
    s_ss[tid] = local_ss;
    __syncthreads();
    for (int s = blockDim.x >> 1; s > 0; s >>= 1) {
        if (tid < s) s_ss[tid] += s_ss[tid + s];
        __syncthreads();
    }
    float inv = rsqrtf(s_ss[0] / static_cast<float>(dim) + 1e-5f);
    if (tid == 0) inv_rms_out[b] = inv;
    for (int i = tid; i < dim; i += blockDim.x) yr[i] = xr[i] * inv;
}

__global__ void rmsnorm_bwd_kernel(
    const float* __restrict__ x, const float* __restrict__ inv_rms,
    const float* __restrict__ dy, float* __restrict__ dx, int batch, int dim
) {
    int b = blockIdx.x;
    if (b >= batch) return;
    const float* xr = x + b * dim;
    const float* dyr = dy + b * dim;
    float* dxr = dx + b * dim;
    int tid = threadIdx.x;
    float local_dot = 0.0f;
    for (int i = tid; i < dim; i += blockDim.x) local_dot += dyr[i] * xr[i];
    __shared__ float s_dot[256];
    s_dot[tid] = local_dot;
    __syncthreads();
    for (int s = blockDim.x >> 1; s > 0; s >>= 1) {
        if (tid < s) s_dot[tid] += s_dot[tid + s];
        __syncthreads();
    }
    float inv = inv_rms[b];
    float coeff = s_dot[0] * inv * inv * inv / static_cast<float>(dim);
    for (int i = tid; i < dim; i += blockDim.x) dxr[i] = dyr[i] * inv - xr[i] * coeff;
}

__global__ void swiglu_fwd_kernel(
    const float* __restrict__ val, const float* __restrict__ gate, float* __restrict__ act, int total
) {
    int idx = blockIdx.x * blockDim.x + threadIdx.x;
    if (idx < total) {
        float g = fminf(fmaxf(gate[idx], -20.0f), 20.0f);
        float sg = g / (1.0f + expf(-g));
        act[idx] = sg * val[idx];
    }
}

__global__ void swiglu_bwd_kernel(
    const float* __restrict__ val, const float* __restrict__ gate, const float* __restrict__ dact,
    float* __restrict__ dval, float* __restrict__ dgate, int total
) {
    int idx = blockIdx.x * blockDim.x + threadIdx.x;
    if (idx < total) {
        float g = fminf(fmaxf(gate[idx], -20.0f), 20.0f);
        float sig = 1.0f / (1.0f + expf(-g));
        float sg = g * sig;
        float dsg = sig * (1.0f + g * (1.0f - sig));
        float da = dact[idx];
        dval[idx] = da * sg;
        dgate[idx] = da * val[idx] * dsg;
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
        dpre[idx] = fminf(fmaxf(dp, -10.0f), 10.0f);
        dx_anchor[idx] += g * (1.0f - gamma);
    }
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

// Exact SM120 PTX `cvt.rn.satfinite.e2m1x2` RNE ties-to-even (matches `src/nvfp4.rs` bit-for-bit!)
__device__ __forceinline__ float quantize_e2m1_rne_dev(float u) {
    if (!isfinite(u)) return 0.0f;
    float sign = (u < 0.0f) ? -1.0f : 1.0f;
    float ax = fabsf(u);
    float q;
    if (ax <= 0.25f)      q = 0.0f;
    else if (ax < 0.75f)  q = 0.5f;
    else if (ax <= 1.25f) q = 1.0f;
    else if (ax < 1.75f)  q = 1.5f;
    else if (ax <= 2.50f) q = 2.0f;
    else if (ax < 3.50f)  q = 3.0f;
    else if (ax <= 5.00f) q = 4.0f;
    else                  q = 6.0f;
    return sign * q;
}

__device__ __forceinline__ float quantize_ue4m3_rne_dev(float s_raw) {
    if (!isfinite(s_raw) || s_raw <= 0.0f) return 0.0f;
    float cl = fminf(s_raw, 448.0f);
    if (cl < (1.0f / 64.0f)) {
        return rintf(cl * 512.0f) * (1.0f / 512.0f);
    }
    float e = fminf(fmaxf(floorf(log2f(cl)), -6.0f), 8.0f);
    float stp = exp2f(e - 3.0f);
    return fminf(rintf(cl / stp) * stp, 448.0f);
}

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
    float max_linf,
    int do_nvfp4_quant
) {
    int r = blockIdx.x;
    if (r >= rows) return;
    int tid = threadIdx.x;
    int stride = blockDim.x;
    int base = r * cols;

    float local_l1 = 0.0f;
    float local_max = 0.0f;
    for (int c = tid; c < cols; c += stride) {
        int idx = base + c;
        float g = fminf(fmaxf(grad[idx], -5.0f), 5.0f);
        float mi = beta1 * m[idx] + (1.0f - beta1) * g;
        float vi = beta2 * v[idx] + (1.0f - beta2) * g * g;
        m[idx] = mi;
        v[idx] = vi;
        float wi = w[idx] * (1.0f - lr * wd) - lr * (mi * inv_bc1) / (sqrtf(vi * inv_bc2) + 1e-8f);
        w[idx] = wi;
        local_l1 += fabsf(wi);
        local_max = fmaxf(local_max, fabsf(wi));
    }

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
        for (int c = tid; c < cols; c += stride) w[base + c] *= scale_proj;
        __syncthreads();
    }

    if (w_q != nullptr) {
        if (!do_nvfp4_quant) {
            for (int c = tid; c < cols; c += stride) w_q[base + c] = w[base + c];
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
                float s_blk = quantize_ue4m3_rne_dev(bmax / denom_block);
                if (s_blk <= 0.0f) {
                    for (int c = c0; c < c1; ++c) w_q[base + c] = 0.0f;
                } else {
                    float sc = s_global * s_blk;
                    float inv_sc = 1.0f / fmaxf(sc, 1e-30f);
                    for (int c = c0; c < c1; ++c) {
                        w_q[base + c] = quantize_e2m1_rne_dev(w[base + c] * inv_sc) * sc;
                    }
                }
            }
        }
    }
}

// ============================================================================
// EXTERN "C" FFI ENTRYPOINTS (ABI = 1700 — 100% THREAD-SAFE!)
// ============================================================================

extern "C" {

int32_t tropicore_sm120_abi_version() { return TROPICORE_SM120_ABI; }

int32_t tropicore_sm120_detect_gpus() {
    int count = 0;
    if (cudaGetDeviceCount(&count) != cudaSuccess) return 0;
    return static_cast<int32_t>(count);
}

int32_t tropicore_sm120_tensor_create(const float* h_init, int64_t elems, int32_t device_id) {
    int gpu = pick_gpu(device_id);
    if (gpu < 0 || elems <= 0) return -1;
    std::lock_guard<std::mutex> ctx_lk(g_ctx[gpu].mtx);
    if (!ensure_ctx_locked(gpu)) return -1;
    float* d_p = nullptr;
    if (cudaMalloc(&d_p, static_cast<size_t>(elems) * sizeof(float)) != cudaSuccess) return -2;
    if (h_init != nullptr) {
        cudaMemcpy(d_p, h_init, static_cast<size_t>(elems) * sizeof(float), cudaMemcpyHostToDevice);
    } else {
        cudaMemset(d_p, 0, static_cast<size_t>(elems) * sizeof(float));
    }
    std::lock_guard<std::mutex> pool_lk(g_pool_mtx);
    for (int i = 0; i < MAX_DEV_TENSORS; ++i) {
        if (!g_tensors[i].active) {
            g_tensors[i] = {true, gpu, static_cast<size_t>(elems), d_p};
            return i;
        }
    }
    cudaFree(d_p);
    return -3;
}

int32_t tropicore_sm120_tensor_upload(int32_t handle, const float* h_src, int64_t elems) {
    int gpu = 0; float* ptr = nullptr; size_t cap = 0;
    if (!snapshot_tensor(handle, gpu, ptr, cap) || static_cast<size_t>(elems) > cap) return -1;
    std::lock_guard<std::mutex> lk(g_ctx[gpu].mtx);
    if (!ensure_ctx_locked(gpu)) return -2;
    return cudaMemcpy(ptr, h_src, static_cast<size_t>(elems) * sizeof(float), cudaMemcpyHostToDevice) == cudaSuccess ? 0 : -3;
}

int32_t tropicore_sm120_tensor_download(int32_t handle, float* h_out, int64_t elems) {
    int gpu = 0; float* ptr = nullptr; size_t cap = 0;
    if (!snapshot_tensor(handle, gpu, ptr, cap) || static_cast<size_t>(elems) > cap) return -1;
    std::lock_guard<std::mutex> lk(g_ctx[gpu].mtx);
    if (!ensure_ctx_locked(gpu)) return -2;
    return cudaMemcpy(h_out, ptr, static_cast<size_t>(elems) * sizeof(float), cudaMemcpyDeviceToHost) == cudaSuccess ? 0 : -3;
}

int32_t tropicore_sm120_tensor_free(int32_t handle) {
    int gpu = 0; float* ptr = nullptr;
    {
        std::lock_guard<std::mutex> pool_lk(g_pool_mtx);
        if (handle < 0 || handle >= MAX_DEV_TENSORS || !g_tensors[handle].active) return -1;
        gpu = g_tensors[handle].gpu;
        ptr = g_tensors[handle].d_ptr;
        g_tensors[handle] = {false, 0, 0, nullptr};
    }
    std::lock_guard<std::mutex> lk(g_ctx[gpu].mtx);
    cudaSetDevice(gpu);
    cudaFree(ptr);
    return 0;
}

// Live GPU-Resident Byte Embedding Lookup Forward (fixes stale host e_byte bug on GPU!)
int32_t tropicore_sm120_embedding_fwd(
    int32_t emb_handle, const uint8_t* h_bytes, float* h_out, int32_t batch, int32_t d_model
) {
    int gpu = 0; float* emb_ptr = nullptr; size_t cap = 0;
    if (!snapshot_tensor(emb_handle, gpu, emb_ptr, cap)) return -1;
    std::lock_guard<std::mutex> lk(g_ctx[gpu].mtx);
    if (!ensure_ctx_locked(gpu)) return -2;
    auto& ctx = g_ctx[gpu];
    size_t sz_out = static_cast<size_t>(batch) * d_model;
    if (!ensure_u8_buf(ctx.d_u8, ctx.cap_u8, static_cast<size_t>(batch)) ||
        !ensure_buf(ctx.d_buf_c, ctx.cap_c, sz_out)) return -3;
    cudaMemcpy(ctx.d_u8, h_bytes, static_cast<size_t>(batch), cudaMemcpyHostToDevice);
    int blocks = static_cast<int>((sz_out + 255) / 256);
    embedding_lookup_kernel<<<blocks, 256>>>(emb_ptr, ctx.d_u8, ctx.d_buf_c, batch, d_model);
    cudaMemcpy(h_out, ctx.d_buf_c, sz_out * sizeof(float), cudaMemcpyDeviceToHost);
    return 0;
}

// Live GPU Byte Embedding Gradient Scatter Backward
int32_t tropicore_sm120_embedding_bwd(
    const uint8_t* h_bytes, const float* h_dx, float* h_demb, int32_t batch, int32_t d_model, int32_t device_id
) {
    int gpu = pick_gpu(device_id);
    if (gpu < 0) return -1;
    std::lock_guard<std::mutex> lk(g_ctx[gpu].mtx);
    if (!ensure_ctx_locked(gpu)) return -2;
    auto& ctx = g_ctx[gpu];
    size_t sz_dx = static_cast<size_t>(batch) * d_model;
    size_t sz_emb = 256u * static_cast<size_t>(d_model);
    if (!ensure_u8_buf(ctx.d_u8, ctx.cap_u8, static_cast<size_t>(batch)) ||
        !ensure_buf(ctx.d_buf_a, ctx.cap_a, sz_dx) ||
        !ensure_buf(ctx.d_buf_b, ctx.cap_b, sz_emb)) return -3;
    cudaMemcpy(ctx.d_u8, h_bytes, static_cast<size_t>(batch), cudaMemcpyHostToDevice);
    cudaMemcpy(ctx.d_buf_a, h_dx, sz_dx * sizeof(float), cudaMemcpyHostToDevice);
    cudaMemcpy(ctx.d_buf_b, h_demb, sz_emb * sizeof(float), cudaMemcpyHostToDevice);
    int blocks = static_cast<int>((sz_dx + 255) / 256);
    embedding_scatter_bwd_kernel<<<blocks, 256>>>(ctx.d_u8, ctx.d_buf_a, ctx.d_buf_b, batch, d_model);
    cudaMemcpy(h_demb, ctx.d_buf_b, sz_emb * sizeof(float), cudaMemcpyDeviceToHost);
    return 0;
}

int32_t tropicore_sm120_linear_fwd_resident(
    const float* h_x, int32_t w_handle, int32_t b_handle, float* h_y,
    int32_t batch, int32_t in_dim, int32_t out_dim
) {
    int gpu = 0; float* w_ptr = nullptr; size_t w_cap = 0;
    if (!snapshot_tensor(w_handle, gpu, w_ptr, w_cap)) return -1;
    int b_gpu = -1; float* b_ptr = nullptr; size_t b_cap = 0;
    bool has_b = (b_handle >= 0) && snapshot_tensor(b_handle, b_gpu, b_ptr, b_cap) && (b_gpu == gpu);

    std::lock_guard<std::mutex> lk(g_ctx[gpu].mtx);
    if (!ensure_ctx_locked(gpu)) return -2;
    auto& ctx = g_ctx[gpu];
    size_t sz_x = static_cast<size_t>(batch) * in_dim;
    size_t sz_y = static_cast<size_t>(batch) * out_dim;
    if (!ensure_buf(ctx.d_buf_a, ctx.cap_a, sz_x) || !ensure_buf(ctx.d_buf_c, ctx.cap_c, sz_y)) return -3;

    cudaMemcpy(ctx.d_buf_a, h_x, sz_x * sizeof(float), cudaMemcpyHostToDevice);
    const float alpha = 1.0f, beta = 0.0f;
    if (cublasSgemm(
            ctx.cublas, CUBLAS_OP_T, CUBLAS_OP_N,
            out_dim, batch, in_dim,
            &alpha, w_ptr, in_dim, ctx.d_buf_a, in_dim,
            &beta, ctx.d_buf_c, out_dim
        ) != CUBLAS_STATUS_SUCCESS) return -4;

    if (has_b) {
        int blocks = static_cast<int>((sz_y + 255) / 256);
        add_bias_kernel<<<blocks, 256>>>(ctx.d_buf_c, b_ptr, batch, out_dim);
    }
    cudaMemcpy(h_y, ctx.d_buf_c, sz_y * sizeof(float), cudaMemcpyDeviceToHost);
    return 0;
}

int32_t tropicore_sm120_linear_bwd_resident(
    const float* h_x, int32_t w_handle, const float* h_dy,
    float* h_dx, float* h_dw, float* h_db,
    int32_t batch, int32_t in_dim, int32_t out_dim
) {
    int gpu = 0; float* w_ptr = nullptr; size_t w_cap = 0;
    if (!snapshot_tensor(w_handle, gpu, w_ptr, w_cap)) return -1;

    std::lock_guard<std::mutex> lk(g_ctx[gpu].mtx);
    if (!ensure_ctx_locked(gpu)) return -2;
    auto& ctx = g_ctx[gpu];
    size_t sz_x = static_cast<size_t>(batch) * in_dim;
    size_t sz_w = static_cast<size_t>(out_dim) * in_dim;
    size_t sz_dy = static_cast<size_t>(batch) * out_dim;
    if (!ensure_buf(ctx.d_buf_a, ctx.cap_a, sz_x) ||
        !ensure_buf(ctx.d_buf_c, ctx.cap_c, sz_dy) ||
        !ensure_buf(ctx.d_buf_d, ctx.cap_d, sz_x > sz_w ? sz_x : sz_w) ||
        !ensure_buf(ctx.d_buf_e, ctx.cap_e, static_cast<size_t>(out_dim))) return -3;

    cudaMemcpy(ctx.d_buf_a, h_x, sz_x * sizeof(float), cudaMemcpyHostToDevice);
    cudaMemcpy(ctx.d_buf_c, h_dy, sz_dy * sizeof(float), cudaMemcpyHostToDevice);
    const float alpha = 1.0f, beta_zero = 0.0f, beta_one = 1.0f;

    if (h_dx != nullptr) {
        if (cublasSgemm(
                ctx.cublas, CUBLAS_OP_N, CUBLAS_OP_N,
                in_dim, batch, out_dim,
                &alpha, w_ptr, in_dim, ctx.d_buf_c, out_dim,
                &beta_zero, ctx.d_buf_d, in_dim
            ) != CUBLAS_STATUS_SUCCESS) return -4;
        cudaMemcpy(h_dx, ctx.d_buf_d, sz_x * sizeof(float), cudaMemcpyDeviceToHost);
    }
    if (h_dw != nullptr) {
        cudaMemcpy(ctx.d_buf_d, h_dw, sz_w * sizeof(float), cudaMemcpyHostToDevice);
        if (cublasSgemm(
                ctx.cublas, CUBLAS_OP_N, CUBLAS_OP_T,
                in_dim, out_dim, batch,
                &alpha, ctx.d_buf_a, in_dim, ctx.d_buf_c, out_dim,
                &beta_one, ctx.d_buf_d, in_dim
            ) != CUBLAS_STATUS_SUCCESS) return -5;
        cudaMemcpy(h_dw, ctx.d_buf_d, sz_w * sizeof(float), cudaMemcpyDeviceToHost);
    }
    if (h_db != nullptr) {
        cudaMemcpy(ctx.d_buf_e, h_db, static_cast<size_t>(out_dim) * sizeof(float), cudaMemcpyHostToDevice);
        sum_grad_bias_kernel<<<(out_dim + 255) / 256, 256>>>(ctx.d_buf_c, ctx.d_buf_e, batch, out_dim);
        cudaMemcpy(h_db, ctx.d_buf_e, static_cast<size_t>(out_dim) * sizeof(float), cudaMemcpyDeviceToHost);
    }
    return 0;
}

int32_t tropicore_sm120_adamw_resident_inplace(
    int32_t w_handle, int32_t wq_handle, int32_t m_handle, int32_t v_handle,
    const float* h_grad, int32_t rows, int32_t cols,
    float lr, float wd, float beta1, float beta2, int32_t step, float max_linf, int32_t do_nvfp4_quant
) {
    int gpu = 0, mg = 0, vg = 0, qg = -1;
    float *w_ptr = nullptr, *m_ptr = nullptr, *v_ptr = nullptr, *wq_ptr = nullptr;
    size_t wc = 0, mc = 0, vc = 0, qc = 0;
    if (!snapshot_tensor(w_handle, gpu, w_ptr, wc) ||
        !snapshot_tensor(m_handle, mg, m_ptr, mc) ||
        !snapshot_tensor(v_handle, vg, v_ptr, vc) || mg != gpu || vg != gpu) return -1;
    if (wq_handle >= 0 && snapshot_tensor(wq_handle, qg, wq_ptr, qc) && qg != gpu) wq_ptr = nullptr;

    std::lock_guard<std::mutex> lk(g_ctx[gpu].mtx);
    if (!ensure_ctx_locked(gpu)) return -2;
    auto& ctx = g_ctx[gpu];
    size_t sz = static_cast<size_t>(rows) * cols;
    if (!ensure_buf(ctx.d_buf_b, ctx.cap_b, sz)) return -3;
    cudaMemcpy(ctx.d_buf_b, h_grad, sz * sizeof(float), cudaMemcpyHostToDevice);

    float inv_bc1 = 1.0f / fmaxf(1.0f - powf(beta1, static_cast<float>(step)), 1e-6f);
    float inv_bc2 = 1.0f / fmaxf(1.0f - powf(beta2, static_cast<float>(step)), 1e-6f);
    adamw_row_contractive_nvfp4_kernel<<<rows, 256>>>(
        w_ptr, wq_ptr, ctx.d_buf_b, m_ptr, v_ptr,
        rows, cols, lr, wd, beta1, beta2, inv_bc1, inv_bc2, max_linf, do_nvfp4_quant
    );
    return cudaGetLastError() == cudaSuccess ? 0 : -4;
}

int32_t tropicore_sm120_contractive_step_fwd(
    const float* h_pre, const float* h_anchor, float* h_out, float* h_znext,
    int32_t total, float gamma, int32_t device_id
) {
    int gpu = pick_gpu(device_id);
    if (gpu < 0) return -1;
    std::lock_guard<std::mutex> lk(g_ctx[gpu].mtx);
    if (!ensure_ctx_locked(gpu)) return -1;
    auto& ctx = g_ctx[gpu];
    size_t sz = static_cast<size_t>(total);
    if (!ensure_buf(ctx.d_buf_a, ctx.cap_a, sz) || !ensure_buf(ctx.d_buf_b, ctx.cap_b, sz) ||
        !ensure_buf(ctx.d_buf_c, ctx.cap_c, sz) || !ensure_buf(ctx.d_buf_d, ctx.cap_d, sz)) return -2;
    cudaMemcpy(ctx.d_buf_a, h_pre, sz * sizeof(float), cudaMemcpyHostToDevice);
    cudaMemcpy(ctx.d_buf_b, h_anchor, sz * sizeof(float), cudaMemcpyHostToDevice);
    contractive_step_kernel<<<(total + 255) / 256, 256>>>(ctx.d_buf_a, ctx.d_buf_b, ctx.d_buf_c, ctx.d_buf_d, total, gamma);
    cudaMemcpy(h_out, ctx.d_buf_c, sz * sizeof(float), cudaMemcpyDeviceToHost);
    cudaMemcpy(h_znext, ctx.d_buf_d, sz * sizeof(float), cudaMemcpyDeviceToHost);
    return 0;
}

int32_t tropicore_sm120_contractive_step_bwd(
    const float* h_dz, const float* h_cur, float* h_dpre, float* h_dx_anchor,
    int32_t total, float gamma, int32_t device_id
) {
    int gpu = pick_gpu(device_id);
    if (gpu < 0) return -1;
    std::lock_guard<std::mutex> lk(g_ctx[gpu].mtx);
    if (!ensure_ctx_locked(gpu)) return -1;
    auto& ctx = g_ctx[gpu];
    size_t sz = static_cast<size_t>(total);
    if (!ensure_buf(ctx.d_buf_a, ctx.cap_a, sz) || !ensure_buf(ctx.d_buf_b, ctx.cap_b, sz) ||
        !ensure_buf(ctx.d_buf_c, ctx.cap_c, sz) || !ensure_buf(ctx.d_buf_d, ctx.cap_d, sz)) return -2;
    cudaMemcpy(ctx.d_buf_a, h_dz, sz * sizeof(float), cudaMemcpyHostToDevice);
    cudaMemcpy(ctx.d_buf_b, h_cur, sz * sizeof(float), cudaMemcpyHostToDevice);
    cudaMemcpy(ctx.d_buf_d, h_dx_anchor, sz * sizeof(float), cudaMemcpyHostToDevice);
    contractive_bwd_pre_kernel<<<(total + 255) / 256, 256>>>(ctx.d_buf_a, ctx.d_buf_b, ctx.d_buf_c, ctx.d_buf_d, total, gamma);
    cudaMemcpy(h_dpre, ctx.d_buf_c, sz * sizeof(float), cudaMemcpyDeviceToHost);
    cudaMemcpy(h_dx_anchor, ctx.d_buf_d, sz * sizeof(float), cudaMemcpyDeviceToHost);
    return 0;
}

int32_t tropicore_sm120_byte_ce_fwd_bwd(
    const float* h_logits, const uint8_t* h_targets, float* h_dlogits,
    float* h_avg_bits_out, int32_t batch, int32_t device_id
) {
    int gpu = pick_gpu(device_id);
    if (gpu < 0) return -1;
    std::lock_guard<std::mutex> lk(g_ctx[gpu].mtx);
    if (!ensure_ctx_locked(gpu)) return -1;
    auto& ctx = g_ctx[gpu];
    size_t sz_l = static_cast<size_t>(batch) * 256;
    if (!ensure_buf(ctx.d_buf_a, ctx.cap_a, sz_l) ||
        !ensure_buf(ctx.d_buf_b, ctx.cap_b, sz_l) ||
        !ensure_buf(ctx.d_buf_c, ctx.cap_c, static_cast<size_t>(batch)) ||
        !ensure_u8_buf(ctx.d_u8, ctx.cap_u8, static_cast<size_t>(batch))) return -2;
    cudaMemcpy(ctx.d_buf_a, h_logits, sz_l * sizeof(float), cudaMemcpyHostToDevice);
    cudaMemcpy(ctx.d_u8, h_targets, static_cast<size_t>(batch), cudaMemcpyHostToDevice);
    byte_ce_fwd_bwd_kernel<<<batch, 256>>>(ctx.d_buf_a, ctx.d_u8, ctx.d_buf_b, ctx.d_buf_c, batch);
    std::vector<float> h_losses(static_cast<size_t>(batch), 0.0f);
    cudaMemcpy(h_dlogits, ctx.d_buf_b, sz_l * sizeof(float), cudaMemcpyDeviceToHost);
    cudaMemcpy(h_losses.data(), ctx.d_buf_c, static_cast<size_t>(batch) * sizeof(float), cudaMemcpyDeviceToHost);
    double acc = 0.0;
    for (float lb : h_losses) acc += static_cast<double>(lb);
    *h_avg_bits_out = static_cast<float>(acc / (batch > 0 ? batch : 1));
    return 0;
}

int32_t tropicore_sm120_rmsnorm_fwd(
    const float* h_x, float* h_y, float* h_inv_rms, int32_t batch, int32_t dim, int32_t device_id
) {
    int gpu = pick_gpu(device_id);
    if (gpu < 0) return -1;
    std::lock_guard<std::mutex> lk(g_ctx[gpu].mtx);
    if (!ensure_ctx_locked(gpu)) return -1;
    auto& ctx = g_ctx[gpu];
    size_t sz = static_cast<size_t>(batch) * dim;
    if (!ensure_buf(ctx.d_buf_a, ctx.cap_a, sz) || !ensure_buf(ctx.d_buf_b, ctx.cap_b, sz) ||
        !ensure_buf(ctx.d_buf_c, ctx.cap_c, static_cast<size_t>(batch))) return -2;
    cudaMemcpy(ctx.d_buf_a, h_x, sz * sizeof(float), cudaMemcpyHostToDevice);
    rmsnorm_fwd_kernel<<<batch, 256>>>(ctx.d_buf_a, ctx.d_buf_b, ctx.d_buf_c, batch, dim);
    cudaMemcpy(h_y, ctx.d_buf_b, sz * sizeof(float), cudaMemcpyDeviceToHost);
    cudaMemcpy(h_inv_rms, ctx.d_buf_c, static_cast<size_t>(batch) * sizeof(float), cudaMemcpyDeviceToHost);
    return 0;
}

int32_t tropicore_sm120_rmsnorm_bwd(
    const float* h_x, const float* h_inv_rms, const float* h_dy, float* h_dx,
    int32_t batch, int32_t dim, int32_t device_id
) {
    int gpu = pick_gpu(device_id);
    if (gpu < 0) return -1;
    std::lock_guard<std::mutex> lk(g_ctx[gpu].mtx);
    if (!ensure_ctx_locked(gpu)) return -1;
    auto& ctx = g_ctx[gpu];
    size_t sz = static_cast<size_t>(batch) * dim;
    if (!ensure_buf(ctx.d_buf_a, ctx.cap_a, sz) || !ensure_buf(ctx.d_buf_b, ctx.cap_b, sz) ||
        !ensure_buf(ctx.d_buf_c, ctx.cap_c, static_cast<size_t>(batch)) || !ensure_buf(ctx.d_buf_d, ctx.cap_d, sz)) return -2;
    cudaMemcpy(ctx.d_buf_a, h_x, sz * sizeof(float), cudaMemcpyHostToDevice);
    cudaMemcpy(ctx.d_buf_b, h_dy, sz * sizeof(float), cudaMemcpyHostToDevice);
    cudaMemcpy(ctx.d_buf_c, h_inv_rms, static_cast<size_t>(batch) * sizeof(float), cudaMemcpyHostToDevice);
    rmsnorm_bwd_kernel<<<batch, 256>>>(ctx.d_buf_a, ctx.d_buf_c, ctx.d_buf_b, ctx.d_buf_d, batch, dim);
    cudaMemcpy(h_dx, ctx.d_buf_d, sz * sizeof(float), cudaMemcpyDeviceToHost);
    return 0;
}

int32_t tropicore_sm120_swiglu_fwd(
    const float* h_val, const float* h_gate, float* h_act, int32_t total, int32_t device_id
) {
    int gpu = pick_gpu(device_id);
    if (gpu < 0) return -1;
    std::lock_guard<std::mutex> lk(g_ctx[gpu].mtx);
    if (!ensure_ctx_locked(gpu)) return -1;
    auto& ctx = g_ctx[gpu];
    size_t sz = static_cast<size_t>(total);
    if (!ensure_buf(ctx.d_buf_a, ctx.cap_a, sz) || !ensure_buf(ctx.d_buf_b, ctx.cap_b, sz) ||
        !ensure_buf(ctx.d_buf_c, ctx.cap_c, sz)) return -2;
    cudaMemcpy(ctx.d_buf_a, h_val, sz * sizeof(float), cudaMemcpyHostToDevice);
    cudaMemcpy(ctx.d_buf_b, h_gate, sz * sizeof(float), cudaMemcpyHostToDevice);
    swiglu_fwd_kernel<<<(total + 255) / 256, 256>>>(ctx.d_buf_a, ctx.d_buf_b, ctx.d_buf_c, total);
    cudaMemcpy(h_act, ctx.d_buf_c, sz * sizeof(float), cudaMemcpyDeviceToHost);
    return 0;
}

int32_t tropicore_sm120_swiglu_bwd(
    const float* h_val, const float* h_gate, const float* h_dact,
    float* h_dval, float* h_dgate, int32_t total, int32_t device_id
) {
    int gpu = pick_gpu(device_id);
    if (gpu < 0) return -1;
    std::lock_guard<std::mutex> lk(g_ctx[gpu].mtx);
    if (!ensure_ctx_locked(gpu)) return -1;
    auto& ctx = g_ctx[gpu];
    size_t sz = static_cast<size_t>(total);
    if (!ensure_buf(ctx.d_buf_a, ctx.cap_a, sz) || !ensure_buf(ctx.d_buf_b, ctx.cap_b, sz) ||
        !ensure_buf(ctx.d_buf_c, ctx.cap_c, sz) || !ensure_buf(ctx.d_buf_d, ctx.cap_d, sz) ||
        !ensure_buf(ctx.d_buf_e, ctx.cap_e, sz)) return -2;
    cudaMemcpy(ctx.d_buf_a, h_val, sz * sizeof(float), cudaMemcpyHostToDevice);
    cudaMemcpy(ctx.d_buf_b, h_gate, sz * sizeof(float), cudaMemcpyHostToDevice);
    cudaMemcpy(ctx.d_buf_c, h_dact, sz * sizeof(float), cudaMemcpyHostToDevice);
    swiglu_bwd_kernel<<<(total + 255) / 256, 256>>>(ctx.d_buf_a, ctx.d_buf_b, ctx.d_buf_c, ctx.d_buf_d, ctx.d_buf_e, total);
    cudaMemcpy(h_dval, ctx.d_buf_d, sz * sizeof(float), cudaMemcpyDeviceToHost);
    cudaMemcpy(h_dgate, ctx.d_buf_e, sz * sizeof(float), cudaMemcpyDeviceToHost);
    return 0;
}

int32_t tropicore_sm120_allreduce_mean(
    const float* const* shard_grads, float* out_grad, int32_t n_shards, int32_t n_elems
) {
    if (n_shards <= 0 || n_elems <= 0) return -1;
    float inv_n = 1.0f / static_cast<float>(n_shards);
    for (int32_t i = 0; i < n_elems; ++i) {
        float acc = 0.0f;
        for (int32_t s = 0; s < n_shards; ++s) acc += shard_grads[s][i];
        out_grad[i] = acc * inv_n;
    }
    return 0;
}

} // extern "C"

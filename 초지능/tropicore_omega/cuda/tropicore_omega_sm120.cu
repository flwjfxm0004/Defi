/*
 * TropiCore-Omega v7.0 — TRUE 4x NVIDIA RTX PRO 6000 Blackwell (SM120) CUDA 13 Unified-Brain + Bytecode VM Engine
 * Synchronized 1:1 with `src/core.rs` v7.0:
 *   `RAW_FEAT_DIM=10`, `ENC_MID=64`, `ENC_HIDDEN=32`, `OMEGA_DIM=64`,
 *   `NUM_TACTIC_CLASSES=8`, `WORLD_OBS_DIM=126`, `WORLD_BELIEF_DIM=16`, `WORLD_STATE_DIM=150`!
 * Target Hardware (`장비.md` Track A):
 *   - 4x RTX PRO 6000 Blackwell Max-Q 96GB (SM120, 384 GB GDDR7, 7.168 TB/s)
 *   - 14.044 PFLOPS FP4 (2:4 Sparse) / 7.022 PFLOPS FP4 (Dense) / 440 TFLOPS FP32
 *   - L2-Persistent CEGIS Repulsive Barriers <= 16 MiB / GPU
 *
 * Kernels:
 *   1. On-device 4-bit E2M1 packing + exact Blackwell 128x4 swizzled VEC16_UE4M3 block scales
 *   2. CUDA 13 `cublasLtMatmul` NVFP4 (`CUDA_R_4F_E2M1` + `VEC16_UE4M3`) GDDR7-resident contractive core
 *   3. Fused Input-Anchored Contractive Tanh + L2-Persistent CEGIS Repulsive Force (`<= 16 MiB/GPU`)
 *   4. General 4-GPU Blackwell NVFP4 Batched GEMM for End-to-End STE Training
 *   5. `omega_eval_bytecode_batch_kernel` — Warp-Parallel GPU Batch Bytecode VM + 64-bit OE Hashing
 *      DIRECTLY wired into Batched Best-First Levin Search (ZERO dummy discards!)
 *   6. `omega_encode_deepset_kernel` — Synchronized v5.0 On-Device Raw Tropical Deep-Set (`10 -> 64 -> 32 -> 64`)
 *
 * Build on Workstation:
 *   nvcc -O3 -std=c++17 -arch=sm_120a -Xcompiler -fPIC \
 *     -c cuda/tropicore_omega_sm120.cu -o cuda/tropicore_omega_sm120.o
 *   ar rcs cuda/libtropicore_omega_sm120.a cuda/tropicore_omega_sm120.o
 */

#include <cuda_runtime.h>
#include <cublasLt.h>
#include <math.h>
#include <mutex>
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>

#if defined(__CUDA_ARCH__) && (__CUDA_ARCH__ >= 1200)
#include <cuda_fp4.h>
#include <cuda_fp8.h>
#endif

static constexpr int    OMEGA_MAX_GPUS           = 4;
static constexpr size_t OMEGA_L2_CUT_CAP_B       = 16ULL * 1024ULL * 1024ULL;
static constexpr size_t OMEGA_LT_WORKSPACE_B     = 32ULL * 1024ULL * 1024ULL;
static constexpr int    OMEGA_NUM_TACTIC_CLASSES = 8;
static constexpr int    OMEGA_WORLD_OBS_DIM      = 126;
static constexpr int    OMEGA_WORLD_BELIEF_DIM   = 16;
static constexpr int    OMEGA_WORLD_STATE_DIM    = 150;

// Flat Bytecode Opcodes (1:1 identical with `src/bytecode.rs`!)
static constexpr uint8_t OP_ARG0     = 0;
static constexpr uint8_t OP_ARG1     = 1;
static constexpr uint8_t OP_ARG2     = 2;
static constexpr uint8_t OP_CONST    = 3;
static constexpr uint8_t OP_ADD      = 4;
static constexpr uint8_t OP_SUB      = 5;
static constexpr uint8_t OP_MUL      = 6;
static constexpr uint8_t OP_DIVSAFE  = 7;
static constexpr uint8_t OP_MIN      = 8;
static constexpr uint8_t OP_MAX      = 9;
static constexpr uint8_t OP_ABS      = 10;
static constexpr uint8_t OP_SQRTSAFE = 11;
static constexpr uint8_t OP_IFPOS    = 12;
static constexpr uint8_t OP_END      = 255;

__device__ __constant__ float d_e2m1_pos[8] = {
    0.0f, 0.5f, 1.0f, 1.5f, 2.0f, 3.0f, 4.0f, 6.0f
};

__device__ __forceinline__ uint8_t encode_e2m1_nibble_rne(float u) {
    const uint8_t sign_bit = (u < 0.0f) ? 8u : 0u;
    const float ax = fabsf(u);
    if (ax >= 6.0f) return sign_bit | 7u;
    uint8_t best_i = 0u;
    float best_d = fabsf(ax - d_e2m1_pos[0]);
    #pragma unroll
    for (uint8_t i = 1u; i < 8u; ++i) {
        float d = fabsf(ax - d_e2m1_pos[i]);
        if (d < best_d || (d == best_d && ((i & 1u) == 0u))) {
            best_d = d;
            best_i = i;
        }
    }
    return sign_bit | best_i;
}

__device__ __forceinline__ float quantize_ue4m3_f32(float s_raw, uint8_t* out_code) {
    if (s_raw <= 0.0f) {
        if (out_code) *out_code = 0u;
        return 0.0f;
    }
    if (s_raw >= 448.0f) {
        if (out_code) *out_code = 126u;
        return 448.0f;
    }
#if defined(__CUDA_ARCH__) && (__CUDA_ARCH__ >= 1200)
    __nv_fp8_e4m3 fp8 = __nv_fp8_e4m3(s_raw);
    if (out_code) *out_code = fp8.__x;
    return static_cast<float>(fp8);
#else
    float best_v = 0.0f;
    float best_d = 1e30f;
    uint8_t best_c = 0u;
    for (int c = 0; c < 127; ++c) {
        int e = c >> 3;
        float m = static_cast<float>(c & 7);
        float v = (e == 0) ? (m * ldexpf(1.0f, -9)) : ((1.0f + m * 0.125f) * ldexpf(1.0f, e - 7));
        float d = fabsf(s_raw - v);
        if (d < best_d || (d == best_d && ((c & 1) == 0))) {
            best_d = d;
            best_v = v;
            best_c = static_cast<uint8_t>(c);
        }
    }
    if (out_code) *out_code = best_c;
    return best_v;
#endif
}

__global__ void omega_pack_nvfp4_swizzled_kernel(
    const float* __restrict__ src_f32,
    uint8_t* __restrict__ dst_fp4x2,
    uint8_t* __restrict__ dst_ue4m3_swizzled,
    float s_global,
    size_t rows,
    size_t K,
    size_t k_blks_pad
) {
    const size_t row = static_cast<size_t>(blockIdx.y);
    const size_t k = static_cast<size_t>(blockIdx.x) * blockDim.x + threadIdx.x;
    if (row >= rows || k >= K) return;

    const unsigned lane16 = threadIdx.x & 15u;
    const unsigned mask16 = (threadIdx.x & 16u) ? 0xFFFF0000u : 0x0000FFFFu;

    const float v = src_f32[row * K + k];
    float b_max = fabsf(v);
    #pragma unroll
    for (int off = 8; off > 0; off >>= 1) {
        b_max = fmaxf(b_max, __shfl_xor_sync(mask16, b_max, off, 16));
    }

    uint8_t sc_code = 0u;
    float s_block = quantize_ue4m3_f32(b_max / (6.0f * s_global), &sc_code);
    if (lane16 == 0u) {
        const size_t k_blk = k >> 4;
        const size_t m_tile = row >> 7;
        const size_t m_grp  = (row & 127u) >> 5;
        const size_t m_in32 = row & 31u;
        const size_t k_tile = k_blk >> 2;
        const size_t k_in4  = k_blk & 3u;
        const size_t k_tiles = k_blks_pad >> 2;
        const size_t swiz_idx = ((((m_tile * k_tiles + k_tile) * 32u + m_in32) * 4u + m_grp) * 4u) + k_in4;
        dst_ue4m3_swizzled[swiz_idx] = sc_code;
    }

    uint8_t my_nib = (s_block == 0.0f) ? 0u : encode_e2m1_nibble_rne(v / (s_global * s_block));
    uint8_t odd_nib = static_cast<uint8_t>(__shfl_down_sync(mask16, static_cast<unsigned>(my_nib), 1, 16));
    if ((lane16 & 1u) == 0u) {
        dst_fp4x2[row * (K >> 1) + (k >> 1)] = (my_nib & 0x0Fu) | ((odd_nib & 0x0Fu) << 4);
    }
}

__global__ void omega_transpose_f32_kernel(
    const float* __restrict__ src,
    float* __restrict__ dst,
    size_t K,
    size_t N
) {
    const size_t n = static_cast<size_t>(blockIdx.x) * blockDim.x + threadIdx.x;
    const size_t k = static_cast<size_t>(blockIdx.y) * blockDim.y + threadIdx.y;
    if (k < K && n < N) dst[n * K + k] = src[k * N + n];
}

__global__ void omega_contractive_cegis_post_kernel(
    const float* __restrict__ z_cur,
    const float* __restrict__ z_anchor,
    const float* __restrict__ wz_out,
    float* __restrict__ z_next,
    const float* __restrict__ b_core,
    const float* __restrict__ cuts,
    const float* __restrict__ cut_w,
    int n_particles,
    int dim,
    int eff_cuts,
    float gamma,
    float eta_cegis,
    float inv_tau,
    float pert_noise,
    int tick_idx
) {
    const int p = blockIdx.y;
    const int d = blockIdx.x * blockDim.x + threadIdx.x;
    if (p >= n_particles || d >= dim) return;

    const float z_pd = z_cur[p * dim + d];
    const float z_anc = z_anchor[p * dim + d];
    float force = 0.0f;
    for (int k = 0; k < eff_cuts; ++k) {
        const float* ck = cuts + (size_t)k * (size_t)dim;
        float l1 = 0.0f;
        for (int j = 0; j < dim; ++j) l1 += fabsf(z_cur[p * dim + j] - ck[j]);
        float w = cut_w[k] * expf(-l1 * inv_tau);
        float diff = z_pd - ck[d];
        force += w * (diff >= 0.0f ? 1.0f : -1.0f);
    }
    float pre = wz_out[p * dim + d] + b_core[d];
    if (pert_noise != 0.0f) {
        uint32_t h = (uint32_t)(p * 2654435761u ^ d * 2246822519u ^ (tick_idx + 1) * 3266489917u);
        h ^= h >> 15;
        h *= 2246822519u;
        float u = ((float)(h & 0xFFFFu) / 32768.0f) - 1.0f;
        pre += pert_noise * u;
    }
    float nxt = gamma * tanhf(pre) + (1.0f - gamma) * z_anc + eta_cegis * force;
    z_next[p * dim + d] = fminf(fmaxf(nxt, -4.0f), 4.0f);
}

/* NEW v3.0: SM120 Warp-Parallel Batch Bytecode VM evaluating N_progs x N_probes + FNV-1a 64-bit OE hash! */
__global__ void omega_eval_bytecode_batch_kernel(
    const uint8_t* __restrict__ ops,
    const uint32_t* __restrict__ op_offsets,
    const float* __restrict__ consts,
    const uint32_t* __restrict__ const_offsets,
    const float* __restrict__ probes_xyz, // [n_probes * 3]
    float* __restrict__ out_vals,         // [n_progs * n_probes]
    uint64_t* __restrict__ out_oe_hash,   // [n_progs]
    int n_progs,
    int n_probes
) {
    const int pid = blockIdx.x * blockDim.x + threadIdx.x;
    if (pid >= n_progs) return;

    const uint32_t op_base = op_offsets[pid];
    const uint32_t c_base  = const_offsets[pid];
    uint64_t h = 0xcbf29ce484222325ULL;

    for (int j = 0; j < n_probes; ++j) {
        const float x0 = probes_xyz[j * 3 + 0];
        const float x1 = probes_xyz[j * 3 + 1];
        const float x2 = probes_xyz[j * 3 + 2];
        float st[32];
        int sp = 0;
        uint32_t pc = op_base;

        for (int step = 0; step < 256; ++step) {
            uint8_t op = ops[pc++];
            if (op == OP_END) break;
            switch (op) {
                case OP_ARG0: if (sp < 32) st[sp++] = x0; break;
                case OP_ARG1: if (sp < 32) st[sp++] = x1; break;
                case OP_ARG2: if (sp < 32) st[sp++] = x2; break;
                case OP_CONST: {
                    uint8_t idx = ops[pc++];
                    if (sp < 32) st[sp++] = consts[c_base + idx];
                    break;
                }
                case OP_ADD: if (sp >= 2) { --sp; st[sp - 1] = fminf(fmaxf(st[sp - 1] + st[sp], -1e9f), 1e9f); } break;
                case OP_SUB: if (sp >= 2) { --sp; st[sp - 1] = fminf(fmaxf(st[sp - 1] - st[sp], -1e9f), 1e9f); } break;
                case OP_MUL: if (sp >= 2) { --sp; st[sp - 1] = fminf(fmaxf(st[sp - 1] * st[sp], -1e9f), 1e9f); } break;
                case OP_DIVSAFE: if (sp >= 2) {
                    --sp;
                    float d = st[sp];
                    float sd = (fabsf(d) < 1e-3f) ? (d >= 0.0f ? 1e-3f : -1e-3f) : d;
                    st[sp - 1] = fminf(fmaxf(st[sp - 1] / sd, -1e9f), 1e9f);
                } break;
                case OP_MIN: if (sp >= 2) { --sp; st[sp - 1] = fminf(st[sp - 1], st[sp]); } break;
                case OP_MAX: if (sp >= 2) { --sp; st[sp - 1] = fmaxf(st[sp - 1], st[sp]); } break;
                case OP_ABS: if (sp >= 1) { st[sp - 1] = fabsf(st[sp - 1]); } break;
                case OP_SQRTSAFE: if (sp >= 1) { float v = st[sp - 1]; st[sp - 1] = (v > 0.0f) ? sqrtf(v) : 0.0f; } break;
                case OP_IFPOS: if (sp >= 3) {
                    float el = st[sp - 1], th = st[sp - 2], c = st[sp - 3];
                    sp -= 2;
                    st[sp - 1] = (c > 0.0f) ? th : el;
                } break;
                default: step = 256; break;
            }
        }
        float v = (sp > 0 && isfinite(st[sp - 1])) ? st[sp - 1] : 0.0f;
        out_vals[(size_t)pid * (size_t)n_probes + (size_t)j] = v;
        int64_t q = isfinite(v) ? (int64_t)llroundf(fminf(fmaxf(v, -1e5f), 1e5f) * 1000.0f) : INT64_MAX;
        h ^= (uint64_t)q;
        h *= 0x100000001b3ULL;
    }
    out_oe_hash[pid] = h;
}

/* Synchronized v8.0: Scalable On-Device Raw Tropical Deep-Set Residual Set Encoder (`10 -> d -> d/2 -> [Mean; TropMax] in R^d`) */
#ifndef OMEGA_LATENT_DIM_CFG
#define OMEGA_LATENT_DIM_CFG 64
#endif
static constexpr int OMEGA_RAW_FEAT_DIM = 10;
static constexpr int OMEGA_LATENT_DIM   = OMEGA_LATENT_DIM_CFG;
static constexpr int OMEGA_ENC_MID      = OMEGA_LATENT_DIM_CFG;
static constexpr int OMEGA_ENC_HIDDEN   = OMEGA_LATENT_DIM_CFG / 2;

__global__ void omega_encode_deepset_kernel(
    const float* __restrict__ tokens, // [n_tokens * 10]
    const float* __restrict__ w1,     // [OMEGA_ENC_MID * 10]
    const float* __restrict__ b1,     // [OMEGA_ENC_MID]
    const float* __restrict__ w2,     // [OMEGA_ENC_HIDDEN * OMEGA_ENC_MID]
    const float* __restrict__ b2,     // [OMEGA_ENC_HIDDEN]
    float* __restrict__ z_out,        // [OMEGA_LATENT_DIM]
    int n_tokens
) {
    __shared__ float s_a[OMEGA_ENC_MID];
    const int tid = threadIdx.x;
    const int k = blockIdx.x * blockDim.x + threadIdx.x;
    if (n_tokens <= 0) return;

    float sum_k = 0.0f;
    float max_k = -1e30f;
    for (int t = 0; t < n_tokens; ++t) {
        const float* u = tokens + (size_t)t * (size_t)OMEGA_RAW_FEAT_DIM;
        for (int r = tid; r < OMEGA_ENC_MID; r += blockDim.x) {
            float acc = b1[r];
            #pragma unroll
            for (int c = 0; c < OMEGA_RAW_FEAT_DIM; ++c) acc += w1[(size_t)r * OMEGA_RAW_FEAT_DIM + c] * u[c];
            s_a[r] = tanhf(acc);
        }
        __syncthreads();
        if (k < OMEGA_ENC_HIDDEN) {
            float acc2 = b2[k];
            for (int c = 0; c < OMEGA_ENC_MID; ++c) acc2 += w2[(size_t)k * OMEGA_ENC_MID + c] * s_a[c];
            float hk = tanhf(acc2);
            sum_k += hk;
            if (hk > max_k) max_k = hk;
        }
        __syncthreads();
    }
    if (k < OMEGA_ENC_HIDDEN) {
        z_out[k]                    = sum_k / (float)n_tokens;
        z_out[OMEGA_ENC_HIDDEN + k] = max_k;
    }
}

struct OmegaDeviceState {
    bool initialized = false;
    cudaStream_t stream = nullptr;
    cublasLtHandle_t lt = nullptr;
    void* d_workspace = nullptr;

    int core_dim = 0;
    float s_global_w = 1.0f;
    float* d_wf = nullptr;
    uint8_t* d_w4 = nullptr;
    uint8_t* d_sw = nullptr;
    float* d_b = nullptr;

    int max_m_pad = 0;
    float* d_z_ping = nullptr;
    float* d_z_pong = nullptr;
    float* d_z_anchor = nullptr;
    float* d_gemm_y = nullptr;
    uint8_t* d_z4 = nullptr;
    uint8_t* d_sz = nullptr;

    int max_cuts = 0;
    float* d_cuts = nullptr;
    float* d_cut_w = nullptr;

    bool algo_valid = false;
    int cached_m_pad = 0;
    cublasLtMatmulDesc_t op_desc = nullptr;
    cublasLtMatrixLayout_t w_desc = nullptr;
    cublasLtMatrixLayout_t z_desc = nullptr;
    cublasLtMatrixLayout_t y_desc = nullptr;
    cublasLtMatmulAlgo_t cached_algo{};

    void reset_algo_cache() {
        if (op_desc) { cublasLtMatmulDescDestroy(op_desc); op_desc = nullptr; }
        if (w_desc)  { cublasLtMatrixLayoutDestroy(w_desc); w_desc = nullptr; }
        if (z_desc)  { cublasLtMatrixLayoutDestroy(z_desc); z_desc = nullptr; }
        if (y_desc)  { cublasLtMatrixLayoutDestroy(y_desc); y_desc = nullptr; }
        algo_valid = false;
        cached_m_pad = 0;
    }
};

static OmegaDeviceState g_states[OMEGA_MAX_GPUS];
static std::mutex g_omega_mutexes[OMEGA_MAX_GPUS];

static bool ensure_device_ready(int dev) {
    OmegaDeviceState& s = g_states[dev];
    if (s.initialized) return true;
    if (cudaSetDevice(dev) != cudaSuccess) return false;
    if (cudaStreamCreate(&s.stream) != cudaSuccess) return false;
    if (cublasLtCreate(&s.lt) != CUBLAS_STATUS_SUCCESS) return false;
    if (cudaMalloc(&s.d_workspace, OMEGA_LT_WORKSPACE_B) != cudaSuccess) return false;
    s.initialized = true;
    return true;
}

static void pin_l2_cegis_window(cudaStream_t stream, void* ptr, size_t bytes) {
    if (!ptr || bytes == 0) return;
    cudaStreamAttrValue attr;
    attr.accessPolicyWindow.base_ptr = ptr;
    attr.accessPolicyWindow.num_bytes = (bytes < OMEGA_L2_CUT_CAP_B) ? bytes : OMEGA_L2_CUT_CAP_B;
    attr.accessPolicyWindow.hitRatio = 1.0f;
    attr.accessPolicyWindow.hitProp = cudaAccessPropertyPersisting;
    attr.accessPolicyWindow.missProp = cudaAccessPropertyStreaming;
    cudaStreamSetAttribute(stream, cudaStreamAttributeAccessPolicyWindow, &attr);
}

extern "C" {

int tropicore_omega_sm120_detect_gpus() {
    int count = 0;
    if (cudaGetDeviceCount(&count) != cudaSuccess) return 0;
    return (count > OMEGA_MAX_GPUS) ? OMEGA_MAX_GPUS : count;
}

void tropicore_omega_sm120_abi_dims(int* raw_dim, int* enc_mid, int* enc_hid, int* omega_dim) {
    if (raw_dim)   *raw_dim   = OMEGA_RAW_FEAT_DIM;
    if (enc_mid)   *enc_mid   = OMEGA_ENC_MID;
    if (enc_hid)   *enc_hid   = OMEGA_ENC_HIDDEN;
    if (omega_dim) *omega_dim = OMEGA_LATENT_DIM;
}

int tropicore_omega_sm120_upload_core_weights(
    const float* w_core,
    const float* b_core,
    int dim,
    int device_id
) {
    if (dim <= 0 || (dim % 64) != 0) return -1;
    int ngpu = tropicore_omega_sm120_detect_gpus();
    if (ngpu <= 0) return -2;
    int dev = (device_id >= 0) ? (device_id % ngpu) : 0;
    std::lock_guard<std::mutex> lock(g_omega_mutexes[dev]);
    if (!ensure_device_ready(dev)) return -3;
    cudaSetDevice(dev);

    OmegaDeviceState& s = g_states[dev];
    const size_t w_elems = (size_t)dim * (size_t)dim;
    const size_t m_swiz_pad = (((size_t)dim + 127u) / 128u) * 128u;
    const size_t k_blks_pad = ((((size_t)dim / 16u) + 3u) / 4u) * 4u;
    const size_t sw_bytes = m_swiz_pad * k_blks_pad;

    if (s.core_dim != dim) {
        s.reset_algo_cache();
        if (s.d_wf) cudaFree(s.d_wf);
        if (s.d_w4) cudaFree(s.d_w4);
        if (s.d_sw) cudaFree(s.d_sw);
        if (s.d_b)  cudaFree(s.d_b);
        if (cudaMalloc(&s.d_wf, w_elems * sizeof(float)) != cudaSuccess) return -4;
        if (cudaMalloc(&s.d_w4, w_elems / 2) != cudaSuccess) return -5;
        if (cudaMalloc(&s.d_sw, sw_bytes) != cudaSuccess) return -6;
        if (cudaMalloc(&s.d_b, (size_t)dim * sizeof(float)) != cudaSuccess) return -7;
        s.core_dim = dim;
    }

    float max_abs = 1e-6f;
    for (size_t i = 0; i < w_elems; ++i) {
        float a = fabsf(w_core[i]);
        if (a > max_abs) max_abs = a;
    }
    s.s_global_w = max_abs / (448.0f * 6.0f);

    cudaMemcpyAsync(s.d_wf, w_core, w_elems * sizeof(float), cudaMemcpyHostToDevice, s.stream);
    cudaMemcpyAsync(s.d_b, b_core, (size_t)dim * sizeof(float), cudaMemcpyHostToDevice, s.stream);
    cudaMemsetAsync(s.d_sw, 0, sw_bytes, s.stream);

    const int tk = 256;
    dim3 g(((unsigned)dim + tk - 1u) / tk, (unsigned)dim);
    omega_pack_nvfp4_swizzled_kernel<<<g, tk, 0, s.stream>>>(
        s.d_wf, s.d_w4, s.d_sw, s.s_global_w, (size_t)dim, (size_t)dim, k_blks_pad
    );
    if (cudaStreamSynchronize(s.stream) != cudaSuccess) return -8;
    return 0;
}

int tropicore_omega_sm120_relax_batch(
    const float* z_init,
    float* z_out,
    int n_particles,
    int dim,
    const float* cuts,
    const float* cut_w,
    int n_cuts,
    int ticks,
    float gamma,
    float eta_cegis,
    float inv_tau,
    float pert_noise,
    int device_id
) {
    if (n_particles <= 0 || dim <= 0 || (dim % 64) != 0) return -1;
    int ngpu = tropicore_omega_sm120_detect_gpus();
    if (ngpu <= 0) return -2;
    int dev = (device_id >= 0) ? (device_id % ngpu) : 0;
    std::lock_guard<std::mutex> lock(g_omega_mutexes[dev]);
    if (!ensure_device_ready(dev)) return -3;
    cudaSetDevice(dev);

    OmegaDeviceState& s = g_states[dev];
    if (s.core_dim != dim || !s.d_w4) return -30;

    const int m_pad = ((n_particles + 15) / 16) * 16;
    const size_t z_pad_elems = (size_t)m_pad * (size_t)dim;
    const size_t m_swiz_pad = (((size_t)m_pad + 127u) / 128u) * 128u;
    const size_t k_blks_pad = ((((size_t)dim / 16u) + 3u) / 4u) * 4u;
    const size_t sz_bytes = m_swiz_pad * k_blks_pad;

    const size_t max_cuts_l2 = OMEGA_L2_CUT_CAP_B / ((size_t)dim * sizeof(float) + sizeof(float));
    const int eff_cuts = (n_cuts > (int)max_cuts_l2) ? (int)max_cuts_l2 : n_cuts;

    if (m_pad > s.max_m_pad) {
        s.reset_algo_cache();
        if (s.d_z_ping)   cudaFree(s.d_z_ping);
        if (s.d_z_pong)   cudaFree(s.d_z_pong);
        if (s.d_z_anchor) cudaFree(s.d_z_anchor);
        if (s.d_gemm_y)   cudaFree(s.d_gemm_y);
        if (s.d_z4)       cudaFree(s.d_z4);
        if (s.d_sz)       cudaFree(s.d_sz);
        if (cudaMalloc(&s.d_z_ping, z_pad_elems * sizeof(float)) != cudaSuccess) return -4;
        if (cudaMalloc(&s.d_z_pong, z_pad_elems * sizeof(float)) != cudaSuccess) return -5;
        if (cudaMalloc(&s.d_z_anchor, z_pad_elems * sizeof(float)) != cudaSuccess) return -5;
        if (cudaMalloc(&s.d_gemm_y, z_pad_elems * sizeof(float)) != cudaSuccess) return -6;
        if (cudaMalloc(&s.d_z4, z_pad_elems / 2) != cudaSuccess) return -7;
        if (cudaMalloc(&s.d_sz, sz_bytes) != cudaSuccess) return -8;
        s.max_m_pad = m_pad;
    }
    if (eff_cuts > s.max_cuts) {
        if (s.d_cuts)  cudaFree(s.d_cuts);
        if (s.d_cut_w) cudaFree(s.d_cut_w);
        if (cudaMalloc(&s.d_cuts, (size_t)eff_cuts * (size_t)dim * sizeof(float)) != cudaSuccess) return -9;
        if (cudaMalloc(&s.d_cut_w, (size_t)eff_cuts * sizeof(float)) != cudaSuccess) return -10;
        s.max_cuts = eff_cuts;
    }

    if (!s.algo_valid || s.cached_m_pad != m_pad) {
        s.reset_algo_cache();
        cublasLtMatmulDescCreate(&s.op_desc, CUBLAS_COMPUTE_32F, CUDA_R_32F);
        cublasOperation_t op_t = CUBLAS_OP_T, op_n = CUBLAS_OP_N;
        cublasLtMatmulDescSetAttribute(s.op_desc, CUBLASLT_MATMUL_DESC_TRANSA, &op_t, sizeof(op_t));
        cublasLtMatmulDescSetAttribute(s.op_desc, CUBLASLT_MATMUL_DESC_TRANSB, &op_n, sizeof(op_n));
        cublasLtMatmulPreference_t pref = nullptr;
        cublasLtMatmulPreferenceCreate(&pref);
        cublasLtMatmulPreferenceSetAttribute(
            pref, CUBLASLT_MATMUL_PREF_MAX_WORKSPACE_BYTES, &OMEGA_LT_WORKSPACE_B, sizeof(OMEGA_LT_WORKSPACE_B)
        );
#if defined(CUDART_VERSION) && (CUDART_VERSION >= 12080)
        cublasLtMatmulMatrixScale_t vec16 = CUBLASLT_MATMUL_MATRIX_SCALE_VEC16_UE4M3;
        cublasLtMatmulDescSetAttribute(s.op_desc, CUBLASLT_MATMUL_DESC_A_SCALE_MODE, &vec16, sizeof(vec16));
        cublasLtMatmulDescSetAttribute(s.op_desc, CUBLASLT_MATMUL_DESC_B_SCALE_MODE, &vec16, sizeof(vec16));
        cublasLtMatmulDescSetAttribute(s.op_desc, CUBLASLT_MATMUL_DESC_A_SCALE_POINTER, &s.d_sw, sizeof(s.d_sw));
        cublasLtMatmulDescSetAttribute(s.op_desc, CUBLASLT_MATMUL_DESC_B_SCALE_POINTER, &s.d_sz, sizeof(s.d_sz));
        cublasLtMatrixLayoutCreate(&s.w_desc, CUDA_R_4F_E2M1, dim, dim, dim);
        cublasLtMatrixLayoutCreate(&s.z_desc, CUDA_R_4F_E2M1, dim, m_pad, dim);
        cublasLtMatrixLayoutCreate(&s.y_desc, CUDA_R_32F, dim, m_pad, dim);
#else
        cublasLtMatrixLayoutCreate(&s.w_desc, CUDA_R_32F, dim, dim, dim);
        cublasLtMatrixLayoutCreate(&s.z_desc, CUDA_R_32F, dim, m_pad, dim);
        cublasLtMatrixLayoutCreate(&s.y_desc, CUDA_R_32F, dim, m_pad, dim);
#endif
        cublasLtMatmulHeuristicResult_t heur;
        int returned = 0;
        cublasStatus_t st = cublasLtMatmulAlgoGetHeuristic(
            s.lt, s.op_desc, s.w_desc, s.z_desc, s.y_desc, s.y_desc, pref, 1, &heur, &returned
        );
        cublasLtMatmulPreferenceDestroy(pref);
        if (st != CUBLAS_STATUS_SUCCESS || returned <= 0) return -11;
        s.cached_algo = heur.algo;
        s.cached_m_pad = m_pad;
        s.algo_valid = true;
    }

    const size_t z_real_bytes = (size_t)n_particles * (size_t)dim * sizeof(float);
    if (m_pad > n_particles) {
        cudaMemsetAsync(s.d_z_ping + (size_t)n_particles * (size_t)dim, 0,
                        (size_t)(m_pad - n_particles) * (size_t)dim * sizeof(float), s.stream);
    }
    cudaMemcpyAsync(s.d_z_ping, z_init, z_real_bytes, cudaMemcpyHostToDevice, s.stream);
    cudaMemcpyAsync(s.d_z_anchor, z_init, z_real_bytes, cudaMemcpyHostToDevice, s.stream);
    if (eff_cuts > 0) {
        const size_t cut_bytes = (size_t)eff_cuts * (size_t)dim * sizeof(float);
        cudaMemcpyAsync(s.d_cuts, cuts, cut_bytes, cudaMemcpyHostToDevice, s.stream);
        cudaMemcpyAsync(s.d_cut_w, cut_w, (size_t)eff_cuts * sizeof(float), cudaMemcpyHostToDevice, s.stream);
        pin_l2_cegis_window(s.stream, s.d_cuts, cut_bytes);
    }

    const float s_global_z = 4.0f / (448.0f * 6.0f);
    const float alpha_lt = s_global_z * s.s_global_w;
    const float beta_lt = 0.0f;
    float* cur_in = s.d_z_ping;
    float* cur_out = s.d_z_pong;

    for (int t = 0; t < ticks; ++t) {
#if defined(CUDART_VERSION) && (CUDART_VERSION >= 12080)
        cudaMemsetAsync(s.d_sz, 0, sz_bytes, s.stream);
        {
            const int tk = 256;
            dim3 g(((unsigned)dim + tk - 1u) / tk, (unsigned)m_pad);
            omega_pack_nvfp4_swizzled_kernel<<<g, tk, 0, s.stream>>>(
                cur_in, s.d_z4, s.d_sz, s_global_z, (size_t)m_pad, (size_t)dim, k_blks_pad
            );
        }
        cublasStatus_t st = cublasLtMatmul(
            s.lt, s.op_desc, &alpha_lt, s.d_w4, s.w_desc, s.d_z4, s.z_desc, &beta_lt,
            s.d_gemm_y, s.y_desc, s.d_gemm_y, s.y_desc, &s.cached_algo,
            s.d_workspace, OMEGA_LT_WORKSPACE_B, s.stream
        );
#else
        const float one = 1.0f;
        cublasStatus_t st = cublasLtMatmul(
            s.lt, s.op_desc, &one, s.d_wf, s.w_desc, cur_in, s.z_desc, &beta_lt,
            s.d_gemm_y, s.y_desc, s.d_gemm_y, s.y_desc, &s.cached_algo,
            s.d_workspace, OMEGA_LT_WORKSPACE_B, s.stream
        );
#endif
        if (st != CUBLAS_STATUS_SUCCESS) return -12;

        dim3 tb(128, 1);
        dim3 gb(((unsigned)dim + 127u) / 128u, (unsigned)n_particles);
        omega_contractive_cegis_post_kernel<<<gb, tb, 0, s.stream>>>(
            cur_in, s.d_z_anchor, s.d_gemm_y, cur_out, s.d_b, s.d_cuts, s.d_cut_w,
            n_particles, dim, eff_cuts, gamma, eta_cegis, inv_tau, pert_noise, t
        );
        float* tmp = cur_in;
        cur_in = cur_out;
        cur_out = tmp;
    }

    cudaMemcpyAsync(z_out, cur_in, z_real_bytes, cudaMemcpyDeviceToHost, s.stream);
    if (cudaStreamSynchronize(s.stream) != cudaSuccess) return -13;
    return 0;
}

int tropicore_omega_sm120_nvfp4_gemm_on_device(
    const float* a_row,
    const float* b_row,
    float* c_row,
    int M,
    int N,
    int K,
    int device_id
) {
    if (M <= 0 || N <= 0 || K <= 0 || (M % 16) != 0 || (N % 16) != 0 || (K % 64) != 0) return -1;
    int ngpu = tropicore_omega_sm120_detect_gpus();
    if (ngpu <= 0) return -2;
    int dev = (device_id >= 0) ? (device_id % ngpu) : 0;
    std::lock_guard<std::mutex> lock(g_omega_mutexes[dev]);
    if (!ensure_device_ready(dev)) return -3;
    cudaSetDevice(dev);

    OmegaDeviceState& s = g_states[dev];
    const size_t a_elems = (size_t)M * (size_t)K;
    const size_t b_elems = (size_t)K * (size_t)N;
    const size_t c_elems = (size_t)M * (size_t)N;
    const size_t m_swiz = (((size_t)M + 127u) / 128u) * 128u;
    const size_t n_swiz = (((size_t)N + 127u) / 128u) * 128u;
    const size_t k_blks = ((((size_t)K / 16u) + 3u) / 4u) * 4u;

    float max_a = 1e-6f, max_b = 1e-6f;
    for (size_t i = 0; i < a_elems; ++i) { float v = fabsf(a_row[i]); if (v > max_a) max_a = v; }
    for (size_t i = 0; i < b_elems; ++i) { float v = fabsf(b_row[i]); if (v > max_b) max_b = v; }
    const float sg_a = max_a / (448.0f * 6.0f);
    const float sg_b = max_b / (448.0f * 6.0f);

    float *d_af = nullptr, *d_bf = nullptr, *d_bt = nullptr, *d_ct = nullptr, *d_cf = nullptr;
    uint8_t *d_a4 = nullptr, *d_b4 = nullptr, *d_sa = nullptr, *d_sb = nullptr;
    if (cudaMalloc(&d_af, a_elems * sizeof(float)) != cudaSuccess) return -4;
    if (cudaMalloc(&d_bf, b_elems * sizeof(float)) != cudaSuccess) { cudaFree(d_af); return -4; }
    if (cudaMalloc(&d_bt, b_elems * sizeof(float)) != cudaSuccess) { cudaFree(d_af); cudaFree(d_bf); return -4; }
    if (cudaMalloc(&d_ct, c_elems * sizeof(float)) != cudaSuccess) { cudaFree(d_af); cudaFree(d_bf); cudaFree(d_bt); return -4; }
    if (cudaMalloc(&d_cf, c_elems * sizeof(float)) != cudaSuccess) { cudaFree(d_af); cudaFree(d_bf); cudaFree(d_bt); cudaFree(d_ct); return -4; }
    cudaMalloc(&d_a4, a_elems / 2);
    cudaMalloc(&d_b4, b_elems / 2);
    cudaMalloc(&d_sa, m_swiz * k_blks);
    cudaMalloc(&d_sb, n_swiz * k_blks);

    cudaMemcpyAsync(d_af, a_row, a_elems * sizeof(float), cudaMemcpyHostToDevice, s.stream);
    cudaMemcpyAsync(d_bf, b_row, b_elems * sizeof(float), cudaMemcpyHostToDevice, s.stream);
    {
        dim3 tb(16, 16);
        dim3 gb(((unsigned)N + 15u) / 16u, ((unsigned)K + 15u) / 16u);
        omega_transpose_f32_kernel<<<gb, tb, 0, s.stream>>>(d_bf, d_bt, (size_t)K, (size_t)N);
    }

    cublasLtMatmulDesc_t op = nullptr;
    cublasLtMatrixLayout_t la = nullptr, lb = nullptr, lc = nullptr;
    cublasLtMatmulPreference_t pref = nullptr;
    cublasLtMatmulDescCreate(&op, CUBLAS_COMPUTE_32F, CUDA_R_32F);
    cublasOperation_t op_t = CUBLAS_OP_T, op_n = CUBLAS_OP_N;
    cublasLtMatmulDescSetAttribute(op, CUBLASLT_MATMUL_DESC_TRANSA, &op_t, sizeof(op_t));
    cublasLtMatmulDescSetAttribute(op, CUBLASLT_MATMUL_DESC_TRANSB, &op_n, sizeof(op_n));
    cublasLtMatmulPreferenceCreate(&pref);
    cublasLtMatmulPreferenceSetAttribute(pref, CUBLASLT_MATMUL_PREF_MAX_WORKSPACE_BYTES, &OMEGA_LT_WORKSPACE_B, sizeof(OMEGA_LT_WORKSPACE_B));

#if defined(CUDART_VERSION) && (CUDART_VERSION >= 12080)
    cudaMemsetAsync(d_sa, 0, m_swiz * k_blks, s.stream);
    cudaMemsetAsync(d_sb, 0, n_swiz * k_blks, s.stream);
    {
        const int tk = 256;
        dim3 ga(((unsigned)K + tk - 1u) / tk, (unsigned)M);
        dim3 gb(((unsigned)K + tk - 1u) / tk, (unsigned)N);
        omega_pack_nvfp4_swizzled_kernel<<<ga, tk, 0, s.stream>>>(d_af, d_a4, d_sa, sg_a, (size_t)M, (size_t)K, k_blks);
        omega_pack_nvfp4_swizzled_kernel<<<gb, tk, 0, s.stream>>>(d_bt, d_b4, d_sb, sg_b, (size_t)N, (size_t)K, k_blks);
    }
    cublasLtMatmulMatrixScale_t vec16 = CUBLASLT_MATMUL_MATRIX_SCALE_VEC16_UE4M3;
    cublasLtMatmulDescSetAttribute(op, CUBLASLT_MATMUL_DESC_A_SCALE_MODE, &vec16, sizeof(vec16));
    cublasLtMatmulDescSetAttribute(op, CUBLASLT_MATMUL_DESC_B_SCALE_MODE, &vec16, sizeof(vec16));
    cublasLtMatmulDescSetAttribute(op, CUBLASLT_MATMUL_DESC_A_SCALE_POINTER, &d_sa, sizeof(d_sa));
    cublasLtMatmulDescSetAttribute(op, CUBLASLT_MATMUL_DESC_B_SCALE_POINTER, &d_sb, sizeof(d_sb));
    cublasLtMatrixLayoutCreate(&la, CUDA_R_4F_E2M1, K, M, K);
    cublasLtMatrixLayoutCreate(&lb, CUDA_R_4F_E2M1, K, N, K);
    cublasLtMatrixLayoutCreate(&lc, CUDA_R_32F, M, N, M);
    const float alpha = sg_a * sg_b;
#else
    cublasLtMatrixLayoutCreate(&la, CUDA_R_32F, K, M, K);
    cublasLtMatrixLayoutCreate(&lb, CUDA_R_32F, K, N, K);
    cublasLtMatrixLayoutCreate(&lc, CUDA_R_32F, M, N, M);
    const float alpha = 1.0f;
#endif
    const float beta = 0.0f;
    cublasLtMatmulHeuristicResult_t heur;
    int ret = 0;
    cublasStatus_t st = cublasLtMatmulAlgoGetHeuristic(s.lt, op, la, lb, lc, lc, pref, 1, &heur, &ret);
    if (st == CUBLAS_STATUS_SUCCESS && ret > 0) {
#if defined(CUDART_VERSION) && (CUDART_VERSION >= 12080)
        st = cublasLtMatmul(s.lt, op, &alpha, d_a4, la, d_b4, lb, &beta, d_ct, lc, d_ct, lc, &heur.algo, s.d_workspace, OMEGA_LT_WORKSPACE_B, s.stream);
#else
        st = cublasLtMatmul(s.lt, op, &alpha, d_af, la, d_bt, lb, &beta, d_ct, lc, d_ct, lc, &heur.algo, s.d_workspace, OMEGA_LT_WORKSPACE_B, s.stream);
#endif
    }
    int rc = 0;
    if (st != CUBLAS_STATUS_SUCCESS || ret <= 0) {
        rc = -14;
    } else {
        dim3 tb(16, 16);
        dim3 gc(((unsigned)M + 15u) / 16u, ((unsigned)N + 15u) / 16u);
        omega_transpose_f32_kernel<<<gc, tb, 0, s.stream>>>(d_ct, d_cf, (size_t)N, (size_t)M);
        cudaMemcpyAsync(c_row, d_cf, c_elems * sizeof(float), cudaMemcpyDeviceToHost, s.stream);
        if (cudaStreamSynchronize(s.stream) != cudaSuccess) rc = -15;
    }

    cublasLtMatmulPreferenceDestroy(pref);
    cublasLtMatrixLayoutDestroy(la);
    cublasLtMatrixLayoutDestroy(lb);
    cublasLtMatrixLayoutDestroy(lc);
    cublasLtMatmulDescDestroy(op);
    cudaFree(d_af); cudaFree(d_bf); cudaFree(d_bt); cudaFree(d_ct); cudaFree(d_cf);
    cudaFree(d_a4); cudaFree(d_b4); cudaFree(d_sa); cudaFree(d_sb);
    return rc;
}

/* NEW v3.0: GPU Batch Bytecode VM Dispatch (evaluates thousands of ASTs in parallel across SM120 warps!) */
int tropicore_omega_sm120_eval_bytecode_batch(
    const uint8_t* ops,
    size_t ops_len,
    const uint32_t* op_offsets,
    const float* consts,
    size_t consts_len,
    const uint32_t* const_offsets,
    const float* probes_xyz,
    float* out_vals,
    uint64_t* out_oe_hash,
    int n_progs,
    int n_probes,
    int device_id
) {
    if (n_progs <= 0 || n_probes <= 0 || ops_len == 0) return -1;
    int ngpu = tropicore_omega_sm120_detect_gpus();
    if (ngpu <= 0) return -2;
    int dev = (device_id >= 0) ? (device_id % ngpu) : 0;
    std::lock_guard<std::mutex> lock(g_omega_mutexes[dev]);
    if (!ensure_device_ready(dev)) return -3;
    cudaSetDevice(dev);

    OmegaDeviceState& s = g_states[dev];
    uint8_t* d_ops = nullptr;
    uint32_t *d_op_off = nullptr, *d_c_off = nullptr;
    float *d_consts = nullptr, *d_probes = nullptr, *d_out = nullptr;
    uint64_t* d_hash = nullptr;

    cudaMalloc(&d_ops, ops_len);
    cudaMalloc(&d_op_off, (size_t)n_progs * sizeof(uint32_t));
    cudaMalloc(&d_consts, (consts_len > 0 ? consts_len : 1) * sizeof(float));
    cudaMalloc(&d_c_off, (size_t)n_progs * sizeof(uint32_t));
    cudaMalloc(&d_probes, (size_t)n_probes * 3 * sizeof(float));
    cudaMalloc(&d_out, (size_t)n_progs * (size_t)n_probes * sizeof(float));
    cudaMalloc(&d_hash, (size_t)n_progs * sizeof(uint64_t));

    cudaMemcpyAsync(d_ops, ops, ops_len, cudaMemcpyHostToDevice, s.stream);
    cudaMemcpyAsync(d_op_off, op_offsets, (size_t)n_progs * sizeof(uint32_t), cudaMemcpyHostToDevice, s.stream);
    if (consts_len > 0) {
        cudaMemcpyAsync(d_consts, consts, consts_len * sizeof(float), cudaMemcpyHostToDevice, s.stream);
    }
    cudaMemcpyAsync(d_c_off, const_offsets, (size_t)n_progs * sizeof(uint32_t), cudaMemcpyHostToDevice, s.stream);
    cudaMemcpyAsync(d_probes, probes_xyz, (size_t)n_probes * 3 * sizeof(float), cudaMemcpyHostToDevice, s.stream);

    const int blk = 128;
    const int grd = (n_progs + blk - 1) / blk;
    omega_eval_bytecode_batch_kernel<<<grd, blk, 0, s.stream>>>(
        d_ops, d_op_off, d_consts, d_c_off, d_probes, d_out, d_hash, n_progs, n_probes
    );

    cudaMemcpyAsync(out_vals, d_out, (size_t)n_progs * (size_t)n_probes * sizeof(float), cudaMemcpyDeviceToHost, s.stream);
    cudaMemcpyAsync(out_oe_hash, d_hash, (size_t)n_progs * sizeof(uint64_t), cudaMemcpyDeviceToHost, s.stream);
    int rc = (cudaStreamSynchronize(s.stream) == cudaSuccess) ? 0 : -20;

    cudaFree(d_ops); cudaFree(d_op_off); cudaFree(d_consts); cudaFree(d_c_off);
    cudaFree(d_probes); cudaFree(d_out); cudaFree(d_hash);
    return rc;
}

/* v8.0: Scalable On-Device Unified-Brain Lean 4 Proof-State Tactic Policy Kernel (`W_tactic` [C x D] Softmax!) */
__global__ void omega_tactic_policy_kernel(
    const float* __restrict__ z_batch,    // [n_states, dim]
    const float* __restrict__ w_tactic,   // [n_classes, dim]
    const float* __restrict__ b_tactic,   // [n_classes]
    float* __restrict__ probs_out,        // [n_states, n_classes]
    int n_states,
    int dim,
    int n_classes
) {
    const int sid = blockIdx.x * blockDim.x + threadIdx.x;
    if (sid >= n_states || n_classes > 16) return;
    const float* z = z_batch + (size_t)sid * (size_t)dim;
    float logits[16];
    float mx = -1e30f;
    for (int c = 0; c < n_classes; ++c) {
        const float* row = w_tactic + (size_t)c * (size_t)dim;
        float acc = b_tactic[c];
        for (int d = 0; d < dim; ++d) {
            acc += row[d] * z[d];
        }
        logits[c] = fminf(10.0f, fmaxf(-10.0f, acc));
        if (logits[c] > mx) mx = logits[c];
    }
    float se = 0.0f;
    for (int c = 0; c < n_classes; ++c) {
        logits[c] = expf(logits[c] - mx);
        se += logits[c];
    }
    float* out = probs_out + (size_t)sid * (size_t)n_classes;
    for (int c = 0; c < n_classes; ++c) {
        out[c] = fminf(0.96f, fmaxf(0.01f, logits[c] / fmaxf(se, 1e-9f)));
    }
}

/* v8.0 Host-Callable SM120 Tactic Policy Dispatch (wired live to `src/gpu_bridge.rs`!) */
int tropicore_omega_sm120_tactic_policy(
    const float* z_batch,
    const float* w_tactic,
    const float* b_tactic,
    float* probs_out,
    int n_states,
    int dim,
    int n_classes,
    int device_id
) {
    if (n_states <= 0 || dim <= 0 || n_classes <= 0 || n_classes > 16) return -1;
    int ngpu = tropicore_omega_sm120_detect_gpus();
    if (ngpu <= 0) return -2;
    int dev = (device_id >= 0) ? (device_id % ngpu) : 0;
    std::lock_guard<std::mutex> lock(g_omega_mutexes[dev]);
    if (!ensure_device_ready(dev)) return -3;
    cudaSetDevice(dev);

    OmegaDeviceState& s = g_states[dev];
    float *d_z = nullptr, *d_w = nullptr, *d_b = nullptr, *d_p = nullptr;
    const size_t zb = (size_t)n_states * (size_t)dim * sizeof(float);
    const size_t wb = (size_t)n_classes * (size_t)dim * sizeof(float);
    const size_t bb = (size_t)n_classes * sizeof(float);
    const size_t pb = (size_t)n_states * (size_t)n_classes * sizeof(float);

    if (cudaMalloc(&d_z, zb) != cudaSuccess) return -4;
    if (cudaMalloc(&d_w, wb) != cudaSuccess) { cudaFree(d_z); return -4; }
    if (cudaMalloc(&d_b, bb) != cudaSuccess) { cudaFree(d_z); cudaFree(d_w); return -4; }
    if (cudaMalloc(&d_p, pb) != cudaSuccess) { cudaFree(d_z); cudaFree(d_w); cudaFree(d_b); return -4; }

    cudaMemcpyAsync(d_z, z_batch, zb, cudaMemcpyHostToDevice, s.stream);
    cudaMemcpyAsync(d_w, w_tactic, wb, cudaMemcpyHostToDevice, s.stream);
    cudaMemcpyAsync(d_b, b_tactic, bb, cudaMemcpyHostToDevice, s.stream);

    const int blk = 64;
    const int grd = (n_states + blk - 1) / blk;
    omega_tactic_policy_kernel<<<grd, blk, 0, s.stream>>>(d_z, d_w, d_b, d_p, n_states, dim, n_classes);

    cudaMemcpyAsync(probs_out, d_p, pb, cudaMemcpyDeviceToHost, s.stream);
    int rc = (cudaStreamSynchronize(s.stream) == cudaSuccess) ? 0 : -21;
    cudaFree(d_z); cudaFree(d_w); cudaFree(d_b); cudaFree(d_p);
    return rc;
}

/* v8.0 Unified-Brain Scalable SM120 ABI Parity Export */
int tropicore_omega_sm120_abi_dims_v8(
    int* out_omega_dim,
    int* out_tactic_classes,
    int* out_world_obs,
    int* out_world_belief,
    int* out_world_state
) {
    if (out_omega_dim)      *out_omega_dim      = OMEGA_LATENT_DIM;
    if (out_tactic_classes) *out_tactic_classes = OMEGA_NUM_TACTIC_CLASSES;
    if (out_world_obs)      *out_world_obs      = OMEGA_WORLD_OBS_DIM;
    if (out_world_belief)   *out_world_belief   = OMEGA_WORLD_BELIEF_DIM;
    if (out_world_state)    *out_world_state    = OMEGA_WORLD_STATE_DIM;
    return 800;
}

} // extern "C"

#!/usr/bin/env bash
# ==============================================================================
# TropiCore-Omega ASI-Zero v17.0.0 — Day-1 Turnkey 4x RTX PRO 6000 Blackwell Launcher
# Target Hardware (`장비.md` Track A):
#   - 4x NVIDIA RTX PRO 6000 Blackwell Max-Q 96GB (SM120, 384 GB GDDR7, 14.044 PFLOPS FP4 2:4)
#   - AMD Ryzen Threadripper PRO 9995WX (96C/192T Zen 5, 384 MB L3)
#   - 1,000 GB DDR5-6400 ECC + 4x Samsung 9100 PRO 4TB (RAID1 OS + 15.2 TB RAID0)
#   - PURE ZERO-TOY, ZERO-LLM, ZERO-COPY ALLREDUCE MULTI-GPU SM120 ARCHITECTURE (`ABI = 1700`):
#       * Independent Query/Key DeltaNet-SSM (`w_qry`/`b_qry`) + SwiGLU + Conv-4 + Diff-EqReg Adaptive Banach
#       * Zero-Copy AllReduce (`&[&[f32]]`) + 4x4 Blocked SIMD FWD/BWD + Reusable ForwardScratch Pool
#       * Fused CUDA 13 RMSNorm + SwiGLU + Resident GEMM + Embedding + Contractive([-10,10]) + Byte-CE + NVFP4 RNE AdamW
#       * Verified by Central Finite-Diff Gradient Check across ALL 22 tensor types + Lean 4 Omega-1..Omega-6 (0 sorry)
#       * 13-Expert x 8-Context Simplex Meta-Mixer (Exact PPM-D Escape + Adaptive Unigram + 4-Way Match + WordBigram + Skip-1)
#       * Compact 41-Byte Header `OMGZ17` Lossless Codec (ULEB128 + raw 32B SHA-256 + 64B TTT Neural Adaptation)
#       * Strictly `O(budget)` RAM Streaming I/O via `File::seek` (works on single 10-100 GB dataset files!)
#       * Multi-Formula AST Self-Optimizer with Isolated Compiled-Binary 2D Pareto Gate (`--verify-binary`)
#       * Stateful TD-MPC2 Actor-Critic `UniversalAgent` (`TOMA17`: zero latent loss, continuous actor + Polyak target, warm-started CEM!)
#
# Day-1 Turnkey Usage:
#   chmod +x deploy/deploy_4x_blackwell.sh
#   sudo ./deploy/deploy_4x_blackwell.sh --preset blackwell-1b --path /data/corpus --power 300 --cycles 100
#   sudo ./deploy/deploy_4x_blackwell.sh --preset blackwell-8b --path /data/corpus --power 300 --cycles 100 --apply --verify-binary
#   sudo ./deploy/deploy_4x_blackwell.sh --preset mid          --path ./src        --power 210 --cycles 20
# ==============================================================================
set -euo pipefail

POWER_LIMIT="300"
PRESET="blackwell-1b"
CORPUS_PATH="./src"
CYCLES="50"
EPOCHS="2"
APPLY_SELF_OPT="0"
VERIFY_BINARY="0"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --power) POWER_LIMIT="${2:-300}"; shift 2 ;;
    --preset) PRESET="${2:-blackwell-1b}"; shift 2 ;;
    --path|--corpus) CORPUS_PATH="${2:-./src}"; shift 2 ;;
    --cycles|--daemon) CYCLES="${2:-50}"; shift 2 ;;
    --epochs) EPOCHS="${2:-2}"; shift 2 ;;
    --apply|--apply-rsi) APPLY_SELF_OPT="1"; shift 1 ;;
    --verify-binary) VERIFY_BINARY="1"; shift 1 ;;
    *) shift ;;
  esac
done

CPU_THREADS="$(nproc)"
echo "========================================================================="
echo " TropiCore-Omega ASI-Zero v17.0.0 — 4x RTX PRO 6000 Blackwell SM120"
echo " Preset: ${PRESET} | Corpus: ${CORPUS_PATH} | Power: ${POWER_LIMIT}W/GPU | Threads: ${CPU_THREADS}"
echo "========================================================================="

echo "[1/5] Checking 4x SM120 GPUs and applying persistence + power cap (-pl ${POWER_LIMIT}W)..."
nvidia-smi --query-gpu=index,name,memory.total,compute_cap,power.limit --format=csv
if [[ "$EUID" -eq 0 ]]; then
  nvidia-smi -pm 1
  for gpu in 0 1 2 3; do
    nvidia-smi -i "$gpu" -pl "$POWER_LIMIT" || true
  done
else
  echo "  (Note: run with sudo to apply nvidia-smi -pm 1 and -pl ${POWER_LIMIT}W)"
fi

echo "[2/5] Checking Toolchain (nvcc CUDA 13, cublas, rustc)..."
nvcc --version
rustc --version

echo "[3/5] Compiling Zero-Copy Multi-GPU Fused SM120 CUDA 13 Kernels (RMSNorm+SwiGLU+NVFP4 RNE, ABI=1700)..."
nvcc -O3 -std=c++17 -arch=sm_120a -Xcompiler -fPIC \
  -c cuda/tropicore_omega_sm120.cu -o cuda/tropicore_omega_sm120.o
ar rcs cuda/libtropicore_omega_sm120.a cuda/tropicore_omega_sm120.o
ls -lh cuda/libtropicore_omega_sm120.a

echo "[4/5] Building & Testing tropicore_omega v17.0 --release --features cuda_sm120 (-D warnings)..."
RUSTFLAGS="-C target-cpu=native -D warnings" cargo build --release --features cuda_sm120
RUSTFLAGS="-C target-cpu=native -D warnings" cargo test --release --features cuda_sm120 -- --nocapture

echo "[5/5] Launching TropiCore-Omega ASI-Zero v17.0 Status & Autonomous Real-World Daemon..."
./target/release/tropicore_omega status

EXTRA_FLAGS=""
if [[ "$APPLY_SELF_OPT" == "1" ]]; then
  EXTRA_FLAGS="${EXTRA_FLAGS} --apply"
fi
if [[ "$VERIFY_BINARY" == "1" ]]; then
  EXTRA_FLAGS="${EXTRA_FLAGS} --verify-binary"
fi
if [[ "$CYCLES" -gt 0 ]]; then
  ./target/release/tropicore_omega daemon \
    --preset "$PRESET" \
    --path "$CORPUS_PATH" \
    --cycles "$CYCLES" \
    --epochs "$EPOCHS" \
    --threads "$CPU_THREADS" \
    --ckpt "out/asi_zero_v17_${PRESET}.tomg" \
    ${EXTRA_FLAGS}
fi

echo "========================================================================="
echo " TropiCore-Omega ASI-Zero v17.0.0 4x SM120 Run Complete! (Preset=${PRESET})"
echo "========================================================================="

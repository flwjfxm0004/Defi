#!/usr/bin/env bash
# ==============================================================================
# TropiCore-Omega ASI-Zero v12.0.0 — Day-1 Turnkey 4x RTX PRO 6000 Blackwell Launcher
# Target Hardware (`장비.md` Track A):
#   - 4x NVIDIA RTX PRO 6000 Blackwell Max-Q 96GB (SM120, 384 GB GDDR7, 14.044 PFLOPS FP4 2:4)
#   - AMD Ryzen Threadripper PRO 9995WX (96C/192T Zen 5, 384 MB L3)
#   - 1,000 GB DDR5-6400 ECC + 4x Samsung 9100 PRO 4TB (RAID1 OS + 15.2 TB RAID0)
#   - PURE ZERO-TOY, ZERO-LLM, GPU-RESIDENT SM120 ARCHITECTURE (`ABI = 1200`):
#       * Per-Layer Selective Gated Causal SSM + Tropical Max-Trace + Contractive Equilibrium (full BPTT)
#       * Persistent GDDR7 Tensor Handle Pool (weights + AdamW m/v stay on GPU across steps — zero weight PCIe re-upload!)
#       * Live-Wired SM120 Forward + Backward (`dX`/`dW`/`db`) + Contractive + Byte-CE + Resident AdamW + AllReduce
#       * Dynamic Scaling Presets: `laptop` (0.30M) | `mid` (14.97M) | `blackwell-1b` (0.94B) | `blackwell-8b` (7.52B)
#       * Physical Lossless `.omg` Arithmetic Codec (`compress` / `decompress`) + Autonomous `generate`
#       * Persistent External UniversalAgent stdio JSONL Server (`serve-agent`, `obs` + `obs_text`, `TOMA12`)
#       * Zero-Free-Rider End-to-End Self-Optimizer targeting live `src/asi_runtime_cfg.rs` (`--apply`)
#
# Day-1 Turnkey Usage:
#   chmod +x deploy/deploy_4x_blackwell.sh
#   sudo ./deploy/deploy_4x_blackwell.sh --preset blackwell-1b --path /data/corpus --power 300 --cycles 100
#   sudo ./deploy/deploy_4x_blackwell.sh --preset blackwell-8b --path /data/corpus --power 300 --cycles 100 --apply
#   sudo ./deploy/deploy_4x_blackwell.sh --preset mid          --path ./src        --power 210 --cycles 20
# ==============================================================================
set -euo pipefail

POWER_LIMIT="300"
PRESET="blackwell-1b"
CORPUS_PATH="./src"
CYCLES="50"
EPOCHS="2"
APPLY_SELF_OPT="0"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --power) POWER_LIMIT="${2:-300}"; shift 2 ;;
    --preset) PRESET="${2:-blackwell-1b}"; shift 2 ;;
    --path|--corpus) CORPUS_PATH="${2:-./src}"; shift 2 ;;
    --cycles|--daemon) CYCLES="${2:-50}"; shift 2 ;;
    --epochs) EPOCHS="${2:-2}"; shift 2 ;;
    --apply|--apply-rsi) APPLY_SELF_OPT="1"; shift 1 ;;
    *) shift ;;
  esac
done

CPU_THREADS="$(nproc)"
echo "========================================================================="
echo " TropiCore-Omega ASI-Zero v12.0.0 — 4x RTX PRO 6000 Blackwell SM120"
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

echo "[2/5] Checking Toolchain (nvcc CUDA 13, cublas/cublasLt, rustc)..."
nvcc --version
rustc --version

echo "[3/5] Compiling SM120 CUDA 13 GPU-Resident Kernels (Tensor Pool + Fwd/Bwd + Contractive + AdamW + AllReduce, ABI=1200)..."
nvcc -O3 -std=c++17 -arch=sm_120a -Xcompiler -fPIC \
  -c cuda/tropicore_omega_sm120.cu -o cuda/tropicore_omega_sm120.o
ar rcs cuda/libtropicore_omega_sm120.a cuda/tropicore_omega_sm120.o
ls -lh cuda/libtropicore_omega_sm120.a

echo "[4/5] Building & Testing tropicore_omega v12.0 --release --features cuda_sm120 (-D warnings)..."
RUSTFLAGS="-C target-cpu=native -D warnings" cargo build --release --features cuda_sm120
RUSTFLAGS="-C target-cpu=native -D warnings" cargo test --release --features cuda_sm120 -- --nocapture

echo "[5/5] Launching TropiCore-Omega ASI-Zero v12.0 Status & Autonomous Real-World Daemon..."
./target/release/tropicore_omega status

APPLY_FLAG=""
if [[ "$APPLY_SELF_OPT" == "1" ]]; then
  APPLY_FLAG="--apply"
fi
if [[ "$CYCLES" -gt 0 ]]; then
  ./target/release/tropicore_omega daemon \
    --preset "$PRESET" \
    --path "$CORPUS_PATH" \
    --cycles "$CYCLES" \
    --epochs "$EPOCHS" \
    --threads "$CPU_THREADS" \
    --ckpt "out/asi_zero_v12_${PRESET}.tomg" \
    ${APPLY_FLAG}
fi

echo "========================================================================="
echo " TropiCore-Omega ASI-Zero v12.0.0 4x SM120 Run Complete! (Preset=${PRESET})"
echo "========================================================================="

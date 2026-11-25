#!/usr/bin/env bash
# ==============================================================================
# TropiCore-Omega v10.0 ASI-Core — Day-1 Turnkey 4x RTX PRO 6000 Blackwell Launcher
# Target Hardware (`장비.md` Track A):
#   - 4x NVIDIA RTX PRO 6000 Blackwell Max-Q 96GB (SM120, 384 GB GDDR7, 14.044 PFLOPS FP4 2:4)
#   - AMD Ryzen Threadripper PRO 9995WX (96C/192T Zen 5, 384 MB L3)
#   - 1,000 GB DDR5-6400 ECC + 4x Samsung 9100 PRO 4TB (RAID1 OS + 15.2 TB RAID0)
#   - STRICT ZERO-CRUTCH GENERAL ASI POLICY (v10.0):
#       0 LLMs | 0 External APIs | 0 Pretraining | 0 External Rust Crates
#       Lean 4 Math Bottleneck REMOVED (Optional Plug-in via `--with-lean`)
#       Tier-1 Zero-Peek Multi-Register SSA Imperative Rust Synthesis (Bayes-Factor Self-Play Wake-Sleep)
#       Tier-2 Nonlinear 2-MLP Residual Neural-Ensemble World Model + Epistemic Curiosity + Fault Recovery
#       Tier-3 Fast Pure-Rust CEGIS -> E-Graph (<10ms)
#       Self-Hosting Repository RSI targeting live `src/evolved_policy.rs` (`--apply-rsi`)
#       1-Flag Scalable Core Dim: `--dim 64 | 256 | 2048 | 4096`
#
# Day-1 Turnkey Usage:
#   chmod +x deploy/deploy_4x_blackwell.sh
#   sudo ./deploy/deploy_4x_blackwell.sh --dim 4096 --power 300 --asi-forever 100              # Flagship 1,550W
#   sudo ./deploy/deploy_4x_blackwell.sh --dim 4096 --power 300 --asi-forever 100 --apply-rsi  # Live Self-Hosting RSI
#   sudo ./deploy/deploy_4x_blackwell.sh --dim 2048 --power 210 --asi-forever 100              # Quiet Dual-Use 1,190W
# ==============================================================================
set -euo pipefail

POWER_LIMIT="300"
OMEGA_DIM="4096"
ASI_CYCLES="100"
WITH_LEAN="0"
RUN_LEGACY_8AXIS="0"
APPLY_RSI="0"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --power) POWER_LIMIT="${2:-300}"; shift 2 ;;
    --dim) OMEGA_DIM="${2:-4096}"; shift 2 ;;
    --asi-forever|--daemon) ASI_CYCLES="${2:-100}"; shift 2 ;;
    --with-lean) WITH_LEAN="1"; shift 1 ;;
    --legacy-8axis) RUN_LEGACY_8AXIS="1"; shift 1 ;;
    --apply-rsi) APPLY_RSI="1"; shift 1 ;;
    *) shift ;;
  esac
done

FEATURES="cuda_sm120"
case "$OMEGA_DIM" in
  64)   ;;
  256)  FEATURES="${FEATURES},scale_256" ;;
  2048) FEATURES="${FEATURES},scale_2048" ;;
  4096) FEATURES="${FEATURES},scale_4096" ;;
  *)
    echo "Unsupported --dim=${OMEGA_DIM} (allowed: 64, 256, 2048, 4096)" >&2
    exit 1
    ;;
esac

if [[ "$WITH_LEAN" == "1" ]]; then
  FEATURES="${FEATURES},lean_oracle"
fi

CPU_THREADS="$(nproc)"
echo "========================================================================="
echo " TropiCore-Omega v10.0 ASI-Core — 4x RTX PRO 6000 Blackwell SM120"
echo " Core Dim: d=${OMEGA_DIM} | Power Cap: ${POWER_LIMIT}W/GPU | Threads: ${CPU_THREADS} | Features: ${FEATURES}"
echo "========================================================================="

echo "[1/6] Checking 4x SM120 GPUs and applying persistence + power cap (-pl ${POWER_LIMIT}W)..."
nvidia-smi --query-gpu=index,name,memory.total,compute_cap,power.limit --format=csv
if [[ "$EUID" -eq 0 ]]; then
  nvidia-smi -pm 1
  for gpu in 0 1 2 3; do
    nvidia-smi -i "$gpu" -pl "$POWER_LIMIT" || true
  done
else
  echo "  (Note: run with sudo to apply nvidia-smi -pm 1 and -pl ${POWER_LIMIT}W)"
fi

echo "[2/6] Checking CPU (9995WX 192T) & Local Toolchain (nvcc CUDA 13, rustc; Lean optional)..."
echo "  Logical Threads: ${CPU_THREADS}"
nvcc --version
rustc --version
if [[ "$WITH_LEAN" == "1" ]]; then
  if ! command -v lean >/dev/null 2>&1; then
    curl -sSf https://raw.githubusercontent.com/leanprover/elan/master/elan-init.sh | sh -s -- -y --default-toolchain leanprover/lean4:stable
    export PATH="$HOME/.elan/bin:$PATH"
  fi
  lean --version
else
  echo "  Lean 4 Oracle: DECOUPLED (skipped for pure General ASI speed; pass --with-lean to enable)"
fi

echo "[3/6] Compiling SM120 Blackwell CUDA 13 NVFP4 + Deep-Set + Warp VM Kernel (d=${OMEGA_DIM}, sm_120a)..."
nvcc -O3 -std=c++17 -arch=sm_120a -DOMEGA_LATENT_DIM_CFG="${OMEGA_DIM}" -Xcompiler -fPIC \
  -c cuda/tropicore_omega_sm120.cu -o cuda/tropicore_omega_sm120.o
ar rcs cuda/libtropicore_omega_sm120.a cuda/tropicore_omega_sm120.o
ls -lh cuda/libtropicore_omega_sm120.a

echo "[4/6] Building tropicore_omega v10.0 --release --features ${FEATURES} (Zen 5 AVX-512 + 4x SM120)..."
RUSTFLAGS="-C target-cpu=native -D warnings" cargo build --release --features "${FEATURES}"

echo "[5/6] Running Release Verification Tests (v10.0 ASI-Core + SM120 ABI + Anti-Cheat Suite)..."
RUSTFLAGS="-C target-cpu=native -D warnings" cargo test --release --features "${FEATURES}" -- --nocapture
if [[ "$WITH_LEAN" == "1" ]]; then
  lean proofs/omega_proof.lean && echo "OMEGA_PROOF_LEAN4_EXIT=0"
fi

echo "[6/6] Running TropiCore-Omega v10.0 ASI-Core Smoke + Launching Autonomous ASI Daemon..."
./target/release/tropicore_omega --asi-smoke --threads "${CPU_THREADS}"

if [[ "$RUN_LEGACY_8AXIS" == "1" ]]; then
  ./target/release/tropicore_omega --legacy-8axis
fi

if [[ "$ASI_CYCLES" -gt 0 ]]; then
  RSI_FLAG=""
  if [[ "$APPLY_RSI" == "1" ]]; then
    RSI_FLAG="--apply-rsi"
  fi
  echo "[ASI-FOREVER] Launching Lossless Open-Ended General ASI Daemon (d=${OMEGA_DIM}, cycles=${ASI_CYCLES}, threads=${CPU_THREADS})..."
  ./target/release/tropicore_omega --asi-forever --cycles "$ASI_CYCLES" --threads "${CPU_THREADS}" --ckpt "out/asi_v10_d${OMEGA_DIM}.ckpt" ${RSI_FLAG}
fi

echo "========================================================================="
echo " TropiCore-Omega v10.0 ASI-Core 4x SM120 Deployment Complete! (d=${OMEGA_DIM})"
echo "========================================================================="

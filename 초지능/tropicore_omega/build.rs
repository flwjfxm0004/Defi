//! Auto-links `cuda/libtropicore_omega_sm120.a` + CUDA 13 runtime (`cublasLt`, `cudart`, `stdc++`)
//! when compiled with `--features cuda_sm120` on the `장비.md` 4x RTX PRO 6000 Blackwell workstation.

use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=cuda/tropicore_omega_sm120.cu");
    if env::var("CARGO_FEATURE_CUDA_SM120").is_err() {
        return;
    }

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let cuda_dir = manifest_dir.join("cuda");
    let lib_a = cuda_dir.join("libtropicore_omega_sm120.a");

    if !lib_a.exists() {
        let cu = cuda_dir.join("tropicore_omega_sm120.cu");
        let obj = cuda_dir.join("tropicore_omega_sm120.o");
        let dim_flag = if env::var("CARGO_FEATURE_SCALE_4096").is_ok() {
            "-DOMEGA_LATENT_DIM_CFG=4096"
        } else if env::var("CARGO_FEATURE_SCALE_2048").is_ok() {
            "-DOMEGA_LATENT_DIM_CFG=2048"
        } else if env::var("CARGO_FEATURE_SCALE_256").is_ok() {
            "-DOMEGA_LATENT_DIM_CFG=256"
        } else {
            "-DOMEGA_LATENT_DIM_CFG=64"
        };
        let st = Command::new("nvcc")
            .args(["-O3", "-std=c++17", "-arch=sm_120a", dim_flag, "-Xcompiler", "-fPIC", "-c"])
            .arg(&cu)
            .arg("-o")
            .arg(&obj)
            .status()
            .expect("Failed to invoke nvcc for SM120 Blackwell CUDA 13 build");
        assert!(st.success(), "nvcc failed to compile cuda/tropicore_omega_sm120.cu");

        let ar = Command::new("ar")
            .args(["rcs"])
            .arg(&lib_a)
            .arg(&obj)
            .status()
            .expect("Failed to archive libtropicore_omega_sm120.a");
        assert!(ar.success(), "ar failed to create libtropicore_omega_sm120.a");
    }

    println!("cargo:rustc-link-search=native={}", cuda_dir.display());
    for cand in ["/usr/local/cuda/lib64", "/usr/local/cuda-13.0/lib64", "/usr/lib/x86_64-linux-gnu"] {
        if PathBuf::from(cand).exists() {
            println!("cargo:rustc-link-search=native={}", cand);
        }
    }
    println!("cargo:rustc-link-lib=static=tropicore_omega_sm120");
    println!("cargo:rustc-link-lib=dylib=cublasLt");
    println!("cargo:rustc-link-lib=dylib=cudart");
    println!("cargo:rustc-link-lib=dylib=stdc++");
}

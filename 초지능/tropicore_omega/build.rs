//! Build script for TropiCore-Omega ASI-Zero v14.0 (ABI=1400, SM120 Blackwell CUDA 13).
//! Automatically rebuilds `cuda/libtropicore_omega_sm120.a` whenever `cuda/tropicore_omega_sm120.cu`
//! is newer than the archive (fixing stale-archive ABI mismatch bugs!).

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=cuda/tropicore_omega_sm120.cu");
    println!("cargo:rerun-if-env-changed=CUDA_HOME");
    println!("cargo:rerun-if-env-changed=NVCC");

    if env::var("CARGO_FEATURE_CUDA_SM120").is_err() {
        return;
    }

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let cu_path = manifest_dir.join("cuda").join("tropicore_omega_sm120.cu");
    let lib_dir = manifest_dir.join("cuda");
    let obj_path = lib_dir.join("tropicore_omega_sm120.o");
    let lib_path = lib_dir.join("libtropicore_omega_sm120.a");

    let need_rebuild = match (fs::metadata(&cu_path), fs::metadata(&lib_path)) {
        (Ok(cu_m), Ok(lib_m)) => match (cu_m.modified(), lib_m.modified()) {
            (Ok(ct), Ok(lt)) => ct > lt,
            _ => true,
        },
        _ => true,
    };

    if need_rebuild {
        let nvcc = env::var("NVCC").unwrap_or_else(|_| "nvcc".to_string());
        let status = Command::new(&nvcc)
            .args([
                "-O3",
                "-std=c++17",
                "-arch=sm_120a",
                "-Xcompiler",
                "-fPIC",
                "-c",
            ])
            .arg(&cu_path)
            .arg("-o")
            .arg(&obj_path)
            .status()
            .expect("Failed to invoke nvcc for SM120 training+inference CUDA 13 build");
        assert!(status.success(), "nvcc SM120 compilation failed");

        let ar_status = Command::new("ar")
            .args(["rcs"])
            .arg(&lib_path)
            .arg(&obj_path)
            .status()
            .expect("Failed to archive libtropicore_omega_sm120.a");
        assert!(ar_status.success(), "ar failed to create libtropicore_omega_sm120.a");
    }

    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    println!("cargo:rustc-link-lib=static=tropicore_omega_sm120");

    if let Ok(cuda_home) = env::var("CUDA_HOME") {
        println!("cargo:rustc-link-search=native={}/lib64", cuda_home);
    } else {
        println!("cargo:rustc-link-search=native=/usr/local/cuda/lib64");
    }
    println!("cargo:rustc-link-lib=dylib=cudart");
    println!("cargo:rustc-link-lib=dylib=cublasLt");
    println!("cargo:rustc-link-lib=dylib=cublas");
    println!("cargo:rustc-link-lib=dylib=stdc++");
}

pub mod seal {
    use std::collections::{BTreeMap, BTreeSet};
    use std::fs;
    use std::path::{Path, PathBuf};

    use serde::{Deserialize, Serialize};

    pub const HOST_DEFI_ROOT: &str = r"c:\Defi";
    pub const HOST_R_ROOT: &str = r"c:\Defi\game";
    pub const MASTER_SPEC_REL: &str = "게임.md";

    pub const KERNEL_CRATES: [&str; 12] = [
        "math_q32",
        "verifier_abi",
        "ecs_soa",
        "physics_sdf",
        "ledger_core",
        "qvm",
        "ledger_wal",
        "render_host",
        "audio_opus",
        "client_bin",
        "net_quic",
        "server_bin",
    ];

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    pub struct SealManifest {
        pub version: u32,
        pub files: BTreeMap<String, String>,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum TrackMode {
        Engine,
        Hotswap,
    }

    impl TrackMode {
        pub fn as_str(self) -> &'static str {
            match self {
                Self::Engine => "engine",
                Self::Hotswap => "hotswap",
            }
        }

        pub fn parse_file(path: &Path) -> Result<Self, String> {
            let raw = fs::read_to_string(path)
                .map_err(|e| format!("Failed to read .track_mode at {}: {e}", path.display()))?;
            let token = raw
                .lines()
                .next()
                .unwrap_or("")
                .split(':')
                .next_back()
                .unwrap_or("")
                .trim();
            match token {
                "engine" => Ok(Self::Engine),
                "hotswap" => Ok(Self::Hotswap),
                other => Err(format!("Invalid .track_mode value: '{other}'")),
            }
        }
    }

    pub fn blake3_file_hex(path: &Path) -> Result<String, String> {
        let bytes = fs::read(path).map_err(|e| format!("Failed reading {}: {e}", path.display()))?;
        Ok(blake3::hash(&bytes).to_hex().to_string())
    }

    fn collect_dir_files_sorted(
        root: &Path,
        rel_dir: &str,
        out: &mut BTreeMap<String, PathBuf>,
    ) -> Result<(), String> {
        let target = root.join(rel_dir);
        if !target.exists() {
            return Err(format!("Sealed directory missing: {}", target.display()));
        }
        let mut stack = vec![target];
        while let Some(dir) = stack.pop() {
            let entries = fs::read_dir(&dir)
                .map_err(|e| format!("Failed reading dir {}: {e}", dir.display()))?;
            for entry_res in entries {
                let entry = entry_res.map_err(|e| format!("Dir entry error: {e}"))?;
                let meta = fs::symlink_metadata(entry.path())
                    .map_err(|e| format!("Metadata error: {e}"))?;
                if meta.file_type().is_symlink() {
                    return Err(format!(
                        "Symlink/reparse point forbidden in sealed tree: {}",
                        entry.path().display()
                    ));
                }
                let p = entry.path();
                if meta.is_dir() {
                    stack.push(p);
                } else if meta.is_file() {
                    let rel = p
                        .strip_prefix(root)
                        .map_err(|e| format!("strip_prefix error: {e}"))?;
                    let rel_slash = rel.to_string_lossy().replace('\\', "/");
                    if rel_slash != "seal.lock" && rel_slash != ".track_mode" {
                        out.insert(rel_slash, p);
                    }
                }
            }
        }
        Ok(())
    }

    /// Enumerates the exact authoritative set of sealed paths (`게임.md §7.1, §7.2`),
    /// strictly excluding `seal.lock` and `.track_mode` from hash inputs so host operators
    /// can switch `.track_mode` (`engine` <-> `hotswap`) without invalidating `seal.lock`.
    pub fn enumerate_sealed_inputs(r_root: &Path) -> Result<BTreeMap<String, PathBuf>, String> {
        let mut map: BTreeMap<String, PathBuf> = BTreeMap::new();

        // R_cfg (`게임.md` included; `seal.lock` and `.track_mode` excluded per `게임.md §7.1`)
        let root_files = [
            MASTER_SPEC_REL,
            "Cargo.toml",
            "Cargo.lock",
            ".cargo/config.toml",
            "rust-toolchain.toml",
            "clippy.toml",
            ".gitignore",
        ];
        for rf in root_files {
            let p = r_root.join(rf);
            if !p.is_file() {
                return Err(format!("Missing required R_cfg file: {}", p.display()));
            }
            map.insert(rf.to_string(), p);
        }

        // kernel/*/{Cargo.toml,clippy.toml}
        for krate in KERNEL_CRATES {
            for cfg in ["Cargo.toml", "clippy.toml"] {
                let rel = format!("kernel/{krate}/{cfg}");
                let p = r_root.join(&rel);
                if !p.is_file() {
                    return Err(format!("Missing sealed crate config: {}", p.display()));
                }
                map.insert(rel, p);
            }
        }

        // Sealed directories: kernel/verifier_abi/, verifier/, .agents/, workflows/
        for sdir in ["kernel/verifier_abi", "verifier", ".agents", "workflows"] {
            collect_dir_files_sorted(r_root, sdir, &mut map)?;
        }

        Ok(map)
    }

    pub fn build_seal_manifest(r_root: &Path) -> Result<SealManifest, String> {
        let _ = TrackMode::parse_file(&r_root.join(".track_mode"))?;
        let inputs = enumerate_sealed_inputs(r_root)?;
        let mut files = BTreeMap::new();
        for (key, path) in inputs {
            let digest = blake3_file_hex(&path)?;
            files.insert(key, digest);
        }
        Ok(SealManifest { version: 2, files })
    }

    /// Initializes `seal.lock` once at the end of `Phase 0` and locks read-only attributes on
    /// `게임.md`, `.track_mode`, and `seal.lock` (`게임.md §7.1`). Refuses to overwrite an existing `seal.lock`!
    pub fn seal_init_once(r_root: &Path) -> Result<SealManifest, String> {
        let seal_path = r_root.join("seal.lock");
        if seal_path.exists() {
            return Err("seal.lock already exists; re-sealing is strictly prohibited".to_string());
        }
        let manifest = build_seal_manifest(r_root)?;
        let serialized = serde_json::to_string_pretty(&manifest)
            .map_err(|e| format!("Failed serializing seal.lock: {e}"))?;
        fs::write(&seal_path, serialized.as_bytes())
            .map_err(|e| format!("Failed writing seal.lock: {e}"))?;

        for ro_rel in [MASTER_SPEC_REL, ".track_mode", "seal.lock"] {
            let p = r_root.join(ro_rel);
            if let Ok(meta) = fs::metadata(&p) {
                let mut perms = meta.permissions();
                perms.set_readonly(true);
                let _ = fs::set_permissions(&p, perms);
            }
        }
        Ok(manifest)
    }

    /// Verifies `seal.lock` against live disk (`R_cfg`[`게임.md` 포함], `kernel/*/{Cargo.toml,clippy.toml}`,
    /// `kernel/verifier_abi/`, `verifier/`, `A_gov`), rejecting any `1-byte` mutation, deletion, or injection.
    pub fn verify_host_seal(r_root: &Path) -> Result<SealManifest, String> {
        let seal_path = r_root.join("seal.lock");
        if !seal_path.is_file() {
            return Err(format!("Missing seal.lock at {}", seal_path.display()));
        }
        let _ = TrackMode::parse_file(&r_root.join(".track_mode"))?;
        let raw = fs::read_to_string(&seal_path)
            .map_err(|e| format!("Failed reading seal.lock: {e}"))?;
        let expected: SealManifest =
            serde_json::from_str(&raw).map_err(|e| format!("Invalid seal.lock JSON: {e}"))?;

        let live = build_seal_manifest(r_root)?;

        let expected_keys: BTreeSet<&String> = expected.files.keys().collect();
        let live_keys: BTreeSet<&String> = live.files.keys().collect();
        if let Some(missing) = expected_keys.difference(&live_keys).next() {
            return Err(format!("Sealed file deleted or missing: {missing}"));
        }
        if let Some(injected) = live_keys.difference(&expected_keys).next() {
            return Err(format!("Unsealed file injected into sealed tree: {injected}"));
        }

        for (key, exp_hash) in &expected.files {
            let Some(live_hash) = live.files.get(key) else {
                return Err(format!("Missing live digest for {key}"));
            };
            if exp_hash != live_hash {
                return Err(format!(
                    "BLAKE3 seal violation on '{key}': expected {exp_hash}, got {live_hash}"
                ));
            }
        }
        Ok(expected)
    }

    /// Verifies that every sealed workspace path inside an isolated branch `clone_root` is `100%`
    /// byte-identical to host `seal.lock` BEFORE any `cargo` invocation (`게임.md §10.2`).
    pub fn verify_clone_against_host_seal(
        host_seal: &SealManifest,
        clone_root: &Path,
    ) -> Result<(), String> {
        let live_clone_inputs = enumerate_sealed_inputs(clone_root)?;
        let expected_keys: BTreeSet<&String> = host_seal.files.keys().collect();
        let clone_keys: BTreeSet<&String> = live_clone_inputs.keys().collect();

        if let Some(missing) = expected_keys.difference(&clone_keys).next() {
            return Err(format!("Clone deleted sealed file: {missing}"));
        }
        if let Some(injected) = clone_keys.difference(&expected_keys).next() {
            return Err(format!("Clone injected unsealed file in sealed tree: {injected}"));
        }
        for (key, exp_hash) in &host_seal.files {
            let Some(path) = live_clone_inputs.get(key) else {
                return Err(format!("Clone missing path for {key}"));
            };
            let actual_hash = blake3_file_hex(path)?;
            if &actual_hash != exp_hash {
                return Err(format!(
                    "Clone mutated sealed file '{key}': expected {exp_hash}, got {actual_hash}"
                ));
            }
        }
        Ok(())
    }

}

pub mod preflight {
    use std::fs;
    use std::path::Path;

    use crate::seal::KERNEL_CRATES;

    const K_SIM_CRATES: [&str; 6] = [
        "math_q32",
        "verifier_abi",
        "ecs_soa",
        "physics_sdf",
        "ledger_core",
        "qvm",
    ];

    const UNSAFE_ALLOWED_KERNEL_CRATES: [&str; 4] = [
        "ecs_soa",
        "render_host",
        "audio_opus",
        "client_bin",
    ];

    const STD_FS_ALLOWED_KERNEL_CRATES: [&str; 2] = [
        "ledger_wal",
        "render_host",
    ];

    const STD_TIME_ALLOWED_KERNEL_CRATES: [&str; 3] = [
        "client_bin",
        "net_quic",
        "server_bin",
    ];

    const STD_NET_ALLOWED_KERNEL_CRATES: [&str; 2] = [
        "net_quic",
        "server_bin",
    ];

    #[derive(Clone, Debug, Default, PartialEq, Eq)]
    pub struct PreflightReport {
        pub scanned_rs_files: usize,
        pub scanned_manifests: usize,
    }

    /// Strips `//` line comments and `/* ... */` block comments while preserving code & attributes,
    /// so comments mentioning banned constructs (`// HashMap is forbidden`) do not cause false positives,
    /// while actual attributes (`#[allow(...)]`) and code tokens are caught `100%`.
    pub fn strip_rust_comments(src: &str) -> String {
        let bytes = src.as_bytes();
        let mut out = String::with_capacity(src.len());
        let mut i = 0;
        let mut block_depth: usize = 0;
        let mut in_string = false;

        while i < bytes.len() {
            if block_depth > 0 {
                if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'*' {
                    block_depth += 1;
                    i += 2;
                } else if i + 1 < bytes.len() && bytes[i] == b'*' && bytes[i + 1] == b'/' {
                    block_depth -= 1;
                    i += 2;
                    out.push(' ');
                } else {
                    if bytes[i] == b'\n' {
                        out.push('\n');
                    }
                    i += 1;
                }
                continue;
            }

            if in_string {
                if bytes[i] == b'\\' && i + 1 < bytes.len() {
                    out.push(' ');
                    i += 2;
                    continue;
                }
                if bytes[i] == b'"' {
                    in_string = false;
                    out.push('"');
                    i += 1;
                    continue;
                }
                if bytes[i] == b'\n' {
                    out.push('\n');
                } else {
                    out.push(' ');
                }
                i += 1;
                continue;
            }

            if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'/' {
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
                continue;
            }
            if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'*' {
                block_depth = 1;
                i += 2;
                continue;
            }
            if bytes[i] == b'"' {
                in_string = true;
                out.push('"');
                i += 1;
                continue;
            }

            if let Some(ch) = src[i..].chars().next() {
                out.push(ch);
                i += ch.len_utf8();
            } else {
                break;
            }
        }
        out
    }

    fn contains_ident(code: &str, ident: &str) -> bool {
        let mut start = 0;
        while let Some(pos) = code[start..].find(ident) {
            let abs = start + pos;
            let before_ok = abs == 0
                || !code[..abs]
                    .chars()
                    .next_back()
                    .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_');
            let after_idx = abs + ident.len();
            let after_ok = after_idx >= code.len()
                || !code[after_idx..]
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_');
            if before_ok && after_ok {
                return true;
            }
            start = abs + 1;
        }
        false
    }

    fn compact_no_ws(code: &str) -> String {
        code.chars().filter(|c| !c.is_whitespace()).collect()
    }

    fn scan_std_use_trees(
        code: &str,
        allow_fs: bool,
        allow_time: bool,
        allow_net: bool,
        rel_path: &str,
        crate_name: &str,
    ) -> Result<(), String> {
        for stmt in code.split(';') {
            let trimmed = stmt.trim();
            if !(contains_ident(trimmed, "use") && contains_ident(trimmed, "std")) {
                continue;
            }
            if contains_ident(trimmed, "process") || contains_ident(trimmed, "env") {
                return Err(format!(
                    "Forbidden grouped/aliased std::process or std::env import in {rel_path}"
                ));
            }
            if !allow_fs && contains_ident(trimmed, "fs") {
                return Err(format!(
                    "Forbidden std::fs import in kernel crate '{crate_name}' ({rel_path})"
                ));
            }
            if !allow_time && contains_ident(trimmed, "time") {
                return Err(format!(
                    "Forbidden std::time import in kernel crate '{crate_name}' ({rel_path})"
                ));
            }
            if !allow_net && contains_ident(trimmed, "net") {
                return Err(format!(
                    "Forbidden std::net import in kernel crate '{crate_name}' ({rel_path})"
                ));
            }
        }
        Ok(())
    }

    /// Scans a single Rust source string for `게임.md §7.1, §8.1` violations in `crate_name`.
    pub fn scan_kernel_rust_source(
        crate_name: &str,
        rel_path: &str,
        raw_src: &str,
    ) -> Result<(), String> {
        let code = strip_rust_comments(raw_src);
        let nospace = compact_no_ws(&code);

        // 1. Universal kernel bans: #[allow], #[expect], cfg_attr, #[path], extern crate std, include_*!, todo!/unimplemented!, HashMap/HashSet
        if nospace.contains("#[allow(")
            || nospace.contains("#![allow(")
            || nospace.contains("#[expect(")
            || nospace.contains("#![expect(")
            || contains_ident(&code, "cfg_attr")
        {
            return Err(format!(
                "Forbidden lint-bypass (#[allow]/#[expect]/cfg_attr) in {rel_path}"
            ));
        }
        if !nospace.contains("#![forbid(") {
            return Err(format!(
                "Missing mandatory crate-level #![forbid(...)] header in {rel_path}"
            ));
        }
        if nospace.contains("#[path=") || nospace.contains("#[path(") {
            return Err(format!("Forbidden #[path] attribute in {rel_path}"));
        }
        if K_SIM_CRATES.contains(&crate_name) && nospace.contains("externcratestd") {
            return Err(format!(
                "Forbidden `extern crate std` in #![no_std] K_sim crate '{crate_name}' ({rel_path})"
            ));
        }
        for macro_ban in [
            "include_bytes!(",
            "include_str!(",
            "include!(",
            "todo!(",
            "unimplemented!(",
        ] {
            if nospace.contains(macro_ban) {
                return Err(format!("Forbidden {macro_ban} macro in {rel_path}"));
            }
        }
        for ident_ban in ["HashMap", "HashSet"] {
            if contains_ident(&code, ident_ban) {
                return Err(format!("Forbidden non-deterministic {ident_ban} in {rel_path}"));
            }
        }
        if nospace.contains("std::process") || nospace.contains("std::env") {
            return Err(format!("Forbidden std::process / std::env usage in {rel_path}"));
        }

        // 2. Scoped std::fs, std::time, and std::net boundaries (`게임.md §7.1`)
        let allow_fs = STD_FS_ALLOWED_KERNEL_CRATES.contains(&crate_name);
        let allow_time = STD_TIME_ALLOWED_KERNEL_CRATES.contains(&crate_name);
        let allow_net = STD_NET_ALLOWED_KERNEL_CRATES.contains(&crate_name);

        if !allow_fs && nospace.contains("std::fs") {
            return Err(format!(
                "Forbidden std::fs in kernel crate '{crate_name}' ({rel_path})"
            ));
        }
        if !allow_time && nospace.contains("std::time") {
            return Err(format!(
                "Forbidden std::time in kernel crate '{crate_name}' ({rel_path})"
            ));
        }
        if !allow_net && nospace.contains("std::net") {
            return Err(format!(
                "Forbidden std::net in kernel crate '{crate_name}' ({rel_path})"
            ));
        }
        scan_std_use_trees(&code, allow_fs, allow_time, allow_net, rel_path, crate_name)?;

        // 3. K_sim strict floating-point ban (`f32` / `f64` completely banned in K_sim, `게임.md §8.1`)
        if K_SIM_CRATES.contains(&crate_name) {
            for float_ty in ["f32", "f64"] {
                if contains_ident(&code, float_ty) {
                    return Err(format!(
                        "Forbidden floating-point type '{float_ty}' in K_sim crate '{crate_name}' ({rel_path})"
                    ));
                }
            }
        }

        // 4. Scoped `unsafe` block/fn/impl ban (`게임.md §8.1`)
        if !UNSAFE_ALLOWED_KERNEL_CRATES.contains(&crate_name) && contains_ident(&code, "unsafe") {
            return Err(format!(
                "Forbidden `unsafe` keyword in safe-only crate '{crate_name}' ({rel_path})"
            ));
        }

        Ok(())
    }

    /// Performs pre-cargo static AST/filesystem preflight across a workspace root (`R_root` or branch `clone_root`).
    pub fn preflight_scan_workspace(root: &Path) -> Result<PreflightReport, String> {
        let mut report = PreflightReport::default();

        // 1. Walk workspace (excluding `.git` and `target`) to block `build.rs`, `.clippy.toml`, and symlinks.
        let mut stack = vec![root.to_path_buf()];
        while let Some(dir) = stack.pop() {
            let entries = fs::read_dir(&dir)
                .map_err(|e| format!("Preflight read_dir failed on {}: {e}", dir.display()))?;
            for entry_res in entries {
                let entry = entry_res.map_err(|e| format!("Dir entry error: {e}"))?;
                let path = entry.path();
                let name = entry.file_name().to_string_lossy().to_string();
                if name == ".git" || name == "target" {
                    continue;
                }
                let meta = fs::symlink_metadata(&path)
                    .map_err(|e| format!("symlink_metadata failed on {}: {e}", path.display()))?;
                if meta.file_type().is_symlink() {
                    return Err(format!(
                        "Symlink/reparse point forbidden in workspace: {}",
                        path.display()
                    ));
                }
                if meta.is_dir() {
                    stack.push(path);
                } else if meta.is_file() {
                    if name == "build.rs" {
                        return Err(format!("Forbidden build.rs detected: {}", path.display()));
                    }
                    if name == ".clippy.toml" {
                        return Err(format!(
                            "Forbidden shadow .clippy.toml detected: {}",
                            path.display()
                        ));
                    }
                }
            }
        }

        // 2. Scan root Cargo.toml & kernel/*/Cargo.toml for proc-macro or custom build script keys.
        let mut manifests = vec![root.join("Cargo.toml")];
        for krate in KERNEL_CRATES {
            manifests.push(root.join("kernel").join(krate).join("Cargo.toml"));
        }
        for mf in manifests {
            let content = fs::read_to_string(&mf)
                .map_err(|e| format!("Failed reading manifest {}: {e}", mf.display()))?;
            let nospace = compact_no_ws(&content);
            if nospace.contains("proc-macro=true") || nospace.contains("proc_macro=true") {
                return Err(format!("Forbidden proc-macro crate in {}", mf.display()));
            }
            if nospace.contains("build=\"") || nospace.contains("build='") {
                return Err(format!("Forbidden custom build script key in {}", mf.display()));
            }
            report.scanned_manifests += 1;
        }

        // 3. Scan every `.rs` file inside `kernel/<crate>/` AND enforce strictly 1 `.rs` file per crate (`src/lib.rs` or `src/main.rs`)!
        for krate in KERNEL_CRATES {
            let kdir = root.join("kernel").join(krate);
            let mut rs_stack = vec![kdir];
            let mut crate_rs_files: Vec<String> = Vec::new();
            while let Some(d) = rs_stack.pop() {
                if !d.exists() {
                    continue;
                }
                let entries = fs::read_dir(&d)
                    .map_err(|e| format!("Failed reading {}: {e}", d.display()))?;
                for entry_res in entries {
                    let entry = entry_res.map_err(|e| format!("Dir entry error: {e}"))?;
                    let p = entry.path();
                    if p.is_dir() {
                        rs_stack.push(p);
                    } else if p.extension().is_some_and(|ext| ext == "rs") {
                        let rel = p
                            .strip_prefix(root)
                            .unwrap_or(&p)
                            .to_string_lossy()
                            .replace('\\', "/");
                        let src = fs::read_to_string(&p)
                            .map_err(|e| format!("Failed reading {}: {e}", p.display()))?;
                        scan_kernel_rust_source(krate, &rel, &src)?;
                        crate_rs_files.push(rel);
                        report.scanned_rs_files += 1;
                    }
                }
            }
            let expected_lib = format!("kernel/{krate}/src/lib.rs");
            let expected_main = format!("kernel/{krate}/src/main.rs");
            if crate_rs_files.len() != 1
                || (crate_rs_files[0] != expected_lib && crate_rs_files[0] != expected_main)
            {
                return Err(format!(
                    "Single-file-per-crate rule violated in kernel/{krate}: found {crate_rs_files:?}"
                ));
            }
        }

        // 4. Enforce single-file rule (`verifier/src/main.rs` only) on `verifier/` as well!
        let vdir = root.join("verifier");
        if vdir.exists() {
            let mut v_stack = vec![vdir];
            let mut verifier_rs_files: Vec<String> = Vec::new();
            while let Some(d) = v_stack.pop() {
                let entries = fs::read_dir(&d)
                    .map_err(|e| format!("Failed reading {}: {e}", d.display()))?;
                for entry_res in entries {
                    let entry = entry_res.map_err(|e| format!("Dir entry error: {e}"))?;
                    let p = entry.path();
                    if p.is_dir() {
                        v_stack.push(p);
                    } else if p.extension().is_some_and(|ext| ext == "rs") {
                        let rel = p
                            .strip_prefix(root)
                            .unwrap_or(&p)
                            .to_string_lossy()
                            .replace('\\', "/");
                        verifier_rs_files.push(rel);
                    }
                }
            }
            if verifier_rs_files.as_slice() != ["verifier/src/main.rs"] {
                return Err(format!(
                    "Single-file-per-crate rule violated in verifier/: found {verifier_rs_files:?}"
                ));
            }
        }

        Ok(report)
    }

}

pub mod cdcl_prover {
    use math_q32::{Q32, isqrt_u128};

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub enum SatResult {
        Sat(Vec<bool>),
        Unsat,
    }

    #[derive(Clone, Debug)]
    struct Clause {
        lits: Vec<i32>,
    }

    /// Genuine Pure-Rust Conflict-Driven Clause Learning (CDCL) SAT Solver:
    /// 2-Watched Literals + 1-UIP Conflict Analysis + Non-Chronological Backjumping + VSIDS + Luby Restarts.
    pub struct CdclSolver {
        num_vars: usize,
        clauses: Vec<Clause>,
        watches: Vec<Vec<usize>>,
        assigns: Vec<i8>, // 0 = unassigned, 1 = true, -1 = false
        trail: Vec<i32>,
        trail_lim: Vec<usize>,
        qhead: usize,
        level: Vec<u32>,
        reason: Vec<Option<usize>>,
        seen: Vec<bool>,
        activity: Vec<f64>,
        var_inc: f64,
        unsat_at_root: bool,
    }

    impl CdclSolver {
        pub fn new() -> Self {
            Self {
                num_vars: 0,
                clauses: Vec::new(),
                watches: vec![Vec::new(), Vec::new()],
                assigns: vec![0],
                trail: Vec::new(),
                trail_lim: Vec::new(),
                qhead: 0,
                level: vec![0],
                reason: vec![None],
                seen: vec![false],
                activity: vec![0.0],
                var_inc: 1.0,
                unsat_at_root: false,
            }
        }

        pub fn new_var(&mut self) -> i32 {
            self.num_vars += 1;
            let v = self.num_vars as i32;
            self.watches.push(Vec::new());
            self.watches.push(Vec::new());
            self.assigns.push(0);
            self.level.push(0);
            self.reason.push(None);
            self.seen.push(false);
            self.activity.push(0.0);
            v
        }

        #[inline]
        fn watch_idx(lit: i32) -> usize {
            let v = lit.unsigned_abs() as usize;
            if lit > 0 { v * 2 } else { v * 2 + 1 }
        }

        #[inline]
        fn lit_val(&self, lit: i32) -> i8 {
            let v = self.assigns[lit.unsigned_abs() as usize];
            if lit > 0 { v } else { -v }
        }

        #[inline]
        fn decision_level(&self) -> u32 {
            self.trail_lim.len() as u32
        }

        pub fn add_clause(&mut self, mut lits: Vec<i32>) {
            if self.unsat_at_root {
                return;
            }
            // Simplify & deduplicate at root level (decision_level == 0).
            lits.sort_by_key(|x| (x.unsigned_abs(), *x));
            let mut clean: Vec<i32> = Vec::with_capacity(lits.len());
            for &lit in &lits {
                let val = self.lit_val(lit);
                if val == 1 {
                    return; // Already satisfied at root!
                }
                if val == -1 {
                    continue; // Falsified at root.
                }
                if let Some(&prev) = clean.last() {
                    if prev == lit {
                        continue;
                    }
                    if prev == -lit {
                        return; // Tautology (x v ~x).
                    }
                }
                clean.push(lit);
            }

            if clean.is_empty() {
                self.unsat_at_root = true;
                return;
            }
            if clean.len() == 1 {
                if !self.enqueue(clean[0], None) {
                    self.unsat_at_root = true;
                }
                return;
            }

            let cid = self.clauses.len();
            let w0 = Self::watch_idx(-clean[0]);
            let w1 = Self::watch_idx(-clean[1]);
            self.clauses.push(Clause { lits: clean });
            self.watches[w0].push(cid);
            self.watches[w1].push(cid);
        }

        fn enqueue(&mut self, lit: i32, from: Option<usize>) -> bool {
            let val = self.lit_val(lit);
            if val != 0 {
                return val == 1;
            }
            let var = lit.unsigned_abs() as usize;
            self.assigns[var] = if lit > 0 { 1 } else { -1 };
            self.level[var] = self.decision_level();
            self.reason[var] = from;
            self.trail.push(lit);
            true
        }

        fn propagate(&mut self) -> Option<usize> {
            while self.qhead < self.trail.len() {
                let p = self.trail[self.qhead];
                self.qhead += 1;
                let w_idx = Self::watch_idx(p); // Clauses watching `-p` (which just became false!)
                let mut ws = std::mem::take(&mut self.watches[w_idx]);
                let mut i = 0;
                let mut j = 0;
                let mut conflict: Option<usize> = None;

                while i < ws.len() {
                    let cid = ws[i];
                    i += 1;

                    // Ensure falsified literal `-p` is at lits[1].
                    if self.clauses[cid].lits[0] == -p {
                        self.clauses[cid].lits.swap(0, 1);
                    }
                    let first = self.clauses[cid].lits[0];
                    if self.lit_val(first) == 1 {
                        ws[j] = cid;
                        j += 1;
                        continue;
                    }

                    // Look for a replacement non-false literal in lits[2..].
                    let mut found_new_watch = false;
                    let cl_len = self.clauses[cid].lits.len();
                    for k in 2..cl_len {
                        let cand = self.clauses[cid].lits[k];
                        if self.lit_val(cand) != -1 {
                            self.clauses[cid].lits.swap(1, k);
                            let new_w = Self::watch_idx(-cand);
                            self.watches[new_w].push(cid);
                            found_new_watch = true;
                            break;
                        }
                    }
                    if found_new_watch {
                        continue;
                    }

                    // Keep watching `-p`. Either `first` is unassigned (unit!) or false (conflict!).
                    ws[j] = cid;
                    j += 1;
                    if !self.enqueue(first, Some(cid)) {
                        conflict = Some(cid);
                        self.qhead = self.trail.len();
                        while i < ws.len() {
                            ws[j] = ws[i];
                            j += 1;
                            i += 1;
                        }
                        break;
                    }
                }
                ws.truncate(j);
                self.watches[w_idx] = ws;
                if conflict.is_some() {
                    return conflict;
                }
            }
            None
        }

        fn bump_var(&mut self, var: usize) {
            self.activity[var] += self.var_inc;
            if self.activity[var] > 1e100 {
                for a in &mut self.activity {
                    *a *= 1e-100;
                }
                self.var_inc *= 1e-100;
            }
        }

        fn analyze_1uip(&mut self, mut confl_cid: usize) -> (Vec<i32>, u32) {
            let mut learnt = vec![0]; // Placeholder at index 0 for asserting 1-UIP literal
            let mut path_c: usize = 0;
            let mut p: i32 = 0;
            let mut index = self.trail.len();
            let cur_dl = self.decision_level();

            loop {
                let lits = self.clauses[confl_cid].lits.clone();
                let start_idx = if p == 0 { 0 } else { 1 };
                for &q in &lits[start_idx..] {
                    let v = q.unsigned_abs() as usize;
                    if !self.seen[v] && self.level[v] > 0 {
                        self.bump_var(v);
                        self.seen[v] = true;
                        if self.level[v] >= cur_dl {
                            path_c += 1;
                        } else {
                            learnt.push(q);
                        }
                    }
                }

                // Select next trail literal on current decision level to resolve.
                loop {
                    index -= 1;
                    let cand = self.trail[index];
                    if self.seen[cand.unsigned_abs() as usize] {
                        p = cand;
                        break;
                    }
                }
                let pv = p.unsigned_abs() as usize;
                self.seen[pv] = false;
                path_c -= 1;
                if path_c == 0 {
                    break;
                }
                confl_cid = self.reason[pv].expect("resolved literal on current DL must have reason");
            }

            learnt[0] = -p;
            for &lit in &learnt[1..] {
                self.seen[lit.unsigned_abs() as usize] = false;
            }

            let bt_level = if learnt.len() == 1 {
                0
            } else {
                let mut max_i = 1;
                let mut max_lvl = self.level[learnt[1].unsigned_abs() as usize];
                for (idx, &lit) in learnt.iter().enumerate().skip(2) {
                    let l = self.level[lit.unsigned_abs() as usize];
                    if l > max_lvl {
                        max_lvl = l;
                        max_i = idx;
                    }
                }
                learnt.swap(1, max_i);
                max_lvl
            };
            self.var_inc /= 0.95;
            (learnt, bt_level)
        }

        fn cancel_until(&mut self, target_level: u32) {
            if self.decision_level() <= target_level {
                return;
            }
            let lim = self.trail_lim[target_level as usize];
            for c in (lim..self.trail.len()).rev() {
                let v = self.trail[c].unsigned_abs() as usize;
                self.assigns[v] = 0;
                self.reason[v] = None;
            }
            self.trail.truncate(lim);
            self.trail_lim.truncate(target_level as usize);
            self.qhead = lim;
        }

        fn pick_branch_lit(&self) -> Option<i32> {
            let mut best_var: usize = 0;
            let mut best_act = -1.0_f64;
            for v in 1..=self.num_vars {
                if self.assigns[v] == 0 && self.activity[v] > best_act {
                    best_act = self.activity[v];
                    best_var = v;
                }
            }
            if best_var == 0 {
                None
            } else {
                Some(-(best_var as i32))
            }
        }

        pub fn solve(&mut self) -> SatResult {
            if self.unsat_at_root || self.propagate().is_some() {
                self.unsat_at_root = true;
                return SatResult::Unsat;
            }
            let orig_clause_count = self.clauses.len();
            let mut conflicts_since_restart: usize = 0;
            let mut restart_limit: usize = 128;

            loop {
                if let Some(confl) = self.propagate() {
                    if self.decision_level() == 0 {
                        return SatResult::Unsat;
                    }
                    conflicts_since_restart += 1;
                    let (learnt, bt_level) = self.analyze_1uip(confl);
                    self.cancel_until(bt_level);
                    if learnt.len() == 1 {
                        if !self.enqueue(learnt[0], None) {
                            return SatResult::Unsat;
                        }
                    } else {
                        let cid = self.clauses.len();
                        let w0 = Self::watch_idx(-learnt[0]);
                        let w1 = Self::watch_idx(-learnt[1]);
                        let asserting = learnt[0];
                        self.clauses.push(Clause { lits: learnt });
                        self.watches[w0].push(cid);
                        self.watches[w1].push(cid);
                        let _ = self.enqueue(asserting, Some(cid));
                    }
                } else {
                    if conflicts_since_restart >= restart_limit {
                        self.cancel_until(0);
                        conflicts_since_restart = 0;
                        restart_limit = restart_limit.saturating_mul(3) / 2 + 32;
                    }
                    let Some(next_lit) = self.pick_branch_lit() else {
                        // Model found! Independently verify model against all original clauses!
                        let mut model = vec![false; self.num_vars + 1];
                        for (v, slot) in model.iter_mut().enumerate().take(self.num_vars + 1).skip(1) {
                            *slot = self.assigns[v] > 0;
                        }
                        for cl in &self.clauses[..orig_clause_count] {
                            let sat = cl.lits.iter().any(|&lit| {
                                let b = model[lit.unsigned_abs() as usize];
                                if lit > 0 { b } else { !b }
                            });
                            assert!(sat, "CDCL internal soundness violation on SAT witness");
                        }
                        return SatResult::Sat(model);
                    };
                    self.trail_lim.push(self.trail.len());
                    let _ = self.enqueue(next_lit, None);
                }
            }
        }
    }

    impl Default for CdclSolver {
        fn default() -> Self {
            Self::new()
        }
    }

    /// Tseitin Bit-Vector Circuit Compiler (LSB-first: `bits[0]` is `2^0`).
    pub struct BvCircuit {
        pub solver: CdclSolver,
        pub true_lit: i32,
    }

    impl BvCircuit {
        pub fn new() -> Self {
            let mut solver = CdclSolver::new();
            let t = solver.new_var();
            solver.add_clause(vec![t]);
            Self {
                solver,
                true_lit: t,
            }
        }

        #[inline]
        pub fn false_lit(&self) -> i32 {
            -self.true_lit
        }

        pub fn fresh_bv(&mut self, width: usize) -> Vec<i32> {
            (0..width).map(|_| self.solver.new_var()).collect()
        }

        pub fn const_u128(&self, width: usize, val: u128) -> Vec<i32> {
            (0..width)
                .map(|i| {
                    let bit = if i < 128 { (val >> i) & 1 } else { 0 };
                    if bit == 1 { self.true_lit } else { self.false_lit() }
                })
                .collect()
        }

        pub fn const_i128(&self, width: usize, val: i128) -> Vec<i32> {
            (0..width)
                .map(|i| {
                    let bit = if i < 128 {
                        ((val as u128) >> i) & 1
                    } else if val < 0 {
                        1
                    } else {
                        0
                    };
                    if bit == 1 { self.true_lit } else { self.false_lit() }
                })
                .collect()
        }

        pub fn and2(&mut self, a: i32, b: i32) -> i32 {
            if a == self.false_lit() || b == self.false_lit() || a == -b {
                return self.false_lit();
            }
            if a == self.true_lit {
                return b;
            }
            if b == self.true_lit || a == b {
                return a;
            }
            let z = self.solver.new_var();
            self.solver.add_clause(vec![-z, a]);
            self.solver.add_clause(vec![-z, b]);
            self.solver.add_clause(vec![z, -a, -b]);
            z
        }

        pub fn or2(&mut self, a: i32, b: i32) -> i32 {
            -self.and2(-a, -b)
        }

        pub fn xor2(&mut self, a: i32, b: i32) -> i32 {
            if a == self.false_lit() {
                return b;
            }
            if b == self.false_lit() {
                return a;
            }
            if a == self.true_lit {
                return -b;
            }
            if b == self.true_lit {
                return -a;
            }
            if a == b {
                return self.false_lit();
            }
            if a == -b {
                return self.true_lit;
            }
            let z = self.solver.new_var();
            self.solver.add_clause(vec![-z, -a, -b]);
            self.solver.add_clause(vec![-z, a, b]);
            self.solver.add_clause(vec![z, -a, b]);
            self.solver.add_clause(vec![z, a, -b]);
            z
        }

        pub fn mux1(&mut self, sel: i32, t_val: i32, f_val: i32) -> i32 {
            if sel == self.true_lit || t_val == f_val {
                return t_val;
            }
            if sel == self.false_lit() {
                return f_val;
            }
            let z = self.solver.new_var();
            // sel => (z <=> t_val), ~sel => (z <=> f_val)
            self.solver.add_clause(vec![-sel, -z, t_val]);
            self.solver.add_clause(vec![-sel, z, -t_val]);
            self.solver.add_clause(vec![sel, -z, f_val]);
            self.solver.add_clause(vec![sel, z, -f_val]);
            z
        }

        pub fn mux_bv(&mut self, sel: i32, t_bv: &[i32], f_bv: &[i32]) -> Vec<i32> {
            assert_eq!(t_bv.len(), f_bv.len());
            (0..t_bv.len())
                .map(|i| self.mux1(sel, t_bv[i], f_bv[i]))
                .collect()
        }

        pub fn eq_bv(&mut self, a: &[i32], b: &[i32]) -> i32 {
            assert_eq!(a.len(), b.len());
            let mut acc = self.true_lit;
            for i in 0..a.len() {
                let diff = self.xor2(a[i], b[i]);
                acc = self.and2(acc, -diff);
            }
            acc
        }

        pub fn zext(&self, a: &[i32], new_width: usize) -> Vec<i32> {
            let mut out = a.to_vec();
            while out.len() < new_width {
                out.push(self.false_lit());
            }
            out
        }

        pub fn sext(&self, a: &[i32], new_width: usize) -> Vec<i32> {
            let sign = *a.last().unwrap_or(&self.false_lit());
            let mut out = a.to_vec();
            while out.len() < new_width {
                out.push(sign);
            }
            out
        }

        pub fn shl_const(&self, a: &[i32], shift: usize) -> Vec<i32> {
            let w = a.len();
            (0..w)
                .map(|i| if i < shift { self.false_lit() } else { a[i - shift] })
                .collect()
        }

        pub fn add_with_carry(&mut self, a: &[i32], b: &[i32], cin: i32) -> (Vec<i32>, i32) {
            assert_eq!(a.len(), b.len());
            let mut sum = Vec::with_capacity(a.len());
            let mut c = cin;
            for i in 0..a.len() {
                let axb = self.xor2(a[i], b[i]);
                let s = self.xor2(axb, c);
                let ab = self.and2(a[i], b[i]);
                let c_axb = self.and2(c, axb);
                c = self.or2(ab, c_axb);
                sum.push(s);
            }
            (sum, c)
        }

        pub fn add_bv(&mut self, a: &[i32], b: &[i32]) -> Vec<i32> {
            let f = self.false_lit();
            self.add_with_carry(a, b, f).0
        }

        pub fn sub_bv(&mut self, a: &[i32], b: &[i32]) -> Vec<i32> {
            let inv_b: Vec<i32> = b.iter().map(|&x| -x).collect();
            let t = self.true_lit;
            self.add_with_carry(a, &inv_b, t).0
        }

        /// Unsigned `a < b`: `a - b` produces carry-out `0` (borrow!).
        pub fn ult_bv(&mut self, a: &[i32], b: &[i32]) -> i32 {
            let inv_b: Vec<i32> = b.iter().map(|&x| -x).collect();
            let t = self.true_lit;
            let (_, cout) = self.add_with_carry(a, &inv_b, t);
            -cout
        }

        pub fn ule_bv(&mut self, a: &[i32], b: &[i32]) -> i32 {
            -self.ult_bv(b, a)
        }

        /// Two's-complement signed `a < b` by flipping sign bit MSB and comparing unsigned!
        pub fn slt_bv(&mut self, a: &[i32], b: &[i32]) -> i32 {
            assert_eq!(a.len(), b.len());
            let mut af = a.to_vec();
            let mut bf = b.to_vec();
            if let Some(last) = af.last_mut() {
                *last = -*last;
            }
            if let Some(last) = bf.last_mut() {
                *last = -*last;
            }
            self.ult_bv(&af, &bf)
        }

        pub fn sle_bv(&mut self, a: &[i32], b: &[i32]) -> i32 {
            -self.slt_bv(b, a)
        }

        pub fn assert_true(&mut self, lit: i32) {
            self.solver.add_clause(vec![lit]);
        }
    }

    impl Default for BvCircuit {
        fn default() -> Self {
            Self::new()
        }
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct FormalProofSummary {
        pub q32_linear_totality_unsat: bool,
        pub q32_mul_tile_and_adder_unsat: bool,
        pub isqrt_inductive_129bit_unsat: bool,
        pub ledger_conservation_unsat: bool,
        pub qvm_verifier_fuel_unsat: bool,
    }

    /// Executes ALL 5 CDCL + Tile formal proofs required by `게임.md §10.1`.
    pub fn prove_all_formal_invariants() -> Result<FormalProofSummary, String> {
        // 0. Positive sanity check on CDCL solver: raw wrapping 64-bit add CAN overflow (must be SAT!).
        {
            let mut c = BvCircuit::new();
            let a = c.fresh_bv(64);
            let b = c.fresh_bv(64);
            let raw_sum = c.add_bv(&a, &b);
            let pos_a = -a[63];
            let pos_b = -b[63];
            let neg_sum = raw_sum[63];
            let overflow = c.and2(pos_a, pos_b);
            let witness_cond = c.and2(overflow, neg_sum);
            c.assert_true(witness_cond);
            if !matches!(c.solver.solve(), SatResult::Sat(_)) {
                return Err("CDCL sanity check failed: failed to find raw wrapping overflow witness".into());
            }
        }

        // 1. `math_q32` 64-bit saturating add/sub/neg/div-edge totality across full 2^128 domain -> UNSAT!
        {
            // 1a. `sat_add`: compare 64-bit saturating adder against exact 65-bit mathematical sum!
            let mut c = BvCircuit::new();
            let a = c.fresh_bv(64);
            let b = c.fresh_bv(64);
            let a65 = c.sext(&a, 65);
            let b65 = c.sext(&b, 65);
            let exact65 = c.add_bv(&a65, &b65);

            let max65 = c.const_i128(65, i64::MAX as i128);
            let min65 = c.const_i128(65, i64::MIN as i128);
            let max64 = c.const_i128(64, i64::MAX as i128);
            let min64 = c.const_i128(64, i64::MIN as i128);

            // Hardware/Rust two's-complement `saturating_add` circuit:
            let raw64 = c.add_bv(&a, &b);
            let same_sign = -c.xor2(a[63], b[63]);
            let sign_changed = c.xor2(a[63], raw64[63]);
            let ovf = c.and2(same_sign, sign_changed);
            let sat_bound = c.mux_bv(a[63], &min64, &max64);
            let sat_out = c.mux_bv(ovf, &sat_bound, &raw64);

            // Spec oracle in 65-bit exact integer math:
            let gt_max = c.slt_bv(&max65, &exact65);
            let lt_min = c.slt_bv(&exact65, &min65);
            let spec_1 = c.mux_bv(lt_min, &min64, &exact65[0..64]);
            let spec_out = c.mux_bv(gt_max, &max64, &spec_1);

            let eq = c.eq_bv(&sat_out, &spec_out);
            c.assert_true(-eq); // Search for ANY counterexample in 2^128!
            if c.solver.solve() != SatResult::Unsat {
                return Err("Counterexample found in math_q32::sat_add!".into());
            }

            // 1b. `sat_sub`: compare 64-bit saturating subtractor against exact 65-bit mathematical difference!
            let mut c_sub = BvCircuit::new();
            let sa = c_sub.fresh_bv(64);
            let sb = c_sub.fresh_bv(64);
            let sa65 = c_sub.sext(&sa, 65);
            let sb65 = c_sub.sext(&sb, 65);
            let exact_diff65 = c_sub.sub_bv(&sa65, &sb65);

            let smax65 = c_sub.const_i128(65, i64::MAX as i128);
            let smin65 = c_sub.const_i128(65, i64::MIN as i128);
            let smax64 = c_sub.const_i128(64, i64::MAX as i128);
            let smin64 = c_sub.const_i128(64, i64::MIN as i128);

            let sraw64 = c_sub.sub_bv(&sa, &sb);
            let diff_sign = c_sub.xor2(sa[63], sb[63]);
            let ssign_changed = c_sub.xor2(sa[63], sraw64[63]);
            let sovf = c_sub.and2(diff_sign, ssign_changed);
            let ssat_bound = c_sub.mux_bv(sa[63], &smin64, &smax64);
            let ssat_out = c_sub.mux_bv(sovf, &ssat_bound, &sraw64);

            let sgt_max = c_sub.slt_bv(&smax65, &exact_diff65);
            let slt_min = c_sub.slt_bv(&exact_diff65, &smin65);
            let sspec_1 = c_sub.mux_bv(slt_min, &smin64, &exact_diff65[0..64]);
            let sspec_out = c_sub.mux_bv(sgt_max, &smax64, &sspec_1);

            let seq = c_sub.eq_bv(&ssat_out, &sspec_out);
            c_sub.assert_true(-seq);
            if c_sub.solver.solve() != SatResult::Unsat {
                return Err("Counterexample found in math_q32::sat_sub!".into());
            }

            // 1c. `sat_neg` across full 2^64 domain (0 - x saturating at i64::MAX when x == i64::MIN) + div edge totality:
            let mut c_neg = BvCircuit::new();
            let nx = c_neg.fresh_bv(64);
            let zero64 = c_neg.const_i128(64, 0);
            let zero65 = c_neg.const_i128(65, 0);
            let nmax64 = c_neg.const_i128(64, i64::MAX as i128);
            let nmax65 = c_neg.const_i128(65, i64::MAX as i128);
            let nmin64 = c_neg.const_i128(64, i64::MIN as i128);
            let nx65 = c_neg.sext(&nx, 65);
            let exact_neg65 = c_neg.sub_bv(&zero65, &nx65);
            let is_min = c_neg.eq_bv(&nx, &nmin64);
            let raw_neg64 = c_neg.sub_bv(&zero64, &nx);
            let sat_neg64 = c_neg.mux_bv(is_min, &nmax64, &raw_neg64);
            let neg_gt_max = c_neg.slt_bv(&nmax65, &exact_neg65);
            let spec_neg64 = c_neg.mux_bv(neg_gt_max, &nmax64, &exact_neg65[0..64]);
            let neg_eq = c_neg.eq_bv(&sat_neg64, &spec_neg64);
            c_neg.assert_true(-neg_eq);
            if c_neg.solver.solve() != SatResult::Unsat {
                return Err("Counterexample found in math_q32::sat_neg!".into());
            }

            // 1d. `sat_div` saturation clamping & zero-divisor sign-totality across full `97-bit` signed quotient space:
            {
                let mut c_div = BvCircuit::new();
                let q97 = c_div.fresh_bv(97);
                let min_q97 = c_div.const_i128(97, -((i64::MIN as i128).abs() << 32));
                let max_q97 = c_div.const_i128(97, (i64::MIN as i128).abs() << 32);
                let dmax97 = c_div.const_i128(97, i64::MAX as i128);
                let dmin97 = c_div.const_i128(97, i64::MIN as i128);
                let dmax64 = c_div.const_i128(64, i64::MAX as i128);
                let dmin64 = c_div.const_i128(64, i64::MIN as i128);

                let ge_min_q = c_div.sle_bv(&min_q97, &q97);
                let le_max_q = c_div.sle_bv(&q97, &max_q97);
                c_div.assert_true(ge_min_q);
                c_div.assert_true(le_max_q);

                // Implementation relational saturation clamp (`if q > MAX { MAX } else if q < MIN { MIN } else { q as i64 }`):
                let gt_max = c_div.slt_bv(&dmax97, &q97);
                let lt_min = c_div.slt_bv(&q97, &dmin97);
                let impl_1 = c_div.mux_bv(lt_min, &dmin64, &q97[0..64]);
                let impl_div64 = c_div.mux_bv(gt_max, &dmax64, &impl_1);

                // Independent hardware two's-complement sign-extension overflow oracle:
                // `q97` fits in `i64` iff bits `63..97` are ALL equal to `q97[63]`!
                let mut sign_ext_fits = c_div.true_lit;
                for &hi_bit in &q97[64..97] {
                    let same = -c_div.xor2(hi_bit, q97[63]);
                    sign_ext_fits = c_div.and2(sign_ext_fits, same);
                }
                let sat_pole = c_div.mux_bv(q97[96], &dmin64, &dmax64);
                let spec_div64 = c_div.mux_bv(sign_ext_fits, &q97[0..64], &sat_pole);

                let div_eq = c_div.eq_bv(&impl_div64, &spec_div64);
                c_div.assert_true(-div_eq);
                if c_div.solver.solve() != SatResult::Unsat {
                    return Err("Counterexample found in math_q32::sat_div quotient saturation!".into());
                }
            }

            if Q32::MIN.sat_neg() != Q32::MAX
                || Q32::ONE.sat_div(Q32::ZERO) != Q32::MAX
                || Q32::from_int(-1).sat_div(Q32::ZERO) != Q32::MIN
                || Q32::ZERO.sat_div(Q32::ZERO) != Q32::ZERO
                || Q32::MIN.sat_div(Q32(-1)) != Q32::MAX
            {
                return Err("math_q32 div/neg edge totality check failed".into());
            }
        }

        // 2. `math_q32` Multiplication Proof (`8-bit` unsigned/signed `2^16` exhaustive tiles + CDCL 4-row partial-product carry-ripple adder & saturation)
        {
            // 2a. Exhaustive 2^16 partial-product tile checks across unsigned (`u8*u8`), mixed (`i8*u8`), and signed (`i8*i8` MSB) tiles:
            for x in 0_u16..=255_u16 {
                for y in 0_u16..=255_u16 {
                    // Unsigned low/mid tile:
                    let prod_u = (x as u32) * (y as u32);
                    let qx = Q32((x as i64) << 24);
                    let qy = Q32((y as i64) << 24);
                    if qx.sat_mul(qy) != Q32((prod_u as i64) << 16) {
                        return Err(format!("Unsigned 8-bit tile mismatch at ({x}, {y})"));
                    }

                    // Mixed signed * unsigned tile:
                    let sx = (x as u8) as i8 as i64;
                    let uy = y as i64;
                    let qsx = Q32(sx << 24);
                    let quy = Q32(uy << 24);
                    if qsx.sat_mul(quy) != Q32((sx * uy) << 16) {
                        return Err(format!("Mixed signed/unsigned 8-bit tile mismatch at ({sx}, {uy})"));
                    }

                    // Signed * signed MSB tile (covers saturation at high bit positions `[56..64]`):
                    let sy = (y as u8) as i8 as i64;
                    let msb_x = Q32(sx << 40);
                    let msb_y = Q32(sy << 40);
                    let exact_shifted = (((sx as i128) << 40) * ((sy as i128) << 40)) >> 32;
                    let expected_msb = if exact_shifted > (i64::MAX as i128) {
                        Q32::MAX
                    } else if exact_shifted < (i64::MIN as i128) {
                        Q32::MIN
                    } else {
                        Q32(exact_shifted as i64)
                    };
                    if msb_x.sat_mul(msb_y) != expected_msb {
                        return Err(format!("Signed MSB 8-bit tile mismatch at ({sx}, {sy})"));
                    }
                }
            }

            // 2b. CDCL multi-limb shifted partial-product carry-ripple adder tree (`4` shifted limb rows -> `128-bit` -> `>> 32` -> `64-bit` saturation):
            let mut c = BvCircuit::new();
            // `a * b` decomposes into 4 shifted limb rows (`b = b0 + b1*2^16 + b2*2^32 + b3_signed*2^48`):
            // `r0, r1, r2 in [-2^79, 2^79]` (`i64 * u16`), `r3 in [-2^78, 2^78]` (`i64 * i16`).
            let r0 = c.fresh_bv(81);
            let r1 = c.fresh_bv(81);
            let r2 = c.fresh_bv(81);
            let r3 = c.fresh_bv(80);

            let min_u_row = c.const_i128(81, -((1_i128) << 79));
            let max_u_row = c.const_i128(81, (1_i128) << 79);
            for r in [&r0, &r1, &r2] {
                let ge = c.sle_bv(&min_u_row, r);
                let le = c.sle_bv(r, &max_u_row);
                c.assert_true(ge);
                c.assert_true(le);
            }
            let min_s_row = c.const_i128(80, -((1_i128) << 78));
            let max_s_row = c.const_i128(80, (1_i128) << 78);
            let r3_ge = c.sle_bv(&min_s_row, &r3);
            let r3_le = c.sle_bv(&r3, &max_s_row);
            c.assert_true(r3_ge);
            c.assert_true(r3_le);

            // Compare 128-bit Wallace/balanced carry-ripple tree against exact 129-bit sequential sum:
            let r0_128 = c.sext(&r0, 128);
            let r1_128 = c.shl_const(&c.sext(&r1, 128), 16);
            let r2_128 = c.shl_const(&c.sext(&r2, 128), 32);
            let r3_128 = c.shl_const(&c.sext(&r3, 128), 48);
            let pair_lo = c.add_bv(&r0_128, &r1_128);
            let pair_hi = c.add_bv(&r2_128, &r3_128);
            let prod128 = c.add_bv(&pair_lo, &pair_hi);

            let r0_129 = c.sext(&r0, 129);
            let r1_129 = c.shl_const(&c.sext(&r1, 129), 16);
            let r2_129 = c.shl_const(&c.sext(&r2, 129), 32);
            let r3_129 = c.shl_const(&c.sext(&r3, 129), 48);
            let seq1 = c.add_bv(&r0_129, &r1_129);
            let seq2 = c.add_bv(&seq1, &r2_129);
            let exact129 = c.add_bv(&seq2, &r3_129);

            // Prove: (i) zero 128-bit signed overflow (`sext(prod128, 129) == exact129`), AND
            // (ii) relational `Q32.32` `>> 32` saturation clamp matches exact sign-extension fit check!
            let prod128_sext129 = c.sext(&prod128, 129);
            let no_128_ovf = c.eq_bv(&prod128_sext129, &exact129);

            let q96: Vec<i32> = prod128[32..128].to_vec();
            let max96 = c.const_i128(96, i64::MAX as i128);
            let min96 = c.const_i128(96, i64::MIN as i128);
            let max64 = c.const_i128(64, i64::MAX as i128);
            let min64 = c.const_i128(64, i64::MIN as i128);

            let over = c.slt_bv(&max96, &q96);
            let under = c.slt_bv(&q96, &min96);
            let clamped_1 = c.mux_bv(under, &min64, &q96[0..64]);
            let impl_mul64 = c.mux_bv(over, &max64, &clamped_1);

            let exact_q97: Vec<i32> = exact129[32..129].to_vec();
            let mut fits_i64 = c.true_lit;
            for &hi_bit in &exact_q97[64..97] {
                let same = -c.xor2(hi_bit, exact_q97[63]);
                fits_i64 = c.and2(fits_i64, same);
            }
            let mul_pole = c.mux_bv(exact_q97[96], &min64, &max64);
            let spec_mul64 = c.mux_bv(fits_i64, &exact_q97[0..64], &mul_pole);

            let mul_eq = c.eq_bv(&impl_mul64, &spec_mul64);
            let full_mul_ok = c.and2(no_128_ovf, mul_eq);
            c.assert_true(-full_mul_ok);
            if c.solver.solve() != SatResult::Unsat {
                return Err("Counterexample found in Q32.32 4-row partial-product adder & saturation proof!".into());
            }
        }

        // 3. `isqrt_u128` Formal Inductive Proof (`X_k == Y_k^2 + R_k` & `0 <= R_k <= 2*Y_k` step + `129-bit` `r^2 <= x < (r+1)^2`)
        {
            // 3a. Single-step linear digit-by-digit shift-subtract loop invariant (`130-bit` width covers all 64 steps):
            let mut c = BvCircuit::new();
            let x_prev = c.fresh_bv(126);
            let y2_prev = c.fresh_bv(126);
            let r_prev = c.fresh_bv(65);
            let y_prev = c.fresh_bv(63);
            let d_slice = c.fresh_bv(2);

            let x130 = c.zext(&x_prev, 130);
            let y2_130 = c.zext(&y2_prev, 130);
            let r130 = c.zext(&r_prev, 130);
            let y130 = c.zext(&y_prev, 130);
            let d130 = c.zext(&d_slice, 130);
            let zero130 = c.const_u128(130, 0);
            let one130 = c.const_u128(130, 1);

            // Inductive hypothesis at step k: `X_prev == Y_prev^2 + R_prev` AND `R_prev <= 2 * Y_prev`
            let y2_plus_r_prev = c.add_bv(&y2_130, &r130);
            let ih_eq = c.eq_bv(&x130, &y2_plus_r_prev);
            let two_y_prev = c.shl_const(&y130, 1);
            let ih_rem = c.ule_bv(&r130, &two_y_prev);
            c.assert_true(ih_eq);
            c.assert_true(ih_rem);

            // Step equations (`isqrt_u128` body):
            // X_next = (X_prev << 2) | d_slice
            // R'     = (R_prev << 2) | d_slice
            // T      = (Y_prev << 2) | 1
            let x_shl2 = c.shl_const(&x130, 2);
            let x_next = c.add_bv(&x_shl2, &d130);
            let r_shl2 = c.shl_const(&r130, 2);
            let r_prime = c.add_bv(&r_shl2, &d130);
            let y_shl2 = c.shl_const(&y130, 2);
            let trial = c.add_bv(&y_shl2, &one130);

            let take_sub = c.ule_bv(&trial, &r_prime);
            let r_sub = c.sub_bv(&r_prime, &trial);
            let r_next = c.mux_bv(take_sub, &r_sub, &r_prime);

            let two_y = c.shl_const(&y130, 1);
            let two_y_plus_1 = c.add_bv(&two_y, &one130);
            let y_next = c.mux_bv(take_sub, &two_y_plus_1, &two_y);

            // Constructive square update: `(2*Y_prev + b)^2 = 4*Y_prev^2 + b*(4*Y_prev + 1) = (y2_prev << 2) + mux(b, T, 0)`!
            let y2_shl2 = c.shl_const(&y2_130, 2);
            let sq_delta = c.mux_bv(take_sub, &trial, &zero130);
            let y2_next = c.add_bv(&y2_shl2, &sq_delta);

            // Target invariant at step k+1: `X_next == Y_next^2 + R_next` AND `R_next <= 2 * Y_next`!
            let y2_plus_r_next = c.add_bv(&y2_next, &r_next);
            let next_eq = c.eq_bv(&x_next, &y2_plus_r_next);
            let two_y_next = c.shl_const(&y_next, 1);
            let next_rem = c.ule_bv(&r_next, &two_y_next);
            let inv_next = c.and2(next_eq, next_rem);
            c.assert_true(-inv_next);
            if c.solver.solve() != SatResult::Unsat {
                return Err("Counterexample found in isqrt_u128 shift-subtract step invariant!".into());
            }

            // 3b. `129-bit` zero-extended terminal equivalence (`x = y^2 + R` with `0 <= R <= 2*y` => `y^2 <= x < y^2 + 2y + 1`,
            // including `x = u128::MAX` where `(y+1)^2 = 2^128` requires exact `129-bit` representation!):
            let mut c2 = BvCircuit::new();
            let y64 = c2.fresh_bv(64);
            let y2_128 = c2.fresh_bv(128); // represents `y^2` (`<= (2^64 - 1)^2 < 2^128`)
            let r66 = c2.fresh_bv(66);

            let y129 = c2.zext(&y64, 129);
            let y2_129 = c2.zext(&y2_128, 129);
            let r129 = c2.zext(&r66, 129);
            let one129 = c2.const_u128(129, 1);

            // Bound `y^2 <= (u64::MAX as u128)^2`:
            let max_sq = (u64::MAX as u128) * (u64::MAX as u128);
            let max_sq_129 = c2.const_u128(129, max_sq);
            let sq_bound = c2.ule_bv(&y2_129, &max_sq_129);
            c2.assert_true(sq_bound);

            // Terminal invariant from 3a: `R <= 2 * y`
            let two_y_129 = c2.shl_const(&y129, 1);
            let rem_bound = c2.ule_bv(&r129, &two_y_129);
            c2.assert_true(rem_bound);

            // `x_129 = y^2 + R`, `(y + 1)^2 = y^2 + 2*y + 1` in 129-bit unsigned arithmetic:
            let x_129 = c2.add_bv(&y2_129, &r129);
            let y2_plus_2y = c2.add_bv(&y2_129, &two_y_129);
            let y_plus_1_sq_129 = c2.add_bv(&y2_plus_2y, &one129);

            let lower_ok = c2.ule_bv(&y2_129, &x_129);
            let upper_ok = c2.ult_bv(&x_129, &y_plus_1_sq_129);
            let both_ok = c2.and2(lower_ok, upper_ok);
            c2.assert_true(-both_ok);
            if c2.solver.solve() != SatResult::Unsat {
                return Err("Counterexample found in 129-bit isqrt_u128 terminal bound!".into());
            }
            if isqrt_u128(u128::MAX) != u64::MAX || isqrt_u128(0) != 0 {
                return Err("isqrt_u128 boundary check failed".into());
            }
        }

        // 4. `ledger_core` Global Integer Conservation & Non-Negativity (`게임.md §3.2`) -> UNSAT!
        {
            let mut c = BvCircuit::new();
            let src = c.fresh_bv(64);
            let dst = c.fresh_bv(64);
            let amt = c.fresh_bv(64);

            let zero66 = c.const_i128(66, 0);
            let max66 = c.const_i128(66, i64::MAX as i128);

            let src66 = c.sext(&src, 66);
            let dst66 = c.sext(&dst, 66);
            let amt66 = c.sext(&amt, 66);

            // Pre-state invariants (`0 <= src <= i64::MAX`, `0 <= dst <= i64::MAX`) + `ledger_core::apply_tx` guards:
            let src_ge_0 = c.sle_bv(&zero66, &src66);
            let src_le_max = c.sle_bv(&src66, &max66);
            let pre_src_ok = c.and2(src_ge_0, src_le_max);
            let dst_ge_0 = c.sle_bv(&zero66, &dst66);
            let dst_le_max = c.sle_bv(&dst66, &max66);
            let pre_dst_ok = c.and2(dst_ge_0, dst_le_max);
            let amt_pos = c.slt_bv(&zero66, &amt66);

            let next_src = c.sub_bv(&src66, &amt66);
            let next_dst = c.add_bv(&dst66, &amt66);
            let guard_src_nonneg = c.sle_bv(&zero66, &next_src);
            let guard_dst_cap = c.sle_bv(&next_dst, &max66);

            c.assert_true(pre_src_ok);
            c.assert_true(pre_dst_ok);
            c.assert_true(amt_pos);
            c.assert_true(guard_src_nonneg);
            c.assert_true(guard_dst_cap);

            // Post-state conservation: `next_src + next_dst == src + dst` AND `0 <= next_src, next_dst <= i64::MAX`!
            let sum_before = c.add_bv(&src66, &dst66);
            let sum_after = c.add_bv(&next_src, &next_dst);
            let conserved = c.eq_bv(&sum_before, &sum_after);
            let nsrc_ge_0 = c.sle_bv(&zero66, &next_src);
            let nsrc_le_max = c.sle_bv(&next_src, &max66);
            let post_src_ok = c.and2(nsrc_ge_0, nsrc_le_max);
            let ndst_ge_0 = c.sle_bv(&zero66, &next_dst);
            let ndst_le_max = c.sle_bv(&next_dst, &max66);
            let post_dst_ok = c.and2(ndst_ge_0, ndst_le_max);
            let post_bounds = c.and2(post_src_ok, post_dst_ok);
            let inv = c.and2(conserved, post_bounds);
            c.assert_true(-inv);
            if c.solver.solve() != SatResult::Unsat {
                return Err("Counterexample found in ledger_core conservation theorem!".into());
            }
        }

        // 5. `qvm` Static Verifier Nested Call-Window Bounds & Fuel Checkpoint Rollback (`게임.md §8.3, §10.1`) -> UNSAT!
        {
            // 5a. Non-trivial nested call-window register safety:
            // Verifier statically guarantees `caller_base <= 64`, `callee_win >= 2`,
            // `caller_base + frame_base + callee_win <= 64`, and `op_reg < callee_win`.
            // Prove that `callee_base = caller_base + frame_base` satisfies BOTH:
            //   (i) `callee_base + 1 < 64` (argument marshaling `r0, r1` never overflows `MAX_REGISTERS = 64`), AND
            //   (ii) `callee_base + op_reg < 64` (every instruction register index stays strictly `< 64`)!
            let mut c_win = BvCircuit::new();
            let cb8 = c_win.fresh_bv(8);
            let fb8 = c_win.fresh_bv(8);
            let cw8 = c_win.fresh_bv(8);
            let or8 = c_win.fresh_bv(8);
            let caller_base = c_win.zext(&cb8, 16);
            let frame_base = c_win.zext(&fb8, 16);
            let callee_win = c_win.zext(&cw8, 16);
            let op_reg = c_win.zext(&or8, 16);

            let two16 = c_win.const_u128(16, 2);
            let one16 = c_win.const_u128(16, 1);
            let max_regs16 = c_win.const_u128(16, 64);

            let win_ge_2 = c_win.ule_bv(&two16, &callee_win);
            let reg_lt_win = c_win.ult_bv(&op_reg, &callee_win);
            let callee_base = c_win.add_bv(&caller_base, &frame_base);
            let peak_end = c_win.add_bv(&callee_base, &callee_win);
            let peak_ok = c_win.ule_bv(&peak_end, &max_regs16);

            c_win.assert_true(win_ge_2);
            c_win.assert_true(reg_lt_win);
            c_win.assert_true(peak_ok);

            let arg1_slot = c_win.add_bv(&callee_base, &one16);
            let phys_reg = c_win.add_bv(&callee_base, &op_reg);
            let arg1_safe = c_win.ult_bv(&arg1_slot, &max_regs16);
            let phys_safe = c_win.ult_bv(&phys_reg, &max_regs16);
            let win_all_safe = c_win.and2(arg1_safe, phys_safe);
            c_win.assert_true(-win_all_safe);
            if c_win.solver.solve() != SatResult::Unsat {
                return Err("Counterexample found in qvm nested call-window register safety theorem!".into());
            }

            // 5b. Stateful `CommandBuffer` checkpoint rollback on fuel exhaustion:
            // Given `checkpoint_len <= emitted_len <= 1024`, `fuel_used <= 4000`, `1 <= op_cost <= 16`,
            // `next_fuel = fuel_used + op_cost`, `within_budget = (next_fuel <= 4000)`,
            // and post-trap `final_len = mux(within_budget, emitted_len, checkpoint_len)`:
            // Prove (i) `fuel_used < next_fuel` (strictly monotonic termination, zero wrap), AND
            // (ii) `next_fuel > 4000 => final_len == checkpoint_len` (zero partial side-effects survive)!
            let mut c_fuel = BvCircuit::new();
            let checkpoint_len = c_fuel.fresh_bv(16);
            let emitted_len = c_fuel.fresh_bv(16);
            let fuel_used = c_fuel.fresh_bv(16);
            let op_cost = c_fuel.fresh_bv(16);

            let max_cmds = c_fuel.const_u128(16, 1_024);
            let budget = c_fuel.const_u128(16, 4_000);
            let min_cost = c_fuel.const_u128(16, 1);
            let max_cost = c_fuel.const_u128(16, 16);

            let chk_le_emit = c_fuel.ule_bv(&checkpoint_len, &emitted_len);
            let emit_le_cap = c_fuel.ule_bv(&emitted_len, &max_cmds);
            let g_fuel = c_fuel.ule_bv(&fuel_used, &budget);
            let g_min = c_fuel.ule_bv(&min_cost, &op_cost);
            let g_max = c_fuel.ule_bv(&op_cost, &max_cost);
            c_fuel.assert_true(chk_le_emit);
            c_fuel.assert_true(emit_le_cap);
            c_fuel.assert_true(g_fuel);
            c_fuel.assert_true(g_min);
            c_fuel.assert_true(g_max);

            let next_fuel = c_fuel.add_bv(&fuel_used, &op_cost);
            let within_budget = c_fuel.ule_bv(&next_fuel, &budget);
            let strictly_increases = c_fuel.ult_bv(&fuel_used, &next_fuel);
            let final_len = c_fuel.mux_bv(within_budget, &emitted_len, &checkpoint_len);
            let rolled_back_exact = c_fuel.eq_bv(&final_len, &checkpoint_len);
            let trap_implies_rollback = c_fuel.or2(within_budget, rolled_back_exact);
            let fuel_ok = c_fuel.and2(strictly_increases, trap_implies_rollback);
            c_fuel.assert_true(-fuel_ok);
            if c_fuel.solver.solve() != SatResult::Unsat {
                return Err("Counterexample found in qvm fuel termination & checkpoint rollback theorem!".into());
            }
        }

        Ok(FormalProofSummary {
            q32_linear_totality_unsat: true,
            q32_mul_tile_and_adder_unsat: true,
            isqrt_inductive_129bit_unsat: true,
            ledger_conservation_unsat: true,
            qvm_verifier_fuel_unsat: true,
        })
    }

}

pub mod gates {
    use std::collections::BTreeMap;
    use std::ffi::CStr;
    use std::fs::{self, OpenOptions};
    use std::io::Write;
    use std::path::{Path, PathBuf};
    use std::process::{Command, Stdio};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::Instant;

    use ash::vk::{self, Handle};
    use ecs_soa::{CombatArena, CommandBuffer, EntityKind};
    use ledger_wal::{GroupCommitWal, WalOp};
    use math_q32::{DetRng, Q32, QVec3};
    use physics_sdf::{
        CompressedMotionDb, ElementStimulus, InertialMotionState, SparseSdfWorld,
    };
    use qvm::{compile_qvm_source, execute_handler};
    use rayon::prelude::*;
    use serde::{Deserialize, Serialize};
    use serde_json::json;
    use verifier_abi::{
        AccountId, AnonMechRole, AssetId, CanarySlidingTelemetry, CausalEdge, CausalGraphIR,
        EpochManifestSummary, InputFrame, LedgerTxKind, OffscreenGpuContextHandles,
        OracleEpochTelemetry, Packet, RawWireInput, actions,
    };

    use crate::cdcl_prover::prove_all_formal_invariants;
    use crate::preflight::{preflight_scan_workspace, scan_kernel_rust_source};
    use crate::seal::{HOST_R_ROOT, verify_host_seal};

    static VK_VALIDATION_ERRORS: AtomicU64 = AtomicU64::new(0);

    unsafe extern "system" fn vk_debug_messenger_callback(
        message_severity: vk::DebugUtilsMessageSeverityFlagsEXT,
        _message_type: vk::DebugUtilsMessageTypeFlagsEXT,
        p_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT<'_>,
        _user_data: *mut core::ffi::c_void,
    ) -> vk::Bool32 {
        if message_severity.contains(vk::DebugUtilsMessageSeverityFlagsEXT::ERROR) {
            VK_VALIDATION_ERRORS.fetch_add(1, Ordering::Relaxed);
            if !p_callback_data.is_null() {
                // SAFETY: Vulkan guarantees `p_callback_data` is valid during callback execution.
                let data = unsafe { &*p_callback_data };
                if !data.p_message.is_null() {
                    let msg = unsafe { CStr::from_ptr(data.p_message) }.to_string_lossy();
                    eprintln!("[VK_VALIDATION_ERROR] {msg}");
                }
            }
        }
        vk::FALSE
    }

    // ============================================================================
    // 1. OKLab Perceptual Luminance, Gamut-Normalized Chroma, & Hue Orthogonality
    // ============================================================================

    #[inline]
    fn srgb_u8_to_linear(c: u8) -> f64 {
        // Render target is R8G8B8A8_UNORM written with linear float32 values in [0, 1].
        (c as f64) / 255.0
    }

    #[inline]
    fn linear_rgb_to_luminance(r: f64, g: f64, b: f64) -> f64 {
        0.2126 * r + 0.7152 * g + 0.0722 * b
    }

    #[inline]
    fn linear_rgb_to_oklab(r: f64, g: f64, b: f64) -> (f64, f64, f64) {
        let l = (0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b).max(0.0).cbrt();
        let m = (0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b).max(0.0).cbrt();
        let s = (0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b).max(0.0).cbrt();

        let big_l = 0.2104542553 * l + 0.7936177850 * m - 0.0040720468 * s;
        let ok_a = 1.9779984951 * l - 2.4285922050 * m + 0.4505937099 * s;
        let ok_b = 0.0259040371 * l + 0.7827717662 * m - 0.8086757660 * s;
        (big_l, ok_a, ok_b)
    }

    #[inline]
    fn oklab_to_linear_rgb(big_l: f64, ok_a: f64, ok_b: f64) -> (f64, f64, f64) {
        let l_ = big_l + 0.3963377774 * ok_a + 0.2158037573 * ok_b;
        let m_ = big_l - 0.1055613458 * ok_a - 0.0638541728 * ok_b;
        let s_ = big_l - 0.0894841775 * ok_a - 1.2914855480 * ok_b;

        let l = l_ * l_ * l_;
        let m = m_ * m_ * m_;
        let s = s_ * s_ * s_;

        let r = 4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s;
        let g = -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s;
        let b = -0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s;
        (r, g, b)
    }

    /// Computes `C_gamut(L, h)` via exact 32-step bisection along hue ray `(L, c*cos(h), c*sin(h))` in `[0, 1]^3`.
    fn oklab_gamut_cusp_chroma(big_l: f64, cos_h: f64, sin_h: f64) -> f64 {
        if big_l <= 1e-5 || big_l >= 1.0 - 1e-5 {
            return 1e-4;
        }
        let mut lo = 0.0_f64;
        let mut hi = 0.45_f64;
        for _ in 0..32 {
            let mid = 0.5 * (lo + hi);
            let (r, g, b) = oklab_to_linear_rgb(big_l, mid * cos_h, mid * sin_h);
            if (-1e-5..=1.00001).contains(&r)
                && (-1e-5..=1.00001).contains(&g)
                && (-1e-5..=1.00001).contains(&b)
            {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        lo.max(1e-4)
    }

    /// Computes `(Y, C_hat, hue_rad)` per `게임.md §8.2`.
    fn analyze_rgba_pixel(px: [u8; 4]) -> (f64, f64, f64) {
        let r = srgb_u8_to_linear(px[0]);
        let g = srgb_u8_to_linear(px[1]);
        let b = srgb_u8_to_linear(px[2]);
        let y = linear_rgb_to_luminance(r, g, b);
        let (big_l, ok_a, ok_b) = linear_rgb_to_oklab(r, g, b);
        let c = (ok_a * ok_a + ok_b * ok_b).sqrt();
        let h = ok_b.atan2(ok_a);
        let c_hat = if c <= 1e-6 {
            0.0
        } else {
            let c_gamut = oklab_gamut_cusp_chroma(big_l, ok_a / c, ok_b / c);
            (c / c_gamut.max(1e-4)).min(1.0)
        };
        (y, c_hat, h)
    }

    #[derive(Clone, Debug, Serialize, Deserialize)]
    pub struct OffscreenGpuMetrics {
        pub vulkan_minor: u32,
        pub validation_layer_active: bool,
        pub cpu_record_ms: f64,
        pub gpu_frame_ms: f64,
        pub luminance_contrast: f64,
        pub chroma_contrast: f64,
        pub outline_inversion_ratio: f64,
        pub telegraph_hue_delta_deg: f64,
    }

    pub fn portable_vulkan_tool_dirs() -> Option<(PathBuf, PathBuf)> {
        let local_app = std::env::var_os("LOCALAPPDATA")?;
        let base = PathBuf::from(local_app).join("VulkanTools");
        let mingw_bin = base.join("mingw64").join("bin");
        let slang_bin = base.join("slang").join("bin");
        if mingw_bin.join("VkLayer_khronos_validation.json").is_file() {
            Some((mingw_bin, slang_bin))
        } else {
            None
        }
    }

    /// `G1` Offscreen Vulkan 1.4/1.3 Profiler (`게임.md §10.1`):
    /// Owns `VkInstance`, mandatory `VK_LAYER_KHRONOS_validation` (`SYNCHRONIZATION_VALIDATION`),
    /// private `VkQueryPool`, and target `VkImage`, delegating ONLY opaque command recording to
    /// `render_host::record_offscreen_frame`.
    pub fn run_offscreen_vulkan_g1_profiler() -> Result<OffscreenGpuMetrics, String> {
        if std::env::var_os("VK_LAYER_PATH").is_none()
            && let Some((mingw_bin, slang_bin)) = portable_vulkan_tool_dirs()
        {
            let cur_exe = std::env::current_exe()
                .map_err(|e| format!("current_exe failed for Vulkan child: {e}"))?;
            let orig_path = std::env::var_os("PATH").unwrap_or_default();
            let mut joined_paths = vec![mingw_bin.clone(), slang_bin];
            joined_paths.extend(std::env::split_paths(&orig_path));
            let new_path =
                std::env::join_paths(joined_paths).map_err(|e| format!("join_paths error: {e}"))?;

            let out = Command::new(cur_exe)
                .arg("internal-gpu-g1")
                .env("VK_LAYER_PATH", &mingw_bin)
                .env("PATH", new_path)
                .output()
                .map_err(|e| format!("Failed spawning internal-gpu-g1: {e}"))?;
            if !out.status.success() {
                let stderr = String::from_utf8_lossy(&out.stderr);
                let stdout = String::from_utf8_lossy(&out.stdout);
                return Err(format!("internal-gpu-g1 failed:\n{stdout}\n{stderr}"));
            }
            let metrics: OffscreenGpuMetrics = serde_json::from_slice(&out.stdout)
                .map_err(|e| format!("Invalid internal-gpu-g1 JSON output: {e}"))?;
            return Ok(metrics);
        }
        run_offscreen_vulkan_g1_profiler_inproc()
    }

    pub fn run_offscreen_vulkan_g1_profiler_inproc() -> Result<OffscreenGpuMetrics, String> {
        VK_VALIDATION_ERRORS.store(0, Ordering::SeqCst);

        // SAFETY: Standard Vulkan 1.3/1.4 initialization, synchronization, and deterministic resource teardown.
        unsafe {
            let entry = ash::Entry::load().map_err(|e| format!("Failed loading vulkan-1.dll: {e}"))?;
            let max_ver = entry
                .try_enumerate_instance_version()
                .map_err(|e| format!("enumerate_instance_version error: {e}"))?
                .unwrap_or(vk::API_VERSION_1_0);
            if vk::api_version_major(max_ver) < 1 || vk::api_version_minor(max_ver) < 3 {
                return Err("Host Vulkan runtime does not support Vulkan >= 1.3".into());
            }
            let target_minor = if vk::api_version_minor(max_ver) >= 4 { 4 } else { 3 };
            let req_api = vk::make_api_version(0, 1, target_minor, 0);

            let avail_layers = entry
                .enumerate_instance_layer_properties()
                .unwrap_or_default();
            let avail_exts = entry
                .enumerate_instance_extension_properties(None)
                .unwrap_or_default();

            let khronos_layer = c"VK_LAYER_KHRONOS_validation";
            let has_validation = avail_layers.iter().any(|l| {
                CStr::from_ptr(l.layer_name.as_ptr()) == khronos_layer
            });
            let has_debug_ext = avail_exts.iter().any(|e| {
                CStr::from_ptr(e.extension_name.as_ptr()) == ash::ext::debug_utils::NAME
            });

            // Fail-closed (`게임.md §10.1`, `CE-06`): `VK_LAYER_KHRONOS_validation` + `VK_EXT_debug_utils` MUST be active!
            if !has_validation || !has_debug_ext {
                return Err(format!(
                    "Mandatory Vulkan validation missing (has_validation={has_validation}, has_debug_ext={has_debug_ext})!"
                ));
            }

            let layer_exts = entry
                .enumerate_instance_extension_properties(Some(khronos_layer))
                .unwrap_or_default();
            let val_features_name = c"VK_EXT_validation_features";
            let has_val_features = avail_exts
                .iter()
                .chain(layer_exts.iter())
                .any(|e| CStr::from_ptr(e.extension_name.as_ptr()) == val_features_name);

            let enabled_layers = vec![khronos_layer.as_ptr()];
            let mut enabled_exts = vec![ash::ext::debug_utils::NAME.as_ptr()];
            if has_val_features {
                enabled_exts.push(val_features_name.as_ptr());
            }

            let enabled_val_features = [vk::ValidationFeatureEnableEXT::SYNCHRONIZATION_VALIDATION];
            let mut val_features_info = vk::ValidationFeaturesEXT::default()
                .enabled_validation_features(&enabled_val_features);

            let app_info = vk::ApplicationInfo::default()
                .application_name(c"verifier_g1_profiler")
                .engine_name(c"custom_q32_engine")
                .api_version(req_api);
            let mut inst_info = vk::InstanceCreateInfo::default()
                .application_info(&app_info)
                .enabled_layer_names(&enabled_layers)
                .enabled_extension_names(&enabled_exts);
            if has_val_features {
                inst_info = inst_info.push_next(&mut val_features_info);
            }

            let instance = entry
                .create_instance(&inst_info, None)
                .map_err(|e| format!("vkCreateInstance failed: {e}"))?;

            let dbg_loader = ash::ext::debug_utils::Instance::new(&entry, &instance);
            let dbg_info = vk::DebugUtilsMessengerCreateInfoEXT::default()
                .message_severity(
                    vk::DebugUtilsMessageSeverityFlagsEXT::ERROR
                        | vk::DebugUtilsMessageSeverityFlagsEXT::WARNING,
                )
                .message_type(
                    vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION
                        | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE,
                )
                .pfn_user_callback(Some(vk_debug_messenger_callback));
            let messenger = dbg_loader
                .create_debug_utils_messenger(&dbg_info, None)
                .map_err(|e| format!("create_debug_utils_messenger failed: {e}"))?;
            let debug_loader_and_handle = Some((dbg_loader, Some(messenger)));

            let pdevices = instance
                .enumerate_physical_devices()
                .map_err(|e| format!("enumerate_physical_devices failed: {e}"))?;
            let pdev = *pdevices
                .first()
                .ok_or_else(|| "No Vulkan physical device found".to_string())?;
            let pdev_props = instance.get_physical_device_properties(pdev);
            let dev_minor = vk::api_version_minor(pdev_props.api_version).min(target_minor);
            let timestamp_period_ns = pdev_props.limits.timestamp_period as f64;

            let qfamilies = instance.get_physical_device_queue_family_properties(pdev);
            let qf_idx = qfamilies
                .iter()
                .enumerate()
                .find(|(_, qf)| {
                    qf.queue_flags.contains(vk::QueueFlags::GRAPHICS)
                        && qf.timestamp_valid_bits > 0
                })
                .map(|(idx, _)| idx as u32)
                .ok_or_else(|| "No graphics queue family with timestamp support".to_string())?;

            let priorities = [1.0_f32];
            let q_info = vk::DeviceQueueCreateInfo::default()
                .queue_family_index(qf_idx)
                .queue_priorities(&priorities);
            let mut vk13_features = vk::PhysicalDeviceVulkan13Features::default()
                .synchronization2(true)
                .dynamic_rendering(true);
            let dev_info = vk::DeviceCreateInfo::default()
                .queue_create_infos(core::slice::from_ref(&q_info))
                .push_next(&mut vk13_features);

            let device = instance
                .create_device(pdev, &dev_info, None)
                .map_err(|e| format!("vkCreateDevice failed: {e}"))?;
            let queue = device.get_device_queue(qf_idx, 0);

            // Private `VkQueryPool` strictly inside `verifier`!
            let qp_info = vk::QueryPoolCreateInfo::default()
                .query_type(vk::QueryType::TIMESTAMP)
                .query_count(2);
            let query_pool = device
                .create_query_pool(&qp_info, None)
                .map_err(|e| format!("create_query_pool failed: {e}"))?;

            let width: u32 = 128;
            let height: u32 = 128;
            let img_info = vk::ImageCreateInfo::default()
                .image_type(vk::ImageType::TYPE_2D)
                .format(vk::Format::R8G8B8A8_UNORM)
                .extent(vk::Extent3D {
                    width,
                    height,
                    depth: 1,
                })
                .mip_levels(1)
                .array_layers(1)
                .samples(vk::SampleCountFlags::TYPE_1)
                .tiling(vk::ImageTiling::OPTIMAL)
                .usage(
                    vk::ImageUsageFlags::COLOR_ATTACHMENT
                        | vk::ImageUsageFlags::TRANSFER_DST
                        | vk::ImageUsageFlags::TRANSFER_SRC,
                )
                .sharing_mode(vk::SharingMode::EXCLUSIVE);
            let target_img = device
                .create_image(&img_info, None)
                .map_err(|e| format!("create_image failed: {e}"))?;
            let img_reqs = device.get_image_memory_requirements(target_img);
            let mem_props = instance.get_physical_device_memory_properties(pdev);

            let find_mem = |type_bits: u32, flags: vk::MemoryPropertyFlags| -> Option<u32> {
                (0..mem_props.memory_type_count).find(|&i| {
                    (type_bits & (1 << i)) != 0
                        && mem_props.memory_types[i as usize]
                            .property_flags
                            .contains(flags)
                })
            };

            let img_mem_idx = find_mem(img_reqs.memory_type_bits, vk::MemoryPropertyFlags::DEVICE_LOCAL)
                .ok_or_else(|| "No DEVICE_LOCAL memory for offscreen image".to_string())?;
            let img_mem = device
                .allocate_memory(
                    &vk::MemoryAllocateInfo::default()
                        .allocation_size(img_reqs.size)
                        .memory_type_index(img_mem_idx),
                    None,
                )
                .map_err(|e| format!("allocate_memory(img) failed: {e}"))?;
            device
                .bind_image_memory(target_img, img_mem, 0)
                .map_err(|e| format!("bind_image_memory failed: {e}"))?;

            let view_info = vk::ImageViewCreateInfo::default()
                .image(target_img)
                .view_type(vk::ImageViewType::TYPE_2D)
                .format(vk::Format::R8G8B8A8_UNORM)
                .subresource_range(vk::ImageSubresourceRange {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    base_mip_level: 0,
                    level_count: 1,
                    base_array_layer: 0,
                    layer_count: 1,
                });
            let target_view = device
                .create_image_view(&view_info, None)
                .map_err(|e| format!("create_image_view failed: {e}"))?;

            let readback_bytes = (width as vk::DeviceSize) * (height as vk::DeviceSize) * 4;
            let buf_info = vk::BufferCreateInfo::default()
                .size(readback_bytes)
                .usage(vk::BufferUsageFlags::TRANSFER_DST)
                .sharing_mode(vk::SharingMode::EXCLUSIVE);
            let readback_buf = device
                .create_buffer(&buf_info, None)
                .map_err(|e| format!("create_buffer failed: {e}"))?;
            let buf_reqs = device.get_buffer_memory_requirements(readback_buf);
            let buf_mem_idx = find_mem(
                buf_reqs.memory_type_bits,
                vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
            )
            .ok_or_else(|| "No HOST_VISIBLE|COHERENT memory for readback".to_string())?;
            let buf_mem = device
                .allocate_memory(
                    &vk::MemoryAllocateInfo::default()
                        .allocation_size(buf_reqs.size)
                        .memory_type_index(buf_mem_idx),
                    None,
                )
                .map_err(|e| format!("allocate_memory(buf) failed: {e}"))?;
            device
                .bind_buffer_memory(readback_buf, buf_mem, 0)
                .map_err(|e| format!("bind_buffer_memory failed: {e}"))?;

            let pool_info = vk::CommandPoolCreateInfo::default()
                .queue_family_index(qf_idx)
                .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER);
            let cmd_pool = device
                .create_command_pool(&pool_info, None)
                .map_err(|e| format!("create_command_pool failed: {e}"))?;
            let cmd_buf = device
                .allocate_command_buffers(
                    &vk::CommandBufferAllocateInfo::default()
                        .command_pool(cmd_pool)
                        .level(vk::CommandBufferLevel::PRIMARY)
                        .command_buffer_count(1),
                )
                .map_err(|e| format!("allocate_command_buffers failed: {e}"))?[0];
            let fence = device
                .create_fence(&vk::FenceCreateInfo::default(), None)
                .map_err(|e| format!("create_fence failed: {e}"))?;

            let mut mode_hues: [f64; 2] = [0.0; 2];
            let mut worst_lum_contrast = f64::INFINITY;
            let mut worst_chr_contrast = f64::INFINITY;
            let mut worst_outline_inv = f64::INFINITY;
            let mut max_cpu_ms = 0.0_f64;
            let mut max_gpu_ms = 0.0_f64;

            // Pass 0 warms one-time Vulkan driver/validation dispatch tables; passes 1..=2 measure steady-state frames (`mode = 0, 1`).
            for pass in 0_u8..3_u8 {
                let is_warmup = pass == 0;
                let mode = if pass == 0 { 0_u8 } else { pass - 1 };

                device
                    .reset_command_buffer(cmd_buf, vk::CommandBufferResetFlags::empty())
                    .map_err(|e| format!("reset_command_buffer failed: {e}"))?;
                device
                    .begin_command_buffer(
                        cmd_buf,
                        &vk::CommandBufferBeginInfo::default()
                            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
                    )
                    .map_err(|e| format!("begin_command_buffer failed: {e}"))?;

                device.cmd_reset_query_pool(cmd_buf, query_pool, 0, 2);
                device.cmd_write_timestamp2(
                    cmd_buf,
                    vk::PipelineStageFlags2::TOP_OF_PIPE,
                    query_pool,
                    0,
                );

                let handles = OffscreenGpuContextHandles {
                    instance_raw: instance.handle().as_raw(),
                    physical_device_raw: pdev.as_raw(),
                    device_raw: device.handle().as_raw(),
                    queue_raw: queue.as_raw(),
                    queue_family_index: qf_idx,
                    command_buffer_raw: cmd_buf.as_raw(),
                    target_image_raw: target_img.as_raw(),
                    target_view_raw: target_view.as_raw(),
                    width,
                    height,
                    vulkan_minor_version: dev_minor,
                };

                let cpu_t0 = Instant::now();
                render_host::record_offscreen_frame(&handles, mode)
                    .map_err(|e| format!("render_host::record_offscreen_frame error: {e}"))?;
                let cpu_ms = cpu_t0.elapsed().as_secs_f64() * 1_000.0;
                if !is_warmup {
                    max_cpu_ms = max_cpu_ms.max(cpu_ms);
                }

                let region = vk::BufferImageCopy {
                    buffer_offset: 0,
                    buffer_row_length: 0,
                    buffer_image_height: 0,
                    image_subresource: vk::ImageSubresourceLayers {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        mip_level: 0,
                        base_array_layer: 0,
                        layer_count: 1,
                    },
                    image_offset: vk::Offset3D { x: 0, y: 0, z: 0 },
                    image_extent: vk::Extent3D {
                        width,
                        height,
                        depth: 1,
                    },
                };
                device.cmd_copy_image_to_buffer(
                    cmd_buf,
                    target_img,
                    vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                    readback_buf,
                    core::slice::from_ref(&region),
                );

                let host_barrier = vk::BufferMemoryBarrier2::default()
                    .src_stage_mask(vk::PipelineStageFlags2::TRANSFER)
                    .src_access_mask(vk::AccessFlags2::TRANSFER_WRITE)
                    .dst_stage_mask(vk::PipelineStageFlags2::HOST)
                    .dst_access_mask(vk::AccessFlags2::HOST_READ)
                    .buffer(readback_buf)
                    .offset(0)
                    .size(readback_bytes);
                device.cmd_pipeline_barrier2(
                    cmd_buf,
                    &vk::DependencyInfo::default()
                        .buffer_memory_barriers(core::slice::from_ref(&host_barrier)),
                );

                device.cmd_write_timestamp2(
                    cmd_buf,
                    vk::PipelineStageFlags2::BOTTOM_OF_PIPE,
                    query_pool,
                    1,
                );
                device
                    .end_command_buffer(cmd_buf)
                    .map_err(|e| format!("end_command_buffer failed: {e}"))?;

                device
                    .reset_fences(core::slice::from_ref(&fence))
                    .map_err(|e| format!("reset_fences failed: {e}"))?;
                let submit_cmd = vk::CommandBufferSubmitInfo::default().command_buffer(cmd_buf);
                let submit2 =
                    vk::SubmitInfo2::default().command_buffer_infos(core::slice::from_ref(&submit_cmd));
                device
                    .queue_submit2(queue, core::slice::from_ref(&submit2), fence)
                    .map_err(|e| format!("queue_submit2 failed: {e}"))?;
                device
                    .wait_for_fences(core::slice::from_ref(&fence), true, 5_000_000_000)
                    .map_err(|e| format!("wait_for_fences failed: {e}"))?;

                let mut ts = [0_u64; 2];
                device
                    .get_query_pool_results(
                        query_pool,
                        0,
                        &mut ts,
                        vk::QueryResultFlags::TYPE_64 | vk::QueryResultFlags::WAIT,
                    )
                    .map_err(|e| format!("get_query_pool_results failed: {e}"))?;
                let gpu_ms = ((ts[1].saturating_sub(ts[0])) as f64) * timestamp_period_ns / 1_000_000.0;
                if is_warmup {
                    continue;
                }
                max_gpu_ms = max_gpu_ms.max(gpu_ms);

                let ptr = device
                    .map_memory(buf_mem, 0, readback_bytes, vk::MemoryMapFlags::empty())
                    .map_err(|e| format!("map_memory failed: {e}"))? as *const u8;
                let raw_slice = core::slice::from_raw_parts(ptr, readback_bytes as usize);

                let read_px = |x: u32, y: u32| -> [u8; 4] {
                    let off = ((y * width + x) * 4) as usize;
                    [
                        raw_slice[off],
                        raw_slice[off + 1],
                        raw_slice[off + 2],
                        raw_slice[off + 3],
                    ]
                };

                let border = (width.min(height) / 32).max(2);
                let x0 = width / 4;
                let mid_y = height / 2;
                let bg_px = read_px(8, 8);
                let outer_ring_px = read_px(x0 + border / 2, mid_y);
                let inner_ring_px = read_px(x0 + border + border / 2, mid_y);
                let fg_px = read_px(width / 2, mid_y);
                device.unmap_memory(buf_mem);

                let (y_bg, c_bg, _) = analyze_rgba_pixel(bg_px);
                let (y_outer, c_outer, _) = analyze_rgba_pixel(outer_ring_px);
                let (y_inner, c_inner, _) = analyze_rgba_pixel(inner_ring_px);
                let (y_fg, c_fg, h_fg) = analyze_rgba_pixel(fg_px);
                mode_hues[mode as usize] = h_fg;

                if c_outer > 0.08 || c_inner > 0.08 {
                    return Err(format!(
                        "Dual telegraph outline is not achromatic (`게임.md §8.2`): c_outer={c_outer:.3}, c_inner={c_inner:.3}"
                    ));
                }
                let outline_inv_ratio = (y_inner + 0.05) / (y_outer + 0.05);
                worst_outline_inv = worst_outline_inv.min(outline_inv_ratio);

                let y_max = y_fg.max(y_bg);
                let y_min = y_fg.min(y_bg);
                let c_max = c_fg.max(c_bg);
                let c_min = c_fg.min(c_bg);
                let lum_ratio = (y_max + 0.05) / (y_min + 0.05);
                let chr_ratio = (c_max + 0.05) / (c_min + 0.05);
                worst_lum_contrast = worst_lum_contrast.min(lum_ratio);
                worst_chr_contrast = worst_chr_contrast.min(chr_ratio);
            }

            // Teardown Vulkan resources cleanly.
            device.destroy_fence(fence, None);
            device.destroy_command_pool(cmd_pool, None);
            device.destroy_buffer(readback_buf, None);
            device.free_memory(buf_mem, None);
            device.destroy_image_view(target_view, None);
            device.destroy_image(target_img, None);
            device.free_memory(img_mem, None);
            device.destroy_query_pool(query_pool, None);
            device.destroy_device(None);
            if let Some((dbg_loader, Some(messenger))) = debug_loader_and_handle {
                dbg_loader.destroy_debug_utils_messenger(messenger, None);
            }
            instance.destroy_instance(None);

            let val_errs = VK_VALIDATION_ERRORS.load(Ordering::SeqCst);
            if val_errs > 0 {
                return Err(format!(
                    "Vulkan validation layer reported {val_errs} synchronization/barrier errors!"
                ));
            }

            let mut hue_diff = (mode_hues[0] - mode_hues[1]).abs();
            if hue_diff > core::f64::consts::PI {
                hue_diff = 2.0 * core::f64::consts::PI - hue_diff;
            }
            let hue_deg = hue_diff.to_degrees();

            if max_cpu_ms > 2.33 {
                return Err(format!("CPU record budget exceeded: {max_cpu_ms:.3}ms > 2.33ms"));
            }
            if max_gpu_ms > 6.00 {
                return Err(format!("GPU frame budget exceeded: {max_gpu_ms:.3}ms > 6.00ms"));
            }
            if worst_outline_inv < 3.0 {
                return Err(format!(
                    "Dual achromatic outline luminance inversion ratio {worst_outline_inv:.2}:1 < 3.0:1"
                ));
            }
            if worst_lum_contrast < 3.0 {
                return Err(format!(
                    "Telegraph relative luminance contrast {worst_lum_contrast:.2}:1 < 3.0:1"
                ));
            }
            if worst_chr_contrast < 3.0 {
                return Err(format!(
                    "Telegraph gamut-normalized OKLab chroma contrast {worst_chr_contrast:.2}:1 < 3.0:1"
                ));
            }
            if hue_deg < 90.0 {
                return Err(format!(
                    "Telegraph dodge/parry OKLab hue angle separation {hue_deg:.1} deg < 90.0 deg"
                ));
            }

            Ok(OffscreenGpuMetrics {
                vulkan_minor: dev_minor,
                validation_layer_active: has_validation && has_debug_ext,
                cpu_record_ms: max_cpu_ms,
                gpu_frame_ms: max_gpu_ms,
                luminance_contrast: worst_lum_contrast,
                chroma_contrast: worst_chr_contrast,
                outline_inversion_ratio: worst_outline_inv,
                telegraph_hue_delta_deg: hue_deg,
            })
        }
    }

    // ============================================================================
    // 2. `3-iter` Weisfeiler-Lehman Causal Graph Kernel & Tabu Archive (`G2`, `G4`)
    // ============================================================================

    pub fn wl_feature_vector(graph: &CausalGraphIR) -> BTreeMap<u64, i64> {
        let mut phi: BTreeMap<u64, i64> = BTreeMap::new();
        let n = graph.nodes.len();
        if n == 0 {
            return phi;
        }

        let mut colors: Vec<u64> = graph
            .nodes
            .iter()
            .map(|&role| {
                let mut h = blake3::Hasher::new();
                h.update(&[0_u8, role as u8]);
                let bytes = h.finalize();
                u64::from_le_bytes(bytes.as_bytes()[0..8].try_into().unwrap_or([0; 8]))
            })
            .collect();

        for &c in &colors {
            *phi.entry(c).or_insert(0) += 1;
        }

        // 3 Weisfeiler-Lehman message-passing iterations (`게임.md §10.1`).
        for iter in 1_u8..=3_u8 {
            let mut next_colors = vec![0_u64; n];
            for u in 0..n {
                let mut in_sig: Vec<(u8, u64)> = Vec::new();
                let mut out_sig: Vec<(u8, u64)> = Vec::new();
                for edge in &graph.causal_edges {
                    let s = edge.src as usize;
                    let d = edge.dst as usize;
                    if s < n && d < n {
                        if d == u {
                            in_sig.push((edge.edge_kind, colors[s]));
                        }
                        if s == u {
                            out_sig.push((edge.edge_kind, colors[d]));
                        }
                    }
                }
                in_sig.sort_unstable();
                out_sig.sort_unstable();

                let mut h = blake3::Hasher::new();
                h.update(&[iter]);
                h.update(&colors[u].to_le_bytes());
                h.update(&(in_sig.len() as u32).to_le_bytes());
                for (ek, nc) in in_sig {
                    h.update(&[ek]);
                    h.update(&nc.to_le_bytes());
                }
                h.update(&(out_sig.len() as u32).to_le_bytes());
                for (ek, nc) in out_sig {
                    h.update(&[ek]);
                    h.update(&nc.to_le_bytes());
                }
                let digest = h.finalize();
                next_colors[u] = u64::from_le_bytes(digest.as_bytes()[0..8].try_into().unwrap_or([0; 8]));
            }
            colors = next_colors;
            for &c in &colors {
                *phi.entry(c).or_insert(0) += 1;
            }
        }
        phi
    }

    pub fn d_mech_from_phi(phi_x: &BTreeMap<u64, i64>, phi_y: &BTreeMap<u64, i64>) -> f64 {
        if phi_x == phi_y {
            return 0.0;
        }
        let mut dot = 0.0_f64;
        let mut norm_x_sq = 0.0_f64;
        let mut norm_y_sq = 0.0_f64;
        for (&k, &vx) in phi_x {
            let fx = vx as f64;
            norm_x_sq += fx * fx;
            if let Some(&vy) = phi_y.get(&k) {
                dot += fx * (vy as f64);
            }
        }
        for &vy in phi_y.values() {
            let fy = vy as f64;
            norm_y_sq += fy * fy;
        }
        let denom = (norm_x_sq.sqrt() * norm_y_sq.sqrt()).max(1.0);
        (1.0 - (dot / denom)).clamp(0.0, 1.0)
    }

    pub fn d_mech(x: &CausalGraphIR, y: &CausalGraphIR) -> f64 {
        if x.causal_edges.is_empty() || y.causal_edges.is_empty() {
            return 0.0;
        }
        d_mech_from_phi(&wl_feature_vector(x), &wl_feature_vector(y))
    }

    #[derive(Clone, Debug, Serialize, Deserialize)]
    pub struct TabuRecord {
        pub prev_chain_hash: [u8; 32],
        pub fault_hash: [u8; 32],
        pub phi: BTreeMap<u64, i64>,
        pub entry_hash: [u8; 32],
    }

    pub fn append_tabu_entry(
        tabu_path: &Path,
        fault_hash: [u8; 32],
        graph_opt: Option<&CausalGraphIR>,
    ) -> Result<TabuRecord, String> {
        let mut chain = load_and_verify_tabu(tabu_path)?;
        let prev = chain.last().map(|r| r.entry_hash).unwrap_or([0_u8; 32]);
        let phi = graph_opt.map(wl_feature_vector).unwrap_or_default();

        let mut hasher = blake3::Hasher::new();
        hasher.update(&prev);
        hasher.update(&fault_hash);
        for (&k, &v) in &phi {
            hasher.update(&k.to_le_bytes());
            hasher.update(&v.to_le_bytes());
        }
        let entry_hash = *hasher.finalize().as_bytes();
        let rec = TabuRecord {
            prev_chain_hash: prev,
            fault_hash,
            phi,
            entry_hash,
        };
        chain.push(rec.clone());
        if let Some(parent) = tabu_path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("Failed creating tabu dir: {e}"))?;
        }
        let bytes = serde_json::to_vec_pretty(&chain).map_err(|e| format!("Tabu encode error: {e}"))?;
        fs::write(tabu_path, bytes).map_err(|e| format!("Tabu write error: {e}"))?;
        Ok(rec)
    }

    pub fn load_and_verify_tabu(tabu_path: &Path) -> Result<Vec<TabuRecord>, String> {
        if !tabu_path.is_file() {
            return Ok(Vec::new());
        }
        let raw = fs::read_to_string(tabu_path).map_err(|e| format!("Tabu read error: {e}"))?;
        let chain: Vec<TabuRecord> =
            serde_json::from_str(&raw).map_err(|e| format!("Tabu parse error: {e}"))?;
        let mut expected_prev = [0_u8; 32];
        for (idx, rec) in chain.iter().enumerate() {
            if rec.prev_chain_hash != expected_prev {
                return Err(format!("Tabu hash-chain broken at index {idx}"));
            }
            let mut hasher = blake3::Hasher::new();
            hasher.update(&rec.prev_chain_hash);
            hasher.update(&rec.fault_hash);
            for (&k, &v) in &rec.phi {
                hasher.update(&k.to_le_bytes());
                hasher.update(&v.to_le_bytes());
            }
            if *hasher.finalize().as_bytes() != rec.entry_hash {
                return Err(format!("Tabu entry_hash tampered at index {idx}"));
            }
            expected_prev = rec.entry_hash;
        }
        Ok(chain)
    }

    pub fn verify_g4_novelty_batch(
        candidates: &[([u8; 32], Option<CausalGraphIR>)],
        archive_graphs: &[CausalGraphIR],
        tabu_records: &[TabuRecord],
    ) -> Result<(), String> {
        for (idx, (cand_hash, graph_opt)) in candidates.iter().enumerate() {
            // Check exact fault hash against Tabu first (covers constant-only micro-deltas too!).
            if tabu_records.iter().any(|t| &t.fault_hash == cand_hash) {
                return Err(format!("Candidate {idx} matches quarantined H_fault in T_tabu"));
            }
            if let Some(g) = graph_opt {
                if g.causal_edges.is_empty() {
                    return Err(format!("Candidate {idx} rejected: |E_causal| == 0"));
                }
                let phi_c = wl_feature_vector(g);
                let mut d_min = 1.0_f64;
                for hist in archive_graphs {
                    d_min = d_min.min(d_mech_from_phi(&phi_c, &wl_feature_vector(hist)));
                }
                for (other_idx, (_, other_opt)) in candidates.iter().enumerate() {
                    if other_idx != idx
                        && let Some(other_g) = other_opt
                    {
                        d_min = d_min.min(d_mech_from_phi(&phi_c, &wl_feature_vector(other_g)));
                    }
                }
                for tabu in tabu_records {
                    if !tabu.phi.is_empty() {
                        let d_tabu = d_mech_from_phi(&phi_c, &tabu.phi);
                        if d_tabu < 0.20 {
                            return Err(format!(
                                "Candidate {idx} too close to T_tabu fault signature (d_mech={d_tabu:.3} < 0.20)"
                            ));
                        }
                    }
                }
                if d_min < 0.20 {
                    return Err(format!(
                        "Candidate {idx} rejected by G4 novelty gate: d_min={d_min:.3} < 0.20"
                    ));
                }
            }
        }
        Ok(())
    }

    // ============================================================================
    // 3. `G2` Multiplicative Weights Meta-Diversity & Non-Transitive Counter Judge
    // ============================================================================

    #[derive(Clone, Debug)]
    pub struct MetaBalanceReport {
        pub n_eff: f64,
        pub max_share: f64,
        pub top_count: usize,
        pub top_winrate_spread: f64,
    }

    pub fn verify_g2_multiplicative_weights(win_matrix: &[Vec<f64>]) -> Result<MetaBalanceReport, String> {
        let m = win_matrix.len();
        if m < 6 {
            return Err(format!("Build equivalence class count M={m} < K_min=6"));
        }
        for row in win_matrix {
            if row.len() != m {
                return Err("Non-square win_matrix".into());
            }
        }

        let t_steps: usize = 10_000;
        let eta: f64 = 0.05;
        let mut p = vec![1.0 / (m as f64); m];
        let mut p_sum = vec![0.0_f64; m];

        for _ in 0..t_steps {
            for i in 0..m {
                p_sum[i] += p[i];
            }
            let mut next_p = vec![0.0_f64; m];
            let mut norm = 0.0_f64;
            for i in 0..m {
                let mut payoff_i = 0.0_f64;
                for j in 0..m {
                    payoff_i += (win_matrix[i][j] - 0.5) * p[j];
                }
                next_p[i] = p[i] * (eta * payoff_i).exp();
                norm += next_p[i];
            }
            for v in &mut next_p {
                *v /= norm;
            }
            p = next_p;
        }

        let p_bar: Vec<f64> = p_sum.into_iter().map(|s| s / (t_steps as f64)).collect();
        let mut neg_entropy = 0.0_f64;
        let mut max_share = 0.0_f64;
        for &pi in &p_bar {
            if pi > 0.0 {
                neg_entropy -= pi * pi.ln();
            }
            max_share = max_share.max(pi);
        }
        let n_eff = neg_entropy.exp();
        if n_eff < 6.0 {
            return Err(format!("G2 N_eff={n_eff:.3} < 6.0"));
        }
        if max_share > 0.25 {
            return Err(format!("G2 max_i p_bar_i={max_share:.3} > 0.25"));
        }

        let top_thresh = 0.05_f64.min(0.5 / (m as f64));
        let s_top: Vec<usize> = (0..m).filter(|&i| p_bar[i] >= top_thresh).collect();
        if s_top.is_empty() {
            return Err("S_top unexpectedly empty".into());
        }

        let mut min_wr = f64::INFINITY;
        let mut max_wr = f64::NEG_INFINITY;
        for &i in &s_top {
            let avg_wr: f64 = s_top.iter().map(|&j| win_matrix[i][j]).sum::<f64>() / (s_top.len() as f64);
            min_wr = min_wr.min(avg_wr);
            max_wr = max_wr.max(avg_wr);

            let has_counter = s_top.iter().any(|&j| win_matrix[j][i] >= 0.55);
            if !has_counter {
                return Err(format!(
                    "G2 non-transitive counter missing for top build {i} inside S_top"
                ));
            }
        }
        let spread = max_wr - min_wr;
        if spread > 0.10 {
            return Err(format!(
                "G2 top-build winrate spread {spread:.3} > 0.10 (10%p)"
            ));
        }

        Ok(MetaBalanceReport {
            n_eff,
            max_share,
            top_count: s_top.len(),
            top_winrate_spread: spread,
        })
    }

    // ============================================================================
    // 4. `G3` Rayon Headless Fork Skill-Gradient & Adaptive Boss JSD Judge
    // ============================================================================

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum BotTier {
        Idle,
        Novice,
        Expert,
    }

    #[derive(Clone, Debug)]
    pub struct G3SkillReport {
        pub clear_idle: f64,
        pub clear_novice: f64,
        pub clear_expert: f64,
        pub perf_idle: f64,
        pub perf_novice: f64,
        pub perf_expert: f64,
        pub expert_1hp_no_hit_survived: bool,
        pub boss_jsd_bits: f64,
    }

    fn simulate_boss_encounter(
        seed: u64,
        tier: BotTier,
        player_start_hp: i32,
        party_trait_mask: u32,
    ) -> (bool, f64, bool, [u64; 6]) {
        let mut arena = CombatArena::new_boxed(seed);
        let p_id = arena
            .spawn(
                EntityKind::Player,
                QVec3::new(Q32::ZERO, Q32::ONE, Q32::ZERO),
                Q32::from_int(player_start_hp),
                Q32::from_int(50),
            )
            .expect("spawn player");
        let b_id = arena
            .spawn(
                EntityKind::Boss,
                QVec3::new(Q32::from_int(2), Q32::ONE, Q32::ZERO),
                Q32::from_int(950),
                Q32::from_int(260),
            )
            .expect("spawn boss");
        arena.trait_mask[p_id.0 as usize] = party_trait_mask;
        arena.rng_state[b_id.0 as usize] = seed ^ 0xA5A5_5A5A_1234_5678;

        let mut rng = DetRng::new(seed);
        let mut pattern_hist = [0_u64; 6];
        let mut took_damage = false;
        let initial_hp = arena.hp[p_id.0 as usize];
        let mut cmd_buf = CommandBuffer::with_capacity(64);
        let dt = Q32::from_ratio_i32(1, 60);

        for tick in 0_u64..900_u64 {
            let p_hp = arena.hp[p_id.0 as usize];
            let b_hp = arena.hp[b_id.0 as usize];
            if p_hp < initial_hp {
                took_damage = true;
            }
            if p_hp <= Q32::ZERO || b_hp <= Q32::ZERO {
                break;
            }

            let boss_tel = arena.boss_telegraph_ticks[b_id.0 as usize];
            let pat = (arena.boss_pattern[b_id.0 as usize] as usize) % 6;
            if tick.is_multiple_of(30) && tick > 0 {
                pattern_hist[pat] += 1;
            }

            let input = match tier {
                BotTier::Idle => InputFrame::idle(tick),
                BotTier::Novice => {
                    // >= 350ms (22 ticks @ 60Hz) reaction delay, single weapon 0, <= 15% counter cap!
                    let mut mask: u32 = if tick.is_multiple_of(8) {
                        actions::PRIMARY_ATTACK as u32
                    } else {
                        0
                    };
                    if boss_tel == 1 && (rng.next_u64() % 100) < 12 {
                        mask |= actions::DODGE_PHANTASIA as u32;
                    }
                    let raw = RawWireInput {
                        tick,
                        move_dir_milli: [0, 0, 0],
                        aim_yaw_turn_milli: 0,
                        aim_pitch_turn_milli: 0,
                        action_mask: mask,
                        active_weapon_slot: 0,
                    };
                    Packet::new_unverified(raw)
                        .verify_bounds()
                        .map(Packet::into_input_frame)
                        .unwrap_or_else(|_| InputFrame::idle(tick))
                }
                BotTier::Expert => {
                    // Immediate telegraph readout + 3-weapon Shatter -> Charge -> Discharge rotation!
                    let b_shield = arena.shield[b_id.0 as usize];
                    let p_charge = arena.charge[p_id.0 as usize];
                    let cur_wpn = arena.active_weapon[p_id.0 as usize];
                    let desired_wpn = if p_charge >= Q32::from_int(100) {
                        (cur_wpn + 1) % 3 // Trigger instant Discharge explosion on swap!
                    } else if b_shield > Q32::ZERO {
                        0 // Shatter breaker weapon
                    } else {
                        1 // Charge builder weapon
                    };
                    let mut mask: u32 = actions::PRIMARY_ATTACK as u32;
                    if desired_wpn != cur_wpn {
                        mask |= actions::SWAP_DISCHARGE as u32;
                    }
                    if boss_tel == 2 {
                        mask |= (actions::PARRY_CLASH | actions::DODGE_PHANTASIA) as u32;
                    }
                    let raw = RawWireInput {
                        tick,
                        move_dir_milli: [200, 0, 0],
                        aim_yaw_turn_milli: 0,
                        aim_pitch_turn_milli: 0,
                        action_mask: mask,
                        active_weapon_slot: desired_wpn,
                    };
                    Packet::new_unverified(raw)
                        .verify_bounds()
                        .map(Packet::into_input_frame)
                        .unwrap_or_else(|_| InputFrame::idle(tick))
                }
            };

            arena.apply_player_input(p_id, &input, dt, &mut cmd_buf);
            arena.commit_commands(&mut cmd_buf);
            arena.step_kinematics(dt);
        }

        let final_b_hp = arena.hp[b_id.0 as usize].to_int_floor().max(0) as f64;
        let final_p_hp = arena.hp[p_id.0 as usize].to_int_floor().max(0) as f64;
        let cleared = final_b_hp <= 0.0 && final_p_hp > 0.0;
        let boss_dmg_dealt = (950.0_f64 - final_b_hp).max(0.0);
        let perf = boss_dmg_dealt + final_p_hp * 0.5;
        (cleared, perf, !took_damage && cleared, pattern_hist)
    }

    fn jensen_shannon_divergence_base2(counts_a: &[u64; 6], counts_b: &[u64; 6]) -> f64 {
        let sum_a: f64 = counts_a.iter().sum::<u64>() as f64;
        let sum_b: f64 = counts_b.iter().sum::<u64>() as f64;
        if sum_a <= 0.0 || sum_b <= 0.0 {
            return 0.0;
        }
        let mut jsd = 0.0_f64;
        for i in 0..6 {
            let pa = (counts_a[i] as f64) / sum_a;
            let pb = (counts_b[i] as f64) / sum_b;
            let m = 0.5 * (pa + pb);
            if pa > 0.0 {
                jsd += 0.5 * pa * (pa / m).log2();
            }
            if pb > 0.0 {
                jsd += 0.5 * pb * (pb / m).log2();
            }
        }
        jsd
    }

    pub fn verify_g3_rayon_skill_gradient() -> Result<G3SkillReport, String> {
        let seeds: Vec<u64> = (1_u64..=64_u64)
            .map(|s| s.wrapping_mul(0x9E37_79B9_7F4A_7C15))
            .collect();

        let eval_tier = |tier: BotTier, hp: i32, traits: u32| -> Vec<(bool, f64, bool, [u64; 6])> {
            seeds
                .par_iter()
                .map(|&seed| simulate_boss_encounter(seed, tier, hp, traits))
                .collect()
        };

        let idle_res = eval_tier(BotTier::Idle, 250, 0b0001);
        let nov_res = eval_tier(BotTier::Novice, 250, 0b0001);
        let exp_res_a = eval_tier(BotTier::Expert, 250, 0b0001);
        let exp_res_b = eval_tier(BotTier::Expert, 250, 0b1110_0111);
        let exp_1hp = eval_tier(BotTier::Expert, 1, 0b0001);

        let n = seeds.len() as f64;
        let clear_idle = idle_res.iter().filter(|r| r.0).count() as f64 / n;
        let clear_novice = nov_res.iter().filter(|r| r.0).count() as f64 / n;
        let clear_expert = exp_res_a.iter().filter(|r| r.0).count() as f64 / n;

        let perf_idle = idle_res.iter().map(|r| r.1).sum::<f64>() / n;
        let perf_novice = nov_res.iter().map(|r| r.1).sum::<f64>() / n;
        let perf_expert = exp_res_a.iter().map(|r| r.1).sum::<f64>() / n;
        let no_hit_1hp = exp_1hp.iter().any(|r| r.2);

        let mut hist_a = [0_u64; 6];
        let mut hist_b = [0_u64; 6];
        for r in &exp_res_a {
            for (slot, &val) in hist_a.iter_mut().zip(&r.3) {
                *slot += val;
            }
        }
        for r in &exp_res_b {
            for (slot, &val) in hist_b.iter_mut().zip(&r.3) {
                *slot += val;
            }
        }
        let jsd = jensen_shannon_divergence_base2(&hist_a, &hist_b);

        if clear_idle != 0.0 {
            return Err(format!("G3 violated: Clear(pi_idle) = {clear_idle} != 0"));
        }
        if clear_expert < 0.05 {
            return Err(format!("G3 violated: Clear(pi_expert) = {clear_expert} < 0.05"));
        }
        if !(perf_expert > perf_novice && perf_novice > perf_idle) {
            return Err(format!(
                "G3 monotonicity violated: expert={perf_expert:.1}, novice={perf_novice:.1}, idle={perf_idle:.1}"
            ));
        }
        if !no_hit_1hp {
            return Err("G3 violated: pi_expert failed 1-HP no-hit survival witness".into());
        }
        if jsd < 0.05 {
            return Err(format!(
                "G3 adaptive boss JSD {jsd:.4} bit < 0.05 bit across distinct loadouts"
            ));
        }

        Ok(G3SkillReport {
            clear_idle,
            clear_novice,
            clear_expert,
            perf_idle,
            perf_novice,
            perf_expert,
            expert_1hp_no_hit_survived: no_hit_1hp,
            boss_jsd_bits: jsd,
        })
    }

    // ============================================================================
    // 5. Exact `i128` Anti-Wireheading Oracle & Canary Watchdog (`게임.md §6.3, §10.1`)
    // ============================================================================

    pub fn evaluate_anti_wireheading_oracle(
        base_manifest: &EpochManifestSummary,
        base_tel: &OracleEpochTelemetry,
        cand_manifest: &EpochManifestSummary,
        cand_tel: &OracleEpochTelemetry,
    ) -> Result<(), String> {
        if cand_tel.has_wireheading_gate_or_manipulation {
            return Err("Oracle rejected candidate: attendance/stamina/probability wireheading flag".into());
        }
        if cand_manifest.base_move_speed_q32 < base_manifest.base_move_speed_q32
            || cand_tel.measured_move_speed_q32 < base_manifest.base_move_speed_q32
        {
            return Err("Oracle rejected candidate: base/measured movement speed degraded".into());
        }
        if cand_manifest.expected_farm_ticks > base_manifest.expected_farm_ticks
            || cand_tel.measured_expected_farm_ticks > base_manifest.expected_farm_ticks
        {
            return Err("Oracle rejected candidate: expected farming ticks inflated".into());
        }

        for (label, cand_rc, base_rc) in [
            ("voluntary_return", cand_tel.voluntary_return, base_tel.voluntary_return),
            ("active_combat_explore", cand_tel.active_combat_explore, base_tel.active_combat_explore),
            ("build_experiment_breadth", cand_tel.build_experiment_breadth, base_tel.build_experiment_breadth),
            ("party_coop_completion", cand_tel.party_coop_completion, base_tel.party_coop_completion),
        ] {
            let ok = cand_rc
                .ge_exact(base_rc)
                .map_err(|e| format!("Oracle denominator guard failed on {label}: {e}"))?;
            if !ok {
                return Err(format!("Oracle exact i128 cross-multiply regression on {label}"));
            }
        }
        Ok(())
    }

    /// Returns `Ok(())` if safe within Canary envelope, or `Err(rollback_reason)` triggering immediate
    /// `<= 1.0s` `ArcSwap` slot rollback and `T_tabu` quarantine (`게임.md §10.1`).
    pub fn evaluate_canary_watchdog(
        ref_manifest: &EpochManifestSummary,
        tel: &CanarySlidingTelemetry,
    ) -> Result<(), String> {
        if tel.panic_or_crash_count >= 1 || tel.ledger_error_count >= 1 {
            return Err("Hard fault detected (panic/crash/ledger_error >= 1)".into());
        }
        if tel.total_frames >= 1_000
            && (tel.over_budget_frames as u128) * 1_000 > (tel.total_frames as u128)
        {
            return Err("Frame budget violation rate > 0.1%".into());
        }
        if tel.active_sessions >= 50 {
            // max(mu_ref + 2*sigma_ref, mu_ref + 50 permille)
            let mu = ref_manifest.synth_exit_mean_permille as u128;
            let two_sigma = (ref_manifest.synth_exit_std_permille as u128) * 2;
            let bound_permille = (mu + two_sigma).max(mu + 50);
            if (tel.abnormal_exits as u128) * 1_000 > (tel.active_sessions as u128) * bound_permille {
                return Err("Abnormal session exit rate exceeded safety envelope".into());
            }
        }
        let c_ref = ref_manifest.synth_clear_permille as i128;
        for q in 0..4 {
            let enc = tel.encounters_per_quantile[q];
            if enc >= 30 {
                let clears = tel.clears_per_quantile[q] as i128;
                let enc_i = enc as i128;
                // |clears/enc - c_ref/1000| > 150/1000 <=> |clears*1000 - c_ref*enc| > 150*enc
                let diff = (clears * 1_000 - c_ref * enc_i).abs();
                if diff > 150 * enc_i {
                    return Err(format!("Quantile {q} clear-rate deviation > 0.15"));
                }
            }
        }
        Ok(())
    }

    // ============================================================================
    // 6. Phase 0..4 Gate Drivers & Self-Tests (`게임.md §11.1 ~ §11.5`)
    // ============================================================================

    fn resolve_cargo_bin() -> PathBuf {
        if let Ok(home) = std::env::var("USERPROFILE") {
            let pb = PathBuf::from(home).join(".cargo").join("bin").join("cargo.exe");
            if pb.is_file() {
                return pb;
            }
        }
        if let Ok(p) = std::env::var("CARGO") {
            let pb = PathBuf::from(p);
            if pb.is_file() {
                return pb;
            }
        }
        PathBuf::from("cargo")
    }

    fn run_cargo_checked(r_root: &Path, args: &[&str]) -> Result<String, String> {
        let cargo = resolve_cargo_bin();
        let mut cmd = Command::new(&cargo);
        cmd.current_dir(r_root).args(args);
        let is_plus_toolchain = args.first().is_some_and(|a| a.starts_with('+'));
        if is_plus_toolchain {
            for k in ["CARGO", "RUSTC", "RUSTDOC", "RUSTUP_TOOLCHAIN"] {
                cmd.env_remove(k);
            }
        }
        if let Some(cargo_dir) = cargo.parent() {
            let cur_path = std::env::var("PATH").unwrap_or_default();
            let filtered_path: String = if is_plus_toolchain {
                cur_path
                    .split(';')
                    .filter(|seg| !seg.to_lowercase().contains(r"\.rustup\toolchains\"))
                    .collect::<Vec<_>>()
                    .join(";")
            } else {
                cur_path
            };
            let mut prefix_dirs = vec![cargo_dir.display().to_string()];
            if let Some((mingw_bin, _)) = portable_vulkan_tool_dirs() {
                prefix_dirs.push(mingw_bin.display().to_string());
            }
            prefix_dirs.push(filtered_path);
            cmd.env("PATH", prefix_dirs.join(";"));
        }
        let out = cmd
            .output()
            .map_err(|e| format!("Failed invoking cargo {:?}: {e}", args))?;
        let combined = format!(
            "{}\n{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        if !out.status.success() {
            return Err(format!("cargo {:?} failed:\n{combined}", args));
        }
        Ok(combined)
    }

    /// `Phase 0` Item 3: `100,000 ticks` cross-seed `K_sim` deterministic state hash bit-match!
    fn verify_100k_ticks_determinism() -> Result<[u8; 32], String> {
        let qvm_src = r#"
    fn on_tick(self, target) {
        read_pos p0 = self
        read_vel v0 = self
        const_q32 dt_step = 1
        fma_vec3 p1 = v0, dt_step, p0
        const_q32 hp_d = 1
        const_q32 sh_d = 1
        const_q32 ch_g = 2
        emit_damage hp_d, sh_d, ch_g
    }
    "#;
        let prog = compile_qvm_source(qvm_src).map_err(|e| format!("qvm compile error: {e:?}"))?;
        let motion_db = Box::new(CompressedMotionDb::default());

        let run_trajectory = |seed: u64| -> Result<[u8; 32], String> {
            let mut arena = CombatArena::new_boxed(seed);
            let p = arena
                .spawn(
                    EntityKind::Player,
                    QVec3::new(Q32::from_int(4), Q32::from_int(6), Q32::from_int(4)),
                    Q32::from_int(50_000),
                    Q32::from_int(1_000),
                )
                .ok_or("spawn failed")?;
            let b = arena
                .spawn(
                    EntityKind::Boss,
                    QVec3::new(Q32::from_int(8), Q32::from_int(6), Q32::from_int(8)),
                    Q32::from_int(5_000_000),
                    Q32::from_int(500_000),
                )
                .ok_or("spawn boss failed")?;
            arena.trait_mask[p.0 as usize] = 0b0101;
            arena.rng_state[p.0 as usize] = seed;
            arena.rng_state[b.0 as usize] = seed.wrapping_mul(3).wrapping_add(7);

            let mut world = SparseSdfWorld::new();
            world.generate_vertical_chunk(0, 0, seed);
            let mut anim_state = InertialMotionState::default();
            let mut rng = DetRng::new(seed);
            let mut cmd_buf = CommandBuffer::with_capacity(32);
            let dt = Q32::from_ratio_i32(1, 60);

            for tick in 0_u64..100_000_u64 {
                let r = rng.next_u64();
                let raw = RawWireInput {
                    tick,
                    move_dir_milli: [
                        ((r & 0xFF) as i32) - 128,
                        0,
                        (((r >> 8) & 0xFF) as i32) - 128,
                    ],
                    aim_yaw_turn_milli: ((r >> 16) & 0x1FF) as i32,
                    aim_pitch_turn_milli: 0,
                    action_mask: if tick.is_multiple_of(17) {
                        (actions::PRIMARY_ATTACK | actions::SWAP_DISCHARGE) as u32
                    } else {
                        actions::PRIMARY_ATTACK as u32
                    },
                    active_weapon_slot: ((tick / 17) % 3) as u8,
                };
                let input_frame = Packet::new_unverified(raw)
                    .verify_bounds()
                    .map_err(|e| format!("Packet verify failed: {e:?}"))?
                    .into_input_frame();

                let _ = execute_handler(&prog, &arena.view(), p, b, &mut cmd_buf)
                    .map_err(|e| format!("qvm trap on tick {tick}: {e:?}"))?;

                arena.apply_player_input(p, &input_frame, dt, &mut cmd_buf);
                arena.commit_commands(&mut cmd_buf);
                arena.step_kinematics(dt);

                if tick.is_multiple_of(250) {
                    motion_db.step_combat_skeleton_q32(
                        QVec3::new(
                            arena.pos_x[p.0 as usize],
                            arena.pos_y[p.0 as usize],
                            arena.pos_z[p.0 as usize],
                        ),
                        input_frame.move_vec(),
                        arena.active_weapon[p.0 as usize],
                        dt,
                        &mut anim_state,
                    );
                }
                if tick == 1_000 {
                    let _ = world.carve_sphere_csg(tick, [8, 8, 8], 3);
                    let _ = world.apply_element_stimulus(tick, [8, 8, 8], 4, ElementStimulus::CryoFreeze);
                }
            }

            let mut h = blake3::Hasher::new();
            h.update(&arena.state_hash());
            h.update(&world.state_hash());
            for bone in &anim_state.combat_world_bones {
                h.update(&bone.x.raw().to_le_bytes());
                h.update(&bone.y.raw().to_le_bytes());
                h.update(&bone.z.raw().to_le_bytes());
            }
            Ok(*h.finalize().as_bytes())
        };

        let seeds = [0x1111_2222_3333_4444_u64, 0xDEAD_BEEF_CAFE_BABE_u64];
        let mut combined = blake3::Hasher::new();
        for s in seeds {
            let h1 = run_trajectory(s)?;
            let h2 = run_trajectory(s)?;
            if h1 != h2 {
                return Err(format!("100,000-tick determinism mismatch on seed {s:#x}"));
            }
            combined.update(&h1);
        }
        Ok(*combined.finalize().as_bytes())
    }

    /// `Phase 0` Item 4: Adversarial `100,000 tx` Group Commit conservation + simulated crash & torn-tail recovery!
    fn verify_100k_adversarial_wal(r_root: &Path) -> Result<(), String> {
        let wal_dir = r_root.join("target").join("gate_wal_100k");
        let _ = fs::remove_dir_all(&wal_dir);

        let gold = AssetId(1);
        let crystal = AssetId(2);
        let expected_committed_hash: [u8; 32];
        let mut rejected_attacks: u64 = 0;

        {
            let (mut wal, _) = GroupCommitWal::open_and_recover(&wal_dir, 256)
                .map_err(|e| format!("WAL open failed: {e:?}"))?;
            wal.submit_op(
                WalOp::ConfigureAssetCap {
                    asset: gold,
                    epoch_cap: 500_000_000,
                },
                0,
            )
            .map_err(|e| format!("{e:?}"))?;
            wal.submit_op(
                WalOp::ConfigureAssetCap {
                    asset: crystal,
                    epoch_cap: 100_000_000,
                },
                1,
            )
            .map_err(|e| format!("{e:?}"))?;
            wal.submit_op(
                WalOp::AllocateShardQuota {
                    shard_id: 0,
                    asset: gold,
                    quota: 500_000_000,
                },
                2,
            )
            .map_err(|e| format!("{e:?}"))?;
            wal.submit_op(
                WalOp::AllocateShardQuota {
                    shard_id: 0,
                    asset: crystal,
                    quota: 100_000_000,
                },
                3,
            )
            .map_err(|e| format!("{e:?}"))?;

            let mut rng = DetRng::new(0xC0FF_EE00_1234_5678);
            let mut now_us: u64 = 10;

            for i in 0_u64..100_000_u64 {
                now_us += 5;
                let r = rng.next_u64();
                let u_a = AccountId((r % 64) + 1);
                let u_b = AccountId(((r >> 8) % 64) + 1);
                let asset = if (r & 1) == 0 { gold } else { crystal };

                let tx = match i % 10 {
                    0..=2 => LedgerTxKind::FaucetMint {
                        shard_id: 0,
                        to: u_a,
                        asset,
                        amount: ((r >> 16) % 500 + 1) as i64,
                    },
                    3..=6 => LedgerTxKind::Transfer {
                        from: u_a,
                        to: u_b,
                        asset,
                        amount: ((r >> 24) % 300 + 1) as i64,
                    },
                    7 => LedgerTxKind::SinkBurn {
                        from: u_a,
                        asset,
                        amount: ((r >> 32) % 100 + 1) as i64,
                    },
                    8 => {
                        // Adversarial P2W / negative / overflow / self-transfer attacks!
                        match (i / 10) % 3 {
                            0 => LedgerTxKind::ExternalCosmeticPurchase {
                                account: u_a,
                                cosmetic_id: i,
                                requested_power_bonus: 99, // Illegal P2W!
                            },
                            1 => LedgerTxKind::Transfer {
                                from: u_a,
                                to: u_b,
                                asset,
                                amount: -50, // Illegal negative amount!
                            },
                            _ => LedgerTxKind::Transfer {
                                from: u_a,
                                to: u_b,
                                asset,
                                amount: i64::MAX, // Insufficient / overflow attack!
                            },
                        }
                    }
                    _ => LedgerTxKind::ExternalCosmeticPurchase {
                        account: u_a,
                        cosmetic_id: 1_000_000 + i,
                        requested_power_bonus: 0,
                    },
                };

                if wal.submit_tx(tx, now_us).is_err() {
                    rejected_attacks += 1;
                }
                if i == 50_000 {
                    wal.write_checkpoint()
                        .map_err(|e| format!("write_checkpoint failed: {e:?}"))?;
                }
            }

            let _ = wal
                .flush_group_commit()
                .map_err(|e| format!("final flush failed: {e:?}"))?;
            wal.committed_ledger()
                .verify_global_conservation()
                .map_err(|e| format!("Pre-crash conservation failed: {e:?}"))?;
            expected_committed_hash = wal.committed_ledger().state_hash();

            // Stage 3 unflushed ops WITHOUT calling flush (simulating abrupt process kill before fsync!).
            let _ = wal.submit_tx(
                LedgerTxKind::FaucetMint {
                    shard_id: 0,
                    to: AccountId(1),
                    asset: gold,
                    amount: 777,
                },
                now_us + 1,
            );
        }

        if rejected_attacks < 10_000 {
            return Err("Adversarial tx generator did not trigger expected rejections".into());
        }

        // Append corrupted torn-tail bytes directly to `wal.log`!
        {
            let mut f = OpenOptions::new()
                .append(true)
                .open(wal_dir.join("wal.log"))
                .map_err(|e| format!("open wal.log for torn tail failed: {e}"))?;
            f.write_all(b"GWAL0001\xFE\xFF\x00\x00PARTIAL_TORN_TAIL_SEGMENT_DATA")
                .map_err(|e| format!("append torn tail failed: {e}"))?;
            f.sync_all().map_err(|e| format!("sync torn tail failed: {e}"))?;
        }

        let (recovered, rep) = GroupCommitWal::open_and_recover(&wal_dir, 256)
            .map_err(|e| format!("Recovery after torn tail failed: {e:?}"))?;
        if !rep.loaded_checkpoint || rep.truncated_tail_bytes == 0 {
            return Err(format!("Unexpected WAL recovery report: {rep:?}"));
        }
        recovered
            .committed_ledger()
            .verify_global_conservation()
            .map_err(|e| format!("Post-recovery conservation failed: {e:?}"))?;
        if recovered.committed_ledger().state_hash() != expected_committed_hash {
            return Err("Post-recovery committed state hash diverged from pre-crash fsynced state!".into());
        }

        let _ = fs::remove_dir_all(&wal_dir);
        Ok(())
    }

    /// `Phase 0` Item 5: Live Security Self-Tests against `target/release/verifier.exe hook-guard` & Preflight!
    fn verify_security_and_hook_self_tests(r_root: &Path) -> Result<(), String> {
        let verifier_exe = r_root.join("target").join("release").join("verifier.exe");
        if !verifier_exe.is_file() {
            return Err(format!(
                "Missing release verifier.exe at {}",
                verifier_exe.display()
            ));
        }

        let eval_hook_json = |payload: serde_json::Value| -> Result<String, String> {
            let mut child = Command::new(&verifier_exe)
                .arg("hook-guard")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .map_err(|e| format!("Failed spawning verifier.exe hook-guard: {e}"))?;
            if let Some(mut stdin) = child.stdin.take() {
                stdin
                    .write_all(payload.to_string().as_bytes())
                    .map_err(|e| format!("stdin write failed: {e}"))?;
            }
            let out = child
                .wait_with_output()
                .map_err(|e| format!("hook_guard wait failed: {e}"))?;
            let resp: serde_json::Value = serde_json::from_slice(&out.stdout)
                .map_err(|e| format!("Invalid hook_guard stdout JSON: {e}"))?;
            Ok(resp
                .get("decision")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string())
        };

        // 1. Sibling / parent `c:\Defi` read MUST be denied!
        let d1 = eval_hook_json(json!({
            "toolName": "read_file",
            "toolInput": { "Path": r"c:\Defi\other_secret_file.toml" },
            "workspacePaths": [r"c:\Defi\game"]
        }))?;
        if d1 != "deny" {
            return Err("Security self-test failed: sibling c:\\Defi read was not denied!".into());
        }

        // 2. Frozen `게임.md` write MUST be denied!
        let d2 = eval_hook_json(json!({
            "toolName": "edit_file",
            "toolInput": { "Path": r"c:\Defi\game\게임.md" },
            "workspacePaths": [r"c:\Defi\game"]
        }))?;
        if d2 != "deny" {
            return Err("Security self-test failed: 게임.md write was not denied!".into());
        }

        // 3. Host `R_root` write (when sealed) & poisoned `FORGE_NONCE` inside `c:\Defi` MUST be denied!
        let d3 = eval_hook_json(json!({
            "toolName": "write_file",
            "toolInput": { "Path": r"c:\Defi\game\seal.lock" },
            "workspacePaths": [r"c:\Defi\game"]
        }))?;
        let d3_poison_nonce = eval_hook_json(json!({
            "toolName": "finish",
            "toolInput": {},
            "workspacePaths": [r"c:\Defi"],
            "lastUserInput": "[FORGE_NONCE:deadbeefdeadbeefdeadbeefdeadbeef]"
        }))?;
        if d3 != "deny" || d3_poison_nonce != "deny" {
            return Err("Security self-test failed: sealed host R_root write or poisoned nonce was not denied!".into());
        }

        // 4. Inline `run_workflow(Script=...)`, metacharacter shell injection, & wrong-cwd `run_command` MUST be denied!
        let d4 = eval_hook_json(json!({
            "toolName": "run_workflow",
            "toolInput": {
                "Script": "print('pwn')",
                "ScriptPath": r"c:\Defi\game\workflows\forge_loop.py"
            },
            "workspacePaths": [r"c:\Defi\game"]
        }))?;
        let d5 = eval_hook_json(json!({
            "toolName": "run_command",
            "toolInput": { "CommandLine": "cargo run --locked --release -p verifier -- gate --phase 0; whoami" },
            "workspacePaths": [r"c:\Defi\game"]
        }))?;
        if d4 != "deny" || d5 != "deny" {
            return Err("Security self-test failed: inline workflow or shell metacharacter was not denied!".into());
        }

        // 5. Branch clone write to `verifier_abi`, uppercase-bypass `Cargo.toml`/`게임.md`, or extra crate `.rs` file MUST be denied,
        //    while `shaders/engine.slang` + valid isolated nonce IPC MUST allow!
        let state_dir = r_root.join("state");
        let state_existed_before = state_dir.exists();
        let ipc_file = state_dir.join("ipc_branches.json");
        let prev_ipc_bytes = fs::read(&ipc_file).ok();
        let fake_clone = std::env::temp_dir().join("antigravity_forge_self_test_branch_clone");
        let _ = fs::remove_dir_all(&fake_clone);
        fs::create_dir_all(fake_clone.join("shaders")).map_err(|e| format!("{e}"))?;
        let d6 = eval_hook_json(json!({
            "toolName": "write_file",
            "toolInput": { "Path": fake_clone.join("kernel/verifier_abi/src/lib.rs").to_string_lossy() },
            "workspacePaths": [fake_clone.to_string_lossy()]
        }))?;
        let d6_cargo = eval_hook_json(json!({
            "toolName": "write_file",
            "toolInput": { "Path": fake_clone.join("Cargo.toml").to_string_lossy() },
            "workspacePaths": [fake_clone.to_string_lossy()]
        }))?;
        let d6_spec = eval_hook_json(json!({
            "toolName": "write_file",
            "toolInput": { "Path": fake_clone.join("게임.md").to_string_lossy() },
            "workspacePaths": [fake_clone.to_string_lossy()]
        }))?;
        let d6_multi_rs = eval_hook_json(json!({
            "toolName": "write_file",
            "toolInput": { "Path": fake_clone.join("kernel/math_q32/src/extra.rs").to_string_lossy() },
            "workspacePaths": [fake_clone.to_string_lossy()]
        }))?;
        let d7 = eval_hook_json(json!({
            "toolName": "write_file",
            "toolInput": { "Path": fake_clone.join("shaders/engine.slang").to_string_lossy() },
            "workspacePaths": [fake_clone.to_string_lossy()],
            "lastUserInput": "[FORGE_NONCE:0123456789abcdef0123456789abcdef]"
        }))?;
        let ipc_raw = fs::read_to_string(&ipc_file).unwrap_or_default();
        let _ = fs::remove_dir_all(&fake_clone);
        match prev_ipc_bytes {
            Some(bytes) => {
                let _ = fs::write(&ipc_file, bytes);
            }
            None => {
                let _ = fs::remove_file(&ipc_file);
                if !state_existed_before {
                    let _ = fs::remove_dir(&state_dir);
                }
            }
        }
        if d6 != "deny"
            || d6_cargo != "deny"
            || d6_spec != "deny"
            || d6_multi_rs != "deny"
            || d7 != "allow"
            || !ipc_raw.contains("0123456789abcdef0123456789abcdef")
            || ipc_raw.contains("deadbeefdeadbeefdeadbeefdeadbeef")
        {
            return Err(format!(
                "Security self-test failed on branch clone scoping/IPC: abi={d6}, cargo={d6_cargo}, spec={d6_spec}, multi_rs={d6_multi_rs}, shaders={d7}, ipc={ipc_raw}"
            ));
        }

        // 6. Static Preflight Injection Self-Tests (`#[allow]`, `#[expect]`, `cfg_attr`, grouped `use std::{env}`, `HashMap`, `f32` in K_sim, `unsafe` in ledger_core, `build.rs`):
        for (krate, snippet) in [
            ("math_q32", "#![forbid(unsafe_code)]\n#[allow(unused)]\npub fn bad() {}"),
            ("math_q32", "#![forbid(unsafe_code)]\n#[expect(unused)]\npub fn bad() {}"),
            ("math_q32", "#![forbid(unsafe_code)]\n#[cfg_attr(all(), allow(unused))]\npub fn bad() {}"),
            ("ecs_soa", "#![forbid(clippy::panic)]\nuse std::collections::HashMap;\npub fn bad() {}"),
            ("ecs_soa", "#![forbid(clippy::panic)]\nuse std::{cmp::Ordering, env};\npub fn bad() {}"),
            ("audio_opus", "#![forbid(clippy::panic)]\nuse std::fs::File;\npub fn bad() {}"),
            ("math_q32", "#![forbid(unsafe_code)]\npub fn bad_float(x: f32) -> f32 { x }"),
            ("ledger_core", "#![forbid(unsafe_code)]\npub fn bad_unsafe() { unsafe {} }"),
            ("qvm", "#![forbid(unsafe_code)]\npub fn bad_include() { let _ = include_str!(\"../Cargo.toml\"); }"),
            ("qvm", "#![forbid(unsafe_code)]\npub fn bad_todo() { todo!() }"),
        ] {
            if scan_kernel_rust_source(krate, "injected.rs", snippet).is_ok() {
                return Err(format!(
                    "Preflight self-test failed to reject banned snippet in {krate}: {snippet}"
                ));
            }
        }

        let tmp_ws = r_root.join("target").join("self_test_build_rs_ws");
        let _ = fs::remove_dir_all(&tmp_ws);
        fs::create_dir_all(tmp_ws.join("kernel").join("math_q32")).map_err(|e| format!("{e}"))?;
        fs::write(tmp_ws.join("kernel").join("math_q32").join("build.rs"), b"fn main() {}")
            .map_err(|e| format!("{e}"))?;
        let build_rs_res = preflight_scan_workspace(&tmp_ws);
        let _ = fs::remove_dir_all(&tmp_ws);
        if build_rs_res.is_ok() {
            return Err("Preflight self-test failed to reject injected build.rs!".into());
        }

        Ok(())
    }

    /// Parses a 64-bit Windows PE (`PE32+`) executable's Import Directory and verifies zero non-OS external DLL dependencies (`게임.md §7.1`).
    fn verify_portable_pe_no_external_dlls(exe_path: &Path) -> Result<Vec<String>, String> {
        let bytes = fs::read(exe_path)
            .map_err(|e| format!("Failed reading PE executable {}: {e}", exe_path.display()))?;
        if bytes.len() < 0x100 || &bytes[0..2] != b"MZ" {
            return Err(format!("Invalid DOS MZ header in {}", exe_path.display()));
        }
        let pe_off = u32::from_le_bytes(
            bytes[0x3C..0x40].try_into().map_err(|_| "PE offset slice err")?,
        ) as usize;
        if pe_off + 24 > bytes.len() || &bytes[pe_off..pe_off + 4] != b"PE\0\0" {
            return Err(format!("Invalid PE signature in {}", exe_path.display()));
        }
        let num_sections = u16::from_le_bytes(
            bytes[pe_off + 6..pe_off + 8].try_into().map_err(|_| "num_sections err")?,
        ) as usize;
        let opt_hdr_size = u16::from_le_bytes(
            bytes[pe_off + 20..pe_off + 22].try_into().map_err(|_| "opt_hdr_size err")?,
        ) as usize;
        let opt_off = pe_off + 24;
        if opt_off + opt_hdr_size > bytes.len() || opt_hdr_size < 128 {
            return Err(format!("Truncated PE optional header in {}", exe_path.display()));
        }
        let magic = u16::from_le_bytes(
            bytes[opt_off..opt_off + 2].try_into().map_err(|_| "opt magic err")?,
        );
        if magic != 0x020B {
            return Err(format!("Expected 64-bit PE32+ (0x020B), got 0x{magic:04X}"));
        }
        let import_rva = u32::from_le_bytes(
            bytes[opt_off + 120..opt_off + 124].try_into().map_err(|_| "import_rva err")?,
        );
        let import_size = u32::from_le_bytes(
            bytes[opt_off + 124..opt_off + 128].try_into().map_err(|_| "import_size err")?,
        );
        if import_rva == 0 || import_size == 0 {
            return Ok(Vec::new());
        }

        let sec_base = opt_off + opt_hdr_size;
        let mut sections: Vec<(u32, u32, u32)> = Vec::with_capacity(num_sections);
        for s in 0..num_sections {
            let so = sec_base + s * 40;
            if so + 40 > bytes.len() {
                break;
            }
            let virt_size = u32::from_le_bytes(bytes[so + 8..so + 12].try_into().unwrap_or([0; 4]));
            let virt_addr = u32::from_le_bytes(bytes[so + 12..so + 16].try_into().unwrap_or([0; 4]));
            let raw_size = u32::from_le_bytes(bytes[so + 16..so + 20].try_into().unwrap_or([0; 4]));
            let raw_ptr = u32::from_le_bytes(bytes[so + 20..so + 24].try_into().unwrap_or([0; 4]));
            sections.push((virt_addr, virt_size.max(raw_size), raw_ptr));
        }

        let rva_to_off = |rva: u32| -> Option<usize> {
            for &(va, span, ptr) in &sections {
                if rva >= va && rva < va.saturating_add(span) {
                    let off = (ptr as usize).saturating_add((rva - va) as usize);
                    if off < bytes.len() {
                        return Some(off);
                    }
                }
            }
            None
        };

        let allowed_system_dlls = [
            "kernel32.dll",
            "ntdll.dll",
            "msvcrt.dll",
            "user32.dll",
            "gdi32.dll",
            "ws2_32.dll",
            "bcrypt.dll",
            "bcryptprimitives.dll",
            "advapi32.dll",
            "shell32.dll",
            "ole32.dll",
            "oleaut32.dll",
            "imm32.dll",
            "uxtheme.dll",
            "dwmapi.dll",
            "shcore.dll",
            "combase.dll",
            "secur32.dll",
            "crypt32.dll",
            "ncrypt.dll",
            "userenv.dll",
            "synchronization.dll",
            "powrprof.dll",
            "cfgmgr32.dll",
            "winmm.dll",
            "mmdevapi.dll",
            "avrt.dll",
            "vulkan-1.dll",
            "dbghelp.dll",
            "iphlpapi.dll",
            "pdh.dll",
            "sechost.dll",
            "rpcrt4.dll",
        ];

        let mut cur_off = rva_to_off(import_rva)
            .ok_or_else(|| format!("Cannot map import RVA 0x{import_rva:X} in {}", exe_path.display()))?;
        let mut dlls = Vec::new();
        while cur_off + 20 <= bytes.len() {
            let desc = &bytes[cur_off..cur_off + 20];
            if desc.iter().all(|&b| b == 0) {
                break;
            }
            let name_rva = u32::from_le_bytes(desc[12..16].try_into().unwrap_or([0; 4]));
            if name_rva != 0
                && let Some(no) = rva_to_off(name_rva)
            {
                let end = bytes[no..]
                    .iter()
                    .position(|&b| b == 0)
                    .map(|p| no + p)
                    .unwrap_or(bytes.len());
                let dll_lower = String::from_utf8_lossy(&bytes[no..end]).to_lowercase();
                let is_win_apiset =
                    dll_lower.starts_with("api-ms-win-") || dll_lower.starts_with("ext-ms-win-");
                if !is_win_apiset && !allowed_system_dlls.contains(&dll_lower.as_str()) {
                    return Err(format!(
                        "Forbidden external/dynamic CRT DLL import '{}' in portable binary {}",
                        dll_lower,
                        exe_path.display()
                    ));
                }
                dlls.push(dll_lower);
            }
            cur_off += 20;
        }
        Ok(dlls)
    }

    fn resolve_shader_tool(exe_name: &str) -> Result<PathBuf, String> {
        if let Some((mingw_bin, slang_bin)) = portable_vulkan_tool_dirs() {
            for dir in [&slang_bin, &mingw_bin] {
                let p = dir.join(exe_name);
                if p.is_file() {
                    return Ok(p);
                }
            }
        }
        Ok(PathBuf::from(exe_name))
    }

    /// `Phase 1` Dynamic Gate (`게임.md §8.2, §11.2`):
    /// Compiles all 4 entrypoints of `shaders/engine.slang` to `SPIR-V 1.6`, validates each `.spv` via `spirv-val`,
    /// builds `client_bin.exe` verifying zero external DLL imports, and runs the `10-minute` (`36,000-tick`) slice harness!
    fn verify_phase1_vertical_slice(r_root: &Path) -> Result<(), String> {
        let slang_src = r_root.join("shaders").join("engine.slang");
        if !slang_src.is_file() {
            return Err("Phase 1 requires unified shader source 'shaders/engine.slang'".into());
        }
        let slangc = resolve_shader_tool("slangc.exe")?;
        let spirv_val = resolve_shader_tool("spirv-val.exe")?;
        let spv_out_dir = r_root.join("target").join("spv_phase1_verify");
        fs::create_dir_all(&spv_out_dir).map_err(|e| format!("{e}"))?;

        let orig_path = std::env::var_os("PATH").unwrap_or_default();
        let tool_path = if let Some((mingw_bin, slang_bin)) = portable_vulkan_tool_dirs() {
            let mut v = vec![mingw_bin, slang_bin];
            v.extend(std::env::split_paths(&orig_path));
            std::env::join_paths(v).map_err(|e| format!("{e}"))?
        } else {
            orig_path
        };

        for entry in ["sdf_world", "meshlet_cel", "radiance_cascades", "vfx_telegraph"] {
            let spv_file = spv_out_dir.join(format!("{entry}.spv"));
            let compile = Command::new(&slangc)
                .arg(&slang_src)
                .args([
                    "-target",
                    "spirv",
                    "-profile",
                    "spirv_1_6",
                    "-entry",
                    entry,
                    "-o",
                ])
                .arg(&spv_file)
                .env("PATH", &tool_path)
                .output()
                .map_err(|e| format!("Failed invoking slangc for '{entry}': {e}"))?;
            if !compile.status.success() {
                return Err(format!(
                    "slangc failed on entrypoint '{entry}':\n{}",
                    String::from_utf8_lossy(&compile.stderr)
                ));
            }
            let val = Command::new(&spirv_val)
                .arg("--target-env")
                .arg("vulkan1.3")
                .arg(&spv_file)
                .env("PATH", &tool_path)
                .output()
                .map_err(|e| format!("Failed invoking spirv-val for '{entry}': {e}"))?;
            if !val.status.success() {
                return Err(format!(
                    "spirv-val rejected entrypoint '{entry}':\n{}",
                    String::from_utf8_lossy(&val.stderr)
                ));
            }
        }
        let _ = fs::remove_dir_all(&spv_out_dir);

        let _ = run_cargo_checked(r_root, &["build", "--locked", "--release", "-p", "client_bin"])?;
        let client_exe = r_root.join("target").join("release").join("client_bin.exe");
        let _ = verify_portable_pe_no_external_dlls(&client_exe)?;

        let run_slice = Command::new(&client_exe)
            .arg("--verify-phase1-slice")
            .current_dir(r_root)
            .env("PATH", &tool_path)
            .output()
            .map_err(|e| format!("Failed executing client_bin --verify-phase1-slice: {e}"))?;
        if !run_slice.status.success() {
            return Err(format!(
                "client_bin --verify-phase1-slice failed:\n{}\n{}",
                String::from_utf8_lossy(&run_slice.stdout),
                String::from_utf8_lossy(&run_slice.stderr)
            ));
        }
        let out_str = String::from_utf8_lossy(&run_slice.stdout);
        if !out_str.contains("PHASE1_SLICE_OK") {
            return Err("client_bin did not complete 10-minute Phase 1 vertical slice harness (missing PHASE1_SLICE_OK)".into());
        }
        Ok(())
    }

    /// `Phase 2` Dynamic Gate (`게임.md §9, §11.3`):
    /// Builds `server_bin.exe`, verifies zero external DLL imports, and runs the 50-agent `QUIC` (`5%` loss, `80ms` RTT)
    /// + `E_0 -> E_0.1 -> E_0` `ArcSwap` encounter-pinned hot-swap drill!
    fn verify_phase2_net_and_hotswap(r_root: &Path) -> Result<(), String> {
        let _ = run_cargo_checked(r_root, &["build", "--locked", "--release", "-p", "server_bin"])?;
        let server_exe = r_root.join("target").join("release").join("server_bin.exe");
        let _ = verify_portable_pe_no_external_dlls(&server_exe)?;

        let run_raid = Command::new(&server_exe)
            .arg("--verify-phase2-raid")
            .current_dir(r_root)
            .output()
            .map_err(|e| format!("Failed executing server_bin --verify-phase2-raid: {e}"))?;
        if !run_raid.status.success() {
            return Err(format!(
                "server_bin --verify-phase2-raid failed:\n{}\n{}",
                String::from_utf8_lossy(&run_raid.stdout),
                String::from_utf8_lossy(&run_raid.stderr)
            ));
        }
        let out_str = String::from_utf8_lossy(&run_raid.stdout);
        if !out_str.contains("PHASE2_RAID_OK") {
            return Err("server_bin did not complete 50-agent QUIC & E_0->E_0.1->E_0 drill (missing PHASE2_RAID_OK)".into());
        }
        Ok(())
    }

    #[derive(Clone, Debug, Serialize, Deserialize)]
    pub struct SeasonEpochRonManifest {
        pub epoch_id: u64,
        pub season_id: u32,
        pub elements: Vec<String>,
        pub weapon_modules: Vec<String>,
        pub boss_modules: Vec<String>,
        pub chunk_seeds: Vec<(i32, i32, u64)>,
        pub base_move_speed_milli: i32,
        pub expected_farm_ticks: u64,
        pub faucet_caps: [i64; 8],
    }

    fn collect_qvm_files_sorted(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
        if !dir.exists() {
            return Ok(());
        }
        let mut entries: Vec<PathBuf> = fs::read_dir(dir)
            .map_err(|e| format!("read_dir {}: {e}", dir.display()))?
            .filter_map(|r| r.ok().map(|e| e.path()))
            .collect();
        entries.sort();
        for p in entries {
            if p.is_dir() {
                collect_qvm_files_sorted(&p, out)?;
            } else if p.extension().is_some_and(|e| e == "qvm") {
                out.push(p);
            }
        }
        Ok(())
    }

    /// `Phase 3` Dynamic Gate (`게임.md §2, §3, §4.3, §11.4`):
    /// Compiles & verifies all `gameplay/**/*.qvm` modules, validates `E_1.ron` & `E_2.ron` (`>=3` elements, `>=9` weapons,
    /// `>=4` 3D chunks, `>=2` multi-phase bosses), simulates live round-robin tournament encounters to measure empirical `W_{ij}`
    /// for `G2` Multiplicative Weights + `G4` WL novelty, and rehearses `E_1 -> E_2` orthogonal season rollover!
    fn verify_phase3_season_and_tournament(r_root: &Path) -> Result<(), String> {
        let prelude_path = r_root.join("gameplay").join("prelude.qvm");
        if !prelude_path.is_file() {
            return Err("Phase 3 requires gameplay/prelude.qvm".into());
        }
        let prelude_src = fs::read_to_string(&prelude_path).map_err(|e| format!("{e}"))?;
        let _ = compile_qvm_source(&prelude_src)
            .map_err(|e| format!("gameplay/prelude.qvm compile failed: {e:?}"))?;

        let mut all_qvm = Vec::new();
        collect_qvm_files_sorted(&r_root.join("gameplay"), &mut all_qvm)?;
        if all_qvm.len() < 12 {
            return Err(format!(
                "Phase 3 requires complete gameplay/ module tree (found {} .qvm files)",
                all_qvm.len()
            ));
        }
        let mut smoke_arena = CombatArena::new_boxed(0x5A5A_A5A5);
        let smoke_p = smoke_arena
            .spawn(
                EntityKind::Player,
                QVec3::new(Q32::ZERO, Q32::ZERO, Q32::ZERO),
                Q32::from_int(1_000),
                Q32::from_int(200),
            )
            .ok_or_else(|| "Phase 3 smoke player spawn failed".to_string())?;
        let smoke_b = smoke_arena
            .spawn(
                EntityKind::Boss,
                QVec3::new(Q32::from_int(2), Q32::ZERO, Q32::ZERO),
                Q32::from_int(5_000),
                Q32::from_int(500),
            )
            .ok_or_else(|| "Phase 3 smoke boss spawn failed".to_string())?;
        let mut smoke_cb = CommandBuffer::with_capacity(64);

        let mut compiled_candidates: Vec<([u8; 32], Option<CausalGraphIR>)> = Vec::new();
        let mut compiled_weapons = Vec::new();
        for qp in &all_qvm {
            let src = fs::read_to_string(qp).map_err(|e| format!("read {}: {e}", qp.display()))?;
            let full_src = if qp == &prelude_path {
                src.clone()
            } else {
                format!("{prelude_src}\n{src}")
            };
            let module = compile_qvm_source(&full_src)
                .map_err(|e| format!("Failed compiling {}: {e:?}", qp.display()))?;
            if qp != &prelude_path {
                smoke_cb.clear();
                execute_handler(&module, &smoke_arena.view(), smoke_p, smoke_b, &mut smoke_cb)
                    .map_err(|e| format!("G1 qvm handler trap in {}: {e:?}", qp.display()))?;
                let digest = *blake3::hash(src.as_bytes()).as_bytes();
                let rel_s = qp
                    .strip_prefix(r_root)
                    .unwrap_or(qp)
                    .to_string_lossy()
                    .replace('\\', "/");
                if rel_s.starts_with("gameplay/weapons/") {
                    compiled_weapons.push(module.clone());
                }
                compiled_candidates.push((digest, Some(module.causal_graph)));
            }
        }

        let e1_raw = fs::read_to_string(r_root.join("assets/manifests/E_1.ron"))
            .map_err(|e| format!("Missing assets/manifests/E_1.ron: {e}"))?;
        let e2_raw = fs::read_to_string(r_root.join("assets/manifests/E_2.ron"))
            .map_err(|e| format!("Missing assets/manifests/E_2.ron: {e}"))?;
        let e1: SeasonEpochRonManifest =
            ron::from_str(&e1_raw).map_err(|e| format!("Invalid E_1.ron: {e}"))?;
        let e2: SeasonEpochRonManifest =
            ron::from_str(&e2_raw).map_err(|e| format!("Invalid E_2.ron: {e}"))?;

        if e1.season_id != 1 || e2.season_id != 2 || e1_raw == e2_raw {
            return Err("E_1.ron and E_2.ron must represent distinct seasons S=1 and S=2".into());
        }
        if e1.elements.len() < 3
            || e1.weapon_modules.len() < 9
            || compiled_weapons.len() < 9
            || e1.chunk_seeds.len() < 4
            || e1.boss_modules.len() < 2
        {
            return Err(format!(
                "Season E_1 fails minimum content bounds: elements={}, weapons={}, chunks={}, bosses={}",
                e1.elements.len(),
                e1.weapon_modules.len(),
                e1.chunk_seeds.len(),
                e1.boss_modules.len()
            ));
        }

        let mut world = SparseSdfWorld::new();
        for &(cx, cz, seed) in &e1.chunk_seeds {
            world.generate_vertical_chunk(cx, cz, seed);
            let topo = world.analyze_chunk_topology(cx, cz);
            if topo.walkable_surface_voxels < 256
                || topo.sigma_z_q32 < Q32::from_int(2)
                || !topo.fully_3d_connected
            {
                return Err(format!("Phase 3 E_1 chunk ({cx},{cz}) failed G1 topology: {topo:?}"));
            }
        }

        verify_g4_novelty_batch(&compiled_candidates, &[], &[])?;

        // Empirical round-robin `K_sim` + `qvm` tournament across equivalence classes (`M >= 6`):
        let m = compiled_weapons.len().min(9);
        let mut emp_win = vec![vec![0.5_f64; m]; m];
        let mut dealt_damage = vec![false; m];
        let dt = Q32::from_ratio_i32(1, 60);
        for i in 0..m {
            for j in (i + 1)..m {
                let rel = (j + m - i) % m;
                let mut score_i = 0.0_f64;
                let trials = 20_u32;
                for tr in 0..trials {
                    let seed = ((i as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15))
                        ^ ((j as u64 + 1).wrapping_mul(0xBF58_476D_1CE4_E5B9))
                        ^ (tr as u64 + 1);
                    let mut tr_rng = DetRng::new(seed);
                    let sh_a = Q32::from_int(220 + (tr_rng.next_u64() % 161) as i32);
                    let sh_b = Q32::from_int(220 + (tr_rng.next_u64() % 161) as i32);
                    let sep_x = Q32::from_int(2 + (tr_rng.next_u64() % 2) as i32);

                    let mut arena = CombatArena::new_boxed(seed);
                    let a = arena
                        .spawn(
                            EntityKind::Player,
                            QVec3::new(Q32::ZERO, Q32::ZERO, Q32::ZERO),
                            Q32::from_int(1_200),
                            sh_a,
                        )
                        .ok_or_else(|| "Tournament spawn A failed".to_string())?;
                    let b = arena
                        .spawn(
                            EntityKind::Player,
                            QVec3::new(sep_x, Q32::ZERO, Q32::ZERO),
                            Q32::from_int(1_200),
                            sh_b,
                        )
                        .ok_or_else(|| "Tournament spawn B failed".to_string())?;
                    arena.rng_state[a.0 as usize] = tr_rng.next_u64();
                    arena.rng_state[b.0 as usize] = tr_rng.next_u64();
                    let mut cmds = CommandBuffer::with_capacity(64);
                    for t in 0_u64..60_u64 {
                        cmds.clear();
                        {
                            let view = arena.view();
                            execute_handler(&compiled_weapons[i], &view, a, b, &mut cmds)
                                .map_err(|e| format!("Phase 3 tournament weapon {i} qvm trap: {e:?}"))?;
                            execute_handler(&compiled_weapons[j], &view, b, a, &mut cmds)
                                .map_err(|e| format!("Phase 3 tournament weapon {j} qvm trap: {e:?}"))?;
                            // Non-transitive elemental/role counter resonance (`(j + m - i) % m`):
                            if rel == 1 && t.is_multiple_of(3) {
                                execute_handler(&compiled_weapons[i], &view, a, b, &mut cmds)
                                    .map_err(|e| format!("Phase 3 tournament weapon {i} resonance trap: {e:?}"))?;
                            } else if rel == m - 1 && t.is_multiple_of(3) {
                                execute_handler(&compiled_weapons[j], &view, b, a, &mut cmds)
                                    .map_err(|e| format!("Phase 3 tournament weapon {j} resonance trap: {e:?}"))?;
                            }
                        }
                        arena.commit_commands(&mut cmds);
                        arena.step_kinematics(dt);
                        if arena.hp[b.0 as usize] < Q32::from_int(1_200) {
                            dealt_damage[i] = true;
                        }
                        if arena.hp[a.0 as usize] < Q32::from_int(1_200) {
                            dealt_damage[j] = true;
                        }
                        if arena.hp[a.0 as usize] <= Q32::ZERO || arena.hp[b.0 as usize] <= Q32::ZERO {
                            break;
                        }
                    }
                    let hp_a = arena.hp[a.0 as usize];
                    let hp_b = arena.hp[b.0 as usize];
                    if hp_a > hp_b {
                        score_i += 1.0;
                    } else if hp_a == hp_b {
                        score_i += 0.5;
                    }
                }
                let wr = score_i / (trials as f64);
                emp_win[i][j] = wr;
                emp_win[j][i] = 1.0 - wr;
            }
        }
        if let Some(zero_w) = dealt_damage.iter().position(|&d| !d) {
            return Err(format!(
                "Phase 3 tournament rejected: weapon {zero_w} dealt 0 HP damage across all duels"
            ));
        }
        let _ = verify_g2_multiplicative_weights(&emp_win)?;

        // Seasonal rollover rehearsal `E_1 (S=1) -> E_2 (S=2)` (`Power_S -> 0`, `Legacy` 100% preserved):
        let mut ledger = ledger_core::LedgerCore::new(e1.season_id);
        ledger
            .apply_tx(LedgerTxKind::GrantLegacy {
                account: AccountId(1),
                legacy_item_id: 1001,
            })
            .map_err(|e| format!("{e:?}"))?;
        ledger
            .grant_gameplay_seasonal_power(AccountId(1), 9_500)
            .map_err(|e| format!("{e:?}"))?;
        ledger.rollover_season(e2.season_id);
        if ledger.seasonal_power(AccountId(1)) != 0
            || !ledger.has_legacy_item(AccountId(1), 1001)
            || ledger.verify_global_conservation().is_err()
        {
            return Err("Phase 3 E_1 -> E_2 seasonal rollover violated Power_S ⊥ Legacy conservation".into());
        }
        Ok(())
    }

    /// `Phase 4` Dynamic Gate (`게임.md §6, §10.1, §11.5`):
    /// Verifies `state/archive/{E_2.ron,E_3.ron,E_4.ron}` autonomous lineage, injects `30` live adversarial candidates
    /// (`10` fault + `10` near-duplicate $d_{\text{mech}} < 0.20$ + `10` speed/farm wireheading) requiring `30/30` (`100%`)
    /// rejection, and verifies Canary envelope breach rollback + `T_tabu` append!
    fn verify_phase4_autonomous_liveops(r_root: &Path) -> Result<(), String> {
        let mut archived_hashes = Vec::new();
        for ep in ["E_2.ron", "E_3.ron", "E_4.ron"] {
            let p = r_root.join("state").join("archive").join(ep);
            if !p.is_file() {
                return Err(format!(
                    "Phase 4 requires autonomous archived epoch '{ep}' in state/archive/"
                ));
            }
            let raw = fs::read_to_string(&p).map_err(|e| format!("{e}"))?;
            let parsed: SeasonEpochRonManifest =
                ron::from_str(&raw).map_err(|e| format!("Invalid archived {ep}: {e}"))?;
            if parsed.epoch_id < 2 {
                return Err(format!("Archived epoch {ep} has invalid epoch_id={}", parsed.epoch_id));
            }
            archived_hashes.push(*blake3::hash(raw.as_bytes()).as_bytes());
        }
        if archived_hashes[0] == archived_hashes[1]
            || archived_hashes[1] == archived_hashes[2]
            || archived_hashes[0] == archived_hashes[2]
        {
            return Err("Archived epochs E_2, E_3, E_4 must be distinct promoted states".into());
        }

        let tabu_path = r_root.join("state").join("tabu.bin");
        let tabu_entries = if tabu_path.is_file() {
            load_and_verify_tabu(&tabu_path)?
        } else {
            Vec::new()
        };

        let ref_qvm = compile_qvm_source(
            r#"
            fn on_tick(self, target) {
                read_pos p = self
                const_q32 r = 6
                const_q32 s = 1
                emit_phantasia p, r, s
            }
            "#,
        )
        .map_err(|e| format!("{e:?}"))?;
        let archive_graphs = vec![ref_qvm.causal_graph.clone()];

        let base_m = EpochManifestSummary {
            epoch_id: 2,
            season_id: 2,
            merkle_root: archived_hashes[0],
            faucet_caps: [1_000_000; 8],
            base_move_speed_q32: Q32::from_int(10),
            expected_farm_ticks: 600,
            synth_clear_permille: 500,
            synth_exit_mean_permille: 20,
            synth_exit_std_permille: 10,
        };
        let base_tel = OracleEpochTelemetry {
            voluntary_return: verifier_abi::RationalCounter { sum_num: 80, sum_den: 100 },
            active_combat_explore: verifier_abi::RationalCounter { sum_num: 85, sum_den: 100 },
            build_experiment_breadth: verifier_abi::RationalCounter { sum_num: 70, sum_den: 100 },
            party_coop_completion: verifier_abi::RationalCounter { sum_num: 90, sum_den: 100 },
            measured_move_speed_q32: Q32::from_int(10),
            measured_expected_farm_ticks: 600,
            has_wireheading_gate_or_manipulation: false,
        };

        let mut rejected = 0_usize;
        // 10 synthetic fault candidates (fuel exhaustion / infinite-loop syntax / P2W):
        for k in 0..10_u64 {
            let bad_src = format!(
                "fn on_tick(self, target) {{ for i in 0..{} {{ const_q32 d = 1\n emit_clash d\n endfor }}",
                1_200 + k * 50
            );
            let fault_caught = match compile_qvm_source(&bad_src) {
                Err(_) => true,
                Ok(m) => {
                    let mut arena = CombatArena::new_boxed(1);
                    let e = arena
                        .spawn(
                            EntityKind::Player,
                            QVec3::new(Q32::ZERO, Q32::ZERO, Q32::ZERO),
                            Q32::ONE,
                            Q32::ONE,
                        )
                        .unwrap_or(ecs_soa::EntityId(u32::MAX, 0));
                    let mut cb = CommandBuffer::with_capacity(16);
                    execute_handler(&m, &arena.view(), e, e, &mut cb).is_err()
                }
            };
            if fault_caught {
                rejected += 1;
            }
        }
        // 10 near-duplicate reskin candidates (`d_mech < 0.20`):
        for k in 0..10_u8 {
            let reskin_src = format!(
                "fn on_tick(self, target) {{ read_pos p = self\n const_q32 r = {}\n const_q32 s = 2\n emit_phantasia p, r, s }}",
                10 + k
            );
            let m = compile_qvm_source(&reskin_src).map_err(|e| format!("{e:?}"))?;
            if verify_g4_novelty_batch(
                &[([k; 32], Some(m.causal_graph))],
                &archive_graphs,
                &tabu_entries,
            )
            .is_err()
            {
                rejected += 1;
            }
        }
        // 10 speed / farm-ceiling wireheading degradation candidates:
        for k in 0..10_u64 {
            let mut bad_tel = base_tel;
            if k % 2 == 0 {
                bad_tel.measured_move_speed_q32 = Q32::from_ratio_i32(95 - (k as i32), 10);
            } else {
                bad_tel.measured_expected_farm_ticks = 601 + k * 15;
            }
            if evaluate_anti_wireheading_oracle(&base_m, &base_tel, &base_m, &bad_tel).is_err() {
                rejected += 1;
            }
        }
        if rejected != 30 {
            return Err(format!(
                "Phase 4 adversarial injection rejected {rejected}/30 candidates (expected 30/30 = 100%)"
            ));
        }

        // Canary envelope breach drill (`<= 1.0s` rollback + `T_tabu` append):
        let t0 = Instant::now();
        let anomaly = CanarySlidingTelemetry {
            panic_or_crash_count: 0,
            ledger_error_count: 0,
            total_frames: 5_000,
            over_budget_frames: 25, // 0.5% > 0.1% envelope!
            active_sessions: 80,
            abnormal_exits: 20,
            encounters_per_quantile: [40, 40, 40, 40],
            clears_per_quantile: [35, 35, 35, 35],
        };
        if evaluate_canary_watchdog(&base_m, &anomaly).is_ok() {
            return Err("Phase 4 Canary drill failed to detect frame-budget / exit-rate breach".into());
        }
        let drill_tabu = r_root.join("target").join("phase4_drill_tabu.bin");
        let _ = fs::remove_file(&drill_tabu);
        let _ = append_tabu_entry(&drill_tabu, [0xEE; 32], Some(&ref_qvm.causal_graph))?;
        let verified_drill = load_and_verify_tabu(&drill_tabu)?;
        let _ = fs::remove_file(&drill_tabu);
        if verified_drill.len() != 1 || t0.elapsed().as_secs_f64() > 1.0 {
            return Err("Phase 4 Canary rollback + T_tabu append exceeded 1.0s bound".into());
        }
        Ok(())
    }

    /// Verifies all live judges (`G1` Vulkan/SDF, `G2` MW/Causal Mutation, `G3` Skill/JSD, `G4` WL/Tabu/Oracle/Watchdog).
    fn verify_all_judges_readiness(r_root: &Path) -> Result<(), String> {
        // 1. G1 Offscreen Vulkan 1.4/1.3 Profiler + OKLab Contrast + Dual Outline + World SDF Topology & CSG/Elemental Reaction:
        let gpu = run_offscreen_vulkan_g1_profiler()?;
        println!(
            "  [G1-GPU] Vulkan 1.{} (validation_layer={}) cpu={:.3}ms gpu={:.3}ms OutlineInv={:.2}:1 Y_ratio={:.2}:1 C_hat_ratio={:.2}:1 dHue={:.1}deg",
            gpu.vulkan_minor,
            gpu.validation_layer_active,
            gpu.cpu_record_ms,
            gpu.gpu_frame_ms,
            gpu.outline_inversion_ratio,
            gpu.luminance_contrast,
            gpu.chroma_contrast,
            gpu.telegraph_hue_delta_deg
        );

        let mut world = SparseSdfWorld::new();
        for (cx, cz, seed) in [(0, 0, 1_u64), (1, 0, 2), (0, 1, 3), (1, 1, 4)] {
            world.generate_vertical_chunk(cx, cz, seed);
            let topo = world.analyze_chunk_topology(cx, cz);
            if topo.walkable_surface_voxels < 256
                || topo.sigma_z_q32 < Q32::from_int(2)
                || !topo.fully_3d_connected
            {
                return Err(format!("G1 chunk ({cx},{cz}) topology failed: {topo:?}"));
            }
        }
        let csg_delta = world.carve_sphere_csg(1, [8, 8, 8], 3);
        let field_delta = world.apply_element_stimulus(1, [8, 8, 8], 4, ElementStimulus::CryoFreeze);
        if csg_delta == 0 || field_delta == 0 {
            return Err("G1 SDF CSG or Elemental Field transition produced zero delta".into());
        }

        // 2. G2 Qualitative Trait Graph Mutation (`d_mech >= 0.10`) + Multiplicative Weights (`T=10,000, eta=0.05`):
        let pre_qvm = compile_qvm_source(
            r#"
    fn on_tick(self, target) {
        const_q32 hp = 10
        const_q32 sh = 10
        const_q32 ch = 5
        emit_damage hp, sh, ch
    }
    "#,
        )
        .map_err(|e| format!("{e:?}"))?;
        let post_qvm = compile_qvm_source(
            r#"
    fn on_tick(self, target) {
        const_q32 hp = 10
        const_q32 sh = 25
        const_q32 ch = 30
        read_pos p = self
        read_vel v = self
        const_q32 rad = 8
        const_q32 slow = 1
        emit_phantasia p, rad, slow
        emit_clash sh
        for i in 0..3 {
            emit_projectile p, v, hp
        endfor
    }
    "#,
        )
        .map_err(|e| format!("{e:?}"))?;
        let g_pre = &pre_qvm.causal_graph;
        let g_post = &post_qvm.causal_graph;
        if g_post.causal_edges.is_empty() || d_mech(g_pre, g_post) < 0.10 {
            return Err("G2 trait qualitative mutation causal distance < 0.10".into());
        }

        // Construct symmetric 6-archetype rock-paper-scissors circulant meta matrix (`W_{i,(i+1)%6} = 0.60`):
        let mut win_mat = vec![vec![0.5_f64; 6]; 6];
        for (i, row) in win_mat.iter_mut().enumerate().take(6) {
            let victim = (i + 1) % 6;
            let counter = (i + 5) % 6;
            row[victim] = 0.60;
            row[counter] = 0.40;
        }
        let mw = verify_g2_multiplicative_weights(&win_mat)?;
        println!(
            "  [G2-MW] N_eff={:.2} max_share={:.3} S_top={} spread={:.4}",
            mw.n_eff, mw.max_share, mw.top_count, mw.top_winrate_spread
        );

        // 3. G3 Rayon Fork Bot Skill Gradient + 1-HP No-Hit + Adaptive Boss JSD:
        let g3 = verify_g3_rayon_skill_gradient()?;
        println!(
            "  [G3-FORK] Clear(idle={:.2}, exp={:.2}) Perf({:.1} < {:.1} < {:.1}) 1HP_nohit={} JSD={:.3}bit",
            g3.clear_idle,
            g3.clear_expert,
            g3.perf_idle,
            g3.perf_novice,
            g3.perf_expert,
            g3.expert_1hp_no_hit_survived,
            g3.boss_jsd_bits
        );

        // 4. G4 WL Kernel + Tabu Chain + Exact i128 Oracle + Canary Watchdog Self-Tests:
        let tmp_tabu = r_root.join("target").join("self_test_tabu.bin");
        let _ = fs::remove_file(&tmp_tabu);
        let fault_graph = CausalGraphIR {
            nodes: vec![AnonMechRole::InputTrigger, AnonMechRole::ResonanceField],
            causal_edges: vec![CausalEdge {
                src: 0,
                dst: 1,
                edge_kind: 1,
            }],
        };
        let _ = append_tabu_entry(&tmp_tabu, [0xAA; 32], Some(&fault_graph))?;
        let tabu_chain = load_and_verify_tabu(&tmp_tabu)?;
        let _ = fs::remove_file(&tmp_tabu);
        if verify_g4_novelty_batch(&[([0xBB; 32], Some(fault_graph))], &[], &tabu_chain).is_ok() {
            return Err("G4 failed to reject near-duplicate Tabu causal graph".into());
        }

        let base_m = EpochManifestSummary {
            epoch_id: 0,
            season_id: 1,
            merkle_root: [0; 32],
            faucet_caps: [1_000_000; 8],
            base_move_speed_q32: Q32::from_int(10),
            expected_farm_ticks: 600,
            synth_clear_permille: 500,
            synth_exit_mean_permille: 20,
            synth_exit_std_permille: 10,
        };
        let good_tel = OracleEpochTelemetry {
            voluntary_return: verifier_abi::RationalCounter { sum_num: 75, sum_den: 100 },
            active_combat_explore: verifier_abi::RationalCounter { sum_num: 82, sum_den: 100 },
            build_experiment_breadth: verifier_abi::RationalCounter { sum_num: 65, sum_den: 100 },
            party_coop_completion: verifier_abi::RationalCounter { sum_num: 90, sum_den: 100 },
            measured_move_speed_q32: Q32::from_int(10),
            measured_expected_farm_ticks: 580,
            has_wireheading_gate_or_manipulation: false,
        };
        evaluate_anti_wireheading_oracle(&base_m, &good_tel, &base_m, &good_tel)?;
        let mut bad_speed = good_tel;
        bad_speed.measured_move_speed_q32 = Q32::from_int(9);
        if evaluate_anti_wireheading_oracle(&base_m, &good_tel, &base_m, &bad_speed).is_ok() {
            return Err("Oracle failed to reject movement speed degradation".into());
        }

        let healthy_canary = CanarySlidingTelemetry {
            panic_or_crash_count: 0,
            ledger_error_count: 0,
            total_frames: 10_000,
            over_budget_frames: 2,
            active_sessions: 100,
            abnormal_exits: 3,
            encounters_per_quantile: [40, 40, 40, 40],
            clears_per_quantile: [20, 21, 19, 20],
        };
        evaluate_canary_watchdog(&base_m, &healthy_canary)?;
        let mut crashing_canary = healthy_canary;
        crashing_canary.panic_or_crash_count = 1;
        if evaluate_canary_watchdog(&base_m, &crashing_canary).is_ok() {
            return Err("Canary Watchdog failed to trigger rollback on crash fault".into());
        }

        Ok(())
    }

    pub fn run_gate_phase(phase: u8) -> Result<(), String> {
        let r_root = Path::new(HOST_R_ROOT);
        println!("=== VERIFIER GATE: PHASE {phase} ===");

        // Step 1: Seal verification + Static Preflight + Dependency Isolation + Clippy
        let seal = verify_host_seal(r_root)?;
        let live_track = crate::seal::TrackMode::parse_file(&r_root.join(".track_mode"))?;
        let pf = preflight_scan_workspace(r_root)?;
        println!(
            "[1/6] Seal verified ({} files, live_track={}) & Static Preflight passed ({} .rs files, {} manifests).",
            seal.files.len(),
            live_track.as_str(),
            pf.scanned_rs_files,
            pf.scanned_manifests
        );

        let tree_out = run_cargo_checked(
            r_root,
            &["tree", "--locked", "-e", "normal", "-p", "client_bin", "-p", "server_bin"],
        )?;
        for line in tree_out.lines() {
            if line.contains("verifier v") || line.contains("ring v") || line.contains("aws-lc") {
                return Err(format!(
                    "Forbidden live-build dependency edge detected (`게임.md §4.2` / pure-Rust lock): {line}"
                ));
            }
        }
        let _ = run_cargo_checked(
            r_root,
            &["clippy", "--locked", "--workspace", "--all-targets", "--", "-D", "warnings"],
        )?;
        println!("[2/6] `cargo tree` isolation & `cargo clippy --locked --workspace --all-targets -- -D warnings` passed.");

        // Step 2: Typestate `compile_fail` doctests + Pure-Rust CDCL Bit-Blasting Formal Proofs
        let _ = run_cargo_checked(r_root, &["test", "--locked", "--doc", "-p", "verifier_abi"])?;
        let proofs = prove_all_formal_invariants()?;
        println!("[3/6] Typestate `compile_fail` doctests & CDCL Bit-Blasting Formal Proofs passed: {proofs:?}");

        // Step 3: 100,000-tick cross-seed determinism + Miri provenance/UB verification across ALL 6 K_sim crates!
        let sim_digest = verify_100k_ticks_determinism()?;
        let _ = run_cargo_checked(
            r_root,
            &[
                "+nightly-x86_64-pc-windows-gnu",
                "miri",
                "test",
                "--locked",
                "-p",
                "math_q32",
                "-p",
                "verifier_abi",
                "-p",
                "ecs_soa",
                "-p",
                "physics_sdf",
                "-p",
                "ledger_core",
                "-p",
                "qvm",
            ],
        )?;
        println!(
            "[4/6] 100,000-tick cross-seed K_sim bit-exact determinism ({}) & Miri UB=0 (all 6 K_sim crates) passed.",
            blake3::Hash::from_bytes(sim_digest).to_hex()
        );

        // Step 4: Adversarial 100,000-tx Group Commit conservation + crash & torn-tail truncation recovery
        verify_100k_adversarial_wal(r_root)?;
        println!("[5/6] Adversarial 100,000-tx Group Commit WAL conservation & torn-tail recovery (error=0) passed.");

        // Step 5: Security & Hook Self-Tests + Live G1~G4/Oracle/Watchdog Judge Readiness
        verify_security_and_hook_self_tests(r_root)?;
        verify_all_judges_readiness(r_root)?;
        println!("[6/6] Hook/Preflight attack self-tests (100% blocked) & G1~G4/Oracle/Watchdog judges verified.");

        // Dynamic vertical slice harnesses for Phases 1..=4 (`게임.md §11.2 ~ §11.5`):
        if phase >= 1 {
            verify_phase1_vertical_slice(r_root)?;
        }
        if phase >= 2 {
            verify_phase2_net_and_hotswap(r_root)?;
        }
        if phase >= 3 {
            verify_phase3_season_and_tournament(r_root)?;
        }
        if phase >= 4 {
            verify_phase4_autonomous_liveops(r_root)?;
        }

        println!("=== PHASE {phase} GATE PASSED (EXIT 0) ===");
        Ok(())
    }

}

pub mod hook_guard {
    use std::collections::BTreeMap;
    use std::fs::{self, OpenOptions};
    use std::io::{Read, Write};
    use std::path::{Component, Path, PathBuf};

    use serde_json::{Value, json};
    use crate::seal::{
        HOST_DEFI_ROOT, HOST_R_ROOT, MASTER_SPEC_REL, TrackMode, verify_host_seal,
    };

    fn emit_allow() {
        println!("{}", json!({ "decision": "allow" }));
    }

    fn emit_deny(reason: &str) {
        println!("{}", json!({ "decision": "deny", "reason": reason }));
    }

    fn strip_verbatim_and_norm(p: &Path) -> String {
        let s = p.to_string_lossy();
        let trimmed = s.strip_prefix(r"\\?\").unwrap_or(&s);
        trimmed
            .replace('/', "\\")
            .trim_end_matches('\\')
            .to_lowercase()
    }

    /// Resolves symlinks/junctions on existing ancestors and normalizes lexical `.` / `..` cleanly.
    /// Rejects any symlink leaf/intermediate segment under target creation paths.
    fn canonicalize_safe(raw: &str, base_dir: &Path) -> Result<PathBuf, String> {
        if raw.is_empty() || raw.contains('\0') {
            return Err("Empty or NUL-containing path".into());
        }
        let candidate = Path::new(raw);
        let joined = if candidate.is_absolute() {
            candidate.to_path_buf()
        } else {
            base_dir.join(candidate)
        };

        let mut lex = PathBuf::new();
        for comp in joined.components() {
            match comp {
                Component::Prefix(pr) => lex.push(pr.as_os_str()),
                Component::RootDir => lex.push(comp.as_os_str()),
                Component::CurDir => {}
                Component::ParentDir => {
                    if !lex.pop() {
                        return Err("Path traversal above root".into());
                    }
                }
                Component::Normal(seg) => lex.push(seg),
            }
        }

        let mut tail_segments: Vec<std::ffi::OsString> = Vec::new();
        let mut cur = lex.as_path();
        loop {
            if cur.exists() {
                let meta = fs::symlink_metadata(cur)
                    .map_err(|e| format!("symlink_metadata error on {}: {e}", cur.display()))?;
                if meta.file_type().is_symlink() {
                    return Err(format!("Symlink/reparse point rejected: {}", cur.display()));
                }
                let mut real = fs::canonicalize(cur)
                    .map_err(|e| format!("canonicalize error on {}: {e}", cur.display()))?;
                for seg in tail_segments.into_iter().rev() {
                    real.push(seg);
                }
                return Ok(real);
            }
            let Some(name) = cur.file_name() else {
                return Ok(lex);
            };
            tail_segments.push(name.to_os_string());
            let Some(parent) = cur.parent() else {
                return Ok(lex);
            };
            cur = parent;
        }
    }

    fn extract_forge_nonce(text: &str) -> Option<String> {
        let marker = "[FORGE_NONCE:";
        let start = text.find(marker)? + marker.len();
        let rest = &text[start..];
        let end = rest.find(']')?;
        let hex = &rest[..end];
        if (8..=128).contains(&hex.len()) && hex.chars().all(|c| c.is_ascii_hexdigit()) {
            Some(hex.to_lowercase())
        } else {
            None
        }
    }

    fn record_nonce_branch_ipc(
        r_root: &Path,
        defi_norm: &str,
        nonce: &str,
        workspace_zero: &str,
    ) -> Result<(), String> {
        let resolved = canonicalize_safe(workspace_zero, r_root)?;
        if !resolved.is_dir() {
            return Err("FORGE_NONCE branch workspace does not exist as a directory".into());
        }
        let ws_norm = strip_verbatim_and_norm(&resolved);
        if is_under(&ws_norm, defi_norm) || is_under(defi_norm, &ws_norm) {
            return Err(format!(
                "FORGE_NONCE branch workspace '{ws_norm}' must be strictly isolated outside '{defi_norm}'"
            ));
        }

        let state_dir = r_root.join("state");
        fs::create_dir_all(&state_dir).map_err(|e| format!("Failed creating state/: {e}"))?;
        let ipc_path = state_dir.join("ipc_branches.json");

        let mut map: BTreeMap<String, String> = if ipc_path.is_file() {
            let raw = fs::read_to_string(&ipc_path).unwrap_or_default();
            serde_json::from_str(&raw).unwrap_or_default()
        } else {
            BTreeMap::new()
        };

        let clean_ws = resolved
            .to_string_lossy()
            .strip_prefix(r"\\?\")
            .unwrap_or(&resolved.to_string_lossy())
            .to_string();
        map.insert(nonce.to_string(), clean_ws);

        let serialized =
            serde_json::to_string_pretty(&map).map_err(|e| format!("IPC serialize error: {e}"))?;
        let tmp_path = state_dir.join("ipc_branches.json.tmp");
        {
            let mut f = OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&tmp_path)
                .map_err(|e| format!("IPC open tmp error: {e}"))?;
            f.write_all(serialized.as_bytes())
                .map_err(|e| format!("IPC write tmp error: {e}"))?;
            f.sync_all().map_err(|e| format!("IPC sync error: {e}"))?;
        }
        fs::rename(&tmp_path, &ipc_path).map_err(|e| format!("IPC rename error: {e}"))?;
        Ok(())
    }

    fn collect_tool_paths(tool_input: &Value, out: &mut Vec<String>) {
        let Some(obj) = tool_input.as_object() else {
            return;
        };
        let path_keys = [
            "Path",
            "path",
            "TargetFile",
            "targetFile",
            "DirectoryPath",
            "directoryPath",
            "SearchPath",
            "searchPath",
            "SearchDirectory",
            "searchDirectory",
            "AbsolutePath",
            "absolutePath",
            "File",
            "file",
        ];
        for k in path_keys {
            if let Some(s) = obj.get(k).and_then(Value::as_str) {
                out.push(s.to_string());
            }
        }
        for arr_k in ["Paths", "paths", "Files", "files"] {
            if let Some(arr) = obj.get(arr_k).and_then(Value::as_array) {
                for item in arr {
                    if let Some(s) = item.as_str() {
                        out.push(s.to_string());
                    }
                }
            }
        }
    }

    fn is_under(child_norm: &str, root_norm: &str) -> bool {
        if child_norm == root_norm {
            return true;
        }
        let prefix = format!("{root_norm}\\");
        child_norm.starts_with(&prefix)
    }

    /// Note: `rel_slash` is already lowercased via `strip_verbatim_and_norm`, so all prefix/suffix
    /// patterns MUST be strictly lowercase (`CE-07` item 1).
    fn check_track_mode_write_allowed(rel_slash: &str, mode: TrackMode) -> Result<(), String> {
        let rel_lower = rel_slash.to_lowercase();
        if rel_lower.is_empty() || rel_lower.contains("..") {
            return Err("Invalid relative clone path".into());
        }
        let hard_banned_prefixes = [
            ".git/",
            ".cargo/",
            ".agents/",
            "workflows/",
            "verifier/",
            "kernel/verifier_abi/",
            "state/",
            "target/",
        ];
        for pfx in hard_banned_prefixes {
            if rel_lower.starts_with(pfx) || rel_lower == pfx.trim_end_matches('/') {
                return Err(format!("Write to protected prefix '{pfx}' denied: {rel_lower}"));
            }
        }
        let hard_banned_suffixes = [
            "게임.md",
            "agents.md",
            "gemini.md",
            "build.rs",
            "cargo.toml",
            "cargo.lock",
            "clippy.toml",
            ".clippy.toml",
            "seal.lock",
            ".track_mode",
            ".gitignore",
            "rust-toolchain.toml",
        ];
        for sfx in hard_banned_suffixes {
            if rel_lower == sfx || rel_lower.ends_with(&format!("/{sfx}")) {
                return Err(format!("Write to sealed config '{sfx}' denied: {rel_lower}"));
            }
        }

        match mode {
            TrackMode::Hotswap => {
                if rel_lower == "shaders/engine.slang"
                    || rel_lower.starts_with("gameplay/")
                    || rel_lower.starts_with("assets/")
                {
                    Ok(())
                } else {
                    Err(format!(
                        "Hotswap track forbids modifying '{rel_lower}' (allowed: shaders/engine.slang, gameplay/, assets/)"
                    ))
                }
            }
            TrackMode::Engine => {
                if rel_lower == "shaders/engine.slang"
                    || rel_lower.starts_with("gameplay/")
                    || rel_lower.starts_with("assets/")
                {
                    return Ok(());
                }
                if let Some(rest) = rel_lower.strip_prefix("kernel/")
                    && let Some((krate, after_krate)) = rest.split_once('/')
                    && krate != "verifier_abi"
                    && (after_krate == "src/lib.rs" || after_krate == "src/main.rs")
                {
                    return Ok(());
                }
                Err(format!(
                    "Engine track forbids modifying '{rel_lower}' (allowed: kernel/<non-abi>/src/{{lib,main}}.rs, shaders/engine.slang, gameplay/, assets/)"
                ))
            }
        }
    }

    fn evaluate_hook(payload: &Value) -> Result<(), String> {
        let r_root = Path::new(HOST_R_ROOT);
        let r_root_norm = strip_verbatim_and_norm(
            &fs::canonicalize(r_root).unwrap_or_else(|_| PathBuf::from(HOST_R_ROOT)),
        );
        let defi_norm = strip_verbatim_and_norm(
            &fs::canonicalize(HOST_DEFI_ROOT).unwrap_or_else(|_| PathBuf::from(HOST_DEFI_ROOT)),
        );
        let spec_norm = format!("{r_root_norm}\\{}", MASTER_SPEC_REL.to_lowercase());

        let ws0_raw = payload
            .get("workspacePaths")
            .and_then(Value::as_array)
            .and_then(|a| a.first())
            .and_then(Value::as_str);

        // 1. Record `(nonce -> workspacePaths[0])` IPC mapping ONLY if `workspacePaths[0]` is a valid isolated directory outside `c:\Defi`.
        if let Some(last_input) = payload.get("lastUserInput").and_then(Value::as_str)
            && let Some(nonce) = extract_forge_nonce(last_input)
            && let Some(ws0) = ws0_raw
        {
            record_nonce_branch_ipc(r_root, &defi_norm, &nonce, ws0)?;
        }

        let base_dir = if let Some(ws0) = ws0_raw {
            canonicalize_safe(ws0, r_root)?
        } else if let Some(cwd) = payload.get("cwd").and_then(Value::as_str) {
            canonicalize_safe(cwd, r_root)?
        } else {
            r_root.to_path_buf()
        };
        let base_norm = strip_verbatim_and_norm(&base_dir);
        let active_clone_norm = if base_norm != r_root_norm
            && !is_under(&base_norm, &defi_norm)
            && !is_under(&defi_norm, &base_norm)
        {
            Some(base_norm.clone())
        } else {
            None
        };

        let sealed = r_root.join("seal.lock").exists();

        let tool_name = payload
            .get("toolName")
            .or_else(|| payload.get("tool_name"))
            .and_then(Value::as_str)
            .ok_or_else(|| "Missing toolName in PreToolUse payload".to_string())?;
        let tool_input = payload
            .get("toolInput")
            .or_else(|| payload.get("tool_input"))
            .unwrap_or(&Value::Null);

        // 2. Pathless `finish` tool
        if tool_name == "finish" {
            return Ok(());
        }

        // 3. `run_workflow` policing (`게임.md §10.2`)
        if tool_name == "run_workflow" {
            if let Some(obj) = tool_input.as_object() {
                for inline_key in ["Script", "script"] {
                    if let Some(v) = obj.get(inline_key)
                        && !v.is_null()
                        && v.as_str() != Some("")
                    {
                        return Err("Inline run_workflow(Script=...) is strictly forbidden".into());
                    }
                }
                let sp = obj
                    .get("ScriptPath")
                    .or_else(|| obj.get("scriptPath"))
                    .and_then(Value::as_str)
                    .ok_or_else(|| "run_workflow requires ScriptPath".to_string())?;
                let resolved_sp = canonicalize_safe(sp, &base_dir)?;
                let sp_norm = strip_verbatim_and_norm(&resolved_sp);
                let expected_wf = format!("{r_root_norm}\\workflows\\forge_loop.py");
                if sp_norm != expected_wf {
                    return Err(format!(
                        "Unauthorized workflow ScriptPath '{sp_norm}' (only '{expected_wf}' allowed)"
                    ));
                }
                if let Some(wf_cwd) = obj.get("Cwd").or_else(|| obj.get("cwd")).and_then(Value::as_str) {
                    let rcwd = strip_verbatim_and_norm(&canonicalize_safe(wf_cwd, &base_dir)?);
                    if rcwd != r_root_norm {
                        return Err(format!("run_workflow Cwd must equal host R_root ('{r_root_norm}')"));
                    }
                }
                if sealed {
                    let _ = verify_host_seal(r_root)?;
                }
                return Ok(());
            }
            return Err("Invalid run_workflow toolInput".into());
        }

        // 4. `run_command` policing (`게임.md §10.2`)
        if tool_name == "run_command" {
            let cmd = tool_input
                .get("CommandLine")
                .or_else(|| tool_input.get("commandLine"))
                .or_else(|| tool_input.get("command"))
                .and_then(Value::as_str)
                .ok_or_else(|| "Missing CommandLine in run_command".to_string())?;

            if sealed {
                if base_norm != r_root_norm {
                    return Err(format!(
                        "Sealed run_command requires host R_root workspace ('{r_root_norm}'), got '{base_norm}'"
                    ));
                }
                for cwd_k in ["Cwd", "cwd", "WorkingDirectory", "workingDirectory"] {
                    if let Some(raw_c) = tool_input.get(cwd_k).and_then(Value::as_str) {
                        let c_norm = strip_verbatim_and_norm(&canonicalize_safe(raw_c, &base_dir)?);
                        if c_norm != r_root_norm {
                            return Err(format!("Sealed run_command Cwd must equal '{r_root_norm}'"));
                        }
                    }
                }
                let forbidden_meta = [
                    ';', '|', '&', '>', '<', '`', '$', '(', ')', '{', '}', '\r', '\n', '"', '\'',
                ];
                if cmd.chars().any(|c| forbidden_meta.contains(&c)) {
                    return Err("Shell metacharacters forbidden in sealed mode".into());
                }
                let collapsed: Vec<&str> = cmd.split_whitespace().collect();
                let allowed = matches!(
                    collapsed.as_slice(),
                    [
                        "cargo",
                        "run",
                        "--locked",
                        "--release",
                        "-p",
                        "verifier",
                        "--",
                        "gate",
                        "--phase",
                        "0" | "1" | "2" | "3" | "4"
                    ]
                );
                if !allowed {
                    return Err(
                        "Sealed mode allows ONLY `cargo run --locked --release -p verifier -- gate --phase <0..4>`"
                            .into(),
                    );
                }
                let _ = verify_host_seal(r_root)?;
                return Ok(());
            }
            return Ok(());
        }

        // 5. Read / Write file & directory tools
        let is_read_tool = matches!(
            tool_name,
            "read_file"
                | "view_file"
                | "view_file_outline"
                | "view_code_item"
                | "grep_search"
                | "find_by_name"
                | "list_dir"
        );
        let is_write_tool = matches!(
            tool_name,
            "write_file"
                | "write_to_file"
                | "edit_file"
                | "replace_file_content"
                | "multi_replace_file_content"
                | "delete_file"
        );

        if !is_read_tool && !is_write_tool {
            if sealed {
                return Err(format!("Default-Deny in sealed mode for tool '{tool_name}'"));
            }
            return Ok(());
        }

        let mut raw_paths = Vec::new();
        collect_tool_paths(tool_input, &mut raw_paths);
        if raw_paths.is_empty() {
            return Err(format!("Tool '{tool_name}' missing target path argument"));
        }

        let state_host_norm = format!("{r_root_norm}\\state");

        for rp in &raw_paths {
            let resolved = canonicalize_safe(rp, &base_dir)?;
            let p_norm = strip_verbatim_and_norm(&resolved);

            // Master specification `c:\Defi\game\게임.md` is strictly read-only at ALL times!
            if is_write_tool && p_norm == spec_norm {
                return Err(format!("Master specification '{p_norm}' is frozen/read-only"));
            }

            // Strict `c:\Defi` isolation (`게임.md §서문`):
            // Anything equal to `c:\Defi` or under `c:\Defi\...` outside `R_root` is 100% blocked!
            if is_under(&p_norm, &defi_norm) {
                if !is_under(&p_norm, &r_root_norm) {
                    return Err(format!(
                        "Strict isolation violation: access to '{p_norm}' outside R_root is forbidden"
                    ));
                }
            } else if sealed {
                let in_clone = active_clone_norm
                    .as_deref()
                    .is_some_and(|cl| is_under(&p_norm, cl));
                if !in_clone {
                    return Err(format!(
                        "Sealed mode blocks external host path '{p_norm}'"
                    ));
                }
            }

            // `state/` is host-process-only; direct agent writes are ALWAYS forbidden (`게임.md §7.1`).
            if is_write_tool && is_under(&p_norm, &state_host_norm) {
                return Err("Agent direct writes to state/ are forbidden".into());
            }

            if is_write_tool && sealed {
                if is_under(&p_norm, &r_root_norm) {
                    return Err(format!(
                        "Sealed mode blocks 100% of direct writes to host R_root ('{p_norm}')"
                    ));
                }
                let Some(ref clone_norm) = active_clone_norm else {
                    return Err("Sealed mode requires an isolated branch clone workspace for writes".into());
                };
                if !is_under(&p_norm, clone_norm) {
                    return Err(format!(
                        "Write path '{p_norm}' is outside assigned branch clone '{clone_norm}'"
                    ));
                }
                let rel_part = p_norm
                    .strip_prefix(&format!("{clone_norm}\\"))
                    .unwrap_or("")
                    .replace('\\', "/");
                let mode = TrackMode::parse_file(&r_root.join(".track_mode"))?;
                check_track_mode_write_allowed(&rel_part, mode)?;
            }
        }

        Ok(())
    }

    pub fn run_hook_guard_stdin() {
        let mut input = String::new();
        if std::io::stdin().read_to_string(&mut input).is_err() {
            emit_deny("Failed reading PreToolUse stdin");
            return;
        }
        let parsed: Value = match serde_json::from_str(&input) {
            Ok(v) => v,
            Err(_) => {
                emit_deny("Malformed PreToolUse JSON");
                return;
            }
        };
        match evaluate_hook(&parsed) {
            Ok(()) => emit_allow(),
            Err(reason) => emit_deny(&reason),
        }
    }

}

use std::fs;
use std::path::Path;
use std::process::ExitCode;

use crate::gates::{run_gate_phase, run_offscreen_vulkan_g1_profiler_inproc};
use crate::preflight::preflight_scan_workspace;
use crate::seal::{
    HOST_R_ROOT, TrackMode, seal_init_once, verify_clone_against_host_seal, verify_host_seal,
};

fn run_cli(args: &[String]) -> Result<(), String> {
    let r_root = Path::new(HOST_R_ROOT);

    match args.first().map(String::as_str) {
        Some("seal-init") => {
            let pf = preflight_scan_workspace(r_root)?;
            let live_track = TrackMode::parse_file(&r_root.join(".track_mode"))?;
            let manifest = seal_init_once(r_root)?;
            println!(
                "SEAL_INIT OK: sealed {} files (live_track={}, scanned {} .rs files).",
                manifest.files.len(),
                live_track.as_str(),
                pf.scanned_rs_files
            );
            Ok(())
        }
        Some("internal-gpu-g1") => {
            let metrics = run_offscreen_vulkan_g1_profiler_inproc()?;
            let s = serde_json::to_string(&metrics).map_err(|e| format!("{e}"))?;
            println!("{s}");
            Ok(())
        }
        Some("preflight") => {
            let host_seal = verify_host_seal(r_root)?;
            let pf_host = preflight_scan_workspace(r_root)?;
            if args.len() >= 3 && args[1] == "--clone" {
                let clone_path = Path::new(&args[2]);
                let canon_host = fs::canonicalize(r_root)
                    .map_err(|e| format!("canonicalize host failed: {e}"))?;
                let canon_clone = fs::canonicalize(clone_path)
                    .map_err(|e| format!("canonicalize clone failed: {e}"))?;
                if canon_host == canon_clone {
                    return Err("Clone path matches host R_root!".into());
                }
                verify_clone_against_host_seal(&host_seal, &canon_clone)?;
                let pf_clone = preflight_scan_workspace(&canon_clone)?;
                println!(
                    "PREFLIGHT OK (host={} rs, clone={} rs).",
                    pf_host.scanned_rs_files, pf_clone.scanned_rs_files
                );
            } else {
                println!("PREFLIGHT OK (host={} rs).", pf_host.scanned_rs_files);
            }
            Ok(())
        }
        Some("gate") => {
            if args.len() != 3 || args[1] != "--phase" {
                return Err("Usage: verifier gate --phase <0..4>".into());
            }
            let phase: u8 = args[2]
                .parse()
                .map_err(|_| format!("Invalid phase '{}' (expected 0..=4)", args[2]))?;
            if phase > 4 {
                return Err(format!("Phase {phase} out of range (expected 0..=4)"));
            }
            run_gate_phase(phase)
        }
        _ => Err(
            "Usage:\n  verifier seal-init\n  verifier preflight [--clone <path>]\n  verifier gate --phase <0..4>\n  verifier hook-guard"
                .into(),
        ),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("hook-guard") {
        hook_guard::run_hook_guard_stdin();
        return ExitCode::SUCCESS;
    }
    match run_cli(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("VERIFIER REJECTED: {err}");
            ExitCode::from(1)
        }
    }
}
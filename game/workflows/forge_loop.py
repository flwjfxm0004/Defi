# workflows/forge_loop.py — Antigravity Separation-of-Powers Closed-Loop Orchestrator
import json
import os
import secrets
import shutil
import subprocess

DEFI_ROOT = os.path.normcase(os.path.realpath(r"c:\Defi"))
R_ROOT = os.path.normcase(os.path.realpath(r"c:\Defi\game"))
IPC_PATH = os.path.join(R_ROOT, "state", "ipc_branches.json")
CARGO_BIN = os.path.join(os.path.expanduser("~"), ".cargo", "bin", "cargo.exe")
if not os.path.exists(CARGO_BIN):
    CARGO_BIN = "cargo"

VERIFIER_EXE = os.path.join(R_ROOT, "target", "release", "verifier.exe")

BANNED_SNIPPETS = (
    "#[allow(",
    "#![allow(",
    "#[expect(",
    "#![expect(",
    "cfg_attr",
    "#[path",
    "proc-macro",
    "extern crate std",
    "include_bytes!(",
    "include_str!(",
    "include!(",
    "todo!(",
    "unimplemented!(",
    "std::collections::HashMap",
    "std::collections::HashSet",
    "use std::collections::{HashMap",
    "use std::collections::{HashSet",
)

HARD_BANNED_BASENAMES = (
    "게임.md",
    "agents.md",
    "cargo.toml",
    "cargo.lock",
    "build.rs",
    "clippy.toml",
    ".clippy.toml",
    "seal.lock",
    ".track_mode",
    ".gitignore",
    "rust-toolchain.toml",
)


def norm_path(p):
    return os.path.normcase(os.path.realpath(p))


def is_under(child_norm, parent_norm):
    if child_norm == parent_norm:
        return True
    prefix = parent_norm.rstrip("\\/") + os.sep
    return child_norm.startswith(prefix)


def is_safe_ephemeral_clone(p):
    if not p or not os.path.isdir(p) or os.path.islink(p):
        return False
    norm_p = norm_path(p)
    if is_under(norm_p, DEFI_ROOT) or is_under(DEFI_ROOT, norm_p):
        return False
    return os.path.exists(os.path.join(p, ".git"))


def reset_repo_clean(repo_dir):
    subprocess.run(["git", "reset", "--hard", "HEAD"], cwd=repo_dir, capture_output=True)
    subprocess.run(["git", "clean", "-fd"], cwd=repo_dir, capture_output=True)


def run_host_verifier(subcmd_args):
    env = os.environ.copy()
    cargo_dir = os.path.dirname(CARGO_BIN)
    if cargo_dir:
        env["PATH"] = cargo_dir + os.pathsep + env.get("PATH", "")
    proc = subprocess.run(
        [CARGO_BIN, "run", "--locked", "--release", "-p", "verifier", "--"] + subcmd_args,
        cwd=R_ROOT,
        env=env,
        capture_output=True,
        text=True,
    )
    return proc.returncode, proc.stdout + "\n" + proc.stderr


def static_clone_preflight(clone_root, track_mode):
    if not is_safe_ephemeral_clone(clone_root):
        return False, "clone_path failed ephemeral boundary check", []
    diff_proc = subprocess.run(
        ["git", "status", "--porcelain", "-uall"],
        cwd=clone_root,
        capture_output=True,
        text=True,
    )
    if diff_proc.returncode != 0:
        return False, "git status failed in clone", []

    changed_rel_paths = []
    untracked_rel_paths = []
    for raw_line in diff_proc.stdout.splitlines():
        if len(raw_line) < 4:
            continue
        status_code = raw_line[:2]
        entry = raw_line[3:].strip()
        if " -> " in entry:
            parts = entry.split(" -> ", 1)
            p0 = parts[0].strip('"')
            p1 = parts[1].strip('"')
            changed_rel_paths.extend([p0, p1])
        else:
            p0 = entry.strip('"')
            changed_rel_paths.append(p0)
            if status_code == "??":
                untracked_rel_paths.append(p0)

    for rel in changed_rel_paths:
        rel_slash = rel.replace("\\", "/")
        rel_lower = rel_slash.lower()
        base_lower = rel_lower.rsplit("/", 1)[-1]
        full = os.path.join(clone_root, rel)
        if os.path.islink(full):
            return False, f"symlink forbidden: {rel_slash}", []
        if base_lower in HARD_BANNED_BASENAMES:
            return False, f"protected governance/config file touch forbidden: {rel_slash}", []
        if track_mode == "hotswap":
            ok = rel_slash == "shaders/engine.slang" or rel_slash.startswith(("gameplay/", "assets/"))
        else:
            parts = rel_slash.split("/")
            is_single_kernel_rs = (
                len(parts) == 4
                and parts[0] == "kernel"
                and parts[1] != "verifier_abi"
                and parts[2] == "src"
                and parts[3] in ("lib.rs", "main.rs")
            )
            ok = (
                rel_slash == "shaders/engine.slang"
                or rel_slash.startswith(("gameplay/", "assets/"))
                or is_single_kernel_rs
            )
        if not ok:
            return False, f"path outside .track_mode ({track_mode}) allowlist: {rel_slash}", []
        if os.path.isfile(full) and rel_lower.endswith(".rs"):
            with open(full, "r", encoding="utf-8", errors="replace") as fh:
                src = fh.read()
            for tok in BANNED_SNIPPETS:
                if tok in src:
                    return False, f"banned token '{tok}' in {rel_slash}", []
    return True, "ok", untracked_rel_paths


head_check = subprocess.run(
    ["git", "rev-parse", "--verify", "HEAD"],
    cwd=R_ROOT,
    capture_output=True,
    text=True,
)
if head_check.returncode != 0:
    raise RuntimeError("Host R_root repository has no initial HEAD commit; commit baseline before running forge_loop.")

target_phase = int(args.get("phase", 4)) if isinstance(args, dict) else 4
epoch_goal = args.get("goal", "Evolve next epoch candidate") if isinstance(args, dict) else "Evolve next epoch candidate"
max_rounds = int(args.get("max_rounds", 3)) if isinstance(args, dict) else 3

track_file = os.path.join(R_ROOT, ".track_mode")
track_mode = "hotswap"
if os.path.isfile(track_file):
    with open(track_file, "r", encoding="utf-8") as tf:
        first = tf.readline().strip().split(":")[-1].strip()
        if first in ("engine", "hotswap"):
            track_mode = first

phase(f"1. Architect Spec ({epoch_goal})")
arch_spec = await agent(
    f"Read c:/Defi/game/게임.md (§1~§11). Design a candidate specification for phase={target_phase}, track={track_mode}, goal={epoch_goal}.",
    schema={
        "type": "object",
        "properties": {
            "epoch_id": {"type": "string"},
            "summary": {"type": "string"},
            "target_files": {"type": "array", "items": {"type": "string"}},
        },
        "required": ["epoch_id", "summary", "target_files"],
    },
    role="Game Architect",
    type_name="game-architect",
    workspace="inherit",
    model="inherit",
)
log(f"Architect proposed epoch {arch_spec['epoch_id']}: {arch_spec['summary']}")

builder = agent(
    "Follow c:/Defi/game/게임.md (§1~§11) strictly inside your assigned branch clone.",
    role="Game Builder",
    type_name="game-builder",
    workspace="branch",
    model="inherit",
)

promoted = False
feedback = ""
last_clone_path = None
for attempt in range(1, max_rounds + 1):
    phase(f"2.{attempt} Isolated Branch Build & Triple-Lock Preflight")
    nonce = secrets.token_hex(16)
    build_res = await builder(
        f"[FORGE_NONCE:{nonce}]\nTrack mode: {track_mode}\nArchitect Spec: {json.dumps(arch_spec, ensure_ascii=False)}\nPrior feedback: {feedback}\nImplement cleanly and call finish.",
        schema={
            "type": "object",
            "properties": {
                "modified_files": {"type": "array", "items": {"type": "string"}},
                "notes": {"type": "string"},
            },
            "required": ["modified_files", "notes"],
        },
    )

    if not os.path.isfile(IPC_PATH):
        raise RuntimeError("Missing state/ipc_branches.json; nonce mapping not recorded by hook_guard")
    with open(IPC_PATH, "r", encoding="utf-8") as ipc_f:
        ipc_map = json.load(ipc_f)
    if nonce not in ipc_map:
        raise RuntimeError(f"Nonce {nonce} missing in state/ipc_branches.json")
    clone_path = ipc_map[nonce]
    last_clone_path = clone_path

    if not is_safe_ephemeral_clone(clone_path):
        raise RuntimeError(f"Unsafe or non-ephemeral clone_path rejected before preflight/rmtree: {clone_path}")

    if not os.path.isfile(VERIFIER_EXE):
        raise RuntimeError("Missing sealed host target/release/verifier.exe")
    pre_proc = subprocess.run(
        [VERIFIER_EXE, "preflight", "--clone", clone_path],
        cwd=R_ROOT,
        capture_output=True,
        text=True,
    )
    if pre_proc.returncode != 0:
        shutil.rmtree(clone_path, ignore_errors=True)
        raise RuntimeError(f"Pre-cargo seal/clone preflight rejected:\n{pre_proc.stdout}\n{pre_proc.stderr}")

    ok_clone, reason, untracked_rels = static_clone_preflight(clone_path, track_mode)
    if not ok_clone:
        log(f"Quarantining tainted clone {clone_path}: {reason}")
        shutil.rmtree(clone_path, ignore_errors=True)
        raise RuntimeError(f"Clone preflight rejected: {reason}")

    if untracked_rels:
        intent_proc = subprocess.run(
            ["git", "add", "-N", "--"] + untracked_rels,
            cwd=clone_path,
            capture_output=True,
            text=True,
        )
        if intent_proc.returncode != 0:
            reset_repo_clean(clone_path)
            feedback = f"git add -N failed in clone: {intent_proc.stderr}"
            continue

    diff_bytes = subprocess.run(
        ["git", "diff", "--binary", "HEAD"],
        cwd=clone_path,
        capture_output=True,
    ).stdout

    if diff_bytes.strip():
        apply_proc = subprocess.run(
            ["git", "apply", "--index", "-"],
            cwd=R_ROOT,
            input=diff_bytes,
            capture_output=True,
        )
        if apply_proc.returncode != 0:
            reset_repo_clean(R_ROOT)
            reset_repo_clean(clone_path)
            feedback = f"git apply failed: {apply_proc.stderr.decode('utf-8', 'replace')}"
            continue

    phase(f"3.{attempt} Verifier Gate & Adversarial Prosecutor")
    gate_code, gate_out = run_host_verifier(["gate", "--phase", str(target_phase)])
    pros_verdict = await agent(
        f"Audit the applied candidate for epoch {arch_spec['epoch_id']} against c:/Defi/game/게임.md (§1~§11).\nVerifier exit={gate_code}.\nReturn counterexamples ([] only if 0 defects).",
        schema={
            "type": "object",
            "properties": {
                "counterexamples": {"type": "array", "items": {"type": "string"}},
            },
            "required": ["counterexamples"],
        },
        role="Game Prosecutor",
        type_name="game-prosecutor",
        workspace="inherit",
        model="inherit",
    )

    if gate_code == 0 and len(pros_verdict["counterexamples"]) == 0:
        commit_add = subprocess.run(["git", "add", "-A"], cwd=R_ROOT, capture_output=True, text=True)
        commit_proc = subprocess.run(
            ["git", "commit", "-m", f"Promote epoch {arch_spec['epoch_id']}"],
            cwd=R_ROOT,
            capture_output=True,
            text=True,
        )
        if commit_add.returncode != 0 or commit_proc.returncode != 0:
            reset_repo_clean(R_ROOT)
            reset_repo_clean(clone_path)
            raise RuntimeError(f"Failed to commit promoted epoch {arch_spec['epoch_id']}: {commit_proc.stderr}")
        promoted = True
        log(f"Epoch {arch_spec['epoch_id']} PASSED verifier (0), Prosecutor (0 counterexamples), and committed to HEAD.")
        break

    reset_repo_clean(R_ROOT)
    reset_repo_clean(clone_path)
    feedback = f"Gate exit={gate_code}\n{gate_out[-4000:]}\nProsecutor={pros_verdict['counterexamples']}"
    log(f"Attempt {attempt} rejected; rolled back host R_root and branch clone cleanly.")

del builder
if last_clone_path and is_safe_ephemeral_clone(last_clone_path):
    shutil.rmtree(last_clone_path, ignore_errors=True)
if not promoted:
    raise RuntimeError(f"Candidate rejected after {max_rounds} rounds: {feedback[-1500:]}")

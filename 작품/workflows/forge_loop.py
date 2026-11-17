import json
import os
import subprocess
import uuid

_RAW_ROOT = os.path.abspath(WORKSPACES[0]) if WORKSPACES else os.path.abspath(os.getcwd())
ROOT = _RAW_ROOT if os.path.isfile(os.path.join(_RAW_ROOT, "애니.md")) else os.path.join(_RAW_ROOT, "작품")
VERIFIER_EXE = os.path.join(ROOT, "target", "release", "verifier.exe")
IPC_PATH = os.path.join(ROOT, "state", "ipc_branches.json")


def run_local(args_list, cwd=ROOT):
    proc = subprocess.run(
        args_list,
        cwd=cwd,
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
    )
    return proc.returncode, proc.stdout + proc.stderr


def assert_head_exists():
    code, out = run_local(["git", "rev-parse", "--verify", "HEAD"])
    if code != 0:
        raise RuntimeError(f"Initial HEAD commit missing in {ROOT}: {out}")


def is_outside_defi_tree(candidate_path):
    norm_cand = os.path.normcase(os.path.realpath(candidate_path))
    defi_parent = os.path.normcase(os.path.realpath(os.path.dirname(ROOT)))
    return not (norm_cand == defi_parent or norm_cand.startswith(defi_parent + os.sep))


target_phase = int(args.get("phase", 4)) if isinstance(args, dict) else 4
assert_head_exists()

phase(f"Phase {target_phase} Architect")
blueprint = await agent(
    f"Read 애니.md and design the concrete implementation plan for Phase {target_phase}.",
    schema={
        "type": "object",
        "properties": {
            "summary": {"type": "string"},
            "targets": {"type": "array", "items": {"type": "string"}},
        },
        "required": ["summary", "targets"],
    },
    role="Anime Architect",
    type_name="anime-architect",
    workspace="inherit",
    model="inherit",
)
log("Blueprint:", blueprint["summary"])

phase(f"Phase {target_phase} Isolated Build")
nonce = f"NONCE-{uuid.uuid4().hex}"
build_res = await agent(
    f"[{nonce}] Execute blueprint inside isolated branch per 애니.md:\n{json.dumps(blueprint, ensure_ascii=False)}",
    schema={
        "type": "object",
        "properties": {"changed_files": {"type": "array", "items": {"type": "string"}}},
        "required": ["changed_files"],
    },
    role="Anime Builder",
    type_name="anime-builder",
    workspace="branch",
    model="inherit",
)

if not os.path.exists(IPC_PATH):
    raise RuntimeError("Missing state/ipc_branches.json nonce mapping from hook-guard")
with open(IPC_PATH, "r", encoding="utf-8") as f:
    ipc_map = json.load(f)
if nonce not in ipc_map:
    raise RuntimeError(f"Nonce {nonce} was not registered by PreToolUse hook-guard")

clone_path = ipc_map[nonce]
if not is_outside_defi_tree(clone_path):
    raise RuntimeError(f"Clone path {clone_path} is not isolated outside parent tree")

# Preflight static scan BEFORE cargo runs on clone
code, pre_out = run_local([VERIFIER_EXE, "preflight", "--target", clone_path])
if code != 0:
    raise RuntimeError(f"Preflight rejected isolated clone {clone_path}:\n{pre_out}")

phase(f"Phase {target_phase} Prosecutor & Verifier Gate")
audit = await agent(
    f"Audit Phase {target_phase} changes against 애니.md §1~§5: {json.dumps(build_res, ensure_ascii=False)}",
    schema={
        "type": "object",
        "properties": {
            "counterexamples": {"type": "array", "items": {"type": "string"}},
        },
        "required": ["counterexamples"],
    },
    role="Anime Prosecutor",
    type_name="anime-prosecutor",
    workspace="inherit",
    model="inherit",
)
if audit["counterexamples"]:
    raise RuntimeError(f"Prosecutor rejected candidate: {audit['counterexamples']}")

# Harvest clean diff into host workspace only after preflight + prosecutor pass
code, diff_patch = run_local(["git", "diff", "--binary", "HEAD"], cwd=clone_path)
if code != 0:
    raise RuntimeError(f"Failed to extract git diff from clone: {diff_patch}")

if diff_patch.strip():
    apply_proc = subprocess.run(
        ["git", "apply", "--index", "-"],
        cwd=ROOT,
        input=diff_patch,
        text=True,
        encoding="utf-8",
        capture_output=True,
    )
    if apply_proc.returncode != 0:
        raise RuntimeError(f"git apply failed: {apply_proc.stderr}")

gate_out = await call_tool(
    "run_command",
    CommandLine=f"cargo run --locked --release -p verifier -- gate --phase {target_phase}",
    WaitMsBeforeAsync=10000,
    toolSummary=f"Verifier gate phase {target_phase}",
)
log(gate_out)

code, commit_out = run_local(["git", "commit", "--allow-empty", "-m", f"Promote verified Phase {target_phase}"])
if code != 0:
    raise RuntimeError(f"Commit failed: {commit_out}")
log(commit_out)

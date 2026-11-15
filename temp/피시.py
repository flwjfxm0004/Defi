#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""피시.py — 새 PC 환경 복원 · 디스크/스크래치 클린 · 전원 설정 · 포터블 Git 푸시 통합 도구

Defi 폴더 하나만 있으면 외부 Python/Git/pip 설치 없이 즉시 동작합니다 (Python 3.12 표준 라이브러리 전용).

사용법 (c:\\Defi 루트 기준):
  .\\python.exe -B -X utf8 temp\\피시.py                 # 기본: 도구 검증 + 절전 해제 + 환경 복원(sourcify/build)
  .\\python.exe -B -X utf8 temp\\피시.py verify          # 내장 실행기(python/foundry/duckdb/git) 이식성 점검 및 DLL 보강
  .\\python.exe -B -X utf8 temp\\피시.py restore         # 새 PC 캐시 복원 (--sourcify, --build, --skip-build, --tables)
  .\\python.exe -B -X utf8 temp\\피시.py clean           # Defi 스크래치 + Temp + 브라우저/업데이트 캐시 + 드라이버 + WinSxS 정리 (--fast, --defi-only)
  .\\python.exe -B -X utf8 temp\\피시.py power           # 장시간 작업용 Windows AC 절전/모니터/최대절전 타임아웃 0 설정
  .\\python.exe -B -X utf8 temp\\피시.py push            # 내장 포터블 Git(tools\\git) 기반 커밋 및 원격 푸시
"""
from __future__ import annotations

import sys

sys.dont_write_bytecode = True

import argparse
import concurrent.futures
import ctypes
import json
import os
import re
import shutil
import sqlite3
import subprocess
import threading
import time
import urllib.parse
import urllib.request
import winreg
from pathlib import Path
from typing import Optional, Sequence

for _stream in (sys.stdout, sys.stderr):
    try:
        _stream.reconfigure(encoding="utf-8", errors="replace")  # type: ignore[attr-defined]
    except Exception:
        pass

SELF_PATH: Path = Path(__file__).resolve()
DEFI_DIR: Path = (
    SELF_PATH.parent
    if (SELF_PATH.parent / "Defi_구현.py").is_file()
    else SELF_PATH.parent.parent
)

PYTHON_EXE: Path = DEFI_DIR / "python.exe" if (DEFI_DIR / "python.exe").is_file() else Path(sys.executable).resolve()
TOOLS_DIR: Path = DEFI_DIR / "tools"
FOUNDRY_DIR: Path = TOOLS_DIR / "foundry"
DUCKDB_DIR: Path = TOOLS_DIR / "duckdb"
GIT_ROOT_DIR: Path = TOOLS_DIR / "git"
PY_STDLIB_DIR: Path = TOOLS_DIR / "python"

CACHE_DIR: Path = DEFI_DIR / "audit_artifacts" / "_cache"
SOURCIFY_DIR: Path = CACHE_DIR / "sourcify_v2"
MANIFEST_PATH: Path = CACHE_DIR / "sourcify_v2_manifest.json"
GAME_DIR: Path = DEFI_DIR / "game"

SOURCIFY_ENDPOINT: str = "https://export.sourcify.dev"

_S3_CONTENTS_RE = re.compile(r"<Contents>(.*?)</Contents>", re.DOTALL)
_S3_KEY_RE = re.compile(r"<Key>([^<]+)</Key>")
_S3_SIZE_RE = re.compile(r"<Size>\s*(\d+)\s*</Size>")
_S3_NEXT_TOKEN_RE = re.compile(r"<NextContinuationToken>([^<]+)</NextContinuationToken>")
_S3_NEXT_MARKER_RE = re.compile(r"<NextMarker>([^<]+)</NextMarker>")

_PROTECTED_DRIVES = ("d:", "h:")


def log(msg: str) -> None:
    print(msg, flush=True)


def is_admin() -> bool:
    try:
        return bool(ctypes.windll.shell32.IsUserAnAdmin())
    except Exception:
        return False


def elevate_if_needed(extra_args: Optional[Sequence[str]] = None) -> None:
    if is_admin():
        return
    argv = list(extra_args) if extra_args is not None else sys.argv[1:]
    params = subprocess.list2cmdline(["-B", "-X", "utf8", str(SELF_PATH), *argv])
    log("[권한] 관리자 권한(UAC)으로 재실행합니다...")
    rc = ctypes.windll.shell32.ShellExecuteW(None, "runas", str(PYTHON_EXE), params, str(DEFI_DIR), 1)
    sys.exit(0 if rc > 32 else 1)


def disk_free_gb(drive: str = "C:\\") -> float:
    try:
        free_bytes = ctypes.c_ulonglong(0)
        ctypes.windll.kernel32.GetDiskFreeSpaceExW(
            ctypes.c_wchar_p(drive), None, None, ctypes.byref(free_bytes)
        )
        return free_bytes.value / (1024 ** 3)
    except Exception:
        return 0.0


def run_cmd(
    cmd: Sequence[str],
    *,
    cwd: Optional[Path] = None,
    env: Optional[dict[str, str]] = None,
    timeout: float = 120.0,
    encoding: str = "utf-8",
) -> tuple[int, str]:
    try:
        cp = subprocess.run(
            list(cmd),
            cwd=str(cwd) if cwd else None,
            env=env,
            capture_output=True,
            text=True,
            encoding=encoding,
            errors="replace",
            timeout=timeout,
            check=False,
        )
        return cp.returncode, ((cp.stdout or "") + "\n" + (cp.stderr or "")).strip()
    except Exception as exc:
        return 1, str(exc)


# ─────────────────────────────────────────────────────────────────────────────
# 1. 내장 툴체인/DLL 무결성 검증 및 Git 로컬 설정 동기화 (`verify`)
# ─────────────────────────────────────────────────────────────────────────────

def locate_git_env() -> tuple[Optional[Path], dict[str, str]]:
    env = dict(os.environ)
    extra_paths: list[str] = []
    for sub in (GIT_ROOT_DIR / "cmd", GIT_ROOT_DIR / "ucrt64" / "bin", GIT_ROOT_DIR / "usr" / "bin"):
        if sub.is_dir():
            extra_paths.append(str(sub))
    if extra_paths:
        env["PATH"] = os.pathsep.join([*extra_paths, env.get("PATH", "")])

    bundled_git = GIT_ROOT_DIR / "cmd" / "git.exe"
    if bundled_git.is_file():
        return bundled_git, env
    which_git = shutil.which("git", path=env.get("PATH"))
    if which_git:
        return Path(which_git), env
    for cand in (
        Path(r"C:\Program Files\Git\cmd\git.exe"),
        Path(r"C:\Program Files\Git\bin\git.exe"),
    ):
        if cand.is_file():
            return cand, env
    return None, env


def verify_toolchain() -> bool:
    log(f"[verify] Defi 루트: {DEFI_DIR}")
    CACHE_DIR.mkdir(parents=True, exist_ok=True)

    # 새 PC에 VC++ Redistributable이 없더라도 python.exe 및 forge.exe/cast.exe가 자립 동작하도록 vcruntime140 DLL 자동 보강
    if PY_STDLIB_DIR.is_dir():
        target_dirs: list[Path] = [d for d in (DEFI_DIR, FOUNDRY_DIR) if d.is_dir()]
        sys32_dir = Path(os.environ.get("SystemRoot", r"C:\Windows")) / "System32"
        if is_admin() and sys32_dir.is_dir():
            target_dirs.append(sys32_dir)
        for dst_dir in target_dirs:
            for dll_name in ("vcruntime140.dll", "vcruntime140_1.dll"):
                src_dll = PY_STDLIB_DIR / dll_name
                dst_dll = dst_dir / dll_name
                if src_dll.is_file() and not dst_dll.is_file():
                    try:
                        shutil.copy2(src_dll, dst_dll)
                        rel_lbl = str(dst_dll.relative_to(DEFI_DIR)) if dst_dir != sys32_dir else str(dst_dll)
                        log(f"  [보강] {rel_lbl} 복사 완료 (새 PC VC++ 런타임 자립)")
                    except OSError:
                        pass

    checks: list[tuple[str, Path, list[str]]] = [
        ("python", PYTHON_EXE, ["-B", "-V"]),
        ("forge", FOUNDRY_DIR / "forge.exe", ["--version"]),
        ("cast", FOUNDRY_DIR / "cast.exe", ["--version"]),
        ("solc", FOUNDRY_DIR / "solc.exe", ["--version"]),
        ("duckdb", DUCKDB_DIR / "duckdb.exe", ["--version"]),
    ]
    git_exe, git_env = locate_git_env()
    if git_exe is not None:
        checks.append(("git", git_exe, ["--version"]))

    all_ok = True
    for label, exe_path, ver_args in checks:
        if not exe_path.is_file():
            log(f"  [실패] {label}: 파일 없음 ({exe_path})")
            all_ok = False
            continue
        rc, out = run_cmd([str(exe_path), *ver_args], cwd=DEFI_DIR, env=git_env, timeout=20.0)
        first_line = next((ln.strip() for ln in out.splitlines() if ln.strip()), "")
        if rc == 0:
            log(f"  [정상] {label:<8}: {first_line}")
        else:
            log(f"  [실패] {label:<8}: rc={rc} ({first_line[:120]})")
            all_ok = False

    # 한글 파일명 깨짐 방지 및 줄바꿈 보존 설정
    if git_exe is not None and (DEFI_DIR / ".git").is_dir():
        run_cmd([str(git_exe), "config", "--local", "core.quotepath", "false"], cwd=DEFI_DIR, env=git_env)
        run_cmd([str(git_exe), "config", "--local", "core.autocrlf", "false"], cwd=DEFI_DIR, env=git_env)
        gitignore_p = DEFI_DIR / ".gitignore"
        exclude_p = DEFI_DIR / ".git" / "info" / "exclude"
        if gitignore_p.is_file():
            try:
                exclude_p.parent.mkdir(parents=True, exist_ok=True)
                exclude_p.write_text(gitignore_p.read_text(encoding="utf-8"), encoding="utf-8")
            except OSError:
                pass
    return all_ok


# ─────────────────────────────────────────────────────────────────────────────
# 2. 새 PC 캐시·빌드 복원 (`restore`)
# ─────────────────────────────────────────────────────────────────────────────

def http_get_bytes(url: str, timeout: float = 60.0) -> bytes:
    req = urllib.request.Request(url, headers={"User-Agent": "DefiSetup/2.0"})
    with urllib.request.urlopen(req, timeout=timeout) as resp:  # noqa: S310
        return resp.read()


def list_remote_sourcify_objects() -> list[tuple[str, int]]:
    out: list[tuple[str, int]] = []
    marker: Optional[str] = None
    while True:
        url = f"{SOURCIFY_ENDPOINT}/?prefix=v2/"
        if marker:
            url += "&marker=" + urllib.parse.quote(marker, safe="")
        xml = http_get_bytes(url, timeout=120.0).decode("utf-8", errors="replace")
        page_keys: list[str] = []
        for blk in _S3_CONTENTS_RE.findall(xml):
            km = _S3_KEY_RE.search(blk)
            if not km:
                continue
            key = km.group(1).strip()
            page_keys.append(key)
            if key.startswith("v2/") and key.endswith(".parquet"):
                sm = _S3_SIZE_RE.search(blk)
                out.append((key, int(sm.group(1)) if sm else 0))
        if "<IsTruncated>true</IsTruncated>" not in xml or not page_keys:
            break
        nt = _S3_NEXT_TOKEN_RE.search(xml)
        nm = _S3_NEXT_MARKER_RE.search(xml)
        prev = marker
        if nt:
            marker = nt.group(1).strip()
        elif nm and nm.group(1).strip():
            marker = nm.group(1).strip()
        else:
            marker = page_keys[-1]
        if not marker or marker == prev:
            break
    return out


def _download_sourcify_one(key: str, size: int) -> tuple[str, str, int, Optional[str]]:
    """Returns (status, table_name, size, error_msg): status in {'skipped', 'done', 'failed'}."""
    rel = key[len("v2/"):]
    dest = SOURCIFY_DIR / rel
    tbl = rel.split("/", 1)[0]
    if dest.is_file() and dest.stat().st_size == size:
        return "skipped", tbl, size, None

    dest.parent.mkdir(parents=True, exist_ok=True)
    url = f"{SOURCIFY_ENDPOINT}/{urllib.parse.quote(key, safe='/')}"
    tmp = dest.with_suffix(dest.suffix + f".{os.getpid()}_{threading.get_ident()}.part")
    last_err = ""
    for attempt in range(1, 4):
        try:
            req = urllib.request.Request(url, headers={"User-Agent": "DefiSetup/2.0"})
            with urllib.request.urlopen(req, timeout=300.0) as resp, open(tmp, "wb") as fh:  # noqa: S310
                shutil.copyfileobj(resp, fh, length=1024 * 1024)
            if tmp.stat().st_size != size:
                raise IOError(f"size mismatch ({tmp.stat().st_size} != {size})")
            tmp.replace(dest)
            return "done", tbl, size, None
        except Exception as exc:
            last_err = str(exc)
            try:
                tmp.unlink(missing_ok=True)
            except OSError:
                pass
            if attempt < 3:
                time.sleep(1.5 * attempt)
    return "failed", tbl, 0, last_err


def sync_sourcify(tables: Optional[set[str]] = None, workers: int = 6) -> None:
    log("[sourcify] 원격 Parquet 목록 조회 중...")
    remote = list_remote_sourcify_objects()
    if tables:
        remote = [r for r in remote if len(r[0].split("/")) > 1 and r[0].split("/")[1] in tables]
    total_gb = sum(s for _, s in remote) / 1e9
    log(f"[sourcify] 대상 {len(remote)}개 파일 ({total_gb:.2f} GB), 병렬 워커={max(1, workers)}")

    done = skipped = failed = 0
    bytes_ok = 0
    failed_keys: list[str] = []
    table_counts: dict[str, int] = {}

    with concurrent.futures.ThreadPoolExecutor(max_workers=max(1, int(workers))) as pool:
        futs = {pool.submit(_download_sourcify_one, k, sz): k for k, sz in remote}
        for idx, fut in enumerate(concurrent.futures.as_completed(futs), 1):
            key = futs[fut]
            status, tbl, sz_ok, err = fut.result()
            if status == "skipped":
                skipped += 1
                bytes_ok += sz_ok
                table_counts[tbl] = table_counts.get(tbl, 0) + 1
            elif status == "done":
                done += 1
                bytes_ok += sz_ok
                table_counts[tbl] = table_counts.get(tbl, 0) + 1
                if done % 50 == 0:
                    log(f"[sourcify] 신규 다운로드 {done}개 진행 중 (전체 확인 {idx}/{len(remote)})...")
            else:
                failed += 1
                failed_keys.append(key)
                log(f"[sourcify] 실패 {key}: {err}")

    manifest = {
        "failed_keys": sorted(failed_keys),
        "ok": failed == 0,
        "tables": dict(sorted(table_counts.items())),
        "total_remote_bytes": sum(s for _, s in remote),
        "total_remote_files": len(remote),
        "updated_at": time.time(),
        "verified_bytes": bytes_ok,
        "verified_files": len(remote) - failed,
    }
    CACHE_DIR.mkdir(parents=True, exist_ok=True)
    MANIFEST_PATH.write_text(json.dumps(manifest, indent=2), encoding="utf-8")
    log(f"[sourcify] 완료: 신규={done}, 건너뜀={skipped}, 실패={failed}")


def find_cargo() -> Optional[Path]:
    found = shutil.which("cargo")
    if found:
        return Path(found)
    cand = Path.home() / ".rustup" / "toolchains"
    if cand.is_dir():
        for d in sorted(cand.iterdir(), reverse=True):
            exe = d / "bin" / "cargo.exe"
            if exe.is_file():
                return exe
    return None


def build_game() -> None:
    if not (GAME_DIR / "Cargo.toml").is_file():
        log("[build] game/Cargo.toml 없음 — 스킵")
        return
    if not (GAME_DIR / ".git").exists():
        git_exe, git_env = locate_git_env()
        if git_exe is not None:
            run_cmd([str(git_exe), "init", "-q"], cwd=GAME_DIR, env=git_env)
            run_cmd([str(git_exe), "config", "--local", "user.name", "flwjfxm0004"], cwd=GAME_DIR, env=git_env)
            run_cmd([str(git_exe), "config", "--local", "user.email", "flwjfxm0004@users.noreply.github.com"], cwd=GAME_DIR, env=git_env)
            run_cmd([str(git_exe), "config", "--local", "core.autocrlf", "false"], cwd=GAME_DIR, env=git_env)
            run_cmd([str(git_exe), "add", "-A"], cwd=GAME_DIR, env=git_env)
            run_cmd([str(git_exe), "commit", "-q", "-m", "init"], cwd=GAME_DIR, env=git_env)
    cargo = find_cargo()
    if cargo is None:
        log("[build] cargo를 찾지 못했습니다. 새 PC에서 game 빌드가 필요하면 rustup 설치 후 재실행하세요:")
        log("  winget install Rustlang.Rustup")
        return
    log(f"[build] {cargo} build (cwd={GAME_DIR})")
    cargo_dir = cargo.parent
    toolchain = cargo_dir.parent
    extra: list[Path] = [
        cargo_dir,
        Path.home() / ".cargo" / "bin",
        toolchain / "lib" / "rustlib" / "x86_64-pc-windows-gnullvm" / "bin",
    ]
    if toolchain.parent.is_dir():
        for tc_dir in sorted(toolchain.parent.glob("*windows-gnu*"), reverse=True):
            gnu_self = tc_dir / "lib" / "rustlib" / "x86_64-pc-windows-gnu" / "bin" / "self-contained"
            if gnu_self.is_dir():
                extra.append(gnu_self)
                break
    env = dict(os.environ)
    env["PATH"] = os.pathsep.join([*(str(p) for p in extra if p.is_dir()), env.get("PATH", "")])
    proc = subprocess.run([str(cargo), "build"], cwd=str(GAME_DIR), env=env, check=False)
    log(f"[build] 종료 코드={proc.returncode}")


# ─────────────────────────────────────────────────────────────────────────────
# 3. 장시간 작업용 표준 Windows 전원/절전 해제 (`power`)
# ─────────────────────────────────────────────────────────────────────────────

def configure_power_never_sleep() -> None:
    log("[power] Windows AC 절전·최대절전·모니터·디스크 타임아웃 0 설정 중...")
    for sub_guid, setting_guid in (
        ("SUB_SLEEP", "STANDBYIDLE"),
        ("SUB_SLEEP", "HIBERNATEIDLE"),
        ("SUB_VIDEO", "VIDEOIDLE"),
        ("SUB_DISK", "DISKIDLE"),
    ):
        run_cmd(["powercfg.exe", "/SETACVALUEINDEX", "SCHEME_CURRENT", sub_guid, setting_guid, "0"], timeout=15.0)
    run_cmd(["powercfg.exe", "/SETACTIVE", "SCHEME_CURRENT"], timeout=15.0)
    try:
        with winreg.CreateKeyEx(winreg.HKEY_CURRENT_USER, r"Control Panel\Desktop", 0, winreg.KEY_SET_VALUE) as key:
            winreg.SetValueEx(key, "ScreenSaveActive", 0, winreg.REG_SZ, "0")
    except OSError:
        pass
    log("[power] 절전 및 화면보호기 해제 완료")


# ─────────────────────────────────────────────────────────────────────────────
# 4. 디스크 및 Defi 스크래치 통합 클린 (`clean`)
# ─────────────────────────────────────────────────────────────────────────────

def _is_protected_path(target: Path) -> bool:
    s = str(target).lower()
    return any(s.startswith(drv) for drv in _PROTECTED_DRIVES)


def _is_defi_engine_running() -> bool:
    ps_cmd = (
        "Get-CimInstance Win32_Process -Filter \"Name LIKE 'python%'\" -ErrorAction SilentlyContinue | "
        "Where-Object { $_.CommandLine -like '*Defi_구현.py*' } | Select-Object -ExpandProperty ProcessId"
    )
    rc, out = run_cmd(["powershell.exe", "-NoProfile", "-Command", ps_cmd], timeout=20.0)
    if rc != 0 or not out.strip():
        return False
    my_pid = str(os.getpid())
    return any(tok.strip().isdigit() and tok.strip() != my_pid for tok in out.splitlines())


def _safe_rmtree(path: Path) -> bool:
    if not path.exists() or _is_protected_path(path):
        return False
    try:
        shutil.rmtree(path, ignore_errors=False)
        return True
    except OSError:
        # 대용량/깊은 중첩 폴더는 robocopy /MIR 폴백으로 고속 정리
        empty_dir = Path(os.environ.get("TEMP", r"C:\Windows\Temp")) / f"_clean_empty_{os.getpid()}"
        try:
            empty_dir.mkdir(parents=True, exist_ok=True)
            run_cmd(
                ["robocopy.exe", str(empty_dir), str(path), "/MIR", "/R:0", "/W:0", "/MT:8", "/NFL", "/NDL", "/NJH", "/XJ"],
                timeout=120.0,
            )
            shutil.rmtree(path, ignore_errors=True)
            return not path.exists()
        finally:
            shutil.rmtree(empty_dir, ignore_errors=True)


def _iter_user_roots() -> list[Path]:
    candidates = [Path.home(), Path(r"C:\Users\Administrator")]
    seen: set[str] = set()
    out: list[Path] = []
    for c in candidates:
        key = str(c).lower()
        if key not in seen and c.is_dir():
            seen.add(key)
            out.append(c)
    return out


def clean_defi_scratch() -> None:
    log("--- [1/7] Defi 스크래치·캐시 정리 (본체/산출물 정본 보호) ---")
    if _is_defi_engine_running():
        log("  [스킵] Defi_구현.py 실행 중 감지 — 안전을 위해 Defi 섹션 생략")
        return

    removed_dirs = 0
    removed_files = 0
    art_root = DEFI_DIR / "audit_artifacts"
    if art_root.is_dir():
        for extra_cache in (
            art_root / "_shared_cache",
            CACHE_DIR / "_cache",
            CACHE_DIR / "_restore_tmp",
            CACHE_DIR / "_subproc_tmp",
            CACHE_DIR / "_subproc_home",
            CACHE_DIR / "duckdb_tmp",
            CACHE_DIR / "duckdb_home",
            CACHE_DIR / "pylibs",
        ):
            if extra_cache.is_dir() and _safe_rmtree(extra_cache):
                removed_dirs += 1
        for selftest_ns in art_root.glob("_selftest_ns_*"):
            if selftest_ns.is_dir() and _safe_rmtree(selftest_ns):
                removed_dirs += 1

        for pd in art_root.iterdir():
            if not pd.is_dir() or pd.name.startswith("_"):
                continue
            for sub in ("cache", "traces", "dumps", "out"):
                if _safe_rmtree(pd / sub):
                    removed_dirs += 1
            poc_dir = pd / "poc"
            for sub in ("cache", "out", "broadcast"):
                if _safe_rmtree(poc_dir / sub):
                    removed_dirs += 1
            rep_dir = pd / "replicate"
            if rep_dir.is_dir():
                for sub_d in sorted(rep_dir.rglob("*"), key=lambda p: len(p.parts), reverse=True):
                    if sub_d.is_dir() and sub_d.name in ("cache", "traces", "dumps", "out"):
                        if _safe_rmtree(sub_d):
                            removed_dirs += 1
                    elif sub_d.is_file() and (
                        sub_d.name.startswith("dump") or "_dump" in sub_d.name or sub_d.suffix == ".pkl"
                    ):
                        try:
                            sub_d.unlink()
                            removed_files += 1
                        except OSError:
                            pass
            for scan_dir in (pd, poc_dir):
                if scan_dir.is_dir():
                    for f in scan_dir.iterdir():
                        if f.is_file() and "dump" in f.name and f.suffix == ".json":
                            try:
                                f.unlink()
                                removed_files += 1
                            except OSError:
                                pass

    # Defi 트리 내부 __pycache__ 및 루트 임시 덤프 정리
    for pyc_dir in list(DEFI_DIR.rglob("__pycache__")):
        if ".git" not in pyc_dir.parts and _safe_rmtree(pyc_dir):
            removed_dirs += 1
    for f in DEFI_DIR.iterdir():
        if f.is_file() and (
            (f.name.startswith("dump_") and f.suffix == ".json")
            or f.name == "inv_full_out.json"
            or f.suffix in (".pyc", ".pyo")
        ):
            try:
                f.unlink()
                removed_files += 1
            except OSError:
                pass

    log(f"  폴더 {removed_dirs}개, 덤프/바이트코드 파일 {removed_files}개 정리 완료 (여유 {disk_free_gb():.2f} GB)")


def clean_system(fast: bool = False) -> None:
    user_roots = _iter_user_roots()

    log("--- [2/7] Temp 디렉터리 비우기 ---")
    temp_dirs: list[Path] = [Path(r"C:\Windows\Temp")]
    env_tmp = os.environ.get("TEMP")
    if env_tmp:
        temp_dirs.append(Path(env_tmp))
    for u in user_roots:
        temp_dirs.append(u / "AppData" / "Local" / "Temp")
        local_ad = u / "AppData" / "Local"
        if local_ad.is_dir():
            for tw in local_ad.glob("tw-*-*.tmp"):
                _safe_rmtree(tw)

    seen_t: set[str] = set()
    t_ok = t_locked = 0
    for td in temp_dirs:
        tk = str(td).lower()
        if tk in seen_t or not td.is_dir() or _is_protected_path(td):
            continue
        seen_t.add(tk)
        for item in td.iterdir():
            try:
                if item.is_dir() and not item.is_symlink():
                    shutil.rmtree(item)
                else:
                    item.unlink()
                t_ok += 1
            except OSError:
                t_locked += 1
    log(f"  Temp 항목 삭제={t_ok}, 사용중 유지={t_locked} (여유 {disk_free_gb():.2f} GB)")

    log("--- [3/7] 브라우저 캐시만 정리 (로그인·쿠키 유지) ---")
    cache_names = {"cache", "code cache", "gpucache", "shadercache", "cacheddata"}
    b_cnt = 0
    for u in user_roots:
        for rel in ("Google", "Naver", r"Microsoft\Edge"):
            base = u / "AppData" / "Local" / rel
            if not base.is_dir():
                continue
            for d in list(base.rglob("*")):
                if d.is_dir() and d.name.lower() in cache_names:
                    if _safe_rmtree(d):
                        b_cnt += 1
    log(f"  브라우저 캐시 폴더 {b_cnt}개 삭제 (여유 {disk_free_gb():.2f} GB)")

    log("--- [4/7] 업데이트/인스톨러 캐시 · 휴지통 · Prefetch 정리 ---")
    if is_admin():
        for svc in ("wuauserv", "bits"):
            run_cmd(["sc.exe", "stop", svc], timeout=15.0)
        for upd_dir in (
            Path(r"C:\Windows\SoftwareDistribution\Download"),
            Path(r"C:\Windows\SoftwareDistribution\DeliveryOptimization"),
            Path(r"C:\Windows\Installer\Temp"),
            Path(r"C:\$WinREAgent\Scratch"),
            Path(r"C:\Windows\Prefetch"),
        ):
            if upd_dir.is_dir():
                for item in upd_dir.iterdir():
                    try:
                        if item.is_dir() and not item.is_symlink():
                            shutil.rmtree(item)
                        else:
                            item.unlink()
                    except OSError:
                        pass
        patch_cache = Path(r"C:\Windows\Installer\$PatchCache$")
        if patch_cache.is_dir():
            _safe_rmtree(patch_cache)
        win_logs = Path(r"C:\Windows\Logs")
        if win_logs.is_dir():
            for lf in win_logs.rglob("*"):
                if lf.is_file():
                    try:
                        lf.unlink()
                    except OSError:
                        pass
        for svc in ("bits", "wuauserv"):
            run_cmd(["sc.exe", "start", svc], timeout=15.0)

    try:
        # SHERB_NOCONFIRMATION(1) | SHERB_NOPROGRESSUI(2) | SHERB_NOSOUND(4) = 7
        ctypes.windll.shell32.SHEmptyRecycleBinW(None, None, 7)
    except Exception:
        pass
    log(f"  업데이트 캐시 및 휴지통 정리 완료 (여유 {disk_free_gb():.2f} GB)")

    log("--- [5/7] 구버전 EdgeCore 및 Package Cache 정리 ---")
    if is_admin():
        for pkg_c in (
            Path(r"C:\ProgramData\Package Cache"),
            Path(r"C:\ProgramData\ChocolateyHttpCache"),
            Path(r"C:\Program Files\NVIDIA Corporation\Installer2"),
        ):
            if pkg_c.is_dir():
                _safe_rmtree(pkg_c)
        pd_root = Path(r"C:\ProgramData")
        if pd_root.is_dir():
            for ic in pd_root.glob("Intel Package Cache*"):
                if ic.is_dir():
                    _safe_rmtree(ic)
        edge_core = Path(r"C:\Program Files (x86)\Microsoft\EdgeCore")
        if edge_core.is_dir():
            _, edge_procs = run_cmd(
                [
                    "powershell.exe",
                    "-NoProfile",
                    "-Command",
                    "Get-CimInstance Win32_Process -Filter \"Name='msedge.exe'\" -ErrorAction SilentlyContinue | Select-Object -ExpandProperty ExecutablePath",
                ],
                timeout=15.0,
            )
            active_paths = edge_procs.lower()
            ver_dirs = sorted(
                (d for d in edge_core.iterdir() if d.is_dir() and d.name[0].isdigit()),
                key=lambda p: [int(x) if x.isdigit() else 0 for x in p.name.split(".")],
            )
            for vd in ver_dirs[:-1]:
                if str(vd).lower() not in active_paths:
                    _safe_rmtree(vd)
    log(f"  Package Cache 점검 완료 (여유 {disk_free_gb():.2f} GB)")

    if fast or not is_admin():
        log("--- [6~7/7] --fast 모드 또는 비관리자 실행으로 드라이버/WinSxS 정리 스킵 ---")
        return

    log("--- [6/7] 미사용 OEM 드라이버 정리 ---")
    rc_drv, drv_out = run_cmd(
        [
            "powershell.exe",
            "-NoProfile",
            "-Command",
            "Get-CimInstance Win32_PnPSignedDriver -ErrorAction SilentlyContinue | Select-Object -ExpandProperty InfName -Unique",
        ],
        timeout=45.0,
    )
    in_use = {ln.strip().lower() for ln in drv_out.splitlines() if ln.strip().lower().startswith("oem")}
    inf_dir = Path(r"C:\Windows\INF")
    if rc_drv == 0 and in_use and inf_dir.is_dir():
        removed_drv = kept_drv = 0
        for inf_p in inf_dir.glob("oem*.inf"):
            if inf_p.name.lower() in in_use:
                kept_drv += 1
            else:
                rc_u, _ = run_cmd(["pnputil.exe", "/delete-driver", inf_p.name, "/uninstall"], timeout=30.0)
                if rc_u == 0:
                    removed_drv += 1
                else:
                    kept_drv += 1
        log(f"  미사용 드라이버 삭제={removed_drv}, 유지={kept_drv}")
    else:
        log("  드라이버 사용 목록 확인 불가 — 안전 스킵")

    log("--- [7/7] WinSxS 컴포넌트 정리 (DISM) ---")
    rc_dism, _ = run_cmd(
        ["DISM.exe", "/Online", "/Cleanup-Image", "/StartComponentCleanup"],
        timeout=900.0,
        encoding="cp949",
    )
    log(f"  DISM 종료 코드={rc_dism} (여유 {disk_free_gb():.2f} GB)")


# ─────────────────────────────────────────────────────────────────────────────
# 5. 내장 포터블 Git 원클릭 푸시 (`push`)
# ─────────────────────────────────────────────────────────────────────────────

def _flush_relay_snapshot_before_push() -> None:
    """PC방 이동 푸시 직전 engine_cache.sqlite의 완료 타겟·Sourcify 커서 진행도를 _relay_state.json으로 안전 동기화."""
    art_root = DEFI_DIR / "audit_artifacts"
    db_file = CACHE_DIR / "engine_cache.sqlite"
    seed_idx_src = CACHE_DIR / "mode_b_seed_index_v1.json"
    if seed_idx_src.is_file():
        try:
            art_root.mkdir(parents=True, exist_ok=True)
            shutil.copy2(seed_idx_src, art_root / "_relay_seed_index.json")
        except OSError:
            pass
    if not db_file.is_file():
        return
    try:
        conn = sqlite3.connect(str(db_file), timeout=10.0)
        conn.row_factory = sqlite3.Row
        try:
            done_rows = conn.execute(
                "SELECT slug, priority, mode, chain, tvl_usd, sort_rank, addresses_json, principle_summary, verdict, net_krw "
                "FROM audit_queue WHERE status='done' ORDER BY slug ASC"
            ).fetchall()
            dyn_rows = conn.execute(
                "SELECT q.slug, q.priority, q.mode, q.chain, q.tvl_usd, q.sort_rank, q.addresses_json, q.principle_summary, "
                "COALESCE(m.meta_json, '{}') AS meta_json "
                "FROM audit_queue q LEFT JOIN proto_meta m ON q.slug = m.slug "
                "WHERE q.status != 'done' AND ("
                "q.slug LIKE 'modeb-match-%' "
                "OR q.slug LIKE 'modeb-clone-%' "
                "OR COALESCE(m.meta_json, '') LIKE '%mode_b_zero_seed_recovered%' "
                "OR COALESCE(m.meta_json, '') LIKE '%agent_addresses_recovered%'"
                ") ORDER BY q.slug ASC"
            ).fetchall()
            pv_rows = conn.execute(
                "SELECT chain_id, vault_addr, slug, net_krw, fully_exhausted FROM proven_vaults ORDER BY chain_id ASC, vault_addr ASC"
            ).fetchall()
            cur_row = conn.execute("SELECT meta_val FROM engine_meta WHERE meta_key='mode_b_sourcify_cursor_v1'").fetchone()
        finally:
            conn.close()

        cur_doc: dict = {}
        if cur_row and cur_row["meta_val"]:
            parsed_c = json.loads(cur_row["meta_val"])
            if isinstance(parsed_c, dict):
                cur_doc = dict(parsed_c)
                cur_doc["sig_inflight"] = {}
                cur_doc["code_inflight"] = {}

        if not done_rows and not dyn_rows and not pv_rows and not cur_doc:
            return

        relay_doc = {
            "schema_version": 1,
            "mode_b_sourcify_cursor_v1": cur_doc,
            "done_items": [
                {
                    "slug": str(r["slug"]),
                    "priority": int(r["priority"]),
                    "mode": str(r["mode"] or "A"),
                    "chain": str(r["chain"]),
                    "tvl_usd": float(r["tvl_usd"] or 0.0),
                    "sort_rank": float(r["sort_rank"] or 0.0),
                    "addresses": json.loads(r["addresses_json"] or "[]"),
                    "principle_summary": str(r["principle_summary"] or ""),
                    "verdict": str(r["verdict"] or ""),
                    "net_krw": float(r["net_krw"] or 0.0),
                }
                for r in done_rows
            ],
            "pending_dynamic_items": [
                {
                    "slug": str(r["slug"]),
                    "priority": int(r["priority"]),
                    "mode": str(r["mode"] or "B"),
                    "chain": str(r["chain"]),
                    "tvl_usd": float(r["tvl_usd"] or 0.0),
                    "sort_rank": float(r["sort_rank"] or 0.0),
                    "addresses": json.loads(r["addresses_json"] or "[]"),
                    "principle_summary": str(r["principle_summary"] or ""),
                    "meta": json.loads(r["meta_json"] or "{}"),
                }
                for r in dyn_rows
            ],
            "proven_vaults": [
                {
                    "chain_id": int(r["chain_id"]),
                    "vault_addr": str(r["vault_addr"]),
                    "slug": str(r["slug"]),
                    "net_krw": float(r["net_krw"] or 0.0),
                    "fully_exhausted": bool(r["fully_exhausted"]),
                }
                for r in pv_rows
            ],
        }
        art_root.mkdir(parents=True, exist_ok=True)
        relay_p = art_root / "_relay_state.json"
        tmp_p = art_root / f"_relay_state.{os.getpid()}.tmp"
        tmp_p.write_text(json.dumps(relay_doc, ensure_ascii=False, separators=(",", ":")), encoding="utf-8")
        tmp_p.replace(relay_p)
        log(f"[push] 이어달리기 진행도 동기화 완료 (완료 {len(done_rows)}건, 커서 phase={cur_doc.get('phase', 'init')})")
    except Exception as exc:
        log(f"[push] 이어달리기 스냅샷 참고 경고: {exc}")


def git_push(message: str = "") -> int:
    git_exe, git_env = locate_git_env()
    if git_exe is None:
        log("[push] ERROR: git.exe를 찾을 수 없습니다.")
        return 1

    # 스테이징 전 루트/하위 __pycache__ 제거 및 PC방 이어달리기 스냅샷 동기화
    for pyc_dir in list(DEFI_DIR.rglob("__pycache__")):
        if ".git" not in pyc_dir.parts:
            _safe_rmtree(pyc_dir)
    _flush_relay_snapshot_before_push()
    gitignore_p = DEFI_DIR / ".gitignore"
    exclude_p = DEFI_DIR / ".git" / "info" / "exclude"
    if gitignore_p.is_file() and (DEFI_DIR / ".git").is_dir():
        try:
            exclude_p.parent.mkdir(parents=True, exist_ok=True)
            exclude_p.write_text(gitignore_p.read_text(encoding="utf-8"), encoding="utf-8")
        except OSError:
            pass

    if not sys.stdin or not sys.stdin.isatty():
        git_env["GIT_TERMINAL_PROMPT"] = "0"
    run_cmd([str(git_exe), "config", "--local", "credential.helper", "manager"], cwd=DEFI_DIR, env=git_env)
    run_cmd([str(git_exe), "config", "--local", "credential.useHttpPath", "true"], cwd=DEFI_DIR, env=git_env)
    _, rem_url = run_cmd([str(git_exe), "remote", "get-url", "origin"], cwd=DEFI_DIR, env=git_env)
    m_gh = re.match(r"https://github\.com/([^/\s]+)/", rem_url.strip())
    if m_gh:
        run_cmd(
            [str(git_exe), "config", "--local", "credential.https://github.com.username", m_gh.group(1)],
            cwd=DEFI_DIR,
            env=git_env,
        )
    run_cmd([str(git_exe), "config", "--local", "core.quotepath", "false"], cwd=DEFI_DIR, env=git_env)
    run_cmd([str(git_exe), "config", "--local", "core.autocrlf", "false"], cwd=DEFI_DIR, env=git_env)
    run_cmd([str(git_exe), "config", "--local", "http.lowSpeedLimit", "1000"], cwd=DEFI_DIR, env=git_env)
    run_cmd([str(git_exe), "config", "--local", "http.lowSpeedTime", "15"], cwd=DEFI_DIR, env=git_env)

    rc_n, name_out = run_cmd([str(git_exe), "config", "--get", "user.name"], cwd=DEFI_DIR, env=git_env)
    if rc_n != 0 or not name_out.strip():
        run_cmd([str(git_exe), "config", "--local", "user.name", "flwjfxm0004"], cwd=DEFI_DIR, env=git_env)
    rc_e, email_out = run_cmd([str(git_exe), "config", "--get", "user.email"], cwd=DEFI_DIR, env=git_env)
    if rc_e != 0 or not email_out.strip():
        run_cmd(
            [str(git_exe), "config", "--local", "user.email", "flwjfxm0004@users.noreply.github.com"],
            cwd=DEFI_DIR,
            env=git_env,
        )

    _, br_out = run_cmd([str(git_exe), "rev-parse", "--abbrev-ref", "HEAD"], cwd=DEFI_DIR, env=git_env)
    branch = br_out.strip() if br_out.strip() and br_out.strip() != "HEAD" else "master"

    # 중첩 game/.git가 있으면 상위 git이 빈 gitlink 서브모듈로 취급하여 새 PC에서 game/ 소스가 누락되므로,
    # 스테이징/커밋 동안에만 _cache/_game_git_hold로 잠시 대피시킨 뒤 즉시 원복한다.
    game_git_dir = GAME_DIR / ".git"
    game_git_hold = CACHE_DIR / "_game_git_hold"
    parked_game_git = False
    try:
        if game_git_dir.is_dir():
            CACHE_DIR.mkdir(parents=True, exist_ok=True)
            if game_git_hold.exists():
                _safe_rmtree(game_git_hold)
            shutil.move(str(game_git_dir), str(game_git_hold))
            parked_game_git = True

        rc_add, add_err = run_cmd([str(git_exe), "add", "-A"], cwd=DEFI_DIR, env=git_env, timeout=180.0)
        if rc_add != 0:
            log(f"[push] git add 실패: {add_err}")
            return 1

        # 민감 키/인증서 패턴 및 미완결(pins.json 미생성) 감사 폴더 방어적 언스테이징
        secret_globs = [
            ".env", ".env.*", "*.pem", "*.key", "*.p12", "*.pfx", "id_rsa", "id_ed25519",
            "*secret*", "*credential*", "*.keystore", "*.token", "*.asc", "*.ovpn", "*.jks",
            "secrets.yaml", "secrets.yml", ".npmrc", ".pypirc",
        ]
        run_cmd([str(git_exe), "reset", "-q", "HEAD", "--", *secret_globs], cwd=DEFI_DIR, env=git_env)
        if (DEFI_DIR / "tools" / "git").is_dir():
            run_cmd([str(git_exe), "add", "--", "tools/git"], cwd=DEFI_DIR, env=git_env)
        art_root_chk = DEFI_DIR / "audit_artifacts"
        if art_root_chk.is_dir():
            unsealed_rel = [
                f"audit_artifacts/{d.name}"
                for d in sorted(art_root_chk.iterdir())
                if d.is_dir() and not d.name.startswith("_") and not (d / "pins.json").is_file()
            ]
            for off in range(0, len(unsealed_rel), 64):
                run_cmd([str(git_exe), "reset", "-q", "HEAD", "--", *unsealed_rel[off : off + 64]], cwd=DEFI_DIR, env=git_env)

        rc_diff, _ = run_cmd([str(git_exe), "diff", "--cached", "--quiet"], cwd=DEFI_DIR, env=git_env)
        if rc_diff == 1:
            commit_msg = message.strip() or time.strftime("Auto commit: %Y-%m-%d %H:%M:%S")
            rc_c, c_out = run_cmd([str(git_exe), "commit", "-m", commit_msg], cwd=DEFI_DIR, env=git_env, timeout=120.0)
            if rc_c != 0:
                log(f"[push] 커밋 실패: {c_out}")
                return 1
            log(f"[push] 커밋 완료: {commit_msg}")
        else:
            log("[push] 신규 스테이징 변경 사항 없음 (원격 푸시 상태 확인)")
    finally:
        if parked_game_git and game_git_hold.exists() and not game_git_dir.exists():
            shutil.move(str(game_git_hold), str(game_git_dir))

    for attempt in range(1, 4):
        log(f"[push] origin/{branch} 푸시 시도 ({attempt}/3)...")
        rc_p, p_out = run_cmd([str(git_exe), "push", "origin", branch], cwd=DEFI_DIR, env=git_env, timeout=600.0)
        if rc_p == 0:
            log("[push] 푸시 성공!")
            return 0
        log(f"[push] 푸시 응답: {p_out[:300]}")
        rc_rb, rb_out = run_cmd(
            [str(git_exe), "pull", "--rebase", "--autostash", "origin", branch],
            cwd=DEFI_DIR,
            env=git_env,
            timeout=300.0,
        )
        if rc_rb != 0:
            run_cmd([str(git_exe), "rebase", "--abort"], cwd=DEFI_DIR, env=git_env)
            log(f"[push] 리베이스 충돌로 중단: {rb_out[:300]}")
            return 1
    return 1


# ─────────────────────────────────────────────────────────────────────────────
# 6. CLI 진입점 (사용자 커스텀 서브커맨드 확장 가능)
# ─────────────────────────────────────────────────────────────────────────────

def build_parser() -> argparse.ArgumentParser:
    ap = argparse.ArgumentParser(description="Defi 새 PC 복원 · 디스크 클린 · 전원 · 푸시 통합 스크립트")
    sub = ap.add_subparsers(dest="cmd")

    sub.add_parser("verify", help="내장 실행기(python/foundry/duckdb/git) 점검 및 VC++ DLL 자동 보강")
    sub.add_parser("power", help="Windows AC 절전/최대절전/모니터/디스크 타임아웃 0 설정")

    p_res = sub.add_parser("restore", help="새 PC 캐시 복원 (플래그 생략 시 sourcify + build 전체 실행)")
    p_res.add_argument("--sourcify", action="store_true", help="Sourcify v2 Parquet 동기화")
    p_res.add_argument("--tables", default="", help="Sourcify 테이블 필터 (쉼표 구분, 예: code,compiled_contracts)")
    p_res.add_argument("--workers", type=int, default=6, help="Sourcify 병렬 다운로드 스레드 수 (기본: 6)")
    p_res.add_argument("--build", action="store_true", help="game/ Cargo 빌드 실행")
    p_res.add_argument("--skip-build", action="store_true", help="전체 복원 시 game 빌드 생략")

    p_cln = sub.add_parser("clean", help="Defi 스크래치 및 시스템 캐시/Temp 통합 정리")
    p_cln.add_argument("--defi-only", action="store_true", help="Defi 스크래치/캐시만 빠르게 정리 (무승격)")
    p_cln.add_argument("--fast", action="store_true", help="드라이버/DISM(WinSxS) 단계를 생략하고 고속 정리")
    p_cln.add_argument("--no-elevate", action="store_true", help="관리자 UAC 자동 승격 생략")

    p_psh = sub.add_parser("push", help="내장 포터블 Git으로 변경사항 커밋 및 origin 푸시")
    p_psh.add_argument("-m", "--message", default="", help="커밋 메시지 (미지정 시 타임스탬프 자동 생성)")

    return ap


def main(argv: Optional[Sequence[str]] = None) -> int:
    ap = build_parser()
    args = ap.parse_args(argv)
    cmd = args.cmd or "default"

    if cmd == "verify":
        return 0 if verify_toolchain() else 1

    if cmd == "power":
        configure_power_never_sleep()
        return 0

    if cmd in ("restore", "default"):
        verify_toolchain()
        if cmd == "default":
            configure_power_never_sleep()
        do_sourcify = getattr(args, "sourcify", False)
        do_build = getattr(args, "build", False)
        skip_build = getattr(args, "skip_build", False)
        do_all = not (do_sourcify or do_build)

        if do_all or do_sourcify:
            try:
                raw_t = getattr(args, "tables", "")
                tbl_set = {x.strip() for x in raw_t.split(",") if x.strip()} or None
                sync_sourcify(tables=tbl_set, workers=getattr(args, "workers", 6))
            except Exception as exc:
                log(f"[sourcify] 실패: {exc}")
        if (do_all or do_build) and not skip_build:
            try:
                build_game()
            except Exception as exc:
                log(f"[build] 실패: {exc}")
        log("[완료] 새 PC 환경 준비가 끝났습니다.")
        return 0

    if cmd == "clean":
        if not args.defi_only and not args.no_elevate and not is_admin():
            elevate_if_needed()
        before_gb = disk_free_gb()
        log(f"[clean] 시작 여유 공간: {before_gb:.2f} GB")
        clean_defi_scratch()
        if not args.defi_only:
            clean_system(fast=args.fast)
        after_gb = disk_free_gb()
        log(f"[clean] 종료 여유 공간: {after_gb:.2f} GB (총 회수 {after_gb - before_gb:+.2f} GB)")
        return 0

    if cmd == "push":
        return git_push(message=args.message)

    ap.print_help()
    return 1


if __name__ == "__main__":
    os.environ["PYTHONUTF8"] = "1"
    sys.exit(main())

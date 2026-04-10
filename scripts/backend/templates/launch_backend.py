from __future__ import annotations

import atexit
import ctypes
import json
import os
import runpy
import sys
import threading
import time
from pathlib import Path

BACKEND_DIR = Path(__file__).resolve().parent
APP_DIR = BACKEND_DIR / "app"
_WINDOWS_DLL_DIRECTORY_HANDLES: list[object] = []
STARTUP_HEARTBEAT_ENV = "ASTRBOT_BACKEND_STARTUP_HEARTBEAT_PATH"
STARTUP_HEARTBEAT_INTERVAL_SECONDS = 2.0


def configure_stdio_utf8() -> None:
    os.environ.setdefault("PYTHONUTF8", "1")
    os.environ.setdefault("PYTHONIOENCODING", "utf-8")

    for stream_name in ("stdout", "stderr"):
        stream = getattr(sys, stream_name, None)
        reconfigure = getattr(stream, "reconfigure", None)
        if not callable(reconfigure):
            continue
        try:
            reconfigure(encoding="utf-8", errors="replace")
        except Exception:
            continue


def configure_windows_dll_search_path() -> None:
    if sys.platform != "win32" or not hasattr(os, "add_dll_directory"):
        return

    runtime_executable_dir = Path(sys.executable).resolve().parent
    site_packages_dirs = [
        runtime_executable_dir / "Lib" / "site-packages",
        BACKEND_DIR / "python" / "Lib" / "site-packages",
    ]
    candidates = [
        runtime_executable_dir,
        runtime_executable_dir / "DLLs",
        BACKEND_DIR / "python",
        BACKEND_DIR / "python" / "DLLs",
    ]
    for site_packages_dir in site_packages_dirs:
        candidates.extend(
            [
                site_packages_dir / "cryptography.libs",
                site_packages_dir / "cryptography" / "hazmat" / "bindings",
            ],
        )

    normalized_added: set[str] = set()
    path_entries: list[str] = []
    for candidate in candidates:
        if not candidate.is_dir():
            continue
        candidate_str = str(candidate)
        candidate_key = candidate_str.lower()
        if candidate_key in normalized_added:
            continue
        normalized_added.add(candidate_key)
        path_entries.append(candidate_str)
        try:
            _WINDOWS_DLL_DIRECTORY_HANDLES.append(
                os.add_dll_directory(candidate_str),
            )
        except OSError:
            continue

    if path_entries:
        existing_path = os.environ.get("PATH", "")
        os.environ["PATH"] = (
            ";".join(path_entries + [existing_path])
            if existing_path
            else ";".join(path_entries)
        )


def preload_windows_runtime_dlls() -> None:
    if sys.platform != "win32":
        return

    runtime_executable_dir = Path(sys.executable).resolve().parent
    runtime_dll_dir = runtime_executable_dir / "DLLs"
    backend_runtime_dir = BACKEND_DIR / "python"
    backend_runtime_dll_dir = backend_runtime_dir / "DLLs"
    candidate_dirs = [
        runtime_executable_dir,
        runtime_dll_dir,
        backend_runtime_dir,
        backend_runtime_dll_dir,
    ]
    patterns = [
        "python3.dll",
        "python*.dll",
        "vcruntime*.dll",
        "libcrypto-*.dll",
        "libssl-*.dll",
    ]
    loaded: set[str] = set()
    for candidate_dir in candidate_dirs:
        if not candidate_dir.is_dir():
            continue
        for pattern in patterns:
            for dll_path in candidate_dir.glob(pattern):
                normalized_path = str(dll_path.resolve()).lower()
                if normalized_path in loaded:
                    continue
                loaded.add(normalized_path)
                try:
                    ctypes.WinDLL(str(dll_path))
                except OSError:
                    continue


def resolve_startup_heartbeat_path() -> Path | None:
    raw = os.environ.get(STARTUP_HEARTBEAT_ENV, "").strip()
    if not raw:
        return None
    return Path(raw)


def write_startup_heartbeat(
    path: Path, state: str, *, warn_on_error: bool = False
) -> bool:
    try:
        path.parent.mkdir(parents=True, exist_ok=True)
        payload = {
            "pid": os.getpid(),
            "state": state,
            "updated_at_ms": int(time.time() * 1000),
        }
        temp_path = path.with_name(f"{path.name}.tmp")
        temp_path.write_text(
            json.dumps(payload, separators=(",", ":")),
            encoding="utf-8",
        )
        temp_path.replace(path)
        return True
    except Exception as exc:
        if warn_on_error:
            print(
                f"[startup-heartbeat] failed to write heartbeat to {path}: {exc.__class__.__name__}: {exc}",
                file=sys.stderr,
            )
        return False


def start_startup_heartbeat() -> None:
    heartbeat_path = resolve_startup_heartbeat_path()
    if heartbeat_path is None:
        return

    stop_event = threading.Event()
    warning_emitted = not write_startup_heartbeat(
        heartbeat_path,
        "starting",
        warn_on_error=True,
    )

    def refresh_heartbeat(state: str) -> None:
        nonlocal warning_emitted
        if not write_startup_heartbeat(
            heartbeat_path,
            state,
            warn_on_error=not warning_emitted,
        ):
            warning_emitted = True

    def stop_heartbeat() -> None:
        stop_event.set()
        refresh_heartbeat("stopping")

    def heartbeat_loop() -> None:
        while not stop_event.wait(STARTUP_HEARTBEAT_INTERVAL_SECONDS):
            refresh_heartbeat("starting")

    atexit.register(stop_heartbeat)
    threading.Thread(
        target=heartbeat_loop,
        name="astrbot-startup-heartbeat",
        daemon=True,
    ).start()


configure_stdio_utf8()
configure_windows_dll_search_path()
preload_windows_runtime_dlls()
start_startup_heartbeat()

sys.path.insert(0, str(APP_DIR))

main_file = APP_DIR / "main.py"
if not main_file.is_file():
    raise FileNotFoundError(f"Backend entrypoint not found: {main_file}")

sys.argv[0] = str(main_file)
runpy.run_path(str(main_file), run_name="__main__")

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


class StartupHeartbeat:
    def __init__(self, path: Path, interval_seconds: float) -> None:
        self._path = path
        self._interval_seconds = interval_seconds
        self._stop_event = threading.Event()
        self._had_successful_write = False
        self._warning_emitted = False

    def _write(self, state: str, *, warn_on_error: bool) -> bool:
        effective_warn_on_error = warn_on_error and (
            state == "stopping"
            or not self._warning_emitted
            or not self._had_successful_write
        )
        ok = write_startup_heartbeat(
            self._path,
            state,
            warn_on_error=effective_warn_on_error,
        )
        if ok:
            self._had_successful_write = True
            self._warning_emitted = False
        elif effective_warn_on_error:
            self._warning_emitted = True
        return ok

    def start(self) -> None:
        self._write("starting", warn_on_error=True)
        atexit.register(self.stop)
        threading.Thread(
            target=self._loop,
            name="astrbot-startup-heartbeat",
            daemon=True,
        ).start()

    def stop(self) -> None:
        self._stop_event.set()
        self._write("stopping", warn_on_error=True)

    def _loop(self) -> None:
        while not self._stop_event.wait(self._interval_seconds):
            self._write("starting", warn_on_error=True)


def start_startup_heartbeat() -> None:
    heartbeat_path = resolve_startup_heartbeat_path()
    if heartbeat_path is None:
        return

    StartupHeartbeat(heartbeat_path, STARTUP_HEARTBEAT_INTERVAL_SECONDS).start()


def main() -> None:
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


if __name__ == "__main__":
    main()

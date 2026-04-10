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


def build_heartbeat_payload(state: str) -> dict[str, object]:
    return {
        "pid": os.getpid(),
        "state": state,
        "updated_at_ms": int(time.time() * 1000),
    }


def atomic_write_json(path: Path, payload: dict[str, object]) -> None:
    temp_path = path.with_name(f"{path.name}.tmp")
    temp_path.write_text(
        json.dumps(payload, separators=(",", ":")),
        encoding="utf-8",
    )
    temp_path.replace(path)


def write_startup_heartbeat(
    path: Path, state: str, *, warn_on_error: bool = False
) -> bool:
    try:
        path.parent.mkdir(parents=True, exist_ok=True)
        atomic_write_json(path, build_heartbeat_payload(state))
        return True
    except Exception as exc:
        if warn_on_error:
            print(
                f"[startup-heartbeat] failed to write heartbeat to {path}: {exc.__class__.__name__}: {exc}",
                file=sys.stderr,
            )
        return False


def heartbeat_loop(
    path: Path, interval_seconds: float, stop_event: threading.Event
) -> None:
    # At least one successful write has happened.
    had_successful_write = False
    # A warning has already been emitted since the last successful write.
    warning_emitted_since_last_success = False

    def should_warn() -> bool:
        return (not had_successful_write) or (not warning_emitted_since_last_success)

    ok = write_startup_heartbeat(path, "starting", warn_on_error=True)
    if ok:
        had_successful_write = True
    else:
        warning_emitted_since_last_success = True

    while not stop_event.wait(interval_seconds):
        warn_now = should_warn()
        ok = write_startup_heartbeat(path, "starting", warn_on_error=warn_now)
        if ok:
            had_successful_write = True
            warning_emitted_since_last_success = False
        elif warn_now:
            warning_emitted_since_last_success = True


def start_startup_heartbeat() -> None:
    heartbeat_path = resolve_startup_heartbeat_path()
    if heartbeat_path is None:
        return

    stop_event = threading.Event()

    def on_exit() -> None:
        stop_event.set()
        write_startup_heartbeat(heartbeat_path, "stopping", warn_on_error=True)

    atexit.register(on_exit)
    threading.Thread(
        target=heartbeat_loop,
        args=(heartbeat_path, STARTUP_HEARTBEAT_INTERVAL_SECONDS, stop_event),
        name="astrbot-startup-heartbeat",
        daemon=True,
    ).start()


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

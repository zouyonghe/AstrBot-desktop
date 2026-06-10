#[cfg(target_os = "windows")]
mod platform {
    use std::{
        mem,
        sync::{Mutex, MutexGuard, OnceLock},
        time::Duration,
    };

    use tauri::{AppHandle, Manager};
    use windows_sys::Win32::{
        Foundation::{GetLastError, SetLastError, HWND, LPARAM, LRESULT, WPARAM},
        System::Threading::SetProcessShutdownParameters,
        UI::WindowsAndMessaging::{
            CallWindowProcW, DefWindowProcW, SetWindowLongPtrW, GWLP_WNDPROC, WM_ENDSESSION,
            WM_QUERYENDSESSION, WNDPROC,
        },
    };

    use crate::{append_shutdown_log, BackendState, SYSTEM_SHUTDOWN_STOP_TIMEOUT_MS};

    const SHUTDOWN_PRIORITY_EARLY: u32 = 0x100;

    #[derive(Default)]
    struct ShutdownHookState {
        app_handle: Option<AppHandle>,
        installed: bool,
        previous_wndproc: isize,
        cleanup_started: bool,
    }

    static SHUTDOWN_HOOK: OnceLock<Mutex<ShutdownHookState>> = OnceLock::new();

    fn lock_shutdown_hook<'a>(
        hook: &'a Mutex<ShutdownHookState>,
        context: &str,
    ) -> MutexGuard<'a, ShutdownHookState> {
        match hook.lock() {
            Ok(guard) => guard,
            Err(error) => {
                append_shutdown_log(&format!(
                    "Windows shutdown handler lock poisoned {context}: {error}"
                ));
                error.into_inner()
            }
        }
    }

    pub(crate) fn install(app_handle: &AppHandle) {
        unsafe {
            if SetProcessShutdownParameters(SHUTDOWN_PRIORITY_EARLY, 0) == 0 {
                append_shutdown_log("failed to set Windows shutdown priority");
            }
        }

        let Some(window) = app_handle.get_webview_window("main") else {
            append_shutdown_log("Windows shutdown handler skipped: main window not found");
            return;
        };
        let hwnd = match window.hwnd() {
            Ok(hwnd) => hwnd.0,
            Err(error) => {
                append_shutdown_log(&format!("Windows shutdown handler skipped: {error}"));
                return;
            }
        };

        let hook = SHUTDOWN_HOOK.get_or_init(|| Mutex::new(ShutdownHookState::default()));
        let mut guard = lock_shutdown_hook(hook, "during install");

        guard.app_handle = Some(app_handle.clone());
        if guard.installed {
            return;
        }

        let previous = unsafe {
            SetLastError(0);
            SetWindowLongPtrW(hwnd, GWLP_WNDPROC, shutdown_wndproc as isize)
        };
        let last_error = unsafe { GetLastError() };
        if previous == 0 && last_error != 0 {
            append_shutdown_log(&format!(
                "Windows shutdown handler install failed: error={last_error}"
            ));
            return;
        }
        guard.installed = true;
        guard.previous_wndproc = previous;
        append_shutdown_log("Windows shutdown handler installed");
    }

    unsafe extern "system" fn shutdown_wndproc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match msg {
            WM_QUERYENDSESSION => {
                handle_query_end_session();
                1
            }
            WM_ENDSESSION => {
                let previous_result = call_previous_wndproc(hwnd, msg, wparam, lparam);
                if wparam != 0 {
                    append_shutdown_log("Windows end session confirmed, exiting desktop process");
                    std::process::exit(0);
                }
                reset_shutdown_cleanup();
                previous_result
            }
            _ => call_previous_wndproc(hwnd, msg, wparam, lparam),
        }
    }

    fn handle_query_end_session() {
        let Some(app_handle) = take_shutdown_app_handle_for_cleanup() else {
            append_shutdown_log("Windows shutdown cleanup skipped: app handle unavailable");
            return;
        };

        append_shutdown_log("Windows shutdown requested, stopping backend quickly");
        let state = app_handle.state::<BackendState>();
        // Avoid launching taskkill.exe during OS shutdown. Late in shutdown,
        // Windows can fail to initialize new console/system helper processes
        // with 0xc0000142, which is the issue this hook must prevent.
        if let Err(error) = state.stop_backend_for_system_shutdown(Duration::from_millis(
            SYSTEM_SHUTDOWN_STOP_TIMEOUT_MS,
        )) {
            append_shutdown_log(&format!("backend stop on Windows shutdown failed: {error}"));
        }
    }

    fn take_shutdown_app_handle_for_cleanup() -> Option<AppHandle> {
        let hook = SHUTDOWN_HOOK.get()?;
        let mut guard = lock_shutdown_hook(hook, "during cleanup");
        if guard.cleanup_started {
            return None;
        }
        guard.cleanup_started = true;
        guard.app_handle.clone()
    }

    fn reset_shutdown_cleanup() {
        let Some(hook) = SHUTDOWN_HOOK.get() else {
            return;
        };
        let mut guard = lock_shutdown_hook(hook, "while resetting cleanup");
        guard.cleanup_started = false;
        append_shutdown_log("Windows shutdown canceled, cleanup flag reset");
    }

    unsafe fn call_previous_wndproc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        let previous = SHUTDOWN_HOOK
            .get()
            .map(|hook| lock_shutdown_hook(hook, "while forwarding message").previous_wndproc)
            .unwrap_or_default();
        if previous == 0 {
            return DefWindowProcW(hwnd, msg, wparam, lparam);
        }
        let previous: WNDPROC = mem::transmute(previous);
        CallWindowProcW(previous, hwnd, msg, wparam, lparam)
    }
}

#[cfg(target_os = "windows")]
pub(crate) use platform::install;

#[cfg(not(target_os = "windows"))]
pub(crate) fn install(_app_handle: &tauri::AppHandle) {}

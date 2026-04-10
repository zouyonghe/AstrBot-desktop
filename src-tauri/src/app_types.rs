use serde::Deserialize;
use std::{
    env,
    path::PathBuf,
    process::Child,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};
use tauri::menu::MenuItem;

use crate::{backend, exit_state, DEFAULT_BACKEND_URL};

#[derive(Clone)]
pub(crate) struct TrayMenuState {
    pub(crate) toggle_item: MenuItem<tauri::Wry>,
    pub(crate) reload_item: MenuItem<tauri::Wry>,
    pub(crate) restart_backend_item: MenuItem<tauri::Wry>,
    pub(crate) quit_item: MenuItem<tauri::Wry>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RuntimeManifest {
    pub(crate) python: Option<String>,
    pub(crate) entrypoint: Option<String>,
}

#[derive(Debug)]
pub(crate) struct LaunchPlan {
    pub(crate) cmd: String,
    pub(crate) args: Vec<String>,
    pub(crate) cwd: PathBuf,
    pub(crate) root_dir: Option<PathBuf>,
    pub(crate) webui_dir: Option<PathBuf>,
    pub(crate) startup_heartbeat_path: Option<PathBuf>,
    pub(crate) packaged_mode: bool,
}

#[derive(Debug)]
pub(crate) struct BackendState {
    pub(crate) child: Mutex<Option<Child>>,
    pub(crate) backend_url: String,
    pub(crate) restart_auth_token: Mutex<Option<String>>,
    pub(crate) startup_loading_mode: Mutex<Option<&'static str>>,
    pub(crate) log_rotator_stop: Mutex<Option<Arc<AtomicBool>>>,
    pub(crate) exit_state: Mutex<exit_state::ExitStateMachine>,
    pub(crate) is_spawning: AtomicBool,
    pub(crate) is_restarting: AtomicBool,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackendBridgeState {
    pub(crate) running: bool,
    pub(crate) spawning: bool,
    pub(crate) restarting: bool,
    pub(crate) can_manage: bool,
}

#[derive(Debug, serde::Serialize)]
pub(crate) struct BackendBridgeResult {
    pub(crate) ok: bool,
    pub(crate) reason: Option<String>,
}

pub(crate) struct AtomicFlagGuard<'a> {
    flag: &'a AtomicBool,
}

impl<'a> AtomicFlagGuard<'a> {
    pub(crate) fn set(flag: &'a AtomicBool) -> Self {
        flag.store(true, Ordering::Relaxed);
        Self { flag }
    }

    pub(crate) fn try_set(flag: &'a AtomicBool) -> Option<Self> {
        flag.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .ok()?;
        Some(Self { flag })
    }
}

impl Drop for AtomicFlagGuard<'_> {
    fn drop(&mut self) {
        self.flag.store(false, Ordering::Relaxed);
    }
}

impl Default for BackendState {
    fn default() -> Self {
        Self {
            child: Mutex::new(None),
            backend_url: backend::config::normalize_backend_url(
                &env::var("ASTRBOT_BACKEND_URL")
                    .unwrap_or_else(|_| DEFAULT_BACKEND_URL.to_string()),
                DEFAULT_BACKEND_URL,
            ),
            restart_auth_token: Mutex::new(None),
            startup_loading_mode: Mutex::new(None),
            log_rotator_stop: Mutex::new(None),
            exit_state: Mutex::new(exit_state::ExitStateMachine::default()),
            is_spawning: AtomicBool::new(false),
            is_restarting: AtomicBool::new(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};

    use super::AtomicFlagGuard;

    #[test]
    fn atomic_flag_guard_set_resets_flag_on_drop() {
        let flag = AtomicBool::new(false);
        {
            let _guard = AtomicFlagGuard::set(&flag);
            assert!(flag.load(Ordering::Relaxed));
        }
        assert!(!flag.load(Ordering::Relaxed));
    }

    #[test]
    fn atomic_flag_guard_try_set_rejects_double_set_until_drop() {
        let flag = AtomicBool::new(false);

        let guard = AtomicFlagGuard::try_set(&flag).expect("first set should succeed");
        assert!(flag.load(Ordering::Relaxed));
        assert!(AtomicFlagGuard::try_set(&flag).is_none());

        drop(guard);
        assert!(!flag.load(Ordering::Relaxed));
        assert!(AtomicFlagGuard::try_set(&flag).is_some());
    }
}

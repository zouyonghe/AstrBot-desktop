use std::time::Duration;

pub(crate) const DEFAULT_BACKEND_URL: &str = "http://127.0.0.1:6185/";
pub(crate) const BACKEND_TIMEOUT_ENV: &str = "ASTRBOT_BACKEND_TIMEOUT_MS";
pub(crate) const PACKAGED_BACKEND_TIMEOUT_FALLBACK_MS: u64 = 15 * 60 * 1000;
pub(crate) const GRACEFUL_RESTART_REQUEST_TIMEOUT_MS: u64 = 2_500;
pub(crate) const GRACEFUL_RESTART_START_TIME_TIMEOUT_MS: u64 = 1_800;
pub(crate) const GRACEFUL_RESTART_POLL_INTERVAL_MS: u64 = 350;
pub(crate) const GRACEFUL_STOP_TIMEOUT_MS: u64 = 10_000;
pub(crate) const DEFAULT_BACKEND_READY_POLL_INTERVAL_MS: u64 = 300;
pub(crate) const BACKEND_READY_POLL_INTERVAL_MIN_MS: u64 = 50;
pub(crate) const BACKEND_READY_POLL_INTERVAL_MAX_MS: u64 = 10_000;
pub(crate) const BACKEND_READY_POLL_INTERVAL_ENV: &str = "ASTRBOT_BACKEND_READY_POLL_INTERVAL_MS";
pub(crate) const DEFAULT_BACKEND_READY_HTTP_PATH: &str = "/api/stat/start-time";
pub(crate) const BACKEND_READY_HTTP_PATH_ENV: &str = "ASTRBOT_BACKEND_READY_HTTP_PATH";
pub(crate) const BACKEND_READY_PROBE_TIMEOUT_ENV: &str = "ASTRBOT_BACKEND_READY_PROBE_TIMEOUT_MS";
pub(crate) const BACKEND_READY_PROBE_TIMEOUT_MIN_MS: u64 = 100;
pub(crate) const BACKEND_READY_PROBE_TIMEOUT_MAX_MS: u64 = 30_000;
pub(crate) const BACKEND_READY_TCP_PROBE_TIMEOUT_MAX_MS: u64 = 1_000;
pub(crate) const BACKEND_STARTUP_IDLE_TIMEOUT_ENV: &str = "ASTRBOT_BACKEND_STARTUP_IDLE_TIMEOUT_MS";
pub(crate) const DEFAULT_BACKEND_STARTUP_IDLE_TIMEOUT_MS: u64 = 60 * 1000;
pub(crate) const BACKEND_STARTUP_IDLE_TIMEOUT_MIN_MS: u64 = 5_000;
pub(crate) const BACKEND_STARTUP_IDLE_TIMEOUT_MAX_MS: u64 = 15 * 60 * 1000;
pub(crate) const BACKEND_STARTUP_HEARTBEAT_PATH_ENV: &str =
    "ASTRBOT_BACKEND_STARTUP_HEARTBEAT_PATH";
pub(crate) const DEFAULT_BACKEND_STARTUP_HEARTBEAT_RELATIVE_PATH: &str =
    "data/backend-startup-heartbeat.json";
pub(crate) const DEFAULT_BACKEND_PING_TIMEOUT_MS: u64 = 800;
pub(crate) const BACKEND_PING_TIMEOUT_MIN_MS: u64 = 50;
pub(crate) const BACKEND_PING_TIMEOUT_MAX_MS: u64 = 30_000;
pub(crate) const BACKEND_PING_TIMEOUT_ENV: &str = "ASTRBOT_BACKEND_PING_TIMEOUT_MS";
pub(crate) const BRIDGE_BACKEND_PING_TIMEOUT_ENV: &str = "ASTRBOT_BRIDGE_BACKEND_PING_TIMEOUT_MS";
pub(crate) const DESKTOP_LOG_MAX_BYTES: u64 = 5 * 1024 * 1024;
pub(crate) const BACKEND_LOG_MAX_BYTES: u64 = 20 * 1024 * 1024;
pub(crate) const LOG_BACKUP_COUNT: usize = 5;
pub(crate) const BACKEND_LOG_ROTATION_CHECK_INTERVAL: Duration = Duration::from_secs(20);
pub(crate) const DESKTOP_LOG_FILE: &str = "desktop.log";
pub(crate) const TRAY_ID: &str = "astrbot-tray";
pub(crate) const TRAY_RESTART_BACKEND_EVENT: &str = "astrbot://tray-restart-backend";
pub(crate) const DEFAULT_SHELL_LOCALE: &str = "zh-CN";
pub(crate) const STARTUP_MODE_ENV: &str = "ASTRBOT_DESKTOP_STARTUP_MODE";
#[cfg(target_os = "windows")]
pub(crate) const CREATE_NO_WINDOW: u32 = 0x0800_0000;
#[cfg(target_os = "windows")]
pub(crate) const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::Deserialize;
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
use std::{
    borrow::Cow,
    env,
    ffi::OsString,
    fs::{self, OpenOptions},
    io::{Read, Write},
    net::{IpAddr, TcpStream, ToSocketAddrs},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, OnceLock,
    },
    thread,
    time::{Duration, Instant},
};
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    path::BaseDirectory,
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    webview::PageLoadEvent,
    AppHandle, Manager, RunEvent, WindowEvent,
};
use url::Url;

const DEFAULT_BACKEND_URL: &str = "http://127.0.0.1:6185/";
const PACKAGED_BACKEND_TIMEOUT_FALLBACK_MS: u64 = 5 * 60 * 1000;
const GRACEFUL_RESTART_REQUEST_TIMEOUT_MS: u64 = 2_500;
const GRACEFUL_RESTART_START_TIME_TIMEOUT_MS: u64 = 1_800;
const GRACEFUL_RESTART_POLL_INTERVAL_MS: u64 = 350;
const GRACEFUL_STOP_TIMEOUT_MS: u64 = 10_000;
const DEFAULT_BACKEND_PING_TIMEOUT_MS: u64 = 800;
const BACKEND_PING_TIMEOUT_ENV: &str = "ASTRBOT_BACKEND_PING_TIMEOUT_MS";
const BRIDGE_BACKEND_PING_TIMEOUT_ENV: &str = "ASTRBOT_BRIDGE_BACKEND_PING_TIMEOUT_MS";
const DESKTOP_LOG_MAX_BYTES: u64 = 5 * 1024 * 1024;
const BACKEND_LOG_MAX_BYTES: u64 = 20 * 1024 * 1024;
const LOG_BACKUP_COUNT: usize = 5;
const BACKEND_LOG_ROTATION_CHECK_INTERVAL: Duration = Duration::from_secs(20);
const DESKTOP_LOG_FILE: &str = "desktop.log";
const TRAY_ID: &str = "astrbot-tray";
const TRAY_MENU_TOGGLE_WINDOW: &str = "tray_toggle_window";
const TRAY_MENU_RELOAD_WINDOW: &str = "tray_reload_window";
const TRAY_MENU_RESTART_BACKEND: &str = "tray_restart_backend";
const TRAY_MENU_QUIT: &str = "tray_quit";
const DEFAULT_SHELL_LOCALE: &str = "zh-CN";
const STARTUP_MODE_ENV: &str = "ASTRBOT_DESKTOP_STARTUP_MODE";
// Keep in sync with STARTUP_MODES in ui/index.html.
const STARTUP_MODE_LOADING: &str = "loading";
const STARTUP_MODE_PANEL_UPDATE: &str = "panel-update";
#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
#[cfg(target_os = "windows")]
const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
static BACKEND_PING_TIMEOUT_MS: OnceLock<u64> = OnceLock::new();
static BRIDGE_BACKEND_PING_TIMEOUT_MS: OnceLock<u64> = OnceLock::new();
static DESKTOP_LOG_WRITE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

#[derive(Debug, Clone, Copy)]
struct ShellTexts {
    tray_hide: &'static str,
    tray_show: &'static str,
    tray_reload: &'static str,
    tray_restart_backend: &'static str,
    tray_quit: &'static str,
}

#[derive(Clone)]
struct TrayMenuState {
    toggle_item: MenuItem<tauri::Wry>,
    reload_item: MenuItem<tauri::Wry>,
    restart_backend_item: MenuItem<tauri::Wry>,
    quit_item: MenuItem<tauri::Wry>,
}

#[derive(Debug, Deserialize)]
struct RuntimeManifest {
    python: Option<String>,
    entrypoint: Option<String>,
}

#[derive(Debug)]
struct LaunchPlan {
    cmd: String,
    args: Vec<String>,
    cwd: PathBuf,
    root_dir: Option<PathBuf>,
    webui_dir: Option<PathBuf>,
    packaged_mode: bool,
}

#[derive(Debug)]
struct BackendState {
    child: Mutex<Option<Child>>,
    backend_url: String,
    restart_auth_token: Mutex<Option<String>>,
    startup_loading_mode: Mutex<Option<&'static str>>,
    log_rotator_stop: Mutex<Option<Arc<AtomicBool>>>,
    is_quitting: AtomicBool,
    is_spawning: AtomicBool,
    is_restarting: AtomicBool,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct BackendBridgeState {
    running: bool,
    spawning: bool,
    restarting: bool,
    can_manage: bool,
}

#[derive(Debug, serde::Serialize)]
struct BackendBridgeResult {
    ok: bool,
    reason: Option<String>,
}

#[derive(Debug, Clone, Copy)]
enum RestartStrategy {
    ManagedSkipGraceful,
    ManagedWithGracefulFallback,
    UnmanagedWithGracefulProbe,
}

#[derive(Debug)]
enum GracefulRestartOutcome {
    Completed,
    WaitFailed(String),
    RequestRejected,
}

#[derive(Debug, Clone, Copy)]
struct TrayOriginDecision {
    uses_backend_origin: bool,
    should_log_mismatch: bool,
}

struct AtomicFlagGuard<'a> {
    flag: &'a AtomicBool,
}

impl<'a> AtomicFlagGuard<'a> {
    fn set(flag: &'a AtomicBool) -> Self {
        flag.store(true, Ordering::Relaxed);
        Self { flag }
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
            backend_url: normalize_backend_url(
                &env::var("ASTRBOT_BACKEND_URL")
                    .unwrap_or_else(|_| DEFAULT_BACKEND_URL.to_string()),
            ),
            restart_auth_token: Mutex::new(None),
            startup_loading_mode: Mutex::new(None),
            log_rotator_stop: Mutex::new(None),
            is_quitting: AtomicBool::new(false),
            is_spawning: AtomicBool::new(false),
            is_restarting: AtomicBool::new(false),
        }
    }
}

impl BackendState {
    fn ensure_backend_ready(&self, app: &AppHandle) -> Result<(), String> {
        if self.ping_backend(backend_ping_timeout_ms()) {
            append_desktop_log("backend already reachable, skip spawn");
            return Ok(());
        }

        if env::var("ASTRBOT_BACKEND_AUTO_START").unwrap_or_else(|_| "1".to_string()) == "0" {
            append_desktop_log("backend auto-start disabled by ASTRBOT_BACKEND_AUTO_START=0");
            return Err(
                "Backend auto-start is disabled (ASTRBOT_BACKEND_AUTO_START=0).".to_string(),
            );
        }

        let _spawn_guard = AtomicFlagGuard::set(&self.is_spawning);
        let plan = self.resolve_launch_plan(app)?;
        self.start_backend_process(app, &plan)?;
        self.wait_for_backend(&plan)
    }

    fn resolve_launch_plan(&self, app: &AppHandle) -> Result<LaunchPlan, String> {
        if let Some(custom_cmd) = env::var("ASTRBOT_BACKEND_CMD")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
        {
            return self.resolve_custom_launch(custom_cmd);
        }

        if let Some(plan) = self.resolve_packaged_launch(app)? {
            return Ok(plan);
        }

        self.resolve_dev_launch()
    }

    fn resolve_custom_launch(&self, custom_cmd: String) -> Result<LaunchPlan, String> {
        let mut pieces = shlex::split(&custom_cmd)
            .ok_or_else(|| format!("Invalid ASTRBOT_BACKEND_CMD: {custom_cmd}"))?;
        if pieces.is_empty() {
            return Err("ASTRBOT_BACKEND_CMD is empty.".to_string());
        }

        let cmd = pieces.remove(0);
        let cwd = env::var("ASTRBOT_BACKEND_CWD")
            .map(PathBuf::from)
            .ok()
            .or_else(detect_astrbot_source_root)
            .unwrap_or_else(workspace_root_dir);
        let root_dir = env::var("ASTRBOT_ROOT").ok().map(PathBuf::from);
        let webui_dir = env::var("ASTRBOT_WEBUI_DIR").ok().map(PathBuf::from);

        Ok(LaunchPlan {
            cmd,
            args: pieces,
            cwd,
            root_dir,
            webui_dir,
            packaged_mode: false,
        })
    }

    fn resolve_packaged_launch(&self, app: &AppHandle) -> Result<Option<LaunchPlan>, String> {
        let manifest_path = match resolve_resource_path(app, "backend/runtime-manifest.json") {
            Some(path) if path.is_file() => path,
            _ => return Ok(None),
        };
        let backend_dir = manifest_path
            .parent()
            .ok_or_else(|| format!("Invalid backend manifest path: {}", manifest_path.display()))?;

        let manifest_text = fs::read_to_string(&manifest_path).map_err(|error| {
            format!(
                "Failed to read packaged backend manifest {}: {}",
                manifest_path.display(),
                error
            )
        })?;
        let manifest: RuntimeManifest = serde_json::from_str(&manifest_text).map_err(|error| {
            format!(
                "Failed to parse packaged backend manifest {}: {}",
                manifest_path.display(),
                error
            )
        })?;

        let default_python_relative = if cfg!(target_os = "windows") {
            PathBuf::from("python").join("Scripts").join("python.exe")
        } else {
            PathBuf::from("python").join("bin").join("python3")
        };
        let python_path = backend_dir.join(
            manifest
                .python
                .as_deref()
                .map(PathBuf::from)
                .unwrap_or(default_python_relative),
        );
        if !python_path.is_file() {
            return Err(format!(
                "Packaged runtime python executable is missing: {}",
                python_path.display()
            ));
        }

        let entrypoint_relative = manifest
            .entrypoint
            .as_deref()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("launch_backend.py"));
        let launch_script_path = backend_dir.join(entrypoint_relative);
        if !launch_script_path.is_file() {
            return Err(format!(
                "Packaged backend launch script is missing: {}",
                launch_script_path.display()
            ));
        }

        let root_dir = env::var("ASTRBOT_ROOT")
            .map(PathBuf::from)
            .ok()
            .or_else(default_packaged_root_dir);
        let cwd = env::var("ASTRBOT_BACKEND_CWD")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                root_dir
                    .clone()
                    .unwrap_or_else(|| backend_dir.to_path_buf())
            });
        let webui_dir = env::var("ASTRBOT_WEBUI_DIR")
            .ok()
            .map(PathBuf::from)
            .or_else(|| {
                resolve_resource_path(app, "webui/index.html")
                    .and_then(|index_path| index_path.parent().map(Path::to_path_buf))
            });

        let plan = LaunchPlan {
            cmd: python_path.to_string_lossy().to_string(),
            args: vec![launch_script_path.to_string_lossy().to_string()],
            cwd,
            root_dir,
            webui_dir,
            packaged_mode: true,
        };
        Ok(Some(plan))
    }

    fn resolve_dev_launch(&self) -> Result<LaunchPlan, String> {
        let source_root = detect_astrbot_source_root().ok_or_else(|| {
            "Cannot locate AstrBot source directory. Set ASTRBOT_SOURCE_DIR, or configure ASTRBOT_SOURCE_GIT_URL/ASTRBOT_SOURCE_GIT_REF and run resource prepare.".to_string()
        })?;

        let mut args = vec!["run".to_string(), "main.py".to_string()];
        let webui_dir = env::var("ASTRBOT_WEBUI_DIR")
            .ok()
            .map(PathBuf::from)
            .or_else(|| {
                let candidate = source_root.join("dashboard").join("dist");
                if candidate.join("index.html").is_file() {
                    Some(candidate)
                } else {
                    None
                }
            });
        if let Some(path) = &webui_dir {
            args.push("--webui-dir".to_string());
            args.push(path.to_string_lossy().to_string());
        }

        Ok(LaunchPlan {
            cmd: "uv".to_string(),
            args,
            cwd: env::var("ASTRBOT_BACKEND_CWD")
                .map(PathBuf::from)
                .unwrap_or(source_root),
            root_dir: env::var("ASTRBOT_ROOT").ok().map(PathBuf::from),
            webui_dir,
            packaged_mode: false,
        })
    }

    fn start_backend_process(&self, app: &AppHandle, plan: &LaunchPlan) -> Result<(), String> {
        if self
            .child
            .lock()
            .map_err(|_| "Backend process lock poisoned.")?
            .is_some()
        {
            append_desktop_log("backend child already exists, skip re-spawn");
            return Ok(());
        }

        if !plan.cwd.exists() {
            fs::create_dir_all(&plan.cwd).map_err(|error| {
                format!(
                    "Failed to create backend cwd {}: {}",
                    plan.cwd.display(),
                    error
                )
            })?;
        }
        if let Some(root_dir) = &plan.root_dir {
            if !root_dir.exists() {
                fs::create_dir_all(root_dir).map_err(|error| {
                    format!(
                        "Failed to create backend root directory {}: {}",
                        root_dir.display(),
                        error
                    )
                })?;
            }
        }

        let mut command = Command::new(&plan.cmd);
        command
            .args(&plan.args)
            .current_dir(&plan.cwd)
            .stdin(Stdio::null())
            .env("PYTHONUNBUFFERED", "1")
            .env(
                "PYTHONUTF8",
                env::var("PYTHONUTF8").unwrap_or_else(|_| "1".to_string()),
            )
            .env(
                "PYTHONIOENCODING",
                env::var("PYTHONIOENCODING").unwrap_or_else(|_| "utf-8".to_string()),
            );
        #[cfg(target_os = "windows")]
        {
            // Keep packaged backend fully backgrounded; keep console visible for local/dev debugging.
            if plan.packaged_mode {
                command.creation_flags(CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP);
            }
        }

        if plan.packaged_mode {
            command.env("ASTRBOT_DESKTOP_CLIENT", "1");
            if env::var("DASHBOARD_HOST").is_err() && env::var("ASTRBOT_DASHBOARD_HOST").is_err() {
                command.env("DASHBOARD_HOST", "127.0.0.1");
            }
            if env::var("DASHBOARD_PORT").is_err() && env::var("ASTRBOT_DASHBOARD_PORT").is_err() {
                command.env("DASHBOARD_PORT", "6185");
            }
        }

        if let Some(root_dir) = &plan.root_dir {
            command.env("ASTRBOT_ROOT", root_dir);
        }
        if let Some(webui_dir) = &plan.webui_dir {
            command.env("ASTRBOT_WEBUI_DIR", webui_dir);
        }

        let backend_log_path = backend_log_path(plan.root_dir.as_deref());
        if let Some(log_path) = backend_log_path.as_ref() {
            if let Some(log_parent) = log_path.parent() {
                fs::create_dir_all(log_parent).map_err(|error| {
                    format!(
                        "Failed to create backend log directory {}: {}",
                        log_parent.display(),
                        error
                    )
                })?;
            }
            rotate_log_if_needed(
                log_path,
                BACKEND_LOG_MAX_BYTES,
                LOG_BACKUP_COUNT,
                "backend",
                false,
            );
            let stdout_file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(log_path)
                .map_err(|error| {
                    format!(
                        "Failed to open backend log {}: {}",
                        log_path.display(),
                        error
                    )
                })?;
            let stderr_file = stdout_file
                .try_clone()
                .map_err(|error| format!("Failed to clone backend log handle: {error}"))?;
            command.stdout(Stdio::from(stdout_file));
            command.stderr(Stdio::from(stderr_file));
        } else {
            self.stop_backend_log_rotation_worker();
            command.stdout(Stdio::null());
            command.stderr(Stdio::null());
        }

        let child = command.spawn().map_err(|error| {
            format!(
                "Failed to spawn backend process with command {:?}: {}",
                build_debug_command(plan),
                error
            )
        })?;
        let child_pid = child.id();
        append_desktop_log(&format!(
            "spawned backend: cmd={:?}, cwd={}",
            build_debug_command(plan),
            plan.cwd.display()
        ));
        *self
            .child
            .lock()
            .map_err(|_| "Backend process lock poisoned.")? = Some(child);
        if let Some(log_path) = backend_log_path {
            self.start_backend_log_rotation_worker(app, log_path, child_pid);
        } else {
            self.stop_backend_log_rotation_worker();
        }
        Ok(())
    }

    fn wait_for_backend(&self, plan: &LaunchPlan) -> Result<(), String> {
        let timeout_ms = resolve_backend_timeout_ms(plan.packaged_mode);
        let start_time = Instant::now();

        loop {
            if self.ping_backend(backend_ping_timeout_ms()) {
                return Ok(());
            }

            {
                let mut guard = self
                    .child
                    .lock()
                    .map_err(|_| "Backend process lock poisoned.".to_string())?;
                if let Some(child) = guard.as_mut() {
                    match child.try_wait() {
                        Ok(Some(status)) => {
                            *guard = None;
                            return Err(format!(
                                "Backend process exited before becoming reachable: {status}"
                            ));
                        }
                        Ok(None) => {}
                        Err(error) => {
                            return Err(format!("Failed to poll backend process status: {error}"));
                        }
                    }
                } else {
                    return Err("Backend process is not running.".to_string());
                }
            }

            if let Some(limit) = timeout_ms {
                if start_time.elapsed() >= limit {
                    return Err(format!(
                        "Timed out after {}ms waiting for backend startup.",
                        limit.as_millis()
                    ));
                }
            }

            thread::sleep(Duration::from_millis(600));
        }
    }

    fn ping_backend(&self, timeout_ms: u64) -> bool {
        let parsed = match Url::parse(&self.backend_url) {
            Ok(url) => url,
            Err(_) => return false,
        };
        let host = match parsed.host_str() {
            Some(host) => host.to_string(),
            None => return false,
        };
        let port = parsed.port_or_known_default().unwrap_or(80);
        let timeout = Duration::from_millis(timeout_ms.max(50));

        let addrs = match (host.as_str(), port).to_socket_addrs() {
            Ok(addrs) => addrs.collect::<Vec<_>>(),
            Err(_) => return false,
        };
        addrs
            .iter()
            .any(|address| TcpStream::connect_timeout(address, timeout).is_ok())
    }

    fn request_backend_response_bytes(
        &self,
        method: &str,
        api_path: &str,
        timeout_ms: u64,
        body: Option<&str>,
        auth_token: Option<&str>,
    ) -> Option<Vec<u8>> {
        let base = Url::parse(&self.backend_url).ok()?;
        let request_url = base.join(api_path).ok()?;
        if request_url.scheme() != "http" {
            return None;
        }

        let host = request_url.host_str()?;
        let port = request_url.port_or_known_default().unwrap_or(80);
        let timeout = Duration::from_millis(timeout_ms.max(50));
        let addrs = (host, port).to_socket_addrs().ok()?;
        let mut stream = addrs
            .into_iter()
            .find_map(|address| TcpStream::connect_timeout(&address, timeout).ok())?;
        let _ = stream.set_read_timeout(Some(timeout));
        let _ = stream.set_write_timeout(Some(timeout));

        let mut request_target = request_url.path().to_string();
        if let Some(query) = request_url.query() {
            request_target.push('?');
            request_target.push_str(query);
        }
        if request_target.is_empty() {
            request_target = "/".to_string();
        }

        let payload = body.unwrap_or("");
        let authorization_header = auth_token
            .map(str::trim)
            .filter(|token| !token.is_empty())
            .map(|token| format!("Authorization: Bearer {token}\r\n"))
            .unwrap_or_default();
        let request = format!(
            "{method} {request_target} HTTP/1.1\r\n\
Host: {host}\r\n\
Accept: application/json\r\n\
Accept-Encoding: identity\r\n\
Connection: close\r\n\
{authorization_header}\
Content-Type: application/json\r\n\
Content-Length: {}\r\n\
\r\n\
{}",
            payload.len(),
            payload
        );
        if stream.write_all(request.as_bytes()).is_err() {
            return None;
        }

        let mut response = Vec::new();
        if stream.read_to_end(&mut response).is_err() {
            return None;
        }

        Some(response)
    }

    fn request_backend_with<T, F>(
        &self,
        method: &str,
        api_path: &str,
        timeout_ms: u64,
        body: Option<&str>,
        auth_token: Option<&str>,
        parse: F,
    ) -> Option<T>
    where
        F: FnOnce(&[u8]) -> Option<T>,
    {
        let response =
            self.request_backend_response_bytes(method, api_path, timeout_ms, body, auth_token)?;
        parse(&response)
    }

    fn request_backend_json(
        &self,
        method: &str,
        api_path: &str,
        timeout_ms: u64,
        body: Option<&str>,
        auth_token: Option<&str>,
    ) -> Option<serde_json::Value> {
        self.request_backend_with(
            method,
            api_path,
            timeout_ms,
            body,
            auth_token,
            parse_http_json_response,
        )
    }

    fn request_backend_status_code(
        &self,
        method: &str,
        api_path: &str,
        timeout_ms: u64,
        body: Option<&str>,
        auth_token: Option<&str>,
    ) -> Option<u16> {
        self.request_backend_with(
            method,
            api_path,
            timeout_ms,
            body,
            auth_token,
            parse_http_status_code,
        )
    }

    fn fetch_backend_start_time(&self) -> Option<i64> {
        let payload = self.request_backend_json(
            "GET",
            "/api/stat/start-time",
            GRACEFUL_RESTART_START_TIME_TIMEOUT_MS,
            None,
            None,
        )?;
        parse_backend_start_time(&payload)
    }

    fn sanitize_auth_token(auth_token: Option<&str>) -> Option<String> {
        auth_token
            .map(str::trim)
            .filter(|token| !token.is_empty())
            .map(|token| token.to_string())
    }

    fn get_restart_auth_token(&self) -> Option<String> {
        match self.restart_auth_token.lock() {
            Ok(guard) => guard.clone(),
            Err(error) => {
                append_desktop_log(&format!(
                    "restart auth token lock poisoned when reading: {error}"
                ));
                None
            }
        }
    }

    fn set_restart_auth_token(&self, provided_auth_token: Option<&str>) {
        let normalized = Self::sanitize_auth_token(provided_auth_token);
        match self.restart_auth_token.lock() {
            Ok(mut guard) => {
                *guard = normalized;
            }
            Err(error) => append_desktop_log(&format!(
                "restart auth token lock poisoned when writing: {error}"
            )),
        }
    }

    fn request_graceful_restart(&self, auth_token: Option<&str>) -> bool {
        let status_code = self.request_backend_status_code(
            "POST",
            "/api/stat/restart-core",
            GRACEFUL_RESTART_REQUEST_TIMEOUT_MS,
            Some("{}"),
            auth_token,
        );
        match status_code {
            Some(code) if (200..300).contains(&code) => true,
            Some(code) => {
                append_desktop_log(&format!(
                    "graceful restart request rejected with HTTP status {code}"
                ));
                false
            }
            None => {
                append_desktop_log(
                    "graceful restart request returned no HTTP status; will verify restart by polling backend",
                );
                true
            }
        }
    }

    fn wait_for_graceful_restart(
        &self,
        previous_start_time: Option<i64>,
        packaged_mode: bool,
    ) -> Result<(), String> {
        let max_wait = backend_wait_timeout(packaged_mode);
        let start = Instant::now();
        let mut saw_backend_down = false;

        loop {
            let reachable = self.ping_backend(700);
            if !reachable {
                saw_backend_down = true;
            } else {
                let current_start_time = self.fetch_backend_start_time();
                if let (Some(previous), Some(current)) = (previous_start_time, current_start_time) {
                    if current != previous {
                        return Ok(());
                    }
                } else if previous_start_time.is_none() && saw_backend_down {
                    return Ok(());
                }
            }

            if start.elapsed() >= max_wait {
                return Err(format!(
                    "Timed out after {}ms waiting for graceful restart.",
                    max_wait.as_millis()
                ));
            }

            thread::sleep(Duration::from_millis(GRACEFUL_RESTART_POLL_INTERVAL_MS));
        }
    }

    fn stop_backend(&self) -> Result<(), String> {
        self.stop_backend_log_rotation_worker();
        let mut guard = self
            .child
            .lock()
            .map_err(|_| "Backend process lock poisoned.".to_string())?;

        let Some(child) = guard.as_mut() else {
            return Ok(());
        };

        if stop_child_process_gracefully(child, Duration::from_millis(GRACEFUL_STOP_TIMEOUT_MS)) {
            *guard = None;
            return Ok(());
        }

        Err(format!(
            "Backend process did not exit after {}ms graceful stop timeout.",
            GRACEFUL_STOP_TIMEOUT_MS
        ))
    }

    fn stop_backend_log_rotation_worker(&self) {
        match self.log_rotator_stop.lock() {
            Ok(mut guard) => {
                if let Some(flag) = guard.take() {
                    flag.store(true, Ordering::Relaxed);
                }
            }
            Err(error) => {
                append_desktop_log(&format!(
                    "backend log rotator stop flag lock poisoned: {error}"
                ));
            }
        }
    }

    fn child_matches_pid_and_alive(&self, child_pid: u32) -> bool {
        let mut guard = match self.child.lock() {
            Ok(guard) => guard,
            Err(error) => {
                append_desktop_log(&format!(
                    "backend child lock poisoned while checking log rotator worker pid={child_pid}: {error}"
                ));
                return false;
            }
        };

        let Some(child) = guard.as_mut() else {
            return false;
        };
        if child.id() != child_pid {
            return false;
        }

        match child.try_wait() {
            Ok(None) => true,
            Ok(Some(status)) => {
                append_desktop_log(&format!(
                    "backend process exited, stop log rotator worker: pid={child_pid}, status={status}"
                ));
                false
            }
            Err(error) => {
                append_desktop_log(&format!(
                    "failed to poll backend process status for log rotator worker pid={child_pid}: {error}"
                ));
                false
            }
        }
    }

    fn start_backend_log_rotation_worker(
        &self,
        app: &AppHandle,
        log_path: PathBuf,
        child_pid: u32,
    ) {
        self.stop_backend_log_rotation_worker();
        let stop_flag = Arc::new(AtomicBool::new(false));
        match self.log_rotator_stop.lock() {
            Ok(mut guard) => {
                *guard = Some(stop_flag.clone());
            }
            Err(error) => {
                append_desktop_log(&format!(
                    "backend log rotator stop flag lock poisoned on start: {error}"
                ));
                return;
            }
        }

        let app_handle = app.clone();
        thread::spawn(move || {
            let log_scope = format!("backend(pid={child_pid})");
            loop {
                if stop_flag.load(Ordering::Relaxed) {
                    break;
                }
                thread::sleep(BACKEND_LOG_ROTATION_CHECK_INTERVAL);
                if stop_flag.load(Ordering::Relaxed) {
                    break;
                }
                let state = app_handle.state::<BackendState>();
                if !state.child_matches_pid_and_alive(child_pid) {
                    break;
                }
                rotate_log_if_needed(
                    &log_path,
                    BACKEND_LOG_MAX_BYTES,
                    LOG_BACKUP_COUNT,
                    &log_scope,
                    true,
                );
            }
        });
    }

    fn stop_backend_for_bridge(&self) -> Result<(), String> {
        let has_managed_child = self
            .child
            .lock()
            .map_err(|_| "Backend process lock poisoned.".to_string())?
            .is_some();
        if has_managed_child {
            return self.stop_backend();
        }

        if self.ping_backend(backend_ping_timeout_ms()) {
            return Err("Backend is running but not managed by desktop process.".to_string());
        }
        Ok(())
    }

    fn has_managed_child(&self) -> Result<bool, String> {
        self.child
            .lock()
            .map(|guard| guard.is_some())
            .map_err(|error| {
                let message = format!(
                    "backend child lock poisoned while resolving restart strategy: {error}"
                );
                append_desktop_log(&message);
                message
            })
    }

    fn restart_strategy(&self, plan: &LaunchPlan, has_managed_child: bool) -> RestartStrategy {
        if cfg!(target_os = "windows") && plan.packaged_mode && has_managed_child {
            return RestartStrategy::ManagedSkipGraceful;
        }
        if has_managed_child {
            return RestartStrategy::ManagedWithGracefulFallback;
        }
        RestartStrategy::UnmanagedWithGracefulProbe
    }

    fn try_graceful_restart_and_wait(
        &self,
        auth_token: Option<&str>,
        previous_start_time: Option<i64>,
        packaged_mode: bool,
    ) -> GracefulRestartOutcome {
        if !self.request_graceful_restart(auth_token) {
            return GracefulRestartOutcome::RequestRejected;
        }

        match self.wait_for_graceful_restart(previous_start_time, packaged_mode) {
            Ok(()) => GracefulRestartOutcome::Completed,
            Err(error) => GracefulRestartOutcome::WaitFailed(error),
        }
    }

    fn restart_backend(&self, app: &AppHandle, auth_token: Option<&str>) -> Result<(), String> {
        append_desktop_log("backend restart requested");

        let _restart_guard = AtomicFlagGuard::set(&self.is_restarting);
        let plan = self.resolve_launch_plan(app)?;
        let has_managed_child = self.has_managed_child()?;
        let strategy = self.restart_strategy(&plan, has_managed_child);
        let normalized_param = Self::sanitize_auth_token(auth_token);
        if let Some(token) = normalized_param.as_deref() {
            self.set_restart_auth_token(Some(token));
        }
        let restart_auth_token = normalized_param.or_else(|| self.get_restart_auth_token());
        let previous_start_time = self.fetch_backend_start_time();
        match strategy {
            RestartStrategy::ManagedSkipGraceful => append_desktop_log(
                "skip graceful restart for packaged windows managed backend; using managed restart",
            ),
            RestartStrategy::ManagedWithGracefulFallback => {
                match self.try_graceful_restart_and_wait(
                    restart_auth_token.as_deref(),
                    previous_start_time,
                    plan.packaged_mode,
                ) {
                    GracefulRestartOutcome::Completed => {
                        append_desktop_log("graceful restart completed via backend api");
                        return Ok(());
                    }
                    GracefulRestartOutcome::WaitFailed(error) => append_desktop_log(&format!(
                        "graceful restart did not complete, fallback to managed restart: {error}"
                    )),
                    GracefulRestartOutcome::RequestRejected => append_desktop_log(
                        "graceful restart request was rejected, fallback to managed restart",
                    ),
                }
            }
            RestartStrategy::UnmanagedWithGracefulProbe => {
                match self.try_graceful_restart_and_wait(
                    restart_auth_token.as_deref(),
                    previous_start_time,
                    plan.packaged_mode,
                ) {
                    GracefulRestartOutcome::Completed => {
                        append_desktop_log("graceful restart completed via backend api");
                        return Ok(());
                    }
                    GracefulRestartOutcome::WaitFailed(error) => append_desktop_log(&format!(
                        "graceful restart did not complete for unmanaged backend, bootstrap managed restart: {error}"
                    )),
                    GracefulRestartOutcome::RequestRejected => {
                        return Err(
                            "graceful restart request was rejected and backend is not desktop-managed."
                                .to_string(),
                        );
                    }
                }
            }
        }

        self.stop_backend()?;
        let _spawn_guard = AtomicFlagGuard::set(&self.is_spawning);
        self.start_backend_process(app, &plan)?;
        self.wait_for_backend(&plan)
    }

    fn bridge_state(&self, app: &AppHandle) -> BackendBridgeState {
        let has_managed_child = self
            .child
            .lock()
            .map(|guard| guard.is_some())
            .unwrap_or_else(|error| {
                append_desktop_log(&format!(
                    "backend bridge: child process mutex poisoned in bridge_state: {error}"
                ));
                false
            });
        let can_manage = has_managed_child || self.resolve_launch_plan(app).is_ok();
        BackendBridgeState {
            running: self.ping_backend(bridge_backend_ping_timeout_ms()),
            spawning: self.is_spawning.load(Ordering::Relaxed),
            restarting: self.is_restarting.load(Ordering::Relaxed),
            can_manage,
        }
    }

    fn mark_quitting(&self) {
        self.is_quitting.store(true, Ordering::Relaxed);
    }

    fn is_quitting(&self) -> bool {
        self.is_quitting.load(Ordering::Relaxed)
    }
}

#[tauri::command]
fn desktop_bridge_is_desktop_runtime() -> bool {
    true
}

#[tauri::command]
fn desktop_bridge_is_electron_runtime() -> bool {
    desktop_bridge_is_desktop_runtime()
}

#[tauri::command]
fn desktop_bridge_get_backend_state(app_handle: AppHandle) -> BackendBridgeState {
    let state = app_handle.state::<BackendState>();
    state.bridge_state(&app_handle)
}

#[tauri::command]
fn desktop_bridge_set_auth_token(
    app_handle: AppHandle,
    auth_token: Option<String>,
) -> BackendBridgeResult {
    let state = app_handle.state::<BackendState>();
    state.set_restart_auth_token(auth_token.as_deref());
    BackendBridgeResult {
        ok: true,
        reason: None,
    }
}

#[tauri::command]
async fn desktop_bridge_restart_backend(
    app_handle: AppHandle,
    auth_token: Option<String>,
) -> BackendBridgeResult {
    let state = app_handle.state::<BackendState>();
    if state.is_spawning.load(Ordering::Relaxed) || state.is_restarting.load(Ordering::Relaxed) {
        return BackendBridgeResult {
            ok: false,
            reason: Some("Backend action already in progress.".to_string()),
        };
    }

    let app_handle_cloned = app_handle.clone();
    match tauri::async_runtime::spawn_blocking(move || {
        do_restart_backend(&app_handle_cloned, auth_token.as_deref())
    })
    .await
    {
        Ok(Ok(())) => BackendBridgeResult {
            ok: true,
            reason: None,
        },
        Ok(Err(error)) => BackendBridgeResult {
            ok: false,
            reason: Some(error),
        },
        Err(error) => BackendBridgeResult {
            ok: false,
            reason: Some(format!("Backend restart task failed: {error}")),
        },
    }
}

#[tauri::command]
fn desktop_bridge_stop_backend(app_handle: AppHandle) -> BackendBridgeResult {
    let state = app_handle.state::<BackendState>();
    if state.is_spawning.load(Ordering::Relaxed) || state.is_restarting.load(Ordering::Relaxed) {
        return BackendBridgeResult {
            ok: false,
            reason: Some("Backend action already in progress.".to_string()),
        };
    }

    match state.stop_backend_for_bridge() {
        Ok(()) => BackendBridgeResult {
            ok: true,
            reason: None,
        },
        Err(error) => BackendBridgeResult {
            ok: false,
            reason: Some(error),
        },
    }
}

fn main() {
    append_desktop_log("desktop process starting");
    append_desktop_log(&format!(
        "desktop log path: {}",
        desktop_log_path().display()
    ));
    tauri::Builder::default()
        .manage(BackendState::default())
        .invoke_handler(tauri::generate_handler![
            desktop_bridge_is_desktop_runtime,
            desktop_bridge_is_electron_runtime,
            desktop_bridge_get_backend_state,
            desktop_bridge_set_auth_token,
            desktop_bridge_restart_backend,
            desktop_bridge_stop_backend
        ])
        .on_window_event(|window, event| {
            if window.label() != "main" {
                return;
            }

            match event {
                WindowEvent::CloseRequested { api, .. } => {
                    let app_handle = window.app_handle();
                    let state = app_handle.state::<BackendState>();
                    if state.is_quitting() {
                        return;
                    }

                    api.prevent_close();
                    hide_main_window(app_handle);
                }
                WindowEvent::Focused(false) => {
                    if let Ok(true) = window.is_minimized() {
                        let app_handle = window.app_handle();
                        let state = app_handle.state::<BackendState>();
                        if !state.is_quitting() {
                            hide_main_window(app_handle);
                        }
                    }
                }
                _ => {}
            }
        })
        .on_page_load(|webview, payload| match payload.event() {
            PageLoadEvent::Started => {
                append_desktop_log(&format!("page-load started: {}", payload.url()))
            }
            PageLoadEvent::Finished => {
                append_desktop_log(&format!("page-load finished: {}", payload.url()));
                if should_inject_desktop_bridge(webview.app_handle(), payload.url()) {
                    inject_desktop_bridge(webview);
                } else if should_apply_startup_loading_mode(webview, payload.url()) {
                    apply_startup_loading_mode(webview);
                }
            }
        })
        .setup(|app| {
            let app_handle = app.handle().clone();
            if let Err(error) = setup_tray(&app_handle) {
                append_desktop_log(&format!("failed to initialize tray: {error}"));
            }

            let startup_app_handle = app_handle.clone();
            tauri::async_runtime::spawn(async move {
                let startup_worker_handle = startup_app_handle.clone();
                let startup_result = tauri::async_runtime::spawn_blocking(move || {
                    let state = startup_worker_handle.state::<BackendState>();
                    state.ensure_backend_ready(&startup_worker_handle)
                })
                .await
                .map_err(|error| format!("Backend startup task failed: {error}"))
                .and_then(|result| result);

                match startup_result {
                    Ok(()) => {
                        if let Err(error) = run_on_main_thread_dispatch(
                            &startup_app_handle,
                            "navigate backend",
                            move |main_app| match navigate_main_window_to_backend(main_app) {
                                Ok(()) => {}
                                Err(navigate_error) => {
                                    show_startup_error(main_app, &navigate_error);
                                }
                            },
                        ) {
                            show_startup_error_on_main_thread(&startup_app_handle, &error);
                        }
                    }
                    Err(error) => {
                        show_startup_error_on_main_thread(&startup_app_handle, &error);
                    }
                }
            });
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| match event {
            RunEvent::ExitRequested { .. } => {
                let state = app_handle.state::<BackendState>();
                state.mark_quitting();
                if let Err(error) = state.stop_backend() {
                    append_desktop_log(&format!(
                        "backend graceful stop on ExitRequested failed: {error}"
                    ));
                }
            }
            RunEvent::Exit => {
                let state = app_handle.state::<BackendState>();
                if let Err(error) = state.stop_backend() {
                    append_desktop_log(&format!("backend graceful stop on Exit failed: {error}"));
                }
            }
            _ => {}
        });
}

fn setup_tray(app_handle: &AppHandle) -> Result<(), String> {
    let locale = resolve_shell_locale();
    let shell_texts = shell_texts_for_locale(locale);
    let main_window_visible = app_handle
        .get_webview_window("main")
        .and_then(|window| window.is_visible().ok())
        .unwrap_or(true);
    let toggle_label = if main_window_visible {
        shell_texts.tray_hide
    } else {
        shell_texts.tray_show
    };

    let toggle_item = MenuItem::with_id(
        app_handle,
        TRAY_MENU_TOGGLE_WINDOW,
        toggle_label,
        true,
        None::<&str>,
    )
    .map_err(|error| format!("Failed to create tray toggle menu item: {error}"))?;
    let reload_item = MenuItem::with_id(
        app_handle,
        TRAY_MENU_RELOAD_WINDOW,
        shell_texts.tray_reload,
        true,
        None::<&str>,
    )
    .map_err(|error| format!("Failed to create tray reload menu item: {error}"))?;
    let restart_backend_item = MenuItem::with_id(
        app_handle,
        TRAY_MENU_RESTART_BACKEND,
        shell_texts.tray_restart_backend,
        true,
        None::<&str>,
    )
    .map_err(|error| format!("Failed to create tray restart menu item: {error}"))?;
    let quit_item = MenuItem::with_id(
        app_handle,
        TRAY_MENU_QUIT,
        shell_texts.tray_quit,
        true,
        None::<&str>,
    )
    .map_err(|error| format!("Failed to create tray quit menu item: {error}"))?;
    let separator = PredefinedMenuItem::separator(app_handle)
        .map_err(|error| format!("Failed to create tray separator menu item: {error}"))?;

    let menu = Menu::with_items(
        app_handle,
        &[
            &toggle_item,
            &reload_item,
            &restart_backend_item,
            &separator,
            &quit_item,
        ],
    )
    .map_err(|error| format!("Failed to build tray menu: {error}"))?;

    if !app_handle.manage(TrayMenuState {
        toggle_item: toggle_item.clone(),
        reload_item: reload_item.clone(),
        restart_backend_item: restart_backend_item.clone(),
        quit_item: quit_item.clone(),
    }) {
        append_desktop_log("tray menu state already exists, skipping manage");
    }

    let tray_builder = TrayIconBuilder::with_id(TRAY_ID)
        .menu(&menu)
        .tooltip("AstrBot")
        .icon(tauri::include_image!("./icons/tray.png"))
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| handle_tray_menu_event(app, event.id().as_ref()))
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                update_tray_menu_labels(tray.app_handle());
                if button == MouseButton::Left {
                    toggle_main_window(tray.app_handle());
                }
            }
        });

    #[cfg(target_os = "macos")]
    let tray_builder = tray_builder.icon_as_template(true);

    tray_builder
        .build(app_handle)
        .map_err(|error| format!("Failed to create tray icon: {error}"))?;

    update_tray_menu_labels(app_handle);
    Ok(())
}

fn handle_tray_menu_event(app_handle: &AppHandle, menu_id: &str) {
    match menu_id {
        TRAY_MENU_TOGGLE_WINDOW => toggle_main_window(app_handle),
        TRAY_MENU_RELOAD_WINDOW => reload_main_window(app_handle),
        TRAY_MENU_RESTART_BACKEND => {
            append_desktop_log("tray requested backend restart");
            show_main_window(app_handle);
            if main_window_uses_backend_origin(app_handle) {
                emit_tray_restart_backend_event(app_handle);
                return;
            }

            let app_handle_cloned = app_handle.clone();
            thread::spawn(move || match do_restart_backend(&app_handle_cloned, None) {
                Ok(()) => {
                    append_desktop_log("backend restarted from tray menu");
                    reload_main_window(&app_handle_cloned);
                }
                Err(error) => {
                    append_desktop_log(&format!("backend restart from tray menu failed: {error}"))
                }
            });
        }
        TRAY_MENU_QUIT => {
            let state = app_handle.state::<BackendState>();
            state.mark_quitting();
            app_handle.exit(0);
        }
        _ => {}
    }
}

fn main_window_uses_backend_origin(app_handle: &AppHandle) -> bool {
    let Some(window) = app_handle.get_webview_window("main") else {
        return false;
    };
    let Ok(window_url) = window.url() else {
        return false;
    };
    let state = app_handle.state::<BackendState>();
    let Ok(backend_url) = Url::parse(&state.backend_url) else {
        return false;
    };
    let decision = tray_origin_decision(&backend_url, &window_url);
    if !decision.uses_backend_origin && decision.should_log_mismatch {
        append_desktop_log(&format!(
            "tray restart fallback to desktop-managed flow due to origin mismatch: backend={} window={}",
            backend_url, window_url
        ));
    }
    decision.uses_backend_origin
}

fn emit_tray_restart_backend_event(app_handle: &AppHandle) {
    let Some(window) = app_handle.get_webview_window("main") else {
        return;
    };

    let script = r#"
(() => {
  if (typeof window.__astrbotDesktopEmitTrayRestart === 'function') {
    window.__astrbotDesktopEmitTrayRestart();
    return;
  }
  const state =
    window.__astrbotDesktopTrayRestartState ||
    (window.__astrbotDesktopTrayRestartState = { handlers: new Set(), pending: 0 });
  state.pending = Number(state.pending || 0) + 1;
})();
"#;
    if let Err(error) = window.eval(script) {
        append_desktop_log(&format!(
            "failed to emit tray restart backend event to webview: {error}"
        ));
    }
}

fn do_restart_backend(app_handle: &AppHandle, auth_token: Option<&str>) -> Result<(), String> {
    let state = app_handle.state::<BackendState>();
    state.restart_backend(app_handle, auth_token)
}

fn show_main_window(app_handle: &AppHandle) {
    let Some(window) = app_handle.get_webview_window("main") else {
        append_desktop_log("show_main_window skipped: main window not found");
        return;
    };

    if let Err(error) = window.unminimize() {
        append_desktop_log(&format!("failed to unminimize main window: {error}"));
    }
    if let Err(error) = window.show() {
        append_desktop_log(&format!("failed to show main window: {error}"));
    }
    if let Err(error) = window.set_focus() {
        append_desktop_log(&format!("failed to focus main window: {error}"));
    }
    update_tray_menu_labels(app_handle);
}

fn hide_main_window(app_handle: &AppHandle) {
    let Some(window) = app_handle.get_webview_window("main") else {
        append_desktop_log("hide_main_window skipped: main window not found");
        return;
    };
    if let Err(error) = window.hide() {
        append_desktop_log(&format!("failed to hide main window: {error}"));
    }
    update_tray_menu_labels(app_handle);
}

fn toggle_main_window(app_handle: &AppHandle) {
    let Some(window) = app_handle.get_webview_window("main") else {
        append_desktop_log("toggle_main_window skipped: main window not found");
        return;
    };

    match window.is_visible() {
        Ok(true) => hide_main_window(app_handle),
        Ok(false) => show_main_window(app_handle),
        Err(error) => append_desktop_log(&format!(
            "failed to read main window visibility in toggle_main_window: {error}"
        )),
    }
}

fn reload_main_window(app_handle: &AppHandle) {
    let Some(window) = app_handle.get_webview_window("main") else {
        append_desktop_log("reload_main_window skipped: main window not found");
        return;
    };
    if let Err(error) = window.reload() {
        append_desktop_log(&format!("failed to reload main window: {error}"));
    }
}

fn navigate_main_window_to_backend(app_handle: &AppHandle) -> Result<(), String> {
    let state = app_handle.state::<BackendState>();
    let backend_url =
        serde_json::to_string(&state.backend_url).unwrap_or_else(|_| "\"/\"".to_string());

    let Some(window) = app_handle.get_webview_window("main") else {
        return Err("Main window is unavailable after backend startup.".to_string());
    };

    let js = format!("window.location.replace({backend_url});");
    window
        .eval(&js)
        .map_err(|error| format!("Failed to navigate to backend dashboard: {error}"))
}

fn shell_texts_for_locale(locale: &str) -> ShellTexts {
    if locale == "en-US" {
        return ShellTexts {
            tray_hide: "Hide AstrBot",
            tray_show: "Show AstrBot",
            tray_reload: "Reload",
            tray_restart_backend: "Restart Backend",
            tray_quit: "Quit",
        };
    }

    ShellTexts {
        tray_hide: "隐藏 AstrBot",
        tray_show: "显示 AstrBot",
        tray_reload: "重新加载",
        tray_restart_backend: "重启后端",
        tray_quit: "退出",
    }
}

fn normalize_shell_locale(raw: &str) -> Option<&'static str> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    if raw == "zh-CN" {
        return Some("zh-CN");
    }
    if raw == "en-US" {
        return Some("en-US");
    }

    let lowered = raw.to_ascii_lowercase();
    if lowered.starts_with("zh") {
        return Some("zh-CN");
    }
    if lowered.starts_with("en") {
        return Some("en-US");
    }
    None
}

fn desktop_state_path_for_locale() -> Option<PathBuf> {
    if let Ok(root) = env::var("ASTRBOT_ROOT") {
        let path = PathBuf::from(root.trim());
        if !path.as_os_str().is_empty() {
            return Some(path.join("data").join("desktop_state.json"));
        }
    }

    default_packaged_root_dir().map(|root| root.join("data").join("desktop_state.json"))
}

fn read_cached_shell_locale() -> Option<&'static str> {
    let state_path = desktop_state_path_for_locale()?;
    let raw = fs::read_to_string(state_path).ok()?;
    let parsed: serde_json::Value = serde_json::from_str(&raw).ok()?;
    let locale = parsed.get("locale")?.as_str()?;
    normalize_shell_locale(locale)
}

fn resolve_shell_locale() -> &'static str {
    if let Some(locale) = read_cached_shell_locale() {
        return locale;
    }

    for env_key in ["ASTRBOT_DESKTOP_LOCALE", "LC_ALL", "LANG"] {
        if let Ok(value) = env::var(env_key) {
            if let Some(locale) = normalize_shell_locale(&value) {
                return locale;
            }
        }
    }

    DEFAULT_SHELL_LOCALE
}

fn set_menu_text_safe(item: &MenuItem<tauri::Wry>, text: &str, item_name: &str) {
    if let Err(error) = item.set_text(text) {
        append_desktop_log(&format!(
            "failed to update tray menu text for {}: {}",
            item_name, error
        ));
    }
}

fn update_tray_menu_labels(app_handle: &AppHandle) {
    let Some(tray_state) = app_handle.try_state::<TrayMenuState>() else {
        return;
    };

    let locale = resolve_shell_locale();
    let shell_texts = shell_texts_for_locale(locale);
    let is_visible = app_handle
        .get_webview_window("main")
        .and_then(|window| window.is_visible().ok())
        .unwrap_or(true);
    let toggle_label = if is_visible {
        shell_texts.tray_hide
    } else {
        shell_texts.tray_show
    };

    set_menu_text_safe(
        &tray_state.toggle_item,
        toggle_label,
        TRAY_MENU_TOGGLE_WINDOW,
    );
    set_menu_text_safe(
        &tray_state.reload_item,
        shell_texts.tray_reload,
        TRAY_MENU_RELOAD_WINDOW,
    );
    set_menu_text_safe(
        &tray_state.restart_backend_item,
        shell_texts.tray_restart_backend,
        TRAY_MENU_RESTART_BACKEND,
    );
    set_menu_text_safe(&tray_state.quit_item, shell_texts.tray_quit, TRAY_MENU_QUIT);
}

const DESKTOP_BRIDGE_BOOTSTRAP_SCRIPT: &str = r#"
(() => {
  const invoke = window.__TAURI_INTERNALS__?.invoke;
  if (typeof invoke !== 'function') return;

  const BRIDGE_COMMANDS = Object.freeze({
    IS_DESKTOP_RUNTIME: 'desktop_bridge_is_desktop_runtime',
    IS_ELECTRON_RUNTIME: 'desktop_bridge_is_electron_runtime',
    GET_BACKEND_STATE: 'desktop_bridge_get_backend_state',
    SET_AUTH_TOKEN: 'desktop_bridge_set_auth_token',
    RESTART_BACKEND: 'desktop_bridge_restart_backend',
    STOP_BACKEND: 'desktop_bridge_stop_backend',
  });

  const invokeBridge = async (command, payload = {}) => {
    try {
      return await invoke(command, payload);
    } catch (error) {
      return { ok: false, reason: String(error) };
    }
  };

  const trayRestartState =
    window.__astrbotDesktopTrayRestartState ||
    (window.__astrbotDesktopTrayRestartState = { handlers: new Set(), pending: 0 });

  const emitTrayRestart = () => {
    if (trayRestartState.handlers.size === 0) {
      trayRestartState.pending = Number(trayRestartState.pending || 0) + 1;
      return;
    }
    for (const handler of trayRestartState.handlers) {
      try {
        handler();
      } catch {}
    }
  };

  window.__astrbotDesktopEmitTrayRestart = emitTrayRestart;

  const onTrayRestartBackend = (callback) => {
    if (typeof callback !== 'function') return () => {};
    const handler = () => callback();
    trayRestartState.handlers.add(handler);
    while (trayRestartState.pending > 0) {
      trayRestartState.pending -= 1;
      handler();
    }
    return () => trayRestartState.handlers.delete(handler);
  };

  const getStoredAuthToken = () => {
    try {
      const token = window.localStorage?.getItem('token');
      return typeof token === 'string' && token ? token : null;
    } catch {
      return null;
    }
  };

  const syncAuthToken = (token = getStoredAuthToken()) =>
    invokeBridge(BRIDGE_COMMANDS.SET_AUTH_TOKEN, {
      authToken: typeof token === 'string' && token ? token : null
    });

  const RUNTIME_BRIDGE_DETAIL_MAX_LENGTH = 240;
  const RUNTIME_BRIDGE_DETAIL_MAX_ITEMS = 8;
  const RUNTIME_BRIDGE_TRUE_STRINGS = new Set(['1', 'true', 'yes', 'on']);
  const RUNTIME_BRIDGE_FALSE_STRINGS = new Set(['0', 'false', 'no', 'off']);
  const RUNTIME_BRIDGE_SENSITIVE_KEY_PATTERN =
    /(token|secret|password|passwd|authorization|cookie|api[_-]?key|access[_-]?key|refresh[_-]?token|credential)/i;

  const truncateRuntimeBridgeDetail = (value) => {
    if (typeof value !== 'string') {
      return value;
    }
    if (value.length <= RUNTIME_BRIDGE_DETAIL_MAX_LENGTH) {
      return value;
    }
    return `${value.slice(0, RUNTIME_BRIDGE_DETAIL_MAX_LENGTH)}...`;
  };

  const isSensitiveRuntimeBridgeKey = (key) =>
    typeof key === 'string' && RUNTIME_BRIDGE_SENSITIVE_KEY_PATTERN.test(key);

  const summarizeRuntimeBridgeValue = (value, depth = 0) => {
    if (
      value === null ||
      typeof value === 'string' ||
      typeof value === 'number' ||
      typeof value === 'boolean'
    ) {
      return value;
    }

    if (value instanceof Error) {
      return truncateRuntimeBridgeDetail(`${value.name}: ${value.message}`);
    }

    if (typeof value === 'undefined') {
      return '[undefined]';
    }

    if (typeof value === 'function') {
      return '[function]';
    }

    if (typeof value !== 'object') {
      return `[${typeof value}]`;
    }

    if (depth >= 1) {
      return Array.isArray(value) ? `[array:${value.length}]` : '[object]';
    }

    if (Array.isArray(value)) {
      const sample = value
        .slice(0, RUNTIME_BRIDGE_DETAIL_MAX_ITEMS)
        .map((item) => summarizeRuntimeBridgeValue(item, depth + 1));
      if (value.length > sample.length) {
        sample.push(`[+${value.length - sample.length} items]`);
      }
      return sample;
    }

    const keys = Object.keys(value);
    const sampledKeys = keys.slice(0, RUNTIME_BRIDGE_DETAIL_MAX_ITEMS);
    const sampled = {};
    for (const key of sampledKeys) {
      if (isSensitiveRuntimeBridgeKey(key)) {
        sampled[key] = '[redacted]';
        continue;
      }
      sampled[key] = summarizeRuntimeBridgeValue(value[key], depth + 1);
    }
    if (keys.length > sampledKeys.length) {
      sampled.__omittedKeys = keys.length - sampledKeys.length;
    }
    return sampled;
  };

  const stringifyRuntimeBridgeDetail = (value) => {
    if (!value || typeof value !== 'object') {
      return `type=${Object.prototype.toString.call(value)}`;
    }

    try {
      return truncateRuntimeBridgeDetail(
        JSON.stringify(summarizeRuntimeBridgeValue(value)),
      );
    } catch {
      return `type=${Object.prototype.toString.call(value)}`;
    }
  };

  const sanitizeRuntimeBridgeDetail = (detail) => {
    if (detail instanceof Error) {
      return truncateRuntimeBridgeDetail(`${detail.name}: ${detail.message}`);
    }

    if (typeof detail === 'string') {
      return truncateRuntimeBridgeDetail(detail);
    }

    if (typeof detail === 'number' || typeof detail === 'boolean') {
      return String(detail);
    }

    if (detail && typeof detail === 'object') {
      const hasReason = typeof detail.reason === 'string' && detail.reason;
      const hasOk = typeof detail.ok === 'boolean';
      if (hasReason || hasOk) {
        const summary = [];
        if (hasOk) {
          summary.push(`ok=${detail.ok}`);
        }
        if (hasReason) {
          summary.push(`reason=${detail.reason}`);
        }
        return truncateRuntimeBridgeDetail(summary.join(' '));
      }
      return stringifyRuntimeBridgeDetail(detail);
    }

    return String(detail);
  };

  const logRuntimeBridgeFallback = (command, fallbackValue, detail) => {
    if (typeof console !== 'undefined' && typeof console.warn === 'function') {
      const sanitizedDetail = sanitizeRuntimeBridgeDetail(detail);
      console.warn(
        `[astrbotDesktop] ${command} fallback to ${fallbackValue}`,
        sanitizedDetail,
      );
    }
  };

  const getStrictBooleanFallback = (command, fallbackValue) => {
    if (typeof fallbackValue === 'boolean') {
      return fallbackValue;
    }
    if (typeof fallbackValue === 'undefined') {
      return false;
    }
    if (fallbackValue === null) {
      return false;
    }
    if (typeof fallbackValue === 'number') {
      if (fallbackValue === 1) {
        return true;
      }
      if (fallbackValue === 0) {
        return false;
      }
      logRuntimeBridgeFallback(
        command,
        false,
        `invalid numeric fallback (${fallbackValue}), force false`,
      );
      return false;
    }
    if (typeof fallbackValue === 'string') {
      const normalized = fallbackValue.trim().toLowerCase();
      if (RUNTIME_BRIDGE_TRUE_STRINGS.has(normalized)) {
        return true;
      }
      if (RUNTIME_BRIDGE_FALSE_STRINGS.has(normalized)) {
        return false;
      }
      logRuntimeBridgeFallback(
        command,
        false,
        `invalid string fallback (${truncateRuntimeBridgeDetail(fallbackValue)}), force false`,
      );
      return false;
    }

    logRuntimeBridgeFallback(
      command,
      false,
      `invalid fallback type (${typeof fallbackValue}), force false`,
    );
    return false;
  };

  const isRuntimeBridgeEnabled = async (command, fallbackValue) => {
    const normalizedFallback = getStrictBooleanFallback(command, fallbackValue);

    try {
      const result = await invokeBridge(command);
      if (typeof result === 'boolean') {
        return result;
      }
      logRuntimeBridgeFallback(
        command,
        normalizedFallback,
        `non-boolean result: ${String(result)}`,
      );
    } catch (error) {
      logRuntimeBridgeFallback(command, normalizedFallback, error);
    }

    return normalizedFallback;
  };

  const patchLocalStorageTokenSync = () => {
    try {
      const storage = window.localStorage;
      if (!storage || window.__astrbotDesktopTokenSyncPatched) return;
      window.__astrbotDesktopTokenSyncPatched = true;

      const rawSetItem = storage.setItem?.bind(storage);
      const rawRemoveItem = storage.removeItem?.bind(storage);
      const rawClear = storage.clear?.bind(storage);

      if (typeof rawSetItem === 'function') {
        storage.setItem = (key, value) => {
          rawSetItem(key, value);
          if (key === 'token') {
            void syncAuthToken(value);
          }
        };
      }
      if (typeof rawRemoveItem === 'function') {
        storage.removeItem = (key) => {
          rawRemoveItem(key);
          if (key === 'token') {
            void syncAuthToken(null);
          }
        };
      }
      if (typeof rawClear === 'function') {
        storage.clear = () => {
          rawClear();
          void syncAuthToken(null);
        };
      }
    } catch {}
  };

  window.astrbotDesktop = {
    __tauriBridge: true,
    isDesktop: true,
    isDesktopRuntime: () =>
      isRuntimeBridgeEnabled(BRIDGE_COMMANDS.IS_DESKTOP_RUNTIME, true),
    // Legacy aliases for current dashboard compatibility.
    isElectron: true,
    isElectronRuntime: () =>
      isRuntimeBridgeEnabled(BRIDGE_COMMANDS.IS_ELECTRON_RUNTIME, true),
    getBackendState: () => invokeBridge(BRIDGE_COMMANDS.GET_BACKEND_STATE),
    restartBackend: async (authToken = null) => {
      const normalizedToken =
        typeof authToken === 'string' && authToken ? authToken : getStoredAuthToken();
      await syncAuthToken(normalizedToken);
      return invokeBridge(BRIDGE_COMMANDS.RESTART_BACKEND, {
        authToken: normalizedToken
      });
    },
    stopBackend: () => invokeBridge(BRIDGE_COMMANDS.STOP_BACKEND),
    onTrayRestartBackend,
  };

  patchLocalStorageTokenSync();
  void syncAuthToken();
})();
"#;

fn same_origin(left: &Url, right: &Url) -> bool {
    left.scheme() == right.scheme()
        && left.host_str() == right.host_str()
        && left.port_or_known_default() == right.port_or_known_default()
}

fn is_loopback_host(host: Option<&str>) -> bool {
    match host {
        Some("localhost") => true,
        Some(raw) => raw.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback()),
        None => false,
    }
}

fn tray_origin_decision(backend_url: &Url, window_url: &Url) -> TrayOriginDecision {
    if same_origin(backend_url, window_url) {
        return TrayOriginDecision {
            uses_backend_origin: true,
            should_log_mismatch: false,
        };
    }
    let backend_scheme = backend_url.scheme();
    let window_scheme = window_url.scheme();
    if !matches!(backend_scheme, "http" | "https") || !matches!(window_scheme, "http" | "https") {
        return TrayOriginDecision {
            uses_backend_origin: false,
            should_log_mismatch: false,
        };
    }

    let loopback_http =
        is_loopback_host(backend_url.host_str()) && is_loopback_host(window_url.host_str());
    if !loopback_http {
        return TrayOriginDecision {
            uses_backend_origin: false,
            should_log_mismatch: false,
        };
    }

    let same_port = backend_url.port_or_known_default() == window_url.port_or_known_default();
    TrayOriginDecision {
        uses_backend_origin: same_port,
        should_log_mismatch: !same_port,
    }
}

fn should_inject_desktop_bridge(app_handle: &AppHandle, page_url: &Url) -> bool {
    let state = app_handle.state::<BackendState>();
    let Ok(backend_url) = Url::parse(&state.backend_url) else {
        return false;
    };
    same_origin(&backend_url, page_url)
}

fn inject_desktop_bridge(webview: &tauri::Webview<tauri::Wry>) {
    if let Err(error) = webview.eval(DESKTOP_BRIDGE_BOOTSTRAP_SCRIPT) {
        append_desktop_log(&format!("failed to inject desktop bridge script: {error}"));
    }
}

fn should_apply_startup_loading_mode(webview: &tauri::Webview<tauri::Wry>, page_url: &Url) -> bool {
    if webview.window().label() != "main" {
        return false;
    }

    if matches!(page_url.scheme(), "http" | "https") {
        return false;
    }

    let path = page_url.path();
    path == "/" || path == "/index.html"
}

fn apply_startup_loading_mode(webview: &tauri::Webview<tauri::Wry>) {
    let app_handle = webview.app_handle();
    let mode = resolve_startup_loading_mode(app_handle);
    let mode_js = serde_json::to_string(mode).expect("serializing startup mode");
    let script = format!(
        "if (typeof window !== 'undefined' && typeof window.__astrbotSetStartupMode === 'function') {{ window.__astrbotSetStartupMode({mode_js}); }}"
    );
    if let Err(error) = webview.eval(&script) {
        append_desktop_log(&format!("failed to apply startup loading mode: {error}"));
    }
}

fn resolve_startup_loading_mode(app_handle: &AppHandle) -> &'static str {
    let state = app_handle.state::<BackendState>();
    match state.startup_loading_mode.lock() {
        Ok(guard) => {
            if let Some(mode) = *guard {
                return mode;
            }
        }
        Err(error) => {
            append_desktop_log(&format!(
                "startup loading mode cache lock poisoned (read), recomputing mode: {error}"
            ));
        }
    }

    let mode = resolve_startup_loading_mode_uncached(&state, app_handle);
    match state.startup_loading_mode.lock() {
        Ok(mut guard) => {
            *guard = Some(mode);
        }
        Err(error) => {
            append_desktop_log(&format!(
                "startup loading mode cache lock poisoned (write), skip cache update: {error}"
            ));
        }
    }
    mode
}

fn resolve_startup_loading_mode_uncached(
    state: &BackendState,
    app_handle: &AppHandle,
) -> &'static str {
    if let Ok(raw_mode) = env::var(STARTUP_MODE_ENV) {
        let normalized = raw_mode.trim();
        if normalized.eq_ignore_ascii_case(STARTUP_MODE_PANEL_UPDATE) {
            append_desktop_log("startup mode forced to panel-update by env");
            return STARTUP_MODE_PANEL_UPDATE;
        }
        if !normalized.is_empty() && !normalized.eq_ignore_ascii_case(STARTUP_MODE_LOADING) {
            append_desktop_log(&format!(
                "invalid startup mode in {STARTUP_MODE_ENV}: {normalized}, fallback to loading"
            ));
        }
        return STARTUP_MODE_LOADING;
    }

    match state.resolve_launch_plan(app_handle) {
        Ok(plan) => match plan.webui_dir {
            Some(webui_dir) => {
                if !webui_dir.join("index.html").is_file() {
                    append_desktop_log(&format!(
                        "startup mode set to panel-update: webui index is unavailable at {}",
                        webui_dir.display()
                    ));
                    STARTUP_MODE_PANEL_UPDATE
                } else {
                    STARTUP_MODE_LOADING
                }
            }
            None => {
                append_desktop_log(
                    "startup mode set to panel-update: launch plan does not provide webui_dir",
                );
                STARTUP_MODE_PANEL_UPDATE
            }
        },
        Err(error) => {
            append_desktop_log(&format!(
                "failed to resolve startup mode from launch plan, fallback to loading: {error}"
            ));
            STARTUP_MODE_LOADING
        }
    }
}

fn normalize_backend_url(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return DEFAULT_BACKEND_URL.to_string();
    }

    match Url::parse(trimmed) {
        Ok(mut parsed) => {
            if parsed.path().is_empty() {
                parsed.set_path("/");
            }
            parsed.to_string()
        }
        Err(_) => DEFAULT_BACKEND_URL.to_string(),
    }
}

fn workspace_root_dir() -> PathBuf {
    let candidate = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    candidate
        .canonicalize()
        .unwrap_or_else(|_| candidate.to_path_buf())
}

fn detect_astrbot_source_root() -> Option<PathBuf> {
    if let Ok(source_dir) = env::var("ASTRBOT_SOURCE_DIR") {
        let candidate = PathBuf::from(source_dir.trim());
        if candidate.join("main.py").is_file() && candidate.join("astrbot").is_dir() {
            return Some(candidate.canonicalize().unwrap_or(candidate));
        }
    }

    let workspace_root = workspace_root_dir();
    let candidates = [
        workspace_root.join("vendor").join("AstrBot"),
        workspace_root.join("AstrBot"),
        workspace_root,
    ];
    for candidate in candidates {
        if candidate.join("main.py").is_file() && candidate.join("astrbot").is_dir() {
            return Some(candidate.canonicalize().unwrap_or(candidate));
        }
    }
    None
}

fn default_packaged_root_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|home| home.join(".astrbot"))
}

fn resolve_backend_timeout_ms(packaged_mode: bool) -> Option<Duration> {
    let default_timeout_ms = if packaged_mode { 0_u64 } else { 20_000_u64 };
    let parsed_timeout_ms = env::var("ASTRBOT_BACKEND_TIMEOUT_MS")
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .unwrap_or(default_timeout_ms);

    if parsed_timeout_ms > 0 {
        return Some(Duration::from_millis(parsed_timeout_ms));
    }
    if packaged_mode {
        return Some(Duration::from_millis(PACKAGED_BACKEND_TIMEOUT_FALLBACK_MS));
    }
    None
}

fn backend_wait_timeout(packaged_mode: bool) -> Duration {
    resolve_backend_timeout_ms(packaged_mode).unwrap_or(Duration::from_millis(20_000))
}

fn backend_log_path(root_dir: Option<&Path>) -> Option<PathBuf> {
    if let Some(root) = root_dir {
        return Some(root.join("logs").join("backend.log"));
    }
    if let Ok(root) = env::var("ASTRBOT_ROOT") {
        let path = PathBuf::from(root.trim());
        if !path.as_os_str().is_empty() {
            return Some(path.join("logs").join("backend.log"));
        }
    }
    if let Some(root) = default_packaged_root_dir() {
        return Some(root.join("logs").join("backend.log"));
    }
    Some(
        env::temp_dir()
            .join("astrbot")
            .join("logs")
            .join("backend.log"),
    )
}

fn parse_http_json_response(raw: &[u8]) -> Option<serde_json::Value> {
    let (header_text, body_bytes) = parse_http_response_parts(raw)?;
    let status_code = parse_http_status_code_from_headers(&header_text)?;
    if !(200..300).contains(&status_code) {
        return None;
    }

    let is_chunked = header_text.lines().any(|line| {
        let line = line.trim().to_ascii_lowercase();
        line.starts_with("transfer-encoding:") && line.contains("chunked")
    });
    let payload = if is_chunked {
        decode_chunked_body(body_bytes)?
    } else {
        body_bytes.to_vec()
    };

    serde_json::from_slice(&payload).ok()
}

fn parse_http_response_parts(raw: &[u8]) -> Option<(Cow<'_, str>, &[u8])> {
    let header_end = raw.windows(4).position(|window| window == b"\r\n\r\n")?;
    let (header_bytes, body_bytes) = raw.split_at(header_end + 4);
    Some((String::from_utf8_lossy(header_bytes), body_bytes))
}

fn parse_http_status_code_from_headers(header_text: &str) -> Option<u16> {
    header_text
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|code| code.parse::<u16>().ok())
}

fn parse_http_status_code(raw: &[u8]) -> Option<u16> {
    let (header_text, _) = parse_http_response_parts(raw)?;
    parse_http_status_code_from_headers(&header_text)
}

fn decode_chunked_body(mut input: &[u8]) -> Option<Vec<u8>> {
    let mut output = Vec::new();

    loop {
        let header_end = input.windows(2).position(|window| window == b"\r\n")?;
        let chunk_size_line = std::str::from_utf8(&input[..header_end]).ok()?;
        let chunk_size_hex = chunk_size_line.split(';').next()?.trim();
        let chunk_size = usize::from_str_radix(chunk_size_hex, 16).ok()?;
        input = &input[header_end + 2..];

        if chunk_size == 0 {
            return Some(output);
        }
        if input.len() < chunk_size + 2 {
            return None;
        }

        output.extend_from_slice(&input[..chunk_size]);
        if &input[chunk_size..chunk_size + 2] != b"\r\n" {
            return None;
        }
        input = &input[chunk_size + 2..];
    }
}

fn parse_backend_start_time(payload: &serde_json::Value) -> Option<i64> {
    if payload.get("status").and_then(|value| value.as_str()) != Some("ok") {
        return None;
    }
    let start_time = payload.get("data")?.get("start_time")?;
    if let Some(value) = start_time.as_i64() {
        return Some(value);
    }
    start_time
        .as_u64()
        .and_then(|value| i64::try_from(value).ok())
}

fn wait_for_child_exit(child: &mut Child, timeout: Duration) -> bool {
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return true,
            Ok(None) => {
                if start.elapsed() >= timeout {
                    return false;
                }
                thread::sleep(Duration::from_millis(120));
            }
            Err(_) => return false,
        }
    }
}

fn stop_child_process_gracefully(child: &mut Child, timeout: Duration) -> bool {
    #[cfg(target_os = "windows")]
    {
        let _ = Command::new("taskkill")
            .args(["/pid", &child.id().to_string(), "/t"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .stdin(Stdio::null())
            .status();
        return wait_for_child_exit(child, timeout);
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = Command::new("kill")
            .args(["-TERM", &child.id().to_string()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .stdin(Stdio::null())
            .status();
        wait_for_child_exit(child, timeout)
    }
}

fn build_debug_command(plan: &LaunchPlan) -> Vec<String> {
    let mut parts = vec![plan.cmd.clone()];
    parts.extend(plan.args.clone());
    parts
}

fn resolve_resource_path(app: &AppHandle, relative_path: &str) -> Option<PathBuf> {
    if let Ok(path) = app.path().resolve(relative_path, BaseDirectory::Resource) {
        if path.exists() {
            return Some(path);
        }
    }

    let updater_resource = Path::new("_up_").join("resources").join(relative_path);
    if let Ok(path) = app
        .path()
        .resolve(&updater_resource, BaseDirectory::Resource)
    {
        if path.exists() {
            return Some(path);
        }
    }

    append_desktop_log(&format!(
        "resource not found: {} (checked direct and _up_/resources)",
        relative_path
    ));
    None
}

fn parse_ping_timeout_env(raw: &str, env_name: &str, fallback_ms: u64) -> u64 {
    match raw.trim().parse::<u64>() {
        Ok(timeout_ms) if timeout_ms > 0 => timeout_ms,
        _ => {
            append_desktop_log(&format!(
                "invalid {}='{}', fallback to {}ms",
                env_name, raw, fallback_ms
            ));
            fallback_ms
        }
    }
}

fn backend_ping_timeout_ms() -> u64 {
    *BACKEND_PING_TIMEOUT_MS.get_or_init(|| match env::var(BACKEND_PING_TIMEOUT_ENV) {
        Ok(raw) => parse_ping_timeout_env(
            &raw,
            BACKEND_PING_TIMEOUT_ENV,
            DEFAULT_BACKEND_PING_TIMEOUT_MS,
        ),
        Err(_) => DEFAULT_BACKEND_PING_TIMEOUT_MS,
    })
}

fn bridge_backend_ping_timeout_ms() -> u64 {
    *BRIDGE_BACKEND_PING_TIMEOUT_MS.get_or_init(|| {
        let fallback = backend_ping_timeout_ms();
        match env::var(BRIDGE_BACKEND_PING_TIMEOUT_ENV) {
            Ok(raw) => parse_ping_timeout_env(&raw, BRIDGE_BACKEND_PING_TIMEOUT_ENV, fallback),
            Err(_) => fallback,
        }
    })
}

fn desktop_log_path() -> PathBuf {
    if let Ok(custom) = env::var("ASTRBOT_DESKTOP_LOG_PATH") {
        let candidate = PathBuf::from(custom.trim());
        if !candidate.as_os_str().is_empty() {
            return candidate;
        }
    }

    if let Ok(root) = env::var("ASTRBOT_ROOT") {
        let root = PathBuf::from(root.trim());
        if !root.as_os_str().is_empty() {
            return root.join("logs").join(DESKTOP_LOG_FILE);
        }
    }

    if let Some(root) = default_packaged_root_dir() {
        return root.join("logs").join(DESKTOP_LOG_FILE);
    }

    env::temp_dir()
        .join("astrbot")
        .join("logs")
        .join(DESKTOP_LOG_FILE)
}

fn append_desktop_log(message: &str) {
    let path = desktop_log_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _guard = match DESKTOP_LOG_WRITE_LOCK.get_or_init(|| Mutex::new(())).lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    rotate_log_if_needed(
        &path,
        DESKTOP_LOG_MAX_BYTES,
        LOG_BACKUP_COUNT,
        "desktop",
        false,
    );
    let timestamp = chrono::Local::now()
        .format("%Y-%m-%d %H:%M:%S%.3f %z")
        .to_string();
    let line = format!("[{}] {}\n", timestamp, message);
    let _ = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .and_then(|mut file| std::io::Write::write_all(&mut file, line.as_bytes()));
}

fn rotate_log_if_needed(
    path: &Path,
    max_bytes: u64,
    backup_count: usize,
    log_scope: &str,
    copy_and_truncate: bool,
) {
    if max_bytes == 0 || backup_count == 0 {
        return;
    }

    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) => {
            if error.kind() != std::io::ErrorKind::NotFound {
                eprintln!(
                    "[log rotation:{log_scope}] failed to read metadata for {}: {}",
                    path.display(),
                    error
                );
            }
            return;
        }
    };
    if metadata.len() < max_bytes {
        return;
    }

    let oldest = rotated_log_path(path, backup_count);
    if let Err(error) = fs::remove_file(&oldest) {
        if error.kind() != std::io::ErrorKind::NotFound {
            eprintln!(
                "[log rotation:{log_scope}] failed to remove oldest backup {}: {}",
                oldest.display(),
                error
            );
        }
    }

    for index in (1..backup_count).rev() {
        let source = rotated_log_path(path, index);
        if !source.exists() {
            continue;
        }
        let target = rotated_log_path(path, index + 1);
        if let Err(error) = fs::remove_file(&target) {
            if error.kind() != std::io::ErrorKind::NotFound {
                eprintln!(
                    "[log rotation:{log_scope}] failed to remove backup {}: {}",
                    target.display(),
                    error
                );
            }
        }
        if let Err(error) = fs::rename(&source, &target) {
            eprintln!(
                "[log rotation:{log_scope}] failed to rename {} to {}: {}",
                source.display(),
                target.display(),
                error
            );
        }
    }

    let rotated = rotated_log_path(path, 1);
    if let Err(error) = fs::remove_file(&rotated) {
        if error.kind() != std::io::ErrorKind::NotFound {
            eprintln!(
                "[log rotation:{log_scope}] failed to remove first backup {}: {}",
                rotated.display(),
                error
            );
        }
    }

    if copy_and_truncate {
        match fs::copy(path, &rotated) {
            Ok(_) => {
                if let Err(error) = OpenOptions::new().write(true).truncate(true).open(path) {
                    eprintln!(
                        "[log rotation:{log_scope}] failed to truncate active log {}: {}",
                        path.display(),
                        error
                    );
                }
            }
            Err(error) => {
                eprintln!(
                    "[log rotation:{log_scope}] failed to copy {} to {}: {}",
                    path.display(),
                    rotated.display(),
                    error
                );
            }
        }
    } else if let Err(error) = fs::rename(path, &rotated) {
        eprintln!(
            "[log rotation:{log_scope}] failed to rotate {} to {}: {}",
            path.display(),
            rotated.display(),
            error
        );
    }
}

fn rotated_log_path(path: &Path, index: usize) -> PathBuf {
    let mut value = OsString::from(path.as_os_str());
    value.push(format!(".{index}"));
    PathBuf::from(value)
}

fn show_startup_error(app_handle: &AppHandle, message: &str) {
    append_desktop_log(&format!("startup error: {}", message));
    eprintln!("AstrBot startup failed: {message}");
    app_handle.exit(1);
}

fn show_startup_error_on_main_thread(app_handle: &AppHandle, message: &str) {
    let message_owned = message.to_string();
    if let Err(error) =
        run_on_main_thread_dispatch(app_handle, "show startup error", move |main_app| {
            show_startup_error(main_app, &message_owned);
        })
    {
        append_desktop_log(&format!(
            "failed to dispatch startup error handling to main thread: {error}"
        ));
        show_startup_error(app_handle, message);
    }
}

fn run_on_main_thread_dispatch<F>(
    app_handle: &AppHandle,
    action_name: &str,
    action: F,
) -> Result<(), String>
where
    F: FnOnce(&AppHandle) + Send + 'static,
{
    let app_handle_for_main = app_handle.clone();
    app_handle
        .run_on_main_thread(move || {
            action(&app_handle_for_main);
        })
        .map_err(|error| format!("failed to schedule '{action_name}' on main thread: {error}"))
}

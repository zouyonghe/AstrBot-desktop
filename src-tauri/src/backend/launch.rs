use std::{
    env,
    ffi::{OsStr, OsString},
    fs::{self, OpenOptions},
    io,
    path::Path,
    process::{Command, Stdio},
    thread,
    time::Duration,
};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

use serde::Deserialize;
use serde_json::Value;
use tauri::AppHandle;

use crate::{
    append_desktop_log, backend_path_override, build_debug_command, launch_plan, logging,
    runtime_paths, BackendState, BACKEND_LOG_MAX_BYTES, DEFAULT_SHELL_LOCALE, LOG_BACKUP_COUNT,
};
#[cfg(target_os = "windows")]
use crate::{CREATE_NEW_PROCESS_GROUP, CREATE_NO_WINDOW};

const DASHBOARD_HOST_ENV: &str = "DASHBOARD_HOST";
const ASTRBOT_DASHBOARD_HOST_ENV: &str = "ASTRBOT_DASHBOARD_HOST";
const DASHBOARD_PORT_ENV: &str = "DASHBOARD_PORT";
const ASTRBOT_DASHBOARD_PORT_ENV: &str = "ASTRBOT_DASHBOARD_PORT";
const ASTRBOT_DASHBOARD_SKIP_DEFAULT_PASSWORD_AUTH_ENV: &str =
    "ASTRBOT_DASHBOARD_SKIP_DEFAULT_PASSWORD_AUTH";
const DASHBOARD_SKIP_DEFAULT_PASSWORD_AUTH_ENV: &str = "DASHBOARD_SKIP_DEFAULT_PASSWORD_AUTH";
const DEFAULT_DASHBOARD_HOST: &str = "127.0.0.1";
const DEFAULT_DASHBOARD_PORT: &str = "6185";
const CMD_CONFIG_RELATIVE_PATH: &str = "data/cmd_config.json";
const CMD_CONFIG_READ_RETRY_ATTEMPTS: usize = 5;
#[cfg(not(test))]
const CMD_CONFIG_READ_RETRY_DELAY: Duration = Duration::from_millis(100);
#[cfg(test)]
const CMD_CONFIG_READ_RETRY_DELAY: Duration = Duration::from_millis(1);

#[derive(Debug, Default)]
struct CmdDashboardConfig {
    host: Option<String>,
    port: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct CmdConfigFile {
    dashboard: Option<CmdConfigDashboard>,
}

#[derive(Debug, Deserialize)]
struct CmdConfigDashboard {
    host: Option<String>,
    port: Option<Value>,
}

impl CmdDashboardConfig {
    fn from_file_config(config: CmdConfigFile, log: &mut dyn FnMut(&str)) -> Self {
        let Some(dashboard) = config.dashboard else {
            return Self::default();
        };

        let host = dashboard.host.and_then(|host| {
            let trimmed = host.trim();
            if trimmed.is_empty() {
                log(&format!(
                    "cmd_config: ignoring invalid dashboard.host value: {host:?}"
                ));
                None
            } else {
                Some(trimmed.to_string())
            }
        });

        let port = dashboard.port.and_then(|value| match value {
            Value::Number(port) => match port.as_u64() {
                Some(port) if (1..=65535).contains(&port) => Some(port.to_string()),
                Some(port) => {
                    log(&format!(
                        "cmd_config: ignoring invalid dashboard.port value: {port}"
                    ));
                    None
                }
                None => {
                    log("cmd_config: ignoring non-u64 dashboard.port number");
                    None
                }
            },
            Value::String(port) => {
                let trimmed = port.trim();
                match trimmed.parse::<u64>() {
                    Ok(port) if (1..=65535).contains(&port) => Some(port.to_string()),
                    Ok(port) => {
                        log(&format!(
                            "cmd_config: ignoring invalid dashboard.port value: {port}"
                        ));
                        None
                    }
                    Err(_) => {
                        log(&format!(
                            "cmd_config: ignoring invalid dashboard.port value: {port:?}"
                        ));
                        None
                    }
                }
            }
            _ => {
                log("cmd_config: ignoring non-number/string dashboard.port value");
                None
            }
        });

        Self { host, port }
    }
}

fn sanitize_packaged_python_environment<F>(command: &mut Command, log: F)
where
    F: Fn(&str),
{
    for key in ["PYTHONHOME", "PYTHONPATH"] {
        if env::var_os(key).is_some() {
            log(&format!(
                "clearing inherited {} for packaged backend runtime",
                key
            ));
        }
        command.env_remove(key);
    }
    command.env("PYTHONNOUSERSITE", "1");
}

fn configure_desktop_dashboard_environment(
    command: &mut Command,
    root_dir: Option<&Path>,
    log: &mut dyn FnMut(&str),
) {
    let dashboard_host_env = env::var_os(DASHBOARD_HOST_ENV);
    let astrbot_dashboard_host_env = env::var_os(ASTRBOT_DASHBOARD_HOST_ENV);
    let dashboard_port_env = env::var_os(DASHBOARD_PORT_ENV);
    let astrbot_dashboard_port_env = env::var_os(ASTRBOT_DASHBOARD_PORT_ENV);
    let astrbot_skip_auth_env = env::var_os(ASTRBOT_DASHBOARD_SKIP_DEFAULT_PASSWORD_AUTH_ENV);
    let legacy_skip_auth_env = env::var_os(DASHBOARD_SKIP_DEFAULT_PASSWORD_AUTH_ENV);
    let cmd_config = read_cmd_dashboard_config(root_dir, log);
    let (has_host_env, effective_host) = resolve_dashboard_value(
        dashboard_host_env,
        astrbot_dashboard_host_env,
        cmd_config.host,
        DEFAULT_DASHBOARD_HOST,
    );
    let (has_port_env, effective_port) = resolve_dashboard_value(
        dashboard_port_env,
        astrbot_dashboard_port_env,
        cmd_config.port,
        DEFAULT_DASHBOARD_PORT,
    );

    let has_explicit_skip_auth = astrbot_skip_auth_env.is_some() || legacy_skip_auth_env.is_some();

    if !has_host_env {
        command.env(DASHBOARD_HOST_ENV, &effective_host);
    }
    if !has_port_env {
        command.env(DASHBOARD_PORT_ENV, &effective_port);
    }
    if should_skip_default_password_auth(has_explicit_skip_auth, Some(effective_host.as_os_str())) {
        command.env(ASTRBOT_DASHBOARD_SKIP_DEFAULT_PASSWORD_AUTH_ENV, "true");
    }
}

fn resolve_dashboard_value(
    primary_env: Option<OsString>,
    legacy_env: Option<OsString>,
    config: Option<String>,
    default: &str,
) -> (bool, OsString) {
    if let Some(value) =
        non_blank_env_value(primary_env).or_else(|| non_blank_env_value(legacy_env))
    {
        return (true, value);
    }
    let effective = config
        .map(OsString::from)
        .unwrap_or_else(|| OsString::from(default));
    (false, effective)
}

fn non_blank_env_value(value: Option<OsString>) -> Option<OsString> {
    value.filter(|value| {
        value
            .to_str()
            .map(|value| !value.trim().is_empty())
            .unwrap_or(true)
    })
}

fn read_cmd_dashboard_config(
    root_dir: Option<&Path>,
    log: &mut dyn FnMut(&str),
) -> CmdDashboardConfig {
    let Some(root_dir) = root_dir else {
        return CmdDashboardConfig::default();
    };
    let config_path = root_dir.join(CMD_CONFIG_RELATIVE_PATH);
    if !config_path.is_file() {
        return CmdDashboardConfig::default();
    }

    let parsed = match read_cmd_config_file_with_retry(&config_path) {
        Ok(parsed) => parsed,
        Err(error) => {
            match error {
                CmdConfigError::Read(error) => log(&format!(
                    "failed to read cmd_config {}: {}",
                    config_path.display(),
                    error
                )),
                CmdConfigError::Parse { error, .. } => log(&format!(
                    "failed to parse cmd_config {}: {}",
                    config_path.display(),
                    error
                )),
            }
            return CmdDashboardConfig::default();
        }
    };
    CmdDashboardConfig::from_file_config(parsed, log)
}

#[derive(Debug)]
enum CmdConfigError {
    Read(io::Error),
    Parse {
        error: serde_json::Error,
        is_empty: bool,
        is_nonempty_eof: bool,
    },
}

fn is_retryable_cmd_config_io_error(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::NotFound
            | io::ErrorKind::Interrupted
            | io::ErrorKind::WouldBlock
            | io::ErrorKind::TimedOut
    )
}

fn should_retry_cmd_config_read(error: &CmdConfigError) -> bool {
    match error {
        CmdConfigError::Read(error) => is_retryable_cmd_config_io_error(error),
        CmdConfigError::Parse {
            is_empty,
            is_nonempty_eof,
            ..
        } => {
            // Empty files and truncated JSON can occur while cmd_config is being rewritten.
            // Whitespace-only files are non-empty invalid JSON, so treat them as misconfiguration.
            *is_empty || *is_nonempty_eof
        }
    }
}

fn describe_cmd_config_error(error: &CmdConfigError) -> String {
    match error {
        CmdConfigError::Read(error) => format!("read failed: {error}"),
        CmdConfigError::Parse { error, .. } => format!("parse failed: {error}"),
    }
}

fn read_cmd_config_file_once(config_path: &Path) -> Result<CmdConfigFile, CmdConfigError> {
    let mut raw = fs::read_to_string(config_path).map_err(CmdConfigError::Read)?;
    if raw.starts_with('\u{feff}') {
        raw.remove(0);
    }
    match serde_json::from_str(&raw) {
        Ok(parsed) => Ok(parsed),
        Err(error) => {
            let is_empty = raw.is_empty();
            let is_nonempty_eof = error.is_eof() && !raw.trim().is_empty();
            Err(CmdConfigError::Parse {
                error,
                is_empty,
                is_nonempty_eof,
            })
        }
    }
}

fn read_cmd_config_file_with_retry(config_path: &Path) -> Result<CmdConfigFile, CmdConfigError> {
    read_cmd_config_file_with_retry_and_hook(config_path, |_| {})
}

fn read_cmd_config_file_with_retry_and_hook<F>(
    config_path: &Path,
    mut after_retryable_error: F,
) -> Result<CmdConfigFile, CmdConfigError>
where
    F: FnMut(usize),
{
    let mut attempt = 1;
    loop {
        match read_cmd_config_file_once(config_path) {
            Ok(config) => return Ok(config),
            Err(error) => {
                if !should_retry_cmd_config_read(&error)
                    || attempt >= CMD_CONFIG_READ_RETRY_ATTEMPTS
                {
                    return Err(error);
                }
                append_desktop_log(&format!(
                    "retrying cmd_config read {}/{} for {}: {}",
                    attempt,
                    CMD_CONFIG_READ_RETRY_ATTEMPTS,
                    config_path.display(),
                    describe_cmd_config_error(&error)
                ));
                after_retryable_error(attempt - 1);
                thread::sleep(CMD_CONFIG_READ_RETRY_DELAY);
                attempt += 1;
            }
        }
    }
}

fn should_skip_default_password_auth(
    has_explicit_skip_auth: bool,
    effective_host: Option<&OsStr>,
) -> bool {
    if has_explicit_skip_auth {
        return false;
    }

    let Some(effective_host) = effective_host else {
        return true;
    };
    let Some(host) = effective_host.to_str() else {
        return false;
    };

    is_local_dashboard_host(host)
}

fn is_local_dashboard_host(host: &str) -> bool {
    let trimmed = host.trim();
    trimmed.eq_ignore_ascii_case("127.0.0.1")
        || trimmed.eq_ignore_ascii_case("localhost")
        || trimmed.eq_ignore_ascii_case("::1")
}

impl BackendState {
    pub(crate) fn resolve_launch_plan(&self, app: &AppHandle) -> Result<crate::LaunchPlan, String> {
        if let Some(custom_cmd) = env::var("ASTRBOT_BACKEND_CMD")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
        {
            return launch_plan::resolve_custom_launch(custom_cmd);
        }

        if let Some(plan) =
            launch_plan::resolve_packaged_launch(app, DEFAULT_SHELL_LOCALE, append_desktop_log)?
        {
            return Ok(plan);
        }

        launch_plan::resolve_dev_launch()
    }

    pub(crate) fn start_backend_process(
        &self,
        app: &AppHandle,
        plan: &crate::LaunchPlan,
    ) -> Result<(), String> {
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
        if let Some(path_override) = backend_path_override() {
            command.env("PATH", path_override);
        }
        let mut log = |message: &str| append_desktop_log(message);
        configure_desktop_dashboard_environment(&mut command, plan.root_dir.as_deref(), &mut log);
        #[cfg(target_os = "windows")]
        {
            if plan.packaged_mode {
                command.creation_flags(CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP);
            }
        }

        if plan.packaged_mode {
            sanitize_packaged_python_environment(&mut command, append_desktop_log);
            command.env("ASTRBOT_DESKTOP_CLIENT", "1");
        }

        if let Some(root_dir) = &plan.root_dir {
            command.env(crate::ASTRBOT_ROOT_ENV, root_dir);
        }
        if let Some(heartbeat_path) = plan.startup_heartbeat_path.as_ref() {
            command.env(crate::BACKEND_STARTUP_HEARTBEAT_PATH_ENV, heartbeat_path);
        }
        if let Some(webui_dir) = &plan.webui_dir {
            command.env("ASTRBOT_WEBUI_DIR", webui_dir);
        }

        let backend_log_path = Some(logging::resolve_backend_log_path(
            plan.root_dir.as_deref(),
            runtime_paths::default_packaged_root_dir(),
        ));
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
            logging::rotate_log_if_needed(
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
}

#[cfg(test)]
mod tests {
    use std::{
        env,
        ffi::{OsStr, OsString},
        fs,
        path::Path,
        process::Command,
        sync::Mutex,
    };

    #[cfg(unix)]
    use std::os::unix::ffi::OsStringExt;

    use super::{
        configure_desktop_dashboard_environment, read_cmd_config_file_with_retry_and_hook,
        sanitize_packaged_python_environment, ASTRBOT_DASHBOARD_HOST_ENV,
        ASTRBOT_DASHBOARD_PORT_ENV, ASTRBOT_DASHBOARD_SKIP_DEFAULT_PASSWORD_AUTH_ENV,
        CMD_CONFIG_RELATIVE_PATH, DASHBOARD_HOST_ENV, DASHBOARD_PORT_ENV,
        DASHBOARD_SKIP_DEFAULT_PASSWORD_AUTH_ENV, DEFAULT_DASHBOARD_HOST, DEFAULT_DASHBOARD_PORT,
    };

    static ENV_TEST_LOCK: Mutex<()> = Mutex::new(());

    const DASHBOARD_ENV_KEYS: [&str; 6] = [
        ASTRBOT_DASHBOARD_SKIP_DEFAULT_PASSWORD_AUTH_ENV,
        DASHBOARD_SKIP_DEFAULT_PASSWORD_AUTH_ENV,
        DASHBOARD_HOST_ENV,
        ASTRBOT_DASHBOARD_HOST_ENV,
        DASHBOARD_PORT_ENV,
        ASTRBOT_DASHBOARD_PORT_ENV,
    ];

    fn get_command_env_value(command: &Command, key: &str) -> Option<Option<String>> {
        command
            .get_envs()
            .find(|(existing_key, _)| *existing_key == OsStr::new(key))
            .map(|(_, value)| value.map(|v| v.to_string_lossy().into_owned()))
    }

    struct DashboardEnvGuard {
        saved: [Option<OsString>; 6],
    }

    impl DashboardEnvGuard {
        fn new() -> Self {
            Self {
                saved: DASHBOARD_ENV_KEYS.map(env::var_os),
            }
        }

        fn clear() {
            for key in DASHBOARD_ENV_KEYS {
                env::remove_var(key);
            }
        }
    }

    impl Drop for DashboardEnvGuard {
        fn drop(&mut self) {
            for (key, value) in DASHBOARD_ENV_KEYS.iter().zip(self.saved.iter()) {
                match value {
                    Some(value) => env::set_var(key, value),
                    None => env::remove_var(key),
                }
            }
        }
    }

    fn with_clean_dashboard_env<F>(test: F)
    where
        F: FnOnce(),
    {
        let _lock = ENV_TEST_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let _guard = DashboardEnvGuard::new();

        DashboardEnvGuard::clear();
        test();
    }

    #[test]
    fn sanitize_packaged_python_environment_marks_python_envs_for_removal() {
        let mut command = Command::new("sh");
        command.env("PYTHONHOME", "/tmp/fake-python-home");
        command.env("PYTHONPATH", "/tmp/fake-python-path");

        sanitize_packaged_python_environment(&mut command, |_| {});

        assert_eq!(get_command_env_value(&command, "PYTHONHOME"), Some(None));
        assert_eq!(get_command_env_value(&command, "PYTHONPATH"), Some(None));
    }

    #[test]
    fn sanitize_packaged_python_environment_disables_user_site_packages() {
        let mut command = Command::new("sh");

        sanitize_packaged_python_environment(&mut command, |_| {});

        assert_eq!(
            get_command_env_value(&command, "PYTHONNOUSERSITE"),
            Some(Some("1".to_string()))
        );
    }

    #[test]
    fn configure_desktop_dashboard_environment_enables_local_setup_without_default_password() {
        with_clean_dashboard_env(|| {
            let mut command = Command::new("sh");

            configure_desktop_dashboard_environment(&mut command, None, &mut |_| {});

            assert_eq!(
                get_command_env_value(&command, ASTRBOT_DASHBOARD_SKIP_DEFAULT_PASSWORD_AUTH_ENV),
                Some(Some("true".to_string()))
            );
            assert_eq!(
                get_command_env_value(&command, DASHBOARD_HOST_ENV),
                Some(Some(DEFAULT_DASHBOARD_HOST.to_string()))
            );
        });
    }

    #[test]
    fn configure_desktop_dashboard_environment_preserves_explicit_dashboard_env() {
        with_clean_dashboard_env(|| {
            env::set_var(ASTRBOT_DASHBOARD_SKIP_DEFAULT_PASSWORD_AUTH_ENV, "false");
            env::set_var(DASHBOARD_HOST_ENV, "localhost");
            env::set_var(DASHBOARD_PORT_ENV, "7000");
            let mut command = Command::new("sh");

            configure_desktop_dashboard_environment(&mut command, None, &mut |_| {});

            assert_eq!(
                get_command_env_value(&command, ASTRBOT_DASHBOARD_SKIP_DEFAULT_PASSWORD_AUTH_ENV),
                None
            );
            assert_eq!(get_command_env_value(&command, DASHBOARD_HOST_ENV), None);
            assert_eq!(get_command_env_value(&command, DASHBOARD_PORT_ENV), None);
        });
    }

    #[test]
    fn configure_desktop_dashboard_environment_does_not_skip_auth_for_remote_host() {
        with_clean_dashboard_env(|| {
            env::set_var(DASHBOARD_HOST_ENV, "0.0.0.0");
            let mut command = Command::new("sh");

            configure_desktop_dashboard_environment(&mut command, None, &mut |_| {});

            assert_eq!(
                get_command_env_value(&command, ASTRBOT_DASHBOARD_SKIP_DEFAULT_PASSWORD_AUTH_ENV),
                None
            );
        });
    }

    #[cfg(unix)]
    #[test]
    fn configure_desktop_dashboard_environment_does_not_skip_auth_for_non_utf8_host() {
        with_clean_dashboard_env(|| {
            env::set_var(DASHBOARD_HOST_ENV, std::ffi::OsString::from_vec(vec![0xff]));
            let mut command = Command::new("sh");

            configure_desktop_dashboard_environment(&mut command, None, &mut |_| {});

            assert_eq!(
                get_command_env_value(&command, ASTRBOT_DASHBOARD_SKIP_DEFAULT_PASSWORD_AUTH_ENV),
                None
            );
            assert_eq!(get_command_env_value(&command, DASHBOARD_HOST_ENV), None);
        });
    }

    fn write_cmd_config(root: &Path, contents: &str) {
        let config_path = root.join(CMD_CONFIG_RELATIVE_PATH);
        fs::create_dir_all(config_path.parent().expect("config parent"))
            .expect("create config dir");
        fs::write(config_path, contents).expect("write cmd config");
    }

    #[test]
    fn configure_desktop_dashboard_environment_reads_cmd_config() {
        with_clean_dashboard_env(|| {
            let root = tempfile::tempdir().expect("temp root");
            write_cmd_config(
                root.path(),
                r#"{"dashboard":{"host":"0.0.0.0","port":6185}}"#,
            );
            let mut command = Command::new("sh");

            configure_desktop_dashboard_environment(&mut command, Some(root.path()), &mut |_| {});

            assert_eq!(
                get_command_env_value(&command, DASHBOARD_HOST_ENV),
                Some(Some("0.0.0.0".to_string()))
            );
            assert_eq!(
                get_command_env_value(&command, DASHBOARD_PORT_ENV),
                Some(Some("6185".to_string()))
            );
            assert_eq!(
                get_command_env_value(&command, ASTRBOT_DASHBOARD_SKIP_DEFAULT_PASSWORD_AUTH_ENV),
                None
            );
        });
    }

    #[test]
    fn configure_desktop_dashboard_environment_accepts_utf8_bom_cmd_config() {
        with_clean_dashboard_env(|| {
            let root = tempfile::tempdir().expect("temp root");
            write_cmd_config(
                root.path(),
                "\u{feff}{\"dashboard\":{\"host\":\"0.0.0.0\",\"port\":6185}}",
            );
            let mut command = Command::new("sh");

            configure_desktop_dashboard_environment(&mut command, Some(root.path()), &mut |_| {});

            assert_eq!(
                get_command_env_value(&command, DASHBOARD_HOST_ENV),
                Some(Some("0.0.0.0".to_string()))
            );
            assert_eq!(
                get_command_env_value(&command, DASHBOARD_PORT_ENV),
                Some(Some("6185".to_string()))
            );
        });
    }

    #[test]
    fn configure_desktop_dashboard_environment_retries_transient_empty_cmd_config() {
        with_clean_dashboard_env(|| {
            let root = tempfile::tempdir().expect("temp root");
            write_cmd_config(root.path(), "");
            let config_path = root.path().join(CMD_CONFIG_RELATIVE_PATH);
            let mut retry_count = 0;

            let config = read_cmd_config_file_with_retry_and_hook(&config_path, |attempt| {
                assert_eq!(attempt, 0);
                retry_count += 1;
                fs::write(
                    &config_path,
                    r#"{"dashboard":{"host":"0.0.0.0","port":6185}}"#,
                )
                .expect("write completed cmd config");
            })
            .expect("retry reads completed cmd config");
            let parsed = super::CmdDashboardConfig::from_file_config(config, &mut |_| {});

            assert_eq!(retry_count, 1);
            assert_eq!(parsed.host.as_deref(), Some("0.0.0.0"));
            assert_eq!(parsed.port.as_deref(), Some("6185"));
        });
    }

    #[test]
    fn configure_desktop_dashboard_environment_retries_transient_missing_cmd_config() {
        with_clean_dashboard_env(|| {
            let root = tempfile::tempdir().expect("temp root");
            write_cmd_config(root.path(), r#"{"dashboard":{"host":"127.0.0.1"}}"#);
            let config_path = root.path().join(CMD_CONFIG_RELATIVE_PATH);
            fs::remove_file(&config_path).expect("remove cmd config during rewrite");
            let mut retry_count = 0;

            let config = read_cmd_config_file_with_retry_and_hook(&config_path, |attempt| {
                assert_eq!(attempt, 0);
                retry_count += 1;
                fs::write(
                    &config_path,
                    r#"{"dashboard":{"host":"0.0.0.0","port":6185}}"#,
                )
                .expect("write completed cmd config");
            })
            .expect("retry reads completed cmd config");
            let parsed = super::CmdDashboardConfig::from_file_config(config, &mut |_| {});

            assert_eq!(retry_count, 1);
            assert_eq!(parsed.host.as_deref(), Some("0.0.0.0"));
            assert_eq!(parsed.port.as_deref(), Some("6185"));
        });
    }

    #[test]
    fn configure_desktop_dashboard_environment_does_not_retry_whitespace_only_cmd_config() {
        with_clean_dashboard_env(|| {
            let root = tempfile::tempdir().expect("temp root");
            write_cmd_config(root.path(), "\n\n  ");
            let config_path = root.path().join(CMD_CONFIG_RELATIVE_PATH);
            let mut retry_count = 0;
            let mut command = Command::new("sh");

            configure_desktop_dashboard_environment(&mut command, Some(root.path()), &mut |_| {});
            let result = read_cmd_config_file_with_retry_and_hook(&config_path, |_| {
                retry_count += 1;
            });

            assert_eq!(
                get_command_env_value(&command, DASHBOARD_HOST_ENV),
                Some(Some(DEFAULT_DASHBOARD_HOST.to_string()))
            );
            assert_eq!(
                get_command_env_value(&command, DASHBOARD_PORT_ENV),
                Some(Some(DEFAULT_DASHBOARD_PORT.to_string()))
            );
            assert!(result.is_err());
            assert_eq!(retry_count, 0);
        });
    }

    #[test]
    fn configure_desktop_dashboard_environment_env_overrides_cmd_config() {
        with_clean_dashboard_env(|| {
            let root = tempfile::tempdir().expect("temp root");
            write_cmd_config(
                root.path(),
                r#"{"dashboard":{"host":"0.0.0.0","port":6185}}"#,
            );
            env::set_var(DASHBOARD_HOST_ENV, "localhost");
            env::set_var(DASHBOARD_PORT_ENV, "7000");
            let mut command = Command::new("sh");

            configure_desktop_dashboard_environment(&mut command, Some(root.path()), &mut |_| {});

            assert_eq!(get_command_env_value(&command, DASHBOARD_HOST_ENV), None);
            assert_eq!(get_command_env_value(&command, DASHBOARD_PORT_ENV), None);
        });
    }

    #[test]
    fn configure_desktop_dashboard_environment_ignores_blank_dashboard_env() {
        with_clean_dashboard_env(|| {
            let root = tempfile::tempdir().expect("temp root");
            write_cmd_config(
                root.path(),
                r#"{"dashboard":{"host":"0.0.0.0","port":"7000"}}"#,
            );
            env::set_var(DASHBOARD_HOST_ENV, "  ");
            env::set_var(DASHBOARD_PORT_ENV, "");
            let mut command = Command::new("sh");

            configure_desktop_dashboard_environment(&mut command, Some(root.path()), &mut |_| {});

            assert_eq!(
                get_command_env_value(&command, DASHBOARD_HOST_ENV),
                Some(Some("0.0.0.0".to_string()))
            );
            assert_eq!(
                get_command_env_value(&command, DASHBOARD_PORT_ENV),
                Some(Some("7000".to_string()))
            );
            assert_eq!(
                get_command_env_value(&command, ASTRBOT_DASHBOARD_SKIP_DEFAULT_PASSWORD_AUTH_ENV),
                None
            );
        });
    }

    #[test]
    fn configure_desktop_dashboard_environment_ignores_invalid_cmd_config() {
        with_clean_dashboard_env(|| {
            let root = tempfile::tempdir().expect("temp root");
            write_cmd_config(root.path(), r#"{"dashboard":{"host":" ","port":70000}}"#);
            let mut command = Command::new("sh");
            let mut logs = Vec::new();

            configure_desktop_dashboard_environment(
                &mut command,
                Some(root.path()),
                &mut |message| logs.push(message.to_string()),
            );

            assert_eq!(
                get_command_env_value(&command, DASHBOARD_HOST_ENV),
                Some(Some(DEFAULT_DASHBOARD_HOST.to_string()))
            );
            assert_eq!(
                get_command_env_value(&command, DASHBOARD_PORT_ENV),
                Some(Some(DEFAULT_DASHBOARD_PORT.to_string()))
            );
            assert!(logs
                .iter()
                .any(|message| message.contains("invalid dashboard.host")));
            assert!(logs
                .iter()
                .any(|message| message.contains("invalid dashboard.port")));
        });
    }
}

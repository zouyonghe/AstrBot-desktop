use std::{
    env,
    ffi::OsStr,
    fs::{self, OpenOptions},
    process::{Command, Stdio},
};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

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
const DEFAULT_DASHBOARD_HOST: &str = "0.0.0.0";
const DEFAULT_DASHBOARD_PORT: &str = "6185";

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

fn configure_desktop_dashboard_environment(command: &mut Command) {
    let dashboard_host_env = env::var_os(DASHBOARD_HOST_ENV);
    let astrbot_dashboard_host_env = env::var_os(ASTRBOT_DASHBOARD_HOST_ENV);
    let dashboard_port_env = env::var_os(DASHBOARD_PORT_ENV);
    let astrbot_dashboard_port_env = env::var_os(ASTRBOT_DASHBOARD_PORT_ENV);
    let astrbot_skip_auth_env = env::var_os(ASTRBOT_DASHBOARD_SKIP_DEFAULT_PASSWORD_AUTH_ENV);
    let legacy_skip_auth_env = env::var_os(DASHBOARD_SKIP_DEFAULT_PASSWORD_AUTH_ENV);

    let default_dashboard_host = OsStr::new(DEFAULT_DASHBOARD_HOST);
    let effective_host = dashboard_host_env
        .as_deref()
        .or(astrbot_dashboard_host_env.as_deref())
        .or(Some(default_dashboard_host));
    let has_explicit_skip_auth = astrbot_skip_auth_env.is_some() || legacy_skip_auth_env.is_some();

    if dashboard_host_env.is_none() && astrbot_dashboard_host_env.is_none() {
        command.env(DASHBOARD_HOST_ENV, DEFAULT_DASHBOARD_HOST);
    }
    if dashboard_port_env.is_none() && astrbot_dashboard_port_env.is_none() {
        command.env(DASHBOARD_PORT_ENV, DEFAULT_DASHBOARD_PORT);
    }
    if should_skip_default_password_auth(has_explicit_skip_auth, effective_host) {
        command.env(ASTRBOT_DASHBOARD_SKIP_DEFAULT_PASSWORD_AUTH_ENV, "true");
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
        configure_desktop_dashboard_environment(&mut command);
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
        process::Command,
        sync::Mutex,
    };

    #[cfg(unix)]
    use std::os::unix::ffi::OsStringExt;

    use super::{
        configure_desktop_dashboard_environment, sanitize_packaged_python_environment,
        ASTRBOT_DASHBOARD_HOST_ENV, ASTRBOT_DASHBOARD_PORT_ENV,
        ASTRBOT_DASHBOARD_SKIP_DEFAULT_PASSWORD_AUTH_ENV, DASHBOARD_HOST_ENV, DASHBOARD_PORT_ENV,
        DASHBOARD_SKIP_DEFAULT_PASSWORD_AUTH_ENV, DEFAULT_DASHBOARD_HOST,
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
    fn configure_desktop_dashboard_environment_uses_lan_accessible_host_by_default() {
        with_clean_dashboard_env(|| {
            let mut command = Command::new("sh");

            configure_desktop_dashboard_environment(&mut command);

            assert_eq!(
                get_command_env_value(&command, DASHBOARD_HOST_ENV),
                Some(Some(DEFAULT_DASHBOARD_HOST.to_string()))
            );
            assert_eq!(
                get_command_env_value(&command, ASTRBOT_DASHBOARD_SKIP_DEFAULT_PASSWORD_AUTH_ENV),
                None
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

            configure_desktop_dashboard_environment(&mut command);

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

            configure_desktop_dashboard_environment(&mut command);

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

            configure_desktop_dashboard_environment(&mut command);

            assert_eq!(
                get_command_env_value(&command, ASTRBOT_DASHBOARD_SKIP_DEFAULT_PASSWORD_AUTH_ENV),
                None
            );
            assert_eq!(get_command_env_value(&command, DASHBOARD_HOST_ENV), None);
        });
    }
}

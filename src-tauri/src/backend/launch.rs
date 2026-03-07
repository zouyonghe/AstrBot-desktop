use std::{
    env,
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
        #[cfg(target_os = "windows")]
        {
            if plan.packaged_mode {
                command.creation_flags(CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP);
            }
        }

        if plan.packaged_mode {
            sanitize_packaged_python_environment(&mut command, append_desktop_log);
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
    use std::{ffi::OsStr, process::Command};

    use super::sanitize_packaged_python_environment;

    fn get_command_env_value(command: &Command, key: &str) -> Option<Option<String>> {
        command
            .get_envs()
            .find(|(existing_key, _)| *existing_key == OsStr::new(key))
            .map(|(_, value)| value.map(|v| v.to_string_lossy().into_owned()))
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
}

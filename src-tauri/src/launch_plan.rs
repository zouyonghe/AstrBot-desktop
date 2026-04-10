use std::{
    env, fs,
    path::{Path, PathBuf},
};

use tauri::AppHandle;

use crate::{backend, packaged_webui, runtime_paths, LaunchPlan, RuntimeManifest};

const BACKEND_RESOURCE_ALIAS: &str = env!("ASTRBOT_BACKEND_RESOURCE_ALIAS");
const WEBUI_RESOURCE_ALIAS: &str = env!("ASTRBOT_WEBUI_RESOURCE_ALIAS");

fn build_packaged_resource_relative_path(resource_alias: &str, leaf_name: &str) -> PathBuf {
    PathBuf::from(resource_alias).join(leaf_name)
}

fn resolve_launch_startup_heartbeat_path(
    root_dir: Option<&Path>,
    packaged_mode: bool,
) -> Option<PathBuf> {
    backend::config::resolve_backend_startup_heartbeat_path(
        root_dir,
        packaged_mode
            .then(runtime_paths::default_packaged_root_dir)
            .flatten(),
        crate::DEFAULT_BACKEND_STARTUP_HEARTBEAT_RELATIVE_PATH,
    )
}

pub fn resolve_custom_launch(custom_cmd: String) -> Result<LaunchPlan, String> {
    let mut pieces = shlex::split(&custom_cmd)
        .ok_or_else(|| format!("Invalid ASTRBOT_BACKEND_CMD: {custom_cmd}"))?;
    if pieces.is_empty() {
        return Err("ASTRBOT_BACKEND_CMD is empty.".to_string());
    }

    let cmd = pieces.remove(0);
    let cwd = env::var("ASTRBOT_BACKEND_CWD")
        .map(PathBuf::from)
        .ok()
        .or_else(runtime_paths::detect_astrbot_source_root)
        .unwrap_or_else(runtime_paths::workspace_root_dir);
    let root_dir = env::var(crate::ASTRBOT_ROOT_ENV).ok().map(PathBuf::from);
    let webui_dir = env::var("ASTRBOT_WEBUI_DIR").ok().map(PathBuf::from);
    let startup_heartbeat_path = resolve_launch_startup_heartbeat_path(root_dir.as_deref(), false);

    Ok(LaunchPlan {
        cmd,
        args: pieces,
        cwd,
        root_dir,
        webui_dir,
        startup_heartbeat_path,
        packaged_mode: false,
    })
}

pub fn resolve_packaged_launch<F>(
    app: &AppHandle,
    default_shell_locale: &'static str,
    log: F,
) -> Result<Option<LaunchPlan>, String>
where
    F: Fn(&str) + Copy,
{
    let manifest_relative_path =
        build_packaged_resource_relative_path(BACKEND_RESOURCE_ALIAS, "runtime-manifest.json");
    let manifest_relative_path_string = manifest_relative_path.to_string_lossy().to_string();
    let manifest_path =
        match runtime_paths::resolve_resource_path(app, &manifest_relative_path_string, log) {
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

    let root_dir = env::var(crate::ASTRBOT_ROOT_ENV)
        .map(PathBuf::from)
        .ok()
        .or_else(runtime_paths::default_packaged_root_dir);
    let cwd = env::var("ASTRBOT_BACKEND_CWD")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            root_dir
                .clone()
                .unwrap_or_else(|| backend_dir.to_path_buf())
        });
    let embedded_webui_dir = env::var("ASTRBOT_WEBUI_DIR")
        .ok()
        .map(PathBuf::from)
        .or_else(|| {
            let webui_index_relative_path =
                build_packaged_resource_relative_path(WEBUI_RESOURCE_ALIAS, "index.html");
            let webui_index_relative_path_string =
                webui_index_relative_path.to_string_lossy().to_string();
            runtime_paths::resolve_resource_path(app, &webui_index_relative_path_string, log)
                .and_then(|index_path| index_path.parent().map(Path::to_path_buf))
        });
    let webui_dir = packaged_webui::resolve_packaged_webui_dir(
        embedded_webui_dir,
        root_dir.as_deref(),
        default_shell_locale,
        log,
    )?;

    let args = vec![
        launch_script_path.to_string_lossy().to_string(),
        "--webui-dir".to_string(),
        webui_dir.to_string_lossy().to_string(),
    ];
    let startup_heartbeat_path = resolve_launch_startup_heartbeat_path(root_dir.as_deref(), true);

    let plan = LaunchPlan {
        cmd: python_path.to_string_lossy().to_string(),
        args,
        cwd,
        root_dir,
        webui_dir: Some(webui_dir),
        startup_heartbeat_path,
        packaged_mode: true,
    };
    Ok(Some(plan))
}

pub fn resolve_dev_launch() -> Result<LaunchPlan, String> {
    let source_root = runtime_paths::detect_astrbot_source_root().ok_or_else(|| {
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
    let root_dir = env::var(crate::ASTRBOT_ROOT_ENV).ok().map(PathBuf::from);
    let startup_heartbeat_path = resolve_launch_startup_heartbeat_path(root_dir.as_deref(), false);

    Ok(LaunchPlan {
        cmd: "uv".to_string(),
        args,
        cwd: env::var("ASTRBOT_BACKEND_CWD")
            .map(PathBuf::from)
            .unwrap_or(source_root),
        root_dir,
        webui_dir,
        startup_heartbeat_path,
        packaged_mode: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    struct EnvVarGuard {
        key: &'static str,
        previous: Option<String>,
    }

    impl EnvVarGuard {
        fn set(key: &'static str, value: &str) -> Self {
            let previous = env::var(key).ok();
            env::set_var(key, value);
            Self { key, previous }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            match &self.previous {
                Some(value) => env::set_var(self.key, value),
                None => env::remove_var(self.key),
            }
        }
    }

    #[test]
    fn build_packaged_resource_relative_path_joins_alias_and_leaf_name() {
        assert_eq!(
            build_packaged_resource_relative_path("runtime/backend", "runtime-manifest.json"),
            PathBuf::from("runtime/backend").join("runtime-manifest.json")
        );
        assert_eq!(
            build_packaged_resource_relative_path("runtime/webui", "index.html"),
            PathBuf::from("runtime/webui").join("index.html")
        );
    }

    #[test]
    fn resolve_custom_launch_sets_startup_heartbeat_path_from_root_dir() {
        let _root_guard = EnvVarGuard::set(crate::ASTRBOT_ROOT_ENV, "/tmp/astrbot-root");

        let plan = resolve_custom_launch("python main.py".to_string()).expect("custom plan");

        assert_eq!(
            plan.startup_heartbeat_path,
            Some(PathBuf::from("/tmp/astrbot-root").join("data/backend-startup-heartbeat.json"))
        );
    }
}

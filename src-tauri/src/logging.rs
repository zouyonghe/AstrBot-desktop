use std::{
    env,
    ffi::OsString,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DesktopLogCategory {
    Startup,
    Runtime,
    Restart,
    Shutdown,
}

impl DesktopLogCategory {
    fn as_label(self) -> &'static str {
        match self {
            Self::Startup => "startup",
            Self::Runtime => "runtime",
            Self::Restart => "restart",
            Self::Shutdown => "shutdown",
        }
    }
}

pub fn rotate_log_if_needed(
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

pub fn resolve_desktop_log_path(packaged_root: Option<PathBuf>, desktop_log_file: &str) -> PathBuf {
    if let Ok(custom) = env::var("ASTRBOT_DESKTOP_LOG_PATH") {
        let candidate = PathBuf::from(custom.trim());
        if !candidate.as_os_str().is_empty() {
            return candidate;
        }
    }

    if let Ok(root) = env::var(crate::ASTRBOT_ROOT_ENV) {
        let root = PathBuf::from(root.trim());
        if !root.as_os_str().is_empty() {
            return root.join("logs").join(desktop_log_file);
        }
    }

    if let Some(root) = packaged_root {
        return root.join("logs").join(desktop_log_file);
    }

    env::temp_dir()
        .join("astrbot")
        .join("logs")
        .join(desktop_log_file)
}

pub fn resolve_backend_log_path(
    root_dir: Option<&Path>,
    packaged_root: Option<PathBuf>,
) -> PathBuf {
    if let Some(root) = root_dir {
        return root.join("logs").join("backend.log");
    }
    if let Ok(root) = env::var(crate::ASTRBOT_ROOT_ENV) {
        let path = PathBuf::from(root.trim());
        if !path.as_os_str().is_empty() {
            return path.join("logs").join("backend.log");
        }
    }
    if let Some(root) = packaged_root {
        return root.join("logs").join("backend.log");
    }

    env::temp_dir()
        .join("astrbot")
        .join("logs")
        .join("backend.log")
}

pub fn append_desktop_log(
    category: DesktopLogCategory,
    message: &str,
    packaged_root: Option<PathBuf>,
    desktop_log_file: &str,
    max_bytes: u64,
    backup_count: usize,
    write_lock: &OnceLock<Mutex<()>>,
) {
    let path = resolve_desktop_log_path(packaged_root, desktop_log_file);
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _guard = match write_lock.get_or_init(|| Mutex::new(())).lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    rotate_log_if_needed(&path, max_bytes, backup_count, "desktop", false);
    let timestamp = chrono::Local::now()
        .format("%Y-%m-%d %H:%M:%S%.3f %z")
        .to_string();
    let line = format!("[{}] [{}] {}\n", timestamp, category.as_label(), message);
    let _ = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .and_then(|mut file| file.write_all(line.as_bytes()));
}

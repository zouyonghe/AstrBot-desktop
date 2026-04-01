use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DesktopUpdateMode {
    NativeUpdater,
    ManualDownload,
    Unsupported,
}

const PORTABLE_RUNTIME_MARKER: &str = env!("ASTRBOT_PORTABLE_RUNTIME_MARKER");

fn resolve_desktop_update_mode_for_target(
    target_os: &str,
    has_linux_appimage_runtime: bool,
    has_windows_portable_runtime: bool,
) -> DesktopUpdateMode {
    match target_os {
        "windows" => {
            if has_windows_portable_runtime {
                DesktopUpdateMode::ManualDownload
            } else {
                DesktopUpdateMode::NativeUpdater
            }
        }
        "macos" => DesktopUpdateMode::NativeUpdater,
        "linux" => {
            if has_linux_appimage_runtime {
                DesktopUpdateMode::NativeUpdater
            } else {
                DesktopUpdateMode::ManualDownload
            }
        }
        _ => DesktopUpdateMode::Unsupported,
    }
}

pub(crate) fn is_linux_appimage_runtime() -> bool {
    std::env::var_os("APPIMAGE").is_some() || std::env::var_os("APPDIR").is_some()
}

fn is_windows_portable_runtime_with_exe_dir(exe_dir: Option<&Path>) -> bool {
    exe_dir
        .map(|dir| dir.join(PORTABLE_RUNTIME_MARKER).is_file())
        .unwrap_or(false)
}

pub(crate) fn is_windows_portable_runtime() -> bool {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf));
    is_windows_portable_runtime_with_exe_dir(exe_dir.as_deref())
}

fn windows_portable_runtime_for_target(target_os: &str) -> bool {
    if target_os == "windows" {
        is_windows_portable_runtime()
    } else {
        false
    }
}

pub(crate) fn resolve_desktop_update_mode() -> DesktopUpdateMode {
    let target_os = if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else {
        "other"
    };
    resolve_desktop_update_mode_for_target(
        target_os,
        is_linux_appimage_runtime(),
        windows_portable_runtime_for_target(target_os),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn resolve_desktop_update_mode_for_target_maps_platforms() {
        assert_eq!(
            resolve_desktop_update_mode_for_target("windows", false, false),
            DesktopUpdateMode::NativeUpdater
        );
        assert_eq!(
            resolve_desktop_update_mode_for_target("windows", false, true),
            DesktopUpdateMode::ManualDownload
        );
        assert_eq!(
            resolve_desktop_update_mode_for_target("macos", false, false),
            DesktopUpdateMode::NativeUpdater
        );
        assert_eq!(
            resolve_desktop_update_mode_for_target("linux", true, false),
            DesktopUpdateMode::NativeUpdater
        );
        assert_eq!(
            resolve_desktop_update_mode_for_target("linux", false, false),
            DesktopUpdateMode::ManualDownload
        );
        assert_eq!(
            resolve_desktop_update_mode_for_target("freebsd", false, false),
            DesktopUpdateMode::Unsupported
        );
    }

    #[test]
    fn is_windows_portable_runtime_with_exe_dir_detects_marker_file() {
        let dir = TempDir::with_prefix("portable-marker").expect("create temp case dir");
        fs::write(dir.path().join(PORTABLE_RUNTIME_MARKER), b"").expect("write marker");

        assert!(is_windows_portable_runtime_with_exe_dir(Some(dir.path())));
    }

    #[test]
    fn windows_portable_runtime_for_target_skips_detection_on_non_windows() {
        assert!(!windows_portable_runtime_for_target("linux"));
        assert!(!windows_portable_runtime_for_target("freebsd"));
    }
}

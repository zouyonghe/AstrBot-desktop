#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DesktopUpdateMode {
    NativeUpdater,
    ManualDownload,
    Unsupported,
}

fn resolve_desktop_update_mode_for_target(
    target_os: &str,
    has_linux_appimage_runtime: bool,
) -> DesktopUpdateMode {
    match target_os {
        "windows" | "macos" => DesktopUpdateMode::NativeUpdater,
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
    resolve_desktop_update_mode_for_target(target_os, is_linux_appimage_runtime())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_desktop_update_mode_for_target_maps_platforms() {
        assert_eq!(
            resolve_desktop_update_mode_for_target("windows", false),
            DesktopUpdateMode::NativeUpdater
        );
        assert_eq!(
            resolve_desktop_update_mode_for_target("macos", false),
            DesktopUpdateMode::NativeUpdater
        );
        assert_eq!(
            resolve_desktop_update_mode_for_target("linux", true),
            DesktopUpdateMode::NativeUpdater
        );
        assert_eq!(
            resolve_desktop_update_mode_for_target("linux", false),
            DesktopUpdateMode::ManualDownload
        );
        assert_eq!(
            resolve_desktop_update_mode_for_target("freebsd", false),
            DesktopUpdateMode::Unsupported
        );
    }
}

pub(crate) const DESKTOP_UPDATER_UNSUPPORTED_REASON: &str =
    "Desktop app updater is not available on this platform yet.";
pub(crate) const DESKTOP_UPDATER_MANUAL_DOWNLOAD_REASON: &str =
    "This Linux installation method does not support automatic updates. Please download the latest package from your installation source.";
const DEFAULT_DESKTOP_UPDATER_MANUAL_DOWNLOAD_URL: &str =
    "https://github.com/AstrBotDevs/AstrBot-desktop/releases/latest";

pub(crate) fn resolve_desktop_manual_download_url() -> String {
    std::env::var("ASTRBOT_DESKTOP_MANUAL_DOWNLOAD_URL")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| DEFAULT_DESKTOP_UPDATER_MANUAL_DOWNLOAD_URL.to_string())
}

pub(crate) fn desktop_manual_download_reason() -> String {
    format!(
        "{DESKTOP_UPDATER_MANUAL_DOWNLOAD_REASON} {}",
        resolve_desktop_manual_download_url()
    )
}

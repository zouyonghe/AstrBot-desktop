use serde::Serialize;

use crate::update_channel::UpdateChannel;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DesktopAppUpdateCheckResult {
    pub ok: bool,
    pub reason: Option<String>,
    pub current_version: Option<String>,
    pub latest_version: Option<String>,
    pub has_update: bool,
    pub manual_download_required: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DesktopAppUpdateResult {
    pub ok: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DesktopAppUpdateChannelResult {
    pub ok: bool,
    pub reason: Option<String>,
    pub channel: Option<UpdateChannel>,
}

fn map_update_result(
    current_version: &str,
    latest_version: &str,
    reason: Option<String>,
    has_update: bool,
    manual_download_required: bool,
) -> DesktopAppUpdateCheckResult {
    DesktopAppUpdateCheckResult {
        ok: true,
        reason,
        current_version: Some(current_version.to_string()),
        latest_version: Some(latest_version.to_string()),
        has_update,
        manual_download_required,
    }
}

pub(crate) fn map_no_update_result(current_version: &str) -> DesktopAppUpdateCheckResult {
    map_update_result(current_version, current_version, None, false, false)
}

pub(crate) fn map_update_available_result(
    current_version: &str,
    latest_version: &str,
) -> DesktopAppUpdateCheckResult {
    map_update_result(current_version, latest_version, None, true, false)
}

/// Maps a manual-download install that did find a newer remote release.
pub(crate) fn map_manual_download_update_available_result(
    current_version: &str,
    latest_version: &str,
    reason: impl Into<String>,
) -> DesktopAppUpdateCheckResult {
    map_update_result(
        current_version,
        latest_version,
        Some(reason.into()),
        true,
        true,
    )
}

/// Maps a manual-download install that checked successfully but found no newer release.
pub(crate) fn map_manual_download_no_update_result(
    current_version: &str,
    reason: impl Into<String>,
) -> DesktopAppUpdateCheckResult {
    map_update_result(
        current_version,
        current_version,
        Some(reason.into()),
        false,
        false,
    )
}

pub(crate) fn map_update_check_error(
    current_version: Option<String>,
    reason: impl Into<String>,
) -> DesktopAppUpdateCheckResult {
    DesktopAppUpdateCheckResult {
        ok: false,
        reason: Some(reason.into()),
        current_version: current_version.clone(),
        latest_version: current_version,
        has_update: false,
        manual_download_required: false,
    }
}

pub(crate) fn map_update_install_error(reason: impl Into<String>) -> DesktopAppUpdateResult {
    DesktopAppUpdateResult {
        ok: false,
        reason: Some(reason.into()),
    }
}

pub(crate) fn map_update_install_ok() -> DesktopAppUpdateResult {
    DesktopAppUpdateResult {
        ok: true,
        reason: None,
    }
}

pub(crate) fn map_update_channel_ok(channel: UpdateChannel) -> DesktopAppUpdateChannelResult {
    DesktopAppUpdateChannelResult {
        ok: true,
        reason: None,
        channel: Some(channel),
    }
}

pub(crate) fn map_update_channel_error(reason: impl Into<String>) -> DesktopAppUpdateChannelResult {
    DesktopAppUpdateChannelResult {
        ok: false,
        reason: Some(reason.into()),
        channel: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_no_update_result_keeps_current_version() {
        let result = map_no_update_result("4.19.2");
        assert!(result.ok);
        assert_eq!(result.current_version.as_deref(), Some("4.19.2"));
        assert_eq!(result.latest_version.as_deref(), Some("4.19.2"));
        assert!(!result.has_update);
        assert!(!result.manual_download_required);
    }

    #[test]
    fn map_update_available_result_marks_update_available() {
        let result = map_update_available_result("4.19.2", "4.20.0");
        assert!(result.ok);
        assert_eq!(result.current_version.as_deref(), Some("4.19.2"));
        assert_eq!(result.latest_version.as_deref(), Some("4.20.0"));
        assert!(result.has_update);
        assert!(!result.manual_download_required);
    }

    #[test]
    fn map_manual_download_update_available_result_marks_manual_download_upgrade() {
        let result = map_manual_download_update_available_result(
            "4.19.2",
            "4.20.0",
            crate::bridge::updater_messages::DESKTOP_UPDATER_MANUAL_DOWNLOAD_REASON,
        );
        assert!(result.ok);
        assert_eq!(result.current_version.as_deref(), Some("4.19.2"));
        assert_eq!(result.latest_version.as_deref(), Some("4.20.0"));
        assert!(result.has_update);
        assert!(result.manual_download_required);
    }

    #[test]
    fn map_update_check_error_keeps_known_current_version() {
        let result = map_update_check_error(Some("4.19.2".to_string()), "network error");
        assert!(!result.ok);
        assert_eq!(result.reason.as_deref(), Some("network error"));
        assert_eq!(result.current_version.as_deref(), Some("4.19.2"));
        assert_eq!(result.latest_version.as_deref(), Some("4.19.2"));
        assert!(!result.has_update);
        assert!(!result.manual_download_required);
    }

    #[test]
    fn map_update_install_error_returns_failure_shape() {
        let result = map_update_install_error("install failed");
        assert!(!result.ok);
        assert_eq!(result.reason.as_deref(), Some("install failed"));
    }

    #[test]
    fn map_update_channel_ok_returns_channel() {
        let result = map_update_channel_ok(UpdateChannel::Nightly);
        assert!(result.ok);
        assert_eq!(result.channel, Some(UpdateChannel::Nightly));
    }

    #[test]
    fn map_manual_download_no_update_result_keeps_current_version_and_reason() {
        let result = map_manual_download_no_update_result(
            "4.19.2",
            crate::bridge::updater_messages::DESKTOP_UPDATER_MANUAL_DOWNLOAD_REASON,
        );
        assert!(result.ok);
        assert_eq!(result.current_version.as_deref(), Some("4.19.2"));
        assert_eq!(result.latest_version.as_deref(), Some("4.19.2"));
        assert!(!result.has_update);
        assert!(!result.manual_download_required);
        assert_eq!(
            result.reason.as_deref(),
            Some(crate::bridge::updater_messages::DESKTOP_UPDATER_MANUAL_DOWNLOAD_REASON)
        );
    }

    #[test]
    fn map_manual_download_no_update_result_keeps_manual_download_message_without_update_flag() {
        let result = map_manual_download_no_update_result(
            "4.19.2",
            crate::bridge::updater_messages::desktop_manual_download_reason(),
        );

        assert!(result.ok);
        assert_eq!(result.current_version.as_deref(), Some("4.19.2"));
        assert_eq!(result.latest_version.as_deref(), Some("4.19.2"));
        assert_eq!(
            result.reason.as_deref(),
            Some(crate::bridge::updater_messages::desktop_manual_download_reason().as_str())
        );
        assert!(!result.has_update);
        assert!(!result.manual_download_required);
    }
}

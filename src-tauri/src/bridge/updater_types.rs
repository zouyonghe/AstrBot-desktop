use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DesktopAppUpdateCheckResult {
    pub ok: bool,
    pub reason: Option<String>,
    pub current_version: Option<String>,
    pub latest_version: Option<String>,
    pub has_update: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DesktopAppUpdateResult {
    pub ok: bool,
    pub reason: Option<String>,
}

pub(crate) fn map_no_update_result(current_version: String) -> DesktopAppUpdateCheckResult {
    DesktopAppUpdateCheckResult {
        ok: true,
        reason: None,
        current_version: Some(current_version.clone()),
        latest_version: Some(current_version),
        has_update: false,
    }
}

pub(crate) fn map_update_available_result(
    current_version: String,
    latest_version: String,
) -> DesktopAppUpdateCheckResult {
    DesktopAppUpdateCheckResult {
        ok: true,
        reason: None,
        current_version: Some(current_version),
        latest_version: Some(latest_version),
        has_update: true,
    }
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

pub(crate) fn map_manual_download_result(
    current_version: &str,
    reason: impl Into<String>,
) -> DesktopAppUpdateCheckResult {
    DesktopAppUpdateCheckResult {
        ok: true,
        reason: Some(reason.into()),
        current_version: Some(current_version.to_string()),
        latest_version: Some(current_version.to_string()),
        has_update: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_no_update_result_keeps_current_version() {
        let result = map_no_update_result("4.19.2".to_string());
        assert!(result.ok);
        assert_eq!(result.current_version.as_deref(), Some("4.19.2"));
        assert_eq!(result.latest_version.as_deref(), Some("4.19.2"));
        assert!(!result.has_update);
    }

    #[test]
    fn map_update_available_result_marks_update_available() {
        let result = map_update_available_result("4.19.2".to_string(), "4.20.0".to_string());
        assert!(result.ok);
        assert_eq!(result.current_version.as_deref(), Some("4.19.2"));
        assert_eq!(result.latest_version.as_deref(), Some("4.20.0"));
        assert!(result.has_update);
    }

    #[test]
    fn map_update_check_error_keeps_known_current_version() {
        let result = map_update_check_error(Some("4.19.2".to_string()), "network error");
        assert!(!result.ok);
        assert_eq!(result.reason.as_deref(), Some("network error"));
        assert_eq!(result.current_version.as_deref(), Some("4.19.2"));
        assert_eq!(result.latest_version.as_deref(), Some("4.19.2"));
        assert!(!result.has_update);
    }

    #[test]
    fn map_update_install_error_returns_failure_shape() {
        let result = map_update_install_error("install failed");
        assert!(!result.ok);
        assert_eq!(result.reason.as_deref(), Some("install failed"));
    }

    #[test]
    fn map_manual_download_result_keeps_current_version_and_reason() {
        let result = map_manual_download_result(
            "4.19.2",
            crate::bridge::updater_messages::DESKTOP_UPDATER_MANUAL_DOWNLOAD_REASON,
        );
        assert!(result.ok);
        assert_eq!(result.current_version.as_deref(), Some("4.19.2"));
        assert_eq!(result.latest_version.as_deref(), Some("4.19.2"));
        assert!(!result.has_update);
        assert_eq!(
            result.reason.as_deref(),
            Some(crate::bridge::updater_messages::DESKTOP_UPDATER_MANUAL_DOWNLOAD_REASON)
        );
    }
}

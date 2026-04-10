use std::{
    env,
    path::{Path, PathBuf},
};

pub(crate) fn resolve_desktop_state_path(packaged_root_dir: Option<&Path>) -> Option<PathBuf> {
    resolve_desktop_state_path_with_root(
        env::var(crate::ASTRBOT_ROOT_ENV).ok().as_deref(),
        packaged_root_dir,
    )
}

pub(crate) fn resolve_desktop_state_path_with_root(
    root_override: Option<&str>,
    packaged_root_dir: Option<&Path>,
) -> Option<PathBuf> {
    if let Some(root_override) = root_override {
        let root_override = root_override.trim();
        if !root_override.is_empty() {
            return Some(
                PathBuf::from(root_override)
                    .join("data")
                    .join("desktop_state.json"),
            );
        }
    }

    packaged_root_dir.map(|root| root.join("data").join("desktop_state.json"))
}

#[cfg(test)]
mod tests {
    use super::resolve_desktop_state_path_with_root;
    use std::path::PathBuf;

    #[test]
    fn astrbot_root_overrides_packaged_root_fallback_for_desktop_state_path() {
        let packaged_root = PathBuf::from("/tmp/packaged-root");

        assert_eq!(
            resolve_desktop_state_path_with_root(
                Some("/tmp/astrbot-root"),
                Some(packaged_root.as_path())
            ),
            Some(PathBuf::from("/tmp/astrbot-root/data/desktop_state.json"))
        );
    }

    #[test]
    fn empty_astrbot_root_falls_back_to_packaged_root_for_desktop_state_path() {
        let packaged_root = PathBuf::from("/tmp/packaged-root");

        assert_eq!(
            resolve_desktop_state_path_with_root(Some(""), Some(packaged_root.as_path())),
            Some(PathBuf::from("/tmp/packaged-root/data/desktop_state.json"))
        );
    }

    #[test]
    fn packaged_root_resolves_data_desktop_state_json_path() {
        let packaged_root = PathBuf::from("/tmp/packaged-root");

        assert_eq!(
            resolve_desktop_state_path_with_root(None, Some(packaged_root.as_path())),
            Some(PathBuf::from("/tmp/packaged-root/data/desktop_state.json"))
        );
    }
}

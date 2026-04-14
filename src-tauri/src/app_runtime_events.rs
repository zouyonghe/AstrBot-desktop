use tauri::{webview::PageLoadEvent, RunEvent};

use crate::close_behavior::CloseAction;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MainWindowAction {
    None,
    ShowClosePrompt,
    PreventCloseAndHide,
    ExitApplication,
    HideIfMinimized,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PageLoadAction {
    None,
    InjectDesktopBridge,
    ApplyStartupLoadingMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RunEventAction {
    None,
    #[cfg(target_os = "macos")]
    ShowMainWindow,
    HandleExitRequested,
    HandleExit,
}

pub(crate) fn main_window_action(
    window_label: &str,
    is_quitting: bool,
    minimized_on_focus_lost: bool,
    is_close_requested: bool,
    is_focus_lost: bool,
    saved_close_action: Option<CloseAction>,
) -> MainWindowAction {
    if window_label != "main" {
        return MainWindowAction::None;
    }

    if is_close_requested {
        return if is_quitting {
            MainWindowAction::None
        } else {
            match saved_close_action {
                Some(CloseAction::Tray) => MainWindowAction::PreventCloseAndHide,
                Some(CloseAction::Exit) => MainWindowAction::ExitApplication,
                None => MainWindowAction::ShowClosePrompt,
            }
        };
    }

    if is_focus_lost && minimized_on_focus_lost && !is_quitting {
        return MainWindowAction::HideIfMinimized;
    }

    MainWindowAction::None
}

pub(crate) fn page_load_action(
    event: PageLoadEvent,
    should_inject_bridge: bool,
    should_apply_startup_loading: bool,
) -> PageLoadAction {
    match event {
        PageLoadEvent::Started | PageLoadEvent::Finished if should_inject_bridge => {
            PageLoadAction::InjectDesktopBridge
        }
        PageLoadEvent::Finished if should_apply_startup_loading => {
            PageLoadAction::ApplyStartupLoadingMode
        }
        _ => PageLoadAction::None,
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn reopen_event_action(has_visible_windows: bool) -> RunEventAction {
    if has_visible_windows {
        RunEventAction::None
    } else {
        RunEventAction::ShowMainWindow
    }
}

pub(crate) fn run_event_action(event: &RunEvent) -> RunEventAction {
    match event {
        #[cfg(target_os = "macos")]
        RunEvent::Reopen {
            has_visible_windows,
            ..
        } => reopen_event_action(*has_visible_windows),
        RunEvent::ExitRequested { .. } => RunEventAction::HandleExitRequested,
        RunEvent::Exit => RunEventAction::HandleExit,
        _ => RunEventAction::None,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        main_window_action, page_load_action, run_event_action, MainWindowAction, PageLoadAction,
        RunEventAction,
    };
    use crate::close_behavior::CloseAction;
    use tauri::{webview::PageLoadEvent, RunEvent};

    #[cfg(target_os = "macos")]
    use super::reopen_event_action;

    #[test]
    fn main_window_action_ignores_non_main_windows() {
        assert_eq!(
            main_window_action("settings", false, false, true, false, None),
            MainWindowAction::None
        );
    }

    #[test]
    fn main_window_action_prompts_when_no_saved_close_preference_exists() {
        assert_eq!(
            main_window_action("main", false, false, true, false, None),
            MainWindowAction::ShowClosePrompt
        );
    }

    #[test]
    fn main_window_action_hides_on_close_when_saved_preference_is_tray() {
        assert_eq!(
            main_window_action("main", false, false, true, false, Some(CloseAction::Tray)),
            MainWindowAction::PreventCloseAndHide
        );
    }

    #[test]
    fn main_window_action_exits_on_close_when_saved_preference_is_exit() {
        assert_eq!(
            main_window_action("main", false, false, true, false, Some(CloseAction::Exit)),
            MainWindowAction::ExitApplication
        );
    }

    #[test]
    fn main_window_action_hides_on_minimized_focus_loss() {
        assert_eq!(
            main_window_action("main", false, true, false, true, None),
            MainWindowAction::HideIfMinimized
        );
    }

    #[test]
    fn page_load_action_injects_bridge_when_requested() {
        assert_eq!(
            page_load_action(PageLoadEvent::Started, true, false),
            PageLoadAction::InjectDesktopBridge
        );
    }

    #[test]
    fn page_load_action_applies_startup_loading_only_on_finished() {
        assert_eq!(
            page_load_action(PageLoadEvent::Finished, false, true),
            PageLoadAction::ApplyStartupLoadingMode
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn reopen_event_action_shows_main_window_only_when_no_window_is_visible() {
        assert_eq!(reopen_event_action(false), RunEventAction::ShowMainWindow);
        assert_eq!(reopen_event_action(true), RunEventAction::None);
    }

    #[test]
    fn run_event_action_maps_exit_events() {
        assert_eq!(
            run_event_action(&RunEvent::Exit),
            RunEventAction::HandleExit
        );
    }
}

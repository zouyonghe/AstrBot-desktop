use tauri::{
    menu::{CheckMenuItem, MenuItem},
    AppHandle, Manager,
};

use crate::{runtime_paths, shell_locale, tray::actions, TrayMenuState};

fn set_menu_text_safe<F>(item: &MenuItem<tauri::Wry>, text: &str, item_name: &str, log: F)
where
    F: Fn(&str),
{
    if let Err(error) = item.set_text(text) {
        log(&format!(
            "failed to update tray menu text for {}: {}",
            item_name, error
        ));
    }
}

fn set_check_menu_text_safe<F>(
    item: &CheckMenuItem<tauri::Wry>,
    text: &str,
    item_name: &str,
    log: F,
) where
    F: Fn(&str),
{
    if let Err(error) = item.set_text(text) {
        log(&format!(
            "failed to update tray menu text for {}: {}",
            item_name, error
        ));
    }
}

pub fn update_tray_menu_labels<F>(
    app_handle: &AppHandle,
    default_shell_locale: &'static str,
    log: F,
) where
    F: Fn(&str),
{
    update_tray_menu_labels_with_visibility(app_handle, default_shell_locale, None, log);
}

pub fn update_tray_menu_labels_with_visibility<F>(
    app_handle: &AppHandle,
    default_shell_locale: &'static str,
    visible_override: Option<bool>,
    log: F,
) where
    F: Fn(&str),
{
    let Some(tray_state) = app_handle.try_state::<TrayMenuState>() else {
        return;
    };

    let locale = shell_locale::resolve_shell_locale(
        default_shell_locale,
        runtime_paths::default_packaged_root_dir(),
    );
    let shell_texts = shell_locale::shell_texts_for_locale(locale);
    let effective_visible = if let Some(visible) = visible_override {
        visible
    } else {
        app_handle
            .get_webview_window("main")
            .and_then(|window| window.is_visible().ok())
            .unwrap_or(true)
    };

    let toggle_label = if effective_visible {
        shell_texts.tray_hide
    } else {
        shell_texts.tray_show
    };

    set_menu_text_safe(
        &tray_state.toggle_item,
        toggle_label,
        actions::TRAY_MENU_TOGGLE_WINDOW,
        &log,
    );
    set_menu_text_safe(
        &tray_state.reload_item,
        shell_texts.tray_reload,
        actions::TRAY_MENU_RELOAD_WINDOW,
        &log,
    );
    set_menu_text_safe(
        &tray_state.restart_backend_item,
        shell_texts.tray_restart_backend,
        actions::TRAY_MENU_RESTART_BACKEND,
        &log,
    );
    set_check_menu_text_safe(
        &tray_state.launch_at_login_item,
        shell_texts.tray_launch_at_login,
        actions::TRAY_MENU_LAUNCH_AT_LOGIN,
        &log,
    );
    set_check_menu_text_safe(
        &tray_state.silent_launch_item,
        shell_texts.tray_silent_launch,
        actions::TRAY_MENU_SILENT_LAUNCH,
        &log,
    );
    set_check_menu_text_safe(
        &tray_state.close_to_tray_item,
        shell_texts.tray_close_to_tray,
        actions::TRAY_MENU_CLOSE_TO_TRAY,
        &log,
    );
    set_menu_text_safe(
        &tray_state.quit_item,
        shell_texts.tray_quit,
        actions::TRAY_MENU_QUIT,
        &log,
    );
}

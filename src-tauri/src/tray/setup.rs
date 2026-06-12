use tauri::{
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager,
};
use tauri_plugin_autostart::ManagerExt;

use crate::{
    append_desktop_log, runtime_paths, shell_locale,
    tray::{actions, labels, menu_handler},
    window, TrayMenuState, DEFAULT_SHELL_LOCALE, TRAY_ID,
};

pub fn setup_tray(app_handle: &AppHandle) -> Result<(), String> {
    let locale = shell_locale::resolve_shell_locale(
        DEFAULT_SHELL_LOCALE,
        runtime_paths::default_packaged_root_dir(),
    );
    let shell_texts = shell_locale::shell_texts_for_locale(locale);
    let desktop_settings = app_handle.state::<crate::DesktopSettingsCache>().get();
    let launch_at_login_checked = app_handle
        .autolaunch()
        .is_enabled()
        .unwrap_or(desktop_settings.launch_at_login);
    let main_window_visible = app_handle
        .get_webview_window("main")
        .and_then(|window| window.is_visible().ok())
        .unwrap_or(true);
    let toggle_label = if main_window_visible {
        shell_texts.tray_hide
    } else {
        shell_texts.tray_show
    };

    let toggle_item = MenuItem::with_id(
        app_handle,
        actions::TRAY_MENU_TOGGLE_WINDOW,
        toggle_label,
        true,
        None::<&str>,
    )
    .map_err(|error| format!("Failed to create tray toggle menu item: {error}"))?;
    let reload_item = MenuItem::with_id(
        app_handle,
        actions::TRAY_MENU_RELOAD_WINDOW,
        shell_texts.tray_reload,
        true,
        None::<&str>,
    )
    .map_err(|error| format!("Failed to create tray reload menu item: {error}"))?;
    let restart_backend_item = MenuItem::with_id(
        app_handle,
        actions::TRAY_MENU_RESTART_BACKEND,
        shell_texts.tray_restart_backend,
        true,
        None::<&str>,
    )
    .map_err(|error| format!("Failed to create tray restart menu item: {error}"))?;
    let launch_at_login_item = CheckMenuItem::with_id(
        app_handle,
        actions::TRAY_MENU_LAUNCH_AT_LOGIN,
        shell_texts.tray_launch_at_login,
        true,
        launch_at_login_checked,
        None::<&str>,
    )
    .map_err(|error| format!("Failed to create tray launch at login menu item: {error}"))?;
    let silent_launch_item = CheckMenuItem::with_id(
        app_handle,
        actions::TRAY_MENU_SILENT_LAUNCH,
        shell_texts.tray_silent_launch,
        true,
        desktop_settings.silent_launch,
        None::<&str>,
    )
    .map_err(|error| format!("Failed to create tray silent launch menu item: {error}"))?;
    let close_to_tray_item = CheckMenuItem::with_id(
        app_handle,
        actions::TRAY_MENU_CLOSE_TO_TRAY,
        shell_texts.tray_close_to_tray,
        true,
        desktop_settings.close_to_tray,
        None::<&str>,
    )
    .map_err(|error| format!("Failed to create tray close to tray menu item: {error}"))?;
    let quit_item = MenuItem::with_id(
        app_handle,
        actions::TRAY_MENU_QUIT,
        shell_texts.tray_quit,
        true,
        None::<&str>,
    )
    .map_err(|error| format!("Failed to create tray quit menu item: {error}"))?;
    let separator = PredefinedMenuItem::separator(app_handle)
        .map_err(|error| format!("Failed to create tray separator menu item: {error}"))?;
    let settings_separator = PredefinedMenuItem::separator(app_handle)
        .map_err(|error| format!("Failed to create tray settings separator menu item: {error}"))?;

    let menu = Menu::with_items(
        app_handle,
        &[
            &toggle_item,
            &reload_item,
            &restart_backend_item,
            &settings_separator,
            &launch_at_login_item,
            &silent_launch_item,
            &close_to_tray_item,
            &separator,
            &quit_item,
        ],
    )
    .map_err(|error| format!("Failed to build tray menu: {error}"))?;

    if !app_handle.manage(TrayMenuState {
        toggle_item: toggle_item.clone(),
        reload_item: reload_item.clone(),
        restart_backend_item: restart_backend_item.clone(),
        launch_at_login_item: launch_at_login_item.clone(),
        silent_launch_item: silent_launch_item.clone(),
        close_to_tray_item: close_to_tray_item.clone(),
        quit_item: quit_item.clone(),
    }) {
        append_desktop_log("tray menu state already exists, skipping manage");
    }

    let tray_builder = TrayIconBuilder::with_id(TRAY_ID)
        .menu(&menu)
        .tooltip("AstrBot")
        .icon(tauri::include_image!("./icons/tray.png"))
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| menu_handler::handle_tray_menu_event(app, event.id().as_ref()))
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                labels::update_tray_menu_labels(
                    tray.app_handle(),
                    DEFAULT_SHELL_LOCALE,
                    append_desktop_log,
                );
                if button == MouseButton::Left {
                    window::actions::toggle_main_window(
                        tray.app_handle(),
                        DEFAULT_SHELL_LOCALE,
                        append_desktop_log,
                    );
                }
            }
        });

    #[cfg(target_os = "macos")]
    let tray_builder = tray_builder.icon_as_template(true);

    tray_builder
        .build(app_handle)
        .map_err(|error| format!("Failed to create tray icon: {error}"))?;

    labels::update_tray_menu_labels(app_handle, DEFAULT_SHELL_LOCALE, append_desktop_log);
    Ok(())
}

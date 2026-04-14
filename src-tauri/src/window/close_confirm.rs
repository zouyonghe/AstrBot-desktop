use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

pub(crate) const CLOSE_CONFIRM_WINDOW_LABEL: &str = "close-confirm";
const CLOSE_CONFIRM_WINDOW_WIDTH: f64 = 420.0;
const CLOSE_CONFIRM_WINDOW_HEIGHT: f64 = 320.0;

pub(crate) fn build_close_confirm_path(locale: &str) -> String {
    format!(
        "close-confirm.html?locale={locale}&trayAction={}&exitAction={}",
        crate::close_behavior::CLOSE_ACTION_TRAY,
        crate::close_behavior::CLOSE_ACTION_EXIT,
    )
}

fn build_close_confirm_url(locale: &str) -> WebviewUrl {
    WebviewUrl::App(build_close_confirm_path(locale).into())
}

pub(crate) fn close_confirm_window_size() -> (f64, f64) {
    (CLOSE_CONFIRM_WINDOW_WIDTH, CLOSE_CONFIRM_WINDOW_HEIGHT)
}

fn handle_existing_window_operation_result<F>(
    operation_result: Result<(), String>,
    operation: &str,
    log: F,
) -> Result<(), String>
where
    F: Fn(&str),
{
    match operation_result {
        Ok(()) => Ok(()),
        Err(error) => {
            let message = format!("failed to {operation} close confirm window: {error}");
            log(&message);
            Err(message)
        }
    }
}

pub(crate) fn show_close_confirm_window<F>(
    app_handle: &AppHandle,
    default_shell_locale: &'static str,
    log: F,
) -> Result<(), String>
where
    F: Fn(&str),
{
    if let Some(window) = app_handle.get_webview_window(CLOSE_CONFIRM_WINDOW_LABEL) {
        handle_existing_window_operation_result(
            window.unminimize().map_err(|error| error.to_string()),
            "unminimize",
            &log,
        )?;
        handle_existing_window_operation_result(
            window.show().map_err(|error| error.to_string()),
            "show",
            &log,
        )?;
        handle_existing_window_operation_result(
            window.set_focus().map_err(|error| error.to_string()),
            "focus",
            &log,
        )?;
        return Ok(());
    }

    let locale = crate::shell_locale::resolve_shell_locale(
        default_shell_locale,
        crate::runtime_paths::default_packaged_root_dir(),
    );
    let url = build_close_confirm_url(locale);
    let (width, height) = close_confirm_window_size();

    WebviewWindowBuilder::new(app_handle, CLOSE_CONFIRM_WINDOW_LABEL, url)
        .title("AstrBot")
        .inner_size(width, height)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .visible(true)
        .center()
        .build()
        .map(|_| ())
        .map_err(|error| format!("Failed to create close confirm window: {error}"))
}

#[cfg(test)]
mod tests {
    use super::{
        build_close_confirm_path, close_confirm_window_size,
        handle_existing_window_operation_result,
    };

    #[test]
    fn build_close_confirm_path_appends_locale_query() {
        assert_eq!(
            build_close_confirm_path("en-US"),
            "close-confirm.html?locale=en-US&trayAction=tray&exitAction=exit"
        );
    }

    #[test]
    fn close_confirm_window_size_fits_desktop_dialog_without_clipping() {
        assert_eq!(close_confirm_window_size(), (420.0, 320.0));
    }

    #[test]
    fn handle_existing_window_operation_result_returns_err_after_logging_failure() {
        let logs = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let captured_logs = std::rc::Rc::clone(&logs);

        let result = handle_existing_window_operation_result(
            Err("focus failed".to_string()),
            "focus",
            move |message: &str| {
                captured_logs.borrow_mut().push(message.to_string());
            },
        );

        assert_eq!(
            result,
            Err("failed to focus close confirm window: focus failed".to_string())
        );
        assert_eq!(
            logs.borrow().as_slice(),
            ["failed to focus close confirm window: focus failed"]
        );
    }
}

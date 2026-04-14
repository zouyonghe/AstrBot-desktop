use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

const CLOSE_CONFIRM_WINDOW_LABEL: &str = "close-confirm";
const CLOSE_CONFIRM_WINDOW_WIDTH: f64 = 420.0;
const CLOSE_CONFIRM_WINDOW_HEIGHT: f64 = 320.0;

pub(crate) fn build_close_confirm_path(locale: &str) -> String {
    format!("close-confirm.html?locale={locale}")
}

fn build_close_confirm_url(locale: &str) -> WebviewUrl {
    WebviewUrl::App(build_close_confirm_path(locale).into())
}

pub(crate) fn close_confirm_window_size() -> (f64, f64) {
    (CLOSE_CONFIRM_WINDOW_WIDTH, CLOSE_CONFIRM_WINDOW_HEIGHT)
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
        if let Err(error) = window.unminimize() {
            log(&format!(
                "failed to unminimize close confirm window: {error}"
            ));
        }
        if let Err(error) = window.show() {
            log(&format!("failed to show close confirm window: {error}"));
        }
        if let Err(error) = window.set_focus() {
            log(&format!("failed to focus close confirm window: {error}"));
        }
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
    use super::{build_close_confirm_path, close_confirm_window_size};

    #[test]
    fn build_close_confirm_path_appends_locale_query() {
        assert_eq!(
            build_close_confirm_path("en-US"),
            "close-confirm.html?locale=en-US"
        );
    }

    #[test]
    fn close_confirm_window_size_fits_desktop_dialog_without_clipping() {
        assert_eq!(close_confirm_window_size(), (420.0, 320.0));
    }
}

use tauri::{
    menu::{Menu, MenuItem},
    AppHandle, Manager, WebviewUrl, WebviewWindowBuilder,
};

pub(crate) fn install_menu(app: &tauri::App) -> tauri::Result<()> {
    let handle = app.handle();
    let menu = Menu::default(handle)?;
    let settings = MenuItem::with_id(handle, "settings", "Settings…", true, None::<&str>)?;
    if let Some(application) = menu.items()?.first().and_then(|item| item.as_submenu()) {
        application.insert(&settings, 1)?;
    }
    app.set_menu(menu)?;
    app.on_menu_event(|app, event| {
        if event.id().as_ref() == "settings" {
            show_settings_window(app);
        }
    });
    Ok(())
}

pub(crate) fn show_settings_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("settings") {
        let _ = window.show();
        let _ = window.set_focus();
        return;
    }
    let _ = WebviewWindowBuilder::new(
        app,
        "settings",
        WebviewUrl::App("index.html#/settings".into()),
    )
    .title("ctx-ppteer Settings")
    .inner_size(520.0, 430.0)
    .min_inner_size(420.0, 360.0)
    .resizable(true)
    .center()
    .build();
}

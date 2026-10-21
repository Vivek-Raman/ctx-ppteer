use tauri::Manager;

pub mod desktop;
pub mod integrations;
pub mod mcp;
pub mod paths;
pub mod preferences;
pub mod status;

use desktop::windows::install_menu;

pub(crate) const SKILL_NAME: &str = "ctx-ppteer";

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            desktop::commands::get_snapshot,
            desktop::commands::dismiss_point,
            desktop::commands::get_settings,
            desktop::commands::reset_settings,
            desktop::commands::open_settings,
            desktop::commands::set_appearance_settings,
            desktop::commands::set_project_sort_order,
            desktop::commands::set_auto_clear_after_minutes,
            desktop::commands::get_skill_installation,
            desktop::commands::install_skill,
            desktop::commands::install_skill_for_target,
            desktop::commands::install_mcp_for_harness,
            desktop::commands::uninstall_mcp_for_harness,
            desktop::commands::set_pinned,
        ])
        .setup(|app| {
            install_menu(app)?;
            let settings =
                preferences::settings_owner(app.handle()).map_err(std::io::Error::other)?;
            app.manage(settings.clone());
            app.get_webview_window("main")
                .ok_or_else(|| std::io::Error::other("Main window unavailable"))?
                .set_always_on_top(settings.snapshot().pinned)
                .map_err(std::io::Error::other)?;
            let path = paths::status_path().map_err(std::io::Error::other)?;
            let state = status::snapshot::ViewerState::new(path.clone());
            app.manage(state.clone());
            let _ = status::retention::clear_expired_points(&state, &settings);
            status::snapshot::update(app.handle(), &state);
            status::watcher::start(app.handle().clone(), state, settings);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Tauri application");
}

pub use paths as settings;
pub use status as status_file;

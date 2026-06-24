//! lib 模块初始化 oh-imgbb Tauri 应用。

mod app_state;
mod commands;
mod db;
mod settings;

use app_state::AppState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
/// 启动 Tauri 应用并注册后端命令。
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let app_handle = app.handle().clone();
            let app_state = tauri::async_runtime::block_on(AppState::initialize(&app_handle))?;
            app.manage(app_state);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::parse::parse_album,
            commands::parse::parse_profile,
            commands::download::download_album,
            commands::favorite::list_favorites,
            commands::favorite::save_favorite,
            commands::favorite::remove_favorite,
            commands::settings::get_settings,
            commands::settings::update_settings,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

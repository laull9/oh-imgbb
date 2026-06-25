//! lib 模块初始化 oh-imgbb Tauri 应用。

mod app_state;
mod commands;
mod db;
mod download_tasks;
mod settings;
mod thumbnail_cache;

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
            commands::parse::list_parse_tabs,
            commands::parse::save_parse_tab,
            commands::parse::remove_parse_tab,
            commands::parse::set_active_parse_tab,
            commands::download::download_album,
            commands::download::download_album_images,
            commands::download::download_profile_albums,
            commands::download::list_download_tasks,
            commands::download::cancel_download_task,
            commands::image_detail::download_detail_image,
            commands::image_detail::remove_detail_image,
            commands::favorite::list_favorites,
            commands::favorite::save_favorite,
            commands::favorite::remove_favorite,
            commands::settings::get_settings,
            commands::settings::update_settings,
            commands::settings::clear_thumbnail_cache,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

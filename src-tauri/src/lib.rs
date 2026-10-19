// Sentinel — Tauri 后端入口。
mod commands;
mod crypto;
mod merge;
mod settings;
mod state;
mod sync;
mod totp;
mod touchid;
mod vault;

use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::vault_exists,
            commands::create_vault,
            commands::unlock,
            commands::lock,
            commands::is_unlocked,
            commands::change_master_password,
            commands::list_items,
            commands::create_item,
            commands::update_item,
            commands::delete_item,
            commands::copy_secret,
            commands::generate_totp,
            commands::get_sync_config,
            commands::save_sync_config,
            commands::test_sync_connection,
            commands::sync_now,
            commands::resolve_sync,
            commands::cancel_sync,
            commands::get_settings,
            commands::save_settings,
            commands::get_recovery_hint,
            commands::set_recovery_hint,
            commands::export_vault,
            commands::touch_id_available,
            commands::touch_id_enabled,
            commands::enable_touch_id,
            commands::disable_touch_id,
            commands::unlock_with_touch_id
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

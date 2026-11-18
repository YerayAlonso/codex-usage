#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod codex;
mod i18n;
mod model;
mod storage;
mod tray;

use std::sync::Arc;
use tauri::Manager;
use tauri_plugin_autostart::ManagerExt;

fn main() {
    // Do not allocate a worker per CPU for two infrequent subprocess operations.
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .max_blocking_threads(4)
        .enable_all()
        .build()
        .expect("No s'ha pogut iniciar el runtime");
    tauri::async_runtime::set(runtime.handle().clone());
    let application = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            tray::open_panel(app, None)
        }))
        .plugin(tauri_plugin_autostart::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            app::get_state,
            app::refresh_usage,
            app::execute_prompt,
            app::reset_quota,
            app::save_settings,
            app::set_autostart,
            app::open_logs,
            app::close_panel,
        ])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            let directory = app.path().app_data_dir()?;
            storage::Storage::migrate_legacy(&directory)?;
            let storage = storage::Storage::new(directory)?;
            let disk = storage.load().inspect_err(|error| {
                let _ = storage.append_log("error", error);
            })?;
            let autostart = app.autolaunch().is_enabled().unwrap_or(false);
            let state = Arc::new(app::AppState::new(storage, disk, autostart));
            app.manage(state.clone());
            tray::setup(app.handle(), &state.snapshot())?;
            state.log("info", "@log.started");
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(app::scheduler(handle, state));
            if std::env::args().any(|arg| arg == "--show") {
                tray::open_panel(app.handle(), None);
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("Could not start Codex Usage");

    application.run(|_, event| {
        // Destroying the last webview must NOT stop the Rust scheduler and tray.
        if let tauri::RunEvent::ExitRequested {
            api, code: None, ..
        } = event
        {
            api.prevent_exit();
        }
    });
}

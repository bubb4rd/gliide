//! The easyshmeezy desktop app: Tauri commands over the resume crates (ARCHITECTURE.md, M4).
//!
//! `commands` is the bridge; the modules under it hold the work and run without a webview (`App::open` with any
//! [`state::Emit`]), which is how they're smoke-tested.

mod commands;
pub mod dto;
pub mod error;
pub mod export;
pub mod jobs;
pub mod library;
pub mod review;
pub mod settings;
pub mod state;
mod view;
mod window;

use std::sync::Arc;

use tauri::{Emitter, Manager};

use crate::state::App;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            let handle = app.handle().clone();
            let emit: state::Emit = Box::new(move |p| {
                let _ = handle.emit("progress", p);
            });
            let state = Arc::new(App::open(&dir, emit).map_err(|e| e.message)?);
            let warm = Arc::clone(&state);
            std::thread::spawn(move || warm.warm());
            app.manage(state);
            window::setup(app)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::agent_status,
            commands::get_settings,
            commands::set_settings,
            commands::library,
            commands::import_resume,
            commands::set_master_options,
            commands::jobs,
            commands::add_job,
            commands::open_job,
            commands::delete_job,
            commands::start_review,
            commands::cancel,
            commands::decide,
            commands::set_wording,
            commands::add_edit,
            commands::remove_edit,
            commands::toggle_skill,
            commands::undo,
            commands::redo,
            commands::edit_targets,
            commands::rewrite,
            commands::export_pdf,
            commands::applications,
            commands::set_status,
        ])
        .run(tauri::generate_context!())
        .expect("error while running easyshmeezy");
}

//! The gliide desktop app: Tauri commands over the resume crates (ARCHITECTURE.md, M4).
//!
//! `commands` is the bridge; the modules under it hold the work and run without a webview (`App::open` with any
//! [`state::Emit`]), which is how they're smoke-tested.

mod commands;
pub mod cover;
pub mod dto;
pub mod entry;
pub mod error;
pub mod export;
pub mod import;
pub mod jobs;
pub mod library;
pub mod logo;
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
        .register_asynchronous_uri_scheme_protocol(logo::SCHEME, |ctx, request, responder| {
            let Some(state) = ctx.app_handle().try_state::<Arc<App>>() else {
                return responder.respond(
                    tauri::http::Response::builder()
                        .status(tauri::http::StatusCode::SERVICE_UNAVAILABLE)
                        .body(Vec::new())
                        .unwrap_or_default(),
                );
            };
            let app = Arc::clone(&state);
            let path = request.uri().path().to_owned();
            tauri::async_runtime::spawn_blocking(move || responder.respond(logo::respond(&app, &path)));
        })
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
            commands::read_import,
            commands::import_view,
            commands::finish_import,
            commands::discard_import,
            commands::entry,
            commands::edit_entry,
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
            commands::rewrite_line,
            commands::save_variant,
            commands::export_pdf,
            commands::applications,
            commands::set_status,
            commands::logos,
            commands::set_logo,
            commands::remove_logo,
            commands::cover,
            commands::write_cover,
            commands::set_cover_part,
            commands::rewrite_cover_paragraph,
            commands::export_cover,
        ])
        .run(tauri::generate_context!())
        .expect("error while running gliide");
}

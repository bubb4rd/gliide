//! The gliide desktop app: Tauri commands over the resume crates (ARCHITECTURE.md, M4).
//!
//! `commands` is the bridge; the modules under it hold the work and run without a webview (`App::open` with any
//! [`state::Emit`]), which is how they're smoke-tested.

mod commands;
pub mod cover;
pub mod debrief;
pub mod dig;
pub mod dto;
pub mod entry;
pub mod error;
pub mod export;
pub mod feed;
pub mod folder;
mod ics;
pub mod import;
pub mod jobs;
pub mod library;
pub mod logo;
pub mod notify;
pub mod place;
pub mod prep;
pub mod review;
pub mod rounds;
pub mod settings;
pub mod state;
pub mod stories;
pub mod update;
mod view;
mod window;

use std::sync::Arc;

use tauri::{Emitter, Manager};

use crate::state::{App, Event};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
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
            let emit: state::Emit = Box::new(move |e| {
                let _ = match e {
                    Event::Progress(p) => handle.emit("progress", p),
                    Event::Feed(s) => handle.emit("feed", s),
                };
            });
            let state = match App::open(&dir, emit) {
                Ok(state) => Arc::new(state),
                Err(problem) => return cannot_start(app, problem),
            };
            let warm = Arc::clone(&state);
            std::thread::spawn(move || warm.warm());
            tauri::async_runtime::spawn(feed::schedule(Arc::clone(&state)));
            app.manage(state);
            app.manage(state::Startup(None));
            app.manage(update::Updates::new(app.package_info().version.to_string()));
            tauri::async_runtime::spawn(update::schedule(app.handle().clone()));
            notify::setup(app.handle());
            tauri::async_runtime::spawn(notify::schedule(app.handle().clone()));
            window::setup(app)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::agent_status,
            commands::get_settings,
            commands::set_settings,
            commands::models,
            commands::notification_permission,
            commands::request_notifications,
            commands::open_notification_settings,
            commands::update_status,
            commands::check_update,
            commands::install_update,
            commands::startup_problem,
            commands::relaunch,
            commands::library,
            commands::read_import,
            commands::import_view,
            commands::finish_import,
            commands::discard_import,
            commands::add_entry,
            commands::entry,
            commands::edit_entry,
            commands::set_master_options,
            commands::folder,
            commands::folder_pdf,
            commands::jobs,
            commands::add_job,
            commands::open_job,
            commands::delete_job,
            commands::set_salary,
            commands::places,
            commands::start_review,
            commands::prepare_review,
            commands::cancel,
            commands::running,
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
            commands::keep_fact,
            commands::instruct,
            commands::accept,
            commands::reject,
            commands::withdraw,
            commands::settle,
            commands::export_pdf,
            commands::applications,
            commands::set_status,
            commands::rounds,
            commands::save_round,
            commands::remove_round,
            commands::restore_rounds,
            commands::read_invite,
            commands::read_invite_file,
            commands::sent_resume,
            commands::prep,
            commands::practice,
            commands::nudge,
            commands::practice_turns,
            commands::keep_claim,
            commands::tell_story,
            commands::story_turns,
            commands::stories,
            commands::story,
            commands::save_story,
            commands::remove_story,
            commands::dig_questions,
            commands::dig_capture,
            commands::debrief,
            commands::save_debrief,
            commands::read_debrief,
            commands::logos,
            commands::set_logo,
            commands::remove_logo,
            commands::cover,
            commands::write_cover,
            commands::set_cover_part,
            commands::rewrite_cover_paragraph,
            commands::export_cover,
            commands::feed,
            commands::pull_feed,
            commands::search_feed,
            commands::check_feed,
            commands::dismiss_feed,
            commands::open_feed_job,
            commands::feed_settings,
            commands::set_feed_settings,
        ])
        .run(tauri::generate_context!())
        .expect("error while running gliide");
}

/// The data couldn't be opened: the window opens on the Startup screen alone (why, and what fixes it: Update gliide,
/// Try again, Show in Finder), with nothing else running. Only the updater and Startup's commands work. Should the
/// window itself fail, a native alert says why and quits once dismissed. Never an error out of setup: that panics
/// where unwinding can't, and macOS reports it as a crash.
fn cannot_start(app: &mut tauri::App, problem: dto::StartupProblem) -> Result<(), Box<dyn std::error::Error>> {
    let detail = problem.detail.clone();
    app.manage(state::Startup(Some(problem)));
    app.manage(update::Updates::new(app.package_info().version.to_string()));
    if let Err(e) = window::setup(app) {
        use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
        eprintln!("gliide: the window couldn't open ({e})");
        let handle = app.handle().clone();
        app.dialog()
            .message(detail)
            .title("gliide can’t open its data")
            .kind(MessageDialogKind::Error)
            .buttons(MessageDialogButtons::OkCustom("Quit".into()))
            .show(move |_| handle.exit(1));
    }
    Ok(())
}

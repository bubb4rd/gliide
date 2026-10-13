//! The Tauri commands: `app/src/lib/api/index.ts` names them and their arguments. Each is async and runs its work
//! on the blocking pool or as a model-call task, so the UI thread never waits on SQLite, Typst or `claude`.

use std::sync::Arc;

use tauri::State;

use crate::dto::{
    AgentStatus, AppError, AppStatus, ApplicationRow, CoverPart, CoverView, CustomEdit, Decision,
    EditTargets, Exported, JobRow, JobView, Library, Mode, ReviewView, Rewritten, Settings,
};
use crate::state::{App, blocking};
use crate::{cover, export, jobs, library as lib, review, settings};

type Res<T> = Result<T, AppError>;
type St<'a> = State<'a, Arc<App>>;

// ── Settings and the agent ──

#[tauri::command]
pub async fn agent_status(app: St<'_>) -> Res<AgentStatus> {
    Ok(settings::agent_status(&app).await)
}

#[tauri::command]
pub async fn get_settings(app: St<'_>) -> Res<Settings> {
    Ok(settings::get_settings(&app))
}

#[tauri::command]
pub async fn set_settings(app: St<'_>, settings: Settings) -> Res<Settings> {
    blocking(&app, move |app| settings::set_settings(app, settings)).await
}

// ── Bank and masters ──

#[tauri::command]
pub async fn library(app: St<'_>) -> Res<Library> {
    blocking(&app, lib::library).await
}

#[tauri::command]
pub async fn import_resume(app: St<'_>, path: String) -> Res<Library> {
    blocking(&app, move |app| lib::import_resume(app, &path)).await
}

#[tauri::command]
pub async fn set_master_options(
    app: St<'_>,
    master: String,
    template: String,
    page_limit: u32,
) -> Res<Library> {
    blocking(&app, move |app| {
        lib::set_master_options(app, &master, &template, page_limit)
    })
    .await
}

// ── Jobs ──

#[tauri::command]
pub async fn jobs(app: St<'_>) -> Res<Vec<JobRow>> {
    blocking(&app, jobs::jobs).await
}

#[tauri::command]
pub async fn add_job(app: St<'_>, input: String) -> Res<JobRow> {
    jobs::add_job(&app, &input).await
}

#[tauri::command]
pub async fn open_job(app: St<'_>, job: String) -> Res<JobView> {
    jobs::open_job(&app, &job).await
}

#[tauri::command]
pub async fn delete_job(app: St<'_>, job: String) -> Res<Vec<JobRow>> {
    blocking(&app, move |app| jobs::delete_job(app, &job)).await
}

#[tauri::command]
pub async fn start_review(
    app: St<'_>,
    job: String,
    master: Option<String>,
    mode: Option<Mode>,
    fresh: bool,
) -> Res<ReviewView> {
    review::start_review(&app, &job, master, mode, fresh).await
}

#[tauri::command]
pub async fn cancel(app: St<'_>, job: String) -> Res<()> {
    app.cancel(&job);
    Ok(())
}

// ── Review ──

#[tauri::command]
pub async fn decide(
    app: St<'_>,
    review: String,
    key: String,
    decision: Option<Decision>,
) -> Res<ReviewView> {
    review::decide(&app, &review, key, decision).await
}

#[tauri::command]
pub async fn set_wording(
    app: St<'_>,
    review: String,
    key: String,
    text: Option<String>,
) -> Res<ReviewView> {
    review::set_wording(&app, &review, key, text).await
}

#[tauri::command]
pub async fn add_edit(app: St<'_>, review: String, edit: CustomEdit) -> Res<ReviewView> {
    review::add_edit(&app, &review, edit).await
}

#[tauri::command]
pub async fn remove_edit(app: St<'_>, review: String, key: String) -> Res<ReviewView> {
    review::remove_edit(&app, &review, key).await
}

#[tauri::command]
pub async fn toggle_skill(app: St<'_>, review: String, term: String) -> Res<ReviewView> {
    review::toggle_skill(&app, &review, term).await
}

#[tauri::command]
pub async fn undo(app: St<'_>, review: String) -> Res<ReviewView> {
    review::undo(&app, &review).await
}

#[tauri::command]
pub async fn redo(app: St<'_>, review: String) -> Res<ReviewView> {
    review::redo(&app, &review).await
}

#[tauri::command]
pub async fn edit_targets(app: St<'_>, review: String) -> Res<EditTargets> {
    blocking(&app, move |app| review::edit_targets(app, &review)).await
}

#[tauri::command]
pub async fn rewrite(app: St<'_>, review: String, key: String, note: String) -> Res<Rewritten> {
    review::rewrite(&app, &review, key, note).await
}

#[tauri::command]
pub async fn export_pdf(app: St<'_>, review: String) -> Res<Exported> {
    blocking(&app, move |app| export::export_pdf(app, &review)).await
}

// ── Applications ──

#[tauri::command]
pub async fn applications(app: St<'_>) -> Res<Vec<ApplicationRow>> {
    blocking(&app, export::applications).await
}

#[tauri::command]
pub async fn set_status(
    app: St<'_>,
    application: String,
    status: AppStatus,
) -> Res<ApplicationRow> {
    blocking(&app, move |app| {
        export::set_status(app, &application, status)
    })
    .await
}

// ── Cover letter ──

#[tauri::command]
pub async fn cover(app: St<'_>, review: String) -> Res<CoverView> {
    blocking(&app, move |app| cover::cover_view(app, &review)).await
}

#[tauri::command]
pub async fn write_cover(app: St<'_>, review: String) -> Res<CoverView> {
    cover::write_cover(&app, &review).await
}

#[tauri::command]
pub async fn set_cover_part(
    app: St<'_>,
    review: String,
    part: CoverPart,
    text: String,
) -> Res<CoverView> {
    blocking(&app, move |app| {
        cover::set_cover_part(app, &review, part, &text)
    })
    .await
}

#[tauri::command]
pub async fn rewrite_cover_paragraph(
    app: St<'_>,
    review: String,
    index: u32,
    note: String,
) -> Res<CoverView> {
    cover::rewrite_cover_paragraph(&app, &review, index, note).await
}

#[tauri::command]
pub async fn export_cover(app: St<'_>, review: String) -> Res<Exported> {
    blocking(&app, move |app| export::export_cover(app, &review)).await
}

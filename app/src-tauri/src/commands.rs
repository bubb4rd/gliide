//! The Tauri commands: `app/src/lib/api/index.ts` names them and their arguments. Each is async and runs its work
//! on the blocking pool or as a model-call task, so the UI thread never waits on SQLite, Typst or `claude`.

use std::sync::Arc;

use tauri::State;

use crate::dto::{
    AgentStatus, AppError, AppStatus, ApplicationRow, CoverPart, CoverView, CustomEdit, Decision,
    EditTargets, EntryEdit, EntryView, Exported, FeedSettingsDto, FeedSettingsView, FeedView,
    FolderPdf, FolderView, ImportChoices, ImportView, JobRow, JobView, Library, LogoRow,
    LogoSource, Mode, Place, ReviewView, Rewritten, SalaryRange, SearchCursor, SearchResults,
    Settings,
};
use crate::state::{App, blocking};
use crate::{
    cover, entry, export, feed, folder as files, import, jobs, library as lib, logo, place,
    review, settings,
};

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
pub async fn read_import(app: St<'_>, path: String) -> Res<ImportView> {
    import::read_import(&app, path).await
}

#[tauri::command]
pub async fn import_view(app: St<'_>, choices: ImportChoices) -> Res<ImportView> {
    blocking(&app, move |app| import::import_view(app, &choices)).await
}

#[tauri::command]
pub async fn finish_import(app: St<'_>, choices: ImportChoices) -> Res<Library> {
    blocking(&app, move |app| import::finish_import(app, &choices)).await
}

#[tauri::command]
pub async fn discard_import(app: St<'_>) -> Res<()> {
    import::discard_import(&app);
    Ok(())
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

// ── The export folder ──

#[tauri::command]
pub async fn folder(app: St<'_>) -> Res<FolderView> {
    blocking(&app, files::folder).await
}

#[tauri::command]
pub async fn folder_pdf(app: St<'_>, name: String) -> Res<FolderPdf> {
    blocking(&app, move |app| files::folder_pdf(app, &name)).await
}

#[tauri::command]
pub async fn entry(app: St<'_>, entry: String, preview: Option<String>) -> Res<EntryView> {
    blocking(&app, move |app| {
        entry::entry_view(app, &entry, preview.as_deref())
    })
    .await
}

#[tauri::command]
pub async fn edit_entry(
    app: St<'_>,
    entry: String,
    edit: EntryEdit,
    preview: Option<String>,
) -> Res<EntryView> {
    blocking(&app, move |app| {
        entry::edit_entry(app, &entry, edit, preview.as_deref())
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
pub async fn set_salary(app: St<'_>, job: String, salary: Option<SalaryRange>) -> Res<JobRow> {
    blocking(&app, move |app| jobs::set_salary(app, &job, salary)).await
}

#[tauri::command]
pub async fn places(app: St<'_>, location: String) -> Res<Vec<Place>> {
    place::places(&app, &location).await
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
pub async fn rewrite_line(
    app: St<'_>,
    review: String,
    line: Option<String>,
    note: String,
) -> Res<Rewritten> {
    review::rewrite_line(&app, &review, line, note).await
}

#[tauri::command]
pub async fn save_variant(app: St<'_>, review: String, key: String) -> Res<ReviewView> {
    review::save_variant(&app, &review, key).await
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

// ── Company logos ──

#[tauri::command]
pub async fn logos(app: St<'_>) -> Res<Vec<LogoRow>> {
    blocking(&app, logo::logos).await
}

#[tauri::command]
pub async fn set_logo(app: St<'_>, company: String, source: LogoSource) -> Res<LogoRow> {
    blocking(&app, move |app| logo::set_logo(app, &company, source)).await
}

#[tauri::command]
pub async fn remove_logo(app: St<'_>, company: String) -> Res<()> {
    blocking(&app, move |app| logo::remove_logo(app, &company)).await
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

// ── Job feed ──

#[tauri::command]
pub async fn feed(app: St<'_>) -> Res<FeedView> {
    blocking(&app, feed::feed).await
}

#[tauri::command]
pub async fn pull_feed(app: St<'_>) -> Res<FeedView> {
    feed::pull_feed(&app).await
}

#[tauri::command]
pub async fn search_feed(
    app: St<'_>,
    query: String,
    cursor: Option<SearchCursor>,
) -> Res<SearchResults> {
    feed::search_feed(&app, query, cursor).await
}

#[tauri::command]
pub async fn check_feed(app: St<'_>, slugs: Vec<String>) -> Res<FeedView> {
    feed::check_feed(&app, slugs).await
}

#[tauri::command]
pub async fn dismiss_feed(app: St<'_>, slug: String) -> Res<FeedView> {
    blocking(&app, move |app| feed::dismiss_feed(app, &slug)).await
}

#[tauri::command]
pub async fn open_feed_job(app: St<'_>, slug: String) -> Res<JobRow> {
    feed::open_feed_job(&app, &slug).await
}

#[tauri::command]
pub async fn feed_settings(app: St<'_>) -> Res<FeedSettingsView> {
    blocking(&app, feed::feed_settings).await
}

#[tauri::command]
pub async fn set_feed_settings(app: St<'_>, settings: FeedSettingsDto) -> Res<FeedSettingsView> {
    blocking(&app, move |app| feed::set_feed_settings(app, settings)).await
}

//! The Tauri commands: `app/src/lib/api/index.ts` names them and their arguments. Each is async and runs its work
//! on the blocking pool or as a model-call task, so the UI thread never waits on SQLite, Typst or the agent.

use std::collections::HashMap;
use std::sync::Arc;

use tauri::{AppHandle, State};

use crate::dto::{
    Accepted, AgentStatus, AppError, AppStatus, ApplicationRow, CoverPart, CoverView, CustomEdit,
    Debrief, DebriefView, Decision, DigAnswer, DigCapture, DigQuestions, EditTargets, EntryEdit,
    EntryView, Exported, FeedSettingsDto, FeedSettingsView, FeedView, FolderPdf, FolderView,
    ImportChoices, ImportView, Instructed, JobRow, JobView, KeyDecision, Library, LogoRow,
    LogoSource, Nudge, Place, PracticeTurn, PrepView, Progress, ReviewView, Rewritten, Round,
    RoundDraft, RoundsView, SalaryRange, SearchCursor, SearchResults, SentView, Settings,
    StatusSet, StoriesView, StoryDraft, StoryView, UpdateStatus,
};
use crate::state::{App, blocking};
use crate::{
    cover, debrief, dig, entry, export, feed, folder as files, import, jobs, library as lib, logo,
    place, prep, review, rounds, settings, stories, update,
};

type Res<T> = Result<T, AppError>;
type St<'a> = State<'a, Arc<App>>;

// ── Settings and the agent ──

/// Every agent's status: Claude Code, Codex, Antigravity.
#[tauri::command]
pub async fn agent_statuses(app: St<'_>) -> Res<Vec<AgentStatus>> {
    Ok(settings::agent_statuses(&app).await)
}

#[tauri::command]
pub async fn get_settings(app: St<'_>) -> Res<Settings> {
    Ok(settings::get_settings(&app))
}

#[tauri::command]
pub async fn set_settings(app: St<'_>, settings: Settings) -> Res<Settings> {
    blocking(&app, move |app| settings::set_settings(app, settings)).await
}

/// Each asked `--model` alias or name → the model id that last served it.
#[tauri::command]
pub async fn models(app: St<'_>) -> Res<HashMap<String, String>> {
    blocking(&app, |app| app.served_models()).await
}

// ── Notifications ──

#[tauri::command]
pub async fn notification_permission() -> Res<crate::dto::NotificationPermission> {
    Ok(crate::notify::permission().await)
}

/// Asks macOS for permission (the prompt shows the first time only); only Turn on calls it.
#[tauri::command]
pub async fn request_notifications() -> Res<crate::dto::NotificationPermission> {
    Ok(crate::notify::request().await)
}

#[tauri::command]
pub async fn open_notification_settings(app: AppHandle) -> Res<()> {
    crate::notify::open_settings(&app)
}

// ── Startup ──

/// Why the data couldn't be opened at launch; `None` when it opened (the app then runs as usual).
#[tauri::command]
pub async fn startup_problem(app: AppHandle) -> Res<Option<crate::dto::StartupProblem>> {
    use tauri::Manager;
    Ok(app.state::<crate::state::Startup>().0.clone())
}

/// Quits and launches gliide again (Startup's Try again: setup runs anew).
#[tauri::command]
pub async fn relaunch(app: AppHandle) -> Res<()> {
    app.restart()
}

// ── Updates ──

#[tauri::command]
pub async fn update_status(app: AppHandle) -> Res<UpdateStatus> {
    Ok(update::status(&app))
}

#[tauri::command]
pub async fn check_update(app: AppHandle) -> Res<UpdateStatus> {
    Ok(update::check(&app).await)
}

/// Installs the release the last check found and relaunches the app; returns only when that failed.
#[tauri::command]
pub async fn install_update(app: AppHandle) -> Res<()> {
    update::install(&app).await
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
pub async fn add_entry(app: St<'_>, section: String, name: String) -> Res<String> {
    blocking(&app, move |app| entry::add_entry(app, &section, &name)).await
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
    fresh: bool,
) -> Res<ReviewView> {
    review::start_review(&app, &job, master, fresh).await
}

#[tauri::command]
pub async fn prepare_review(app: St<'_>, job: String) -> Res<()> {
    review::prepare_review(&app, &job).await
}

#[tauri::command]
pub async fn cancel(app: St<'_>, job: String) -> Res<()> {
    app.cancel(&job);
    Ok(())
}

#[tauri::command]
pub async fn running(app: St<'_>) -> Res<Vec<Progress>> {
    Ok(app.running())
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
pub async fn keep_fact(
    app: St<'_>,
    review: String,
    entry: String,
    skill: String,
) -> Res<ReviewView> {
    review::keep_fact(&app, &review, entry, skill).await
}

#[tauri::command]
pub async fn instruct(
    app: St<'_>,
    review: String,
    instruction: String,
    scope: Vec<String>,
    confirmed: Vec<String>,
) -> Res<Instructed> {
    review::instruct(&app, &review, instruction, scope, confirmed).await
}

#[tauri::command]
pub async fn accept(app: St<'_>, review: String, keys: Vec<String>) -> Res<Accepted> {
    review::accept(&app, &review, keys).await
}

#[tauri::command]
pub async fn reject(app: St<'_>, review: String, keys: Vec<String>) -> Res<ReviewView> {
    review::reject(&app, &review, keys).await
}

#[tauri::command]
pub async fn withdraw(
    app: St<'_>,
    review: String,
    keys: Vec<String>,
    restore: Vec<String>,
) -> Res<ReviewView> {
    review::withdraw(&app, &review, keys, restore).await
}

#[tauri::command]
pub async fn settle(app: St<'_>, review: String, decisions: Vec<KeyDecision>) -> Res<ReviewView> {
    review::settle(&app, &review, decisions).await
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
    applications: Vec<String>,
    status: AppStatus,
) -> Res<StatusSet> {
    blocking(&app, move |app| {
        export::set_status(app, &applications, status)
    })
    .await
}

// ── Interviews ──

#[tauri::command]
pub async fn rounds(app: St<'_>, application: String) -> Res<Vec<Round>> {
    blocking(&app, move |app| rounds::rounds(app, &application)).await
}

#[tauri::command]
pub async fn save_round(app: St<'_>, application: String, draft: RoundDraft) -> Res<RoundsView> {
    blocking(&app, move |app| {
        rounds::save_round(app, &application, draft)
    })
    .await
}

#[tauri::command]
pub async fn remove_round(app: St<'_>, round: String) -> Res<RoundsView> {
    blocking(&app, move |app| rounds::remove_round(app, &round)).await
}

#[tauri::command]
pub async fn restore_rounds(app: St<'_>, rounds: Vec<String>) -> Res<Vec<ApplicationRow>> {
    blocking(&app, move |app| crate::rounds::restore_rounds(app, &rounds)).await
}

#[tauri::command]
pub async fn read_invite(app: St<'_>, application: String, text: String) -> Res<RoundDraft> {
    rounds::read_invite(&app, &application, text).await
}

#[tauri::command]
pub async fn read_invite_file(app: St<'_>, path: String) -> Res<RoundDraft> {
    blocking(&app, move |_| rounds::read_invite_file(&path)).await
}

#[tauri::command]
pub async fn sent_resume(app: St<'_>, application: String, marked: Vec<String>) -> Res<SentView> {
    blocking(&app, move |app| {
        rounds::sent_resume(app, &application, &marked)
    })
    .await
}

// ── Prep and practice ──

#[tauri::command]
pub async fn prep(app: St<'_>, round: String, again: bool) -> Res<PrepView> {
    prep::prep(&app, &round, again).await
}

#[tauri::command]
pub async fn practice(
    app: St<'_>,
    round: String,
    question: String,
    answer: String,
) -> Res<PracticeTurn> {
    prep::practice(&app, &round, question, answer).await
}

#[tauri::command]
pub async fn nudge(
    app: St<'_>,
    round: String,
    question: String,
    draft: String,
) -> Res<Vec<Nudge>> {
    prep::nudge(&app, &round, question, draft).await
}

#[tauri::command]
pub async fn practice_turns(app: St<'_>, round: String) -> Res<Vec<PracticeTurn>> {
    blocking(&app, move |app| prep::practice_turns(app, &round)).await
}

#[tauri::command]
pub async fn keep_claim(
    app: St<'_>,
    round: Option<String>,
    entry: String,
    text: String,
) -> Res<()> {
    blocking(&app, move |app| {
        prep::keep_claim(app, round.as_deref(), &entry, &text)
    })
    .await
}

#[tauri::command]
pub async fn tell_story(
    app: St<'_>,
    story: String,
    asked: String,
    answer: String,
) -> Res<PracticeTurn> {
    stories::tell_story(&app, &story, asked, answer).await
}

#[tauri::command]
pub async fn story_turns(app: St<'_>, story: String) -> Res<Vec<PracticeTurn>> {
    blocking(&app, move |app| stories::story_turns(app, &story)).await
}

// ── Stories ──

#[tauri::command]
pub async fn stories(app: St<'_>) -> Res<StoriesView> {
    blocking(&app, stories::stories).await
}

#[tauri::command]
pub async fn story(app: St<'_>, id: String) -> Res<StoryView> {
    blocking(&app, move |app| stories::story(app, &id)).await
}

#[tauri::command]
pub async fn save_story(app: St<'_>, draft: StoryDraft) -> Res<StoryView> {
    blocking(&app, move |app| stories::save_story(app, draft)).await
}

#[tauri::command]
pub async fn remove_story(app: St<'_>, id: String) -> Res<()> {
    blocking(&app, move |app| stories::remove_story(app, &id)).await
}

// ── Dig ──

#[tauri::command]
pub async fn dig_questions(app: St<'_>, entry: String, again: bool) -> Res<DigQuestions> {
    dig::dig_questions(&app, &entry, again).await
}

#[tauri::command]
pub async fn dig_capture(
    app: St<'_>,
    entry: String,
    answers: Vec<DigAnswer>,
    note: String,
) -> Res<DigCapture> {
    dig::dig_capture(&app, &entry, answers, note).await
}

// ── Debriefs ──

#[tauri::command]
pub async fn debrief(app: St<'_>, round: String) -> Res<Option<DebriefView>> {
    blocking(&app, move |app| debrief::debrief(app, &round)).await
}

#[tauri::command]
pub async fn save_debrief(app: St<'_>, round: String, debrief: Debrief) -> Res<DebriefView> {
    blocking(&app, move |app| debrief::save_debrief(app, &round, debrief)).await
}

#[tauri::command]
pub async fn read_debrief(app: St<'_>, round: String) -> Res<DebriefView> {
    debrief::read_debrief(&app, &round).await
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

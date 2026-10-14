//! The job feed (ARCHITECTURE.md §7, Job feed): new jobs pulled at launch and every few hours from freehire, LinkedIn's
//! guest search, the watchlist's company boards and (once a day, with an Apify token) Fantastic.jobs, each kept or
//! skipped by the local check, then checked (read, Extract, every master scored) up to the day's cap, one at a time;
//! plus search, and checks you ask for.
//!
//! Settings → Feed is the `feed` setting ([`FeedSettings`]); the pull's progress is `feed_state` ([`FeedCursor`]).
//! Every change to a row or the status is sent as the `feed` event.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, FixedOffset, Local, SecondsFormat, TimeDelta, Utc};
use resume_core::Master;
use resume_core::feed::{FeedJob, Origin};
use resume_ingest::fantastic::{self, TimeRange};
use resume_ingest::freehire::{self, Query};
use resume_ingest::linkedin;
use resume_ingest::watch::{self, Board};
use resume_ingest::{HttpFetcher, IngestError};
use resume_pipeline::feed::{
    FANTASTIC_HOURS, FIRST_PULL_DAYS, FeedCursor, FeedSettings, PAGE, Profile, fantastic_query,
    linkedin_search, linkedin_searches, local_check, profile, query,
};
use resume_pipeline::today;
use resume_store::{FeedRow, FeedState, NewFeedJob, PostingRow, Store};

use crate::dto::{
    ErrorKind, FeedAuto, FeedItem, FeedItemState, FeedSettingsDto, FeedSettingsView, FeedStatus,
    FeedView, JobRow, MasterChoice, MasterRef, SearchCursor, SearchResults, WatchBoard,
};
use crate::error::{AppError, Result};
use crate::jobs;
use crate::state::{App, Event, blocking, lock};

/// The feed's settings key in the store.
const SETTINGS: &str = "feed";
/// The pull's cursor key in the store.
const CURSOR: &str = "feed_state";
/// A pull reads at most this many pages from freehire.
const MAX_PAGES: u32 = 10;
/// Pages a LinkedIn search reads at most (10 jobs a page).
const LINKEDIN_PAGES: u32 = 5;
/// Between LinkedIn requests, so a pull reads like a person paging.
const LINKEDIN_PAUSE: Duration = Duration::from_millis(700);
/// LinkedIn is searched at most this often, and left alone at least this long after it limits requests.
const LINKEDIN_EVERY: TimeDelta = TimeDelta::hours(1);
/// Search results a page.
const SEARCH_PAGE: u32 = 25;
/// How often the scheduler looks whether a pull is due.
const TICK: Duration = Duration::from_secs(60);

/// A feed row's company when neither freehire nor Extract names it.
const UNNAMED: &str = "Unnamed company";

/// The feed's run, in `App.feed`.
#[derive(Default)]
pub struct FeedRun {
    /// `pull_feed` is running (its pull, then its checks).
    pub busy: bool,
    /// The sources are being read.
    pub pulling: bool,
    /// The slug of the job being checked.
    pub checking: Option<String>,
    /// When the scheduler pulls next.
    pub next_pull: Option<DateTime<Utc>>,
    /// The latest search's jobs by slug, so `check_feed`, `open_feed_job` and `dismiss_feed` can add them to the
    /// feed.
    pub found: HashMap<String, FeedJob>,
    /// `logo::fetch_logos` is running.
    pub fetching_logos: bool,
    /// `logo::fetch_logos` was started again while it ran: it goes once more.
    pub logos_again: bool,
}

/// Clears `busy` and `pulling` however `pull_feed` ends.
struct Running(Arc<App>);

impl Drop for Running {
    fn drop(&mut self) {
        let mut run = lock(&self.0.feed);
        run.busy = false;
        run.pulling = false;
    }
}

// ── The feed and its status ──

/// Every row but skipped and dismissed ones: fits (best score first), not fits, the one being checked, waiting, new.
pub fn feed(app: &App) -> Result<FeedView> {
    let checking = lock(&app.feed).checking.clone();
    let s = settings(app)?;
    let items = {
        let store = lock(&app.store);
        let titles = titles(&store)?;
        let rows = store.feed()?;
        let mut rows: Vec<(FeedItemState, &FeedRow)> = rows
            .iter()
            .map(|r| (state(r, s.threshold, checking.as_deref()), r))
            .filter(|(st, _)| !matches!(st, FeedItemState::Skipped | FeedItemState::Dismissed))
            .collect();
        rows.sort_by(|(sa, a), (sb, b)| {
            rank(*sa)
                .cmp(&rank(*sb))
                .then(b.score.unwrap_or(-1.0).total_cmp(&a.score.unwrap_or(-1.0)))
                .then(queue_order(a, b))
        });
        rows.into_iter()
            .map(|(st, r)| item(&store, r, st, &titles))
            .collect::<Result<Vec<_>>>()?
    };
    Ok(FeedView {
        items,
        status: status(app)?,
    })
}

pub fn status(app: &App) -> Result<FeedStatus> {
    let s = settings(app)?;
    let cursor = cursor(app)?;
    let checked_today = lock(&app.store).feed_checked_since(&midnight())?;
    let run = lock(&app.feed);
    Ok(FeedStatus {
        enabled: s.enabled,
        linkedin: s.linkedin,
        pulling: run.pulling,
        checking: run.checking.clone(),
        last_pull: cursor.last_pull.clone(),
        next_pull: run.next_pull.filter(|_| s.enabled).map(stamp),
        checked_today,
        daily_checks: s.daily_checks,
        threshold: s.threshold,
        paused: linkedin_paused(&s, &cursor, Utc::now()),
        error: cursor.error,
    })
}

/// While LinkedIn is backed off after limiting requests: says when it's asked again.
fn linkedin_paused(s: &FeedSettings, cursor: &FeedCursor, now: DateTime<Utc>) -> Option<String> {
    let until = cursor
        .linkedin_retry_at
        .as_deref()
        .and_then(parse_time)
        .filter(|t| s.linkedin && *t > now)?;
    Some(format!(
        "LinkedIn is limiting requests; the feed asks it again after {}.",
        until.with_timezone(&Local).format("%-I:%M %p")
    ))
}

/// LinkedIn is asked at most hourly (its searches look back an hour at the least), and not while it's backed off.
fn linkedin_due(cursor: &FeedCursor, now: DateTime<Utc>) -> bool {
    let passed =
        |t: Option<&str>, after: TimeDelta| t.and_then(parse_time).is_none_or(|t| now - t >= after);
    passed(cursor.linkedin_retry_at.as_deref(), TimeDelta::zero())
        && passed(cursor.linkedin_at.as_deref(), LINKEDIN_EVERY)
}

/// How long LinkedIn is left alone after `e`: a rate limit its `Retry-After` and at least an hour, a refusal 6 hours;
/// None for anything else (a network failure is an error, asked again next pull).
fn linkedin_backoff(e: &IngestError) -> Option<TimeDelta> {
    match e {
        IngestError::RateLimited { retry_after, .. } => {
            let asked = retry_after
                .and_then(|d| TimeDelta::from_std(d).ok())
                .unwrap_or_default();
            Some(asked.max(LINKEDIN_EVERY))
        }
        IngestError::Blocked { .. } => Some(TimeDelta::hours(6)),
        _ => None,
    }
}

/// Sends the `feed` event with the status now.
pub(crate) fn changed(app: &App) {
    if let Ok(s) = status(app) {
        (app.emit)(Event::Feed(&s));
    }
}

// ── Pull ──

/// Pulls new jobs from every source that's on, then checks the queue up to the day's cap; the feed after both. A pull
/// already running isn't joined: this returns the feed as it is at once (`status.pulling`, and `feed` events as it goes
/// on). Failures go to the status's `error`, a sentence per source, not to the caller. The scheduler runs the same.
pub async fn pull_feed(app: &Arc<App>) -> Result<FeedView> {
    let started = {
        let mut run = lock(&app.feed);
        let free = !run.busy;
        if free {
            run.busy = true;
            run.pulling = true;
        }
        free
    };
    if !started {
        return blocking(app, feed).await;
    }
    changed(app);
    let running = Running(Arc::clone(app));
    let task = tokio::spawn(async move {
        let app = Arc::clone(&running.0);
        let pulled = pull(&app).await;
        lock(&app.feed).pulling = false;
        changed(&app);
        if pulled.is_ok() {
            tokio::spawn(crate::logo::fetch_logos(Arc::clone(&app)));
        }
        let result = match pulled {
            Ok(()) => drain(&app).await,
            Err(e) => Err(e),
        };
        if let Err(e) = result {
            let _ = add_error(&app, e.message);
        }
        let hours = settings(&app).map_or(FeedSettings::default().every_hours, |s| s.every_hours);
        lock(&app.feed).next_pull = Some(Utc::now() + hours_of(hours));
        drop(running);
        changed(&app);
    });
    task.await?;
    blocking(app, feed).await
}

/// Pulls at launch, then every `every_hours` while the feed is on; the settings are read again each minute.
pub async fn schedule(app: Arc<App>) {
    lock(&app.feed).next_pull = Some(Utc::now());
    loop {
        let on = settings(&app).is_ok_and(|s| s.enabled);
        let due = lock(&app.feed).next_pull.is_some_and(|t| t <= Utc::now());
        if on && due {
            let _ = pull_feed(&app).await;
        }
        tokio::time::sleep(TICK).await;
    }
}

/// Reads every source that's on: freehire newest first until a job the last pull saw (the first pull: the last few
/// days), LinkedIn's searches for jobs posted since its last answer, every watchlist board, and Fantastic.jobs once a
/// day. Jobs the feed doesn't have yet go through the local check and are added; a source that fails is named in the
/// cursor's error and the others still count.
async fn pull(app: &Arc<App>) -> Result<()> {
    let (s, mut cursor, profile, known) = blocking(app, |app| {
        let s = settings(app)?;
        let cursor = cursor(app)?;
        let profile = profile_now(app, &s)?;
        let known = known_urls(&lock(&app.store))?;
        Ok((s, cursor, profile, known))
    })
    .await?;
    let q = query(&s, &profile);
    let fetcher = HttpFetcher::new();
    let now = Utc::now();
    let found_now = stamp(now);
    let mut jobs = Vec::new();
    let mut errors = Vec::new();

    match pull_freehire(&fetcher, &q, cursor.found_at.as_deref()).await {
        Ok(found) => {
            if let Some(newest) = found
                .iter()
                .map(|j| j.found_at.as_str())
                .reduce(|a, b| if later(b, a) { b } else { a })
            {
                cursor.found_at = Some(newest.to_owned());
            }
            jobs.extend(found);
        }
        Err(e) => errors.push(pull_error("freehire", &e)),
    }

    if s.linkedin && linkedin_due(&cursor, now) {
        let within = since(cursor.linkedin_at.as_deref(), now);
        let searches = linkedin_searches(&s, &profile, within);
        let (found, failed) = pull_linkedin(&fetcher, &searches, &found_now).await;
        jobs.extend(found);
        match failed {
            // Being limited isn't a failure to fix: LinkedIn is left alone for a while, and the status says until when.
            Some(e) => match linkedin_backoff(&e) {
                Some(wait) => cursor.linkedin_retry_at = Some(stamp(now + wait)),
                None => errors.push(pull_error("LinkedIn", &e)),
            },
            None => {
                cursor.linkedin_retry_at = None;
                if !searches.is_empty() {
                    cursor.linkedin_at = Some(found_now.clone());
                }
            }
        }
    }

    for link in &s.watchlist {
        let Some(board) = Board::parse(link) else {
            errors.push(format!("{link} isn't a board gliide can watch."));
            continue;
        };
        match watch::list(&fetcher, &board, &found_now).await {
            Ok(found) => jobs.extend(found),
            Err(e) => errors.push(pull_error(
                &format!("{} ({})", board.name(), ats(&board)),
                &e,
            )),
        }
    }

    if let Some(token) = s
        .apify_token
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
    {
        let last = cursor.fantastic_at.as_deref().and_then(parse_time);
        let day = TimeDelta::hours(FANTASTIC_HOURS);
        if last.is_none_or(|t| now - t >= day) {
            // A day missed (the app was closed) is caught up with a week's jobs; jobs seen before are left out by slug.
            let range = match last {
                Some(t) if now - t > day + TimeDelta::hours(1) => TimeRange::Week,
                _ => TimeRange::Day,
            };
            if let Some(fq) = fantastic_query(&s, &profile, range) {
                match fantastic::pull(&fetcher, token, &fq).await {
                    Ok(found) => {
                        cursor.fantastic_at = Some(found_now.clone());
                        jobs.extend(found);
                    }
                    Err(e) => errors.push(pull_error("Fantastic.jobs", &e)),
                }
            }
        }
    }

    blocking(app, move |app| {
        let store = lock(&app.store);
        let rows = store.feed()?;
        let mut slugs: HashSet<String> = rows.iter().map(|r| r.job.slug.clone()).collect();
        // The same role posted once per place, or found by two sources: the first one found stands for the others.
        let mut roles: HashSet<(String, String)> = rows.iter().map(|r| role_key(&r.job)).collect();
        let mut urls: HashSet<String> = rows.iter().map(|r| r.job.url.clone()).collect();
        let new: Vec<NewFeedJob> = jobs
            .into_iter()
            .filter(|job| slugs.insert(job.slug.clone()))
            .map(|job| {
                let check = local_check(&job, &profile, &q, &known);
                let (keep, reason) = if !check.keep {
                    (false, check.reason)
                } else if !urls.insert(job.url.clone()) {
                    (
                        false,
                        Some("Another source found this job already.".to_owned()),
                    )
                } else if !roles.insert(role_key(&job)) {
                    (
                        false,
                        Some("The same role at another place is in the feed already.".to_owned()),
                    )
                } else {
                    (true, None)
                };
                NewFeedJob {
                    state: if keep {
                        FeedState::New
                    } else {
                        FeedState::Skipped
                    },
                    overlap: check.overlap,
                    reason,
                    job,
                }
            })
            .collect();
        cursor.last_pull = Some(stamp(Utc::now()));
        cursor.error = (!errors.is_empty()).then(|| errors.join(" "));
        store.add_feed_jobs(&new)?;
        store.put_setting(CURSOR, &cursor)?;
        Ok(())
    })
    .await
}

/// freehire's jobs newest first, down to the one recorded at `cursor` (none: the last few days).
async fn pull_freehire(
    fetcher: &HttpFetcher,
    q: &Query,
    cursor: Option<&str>,
) -> std::result::Result<Vec<FeedJob>, IngestError> {
    let mut q = q.clone();
    if cursor.is_none() {
        q.open_within_days = Some(FIRST_PULL_DAYS);
    }
    let mut jobs = Vec::new();
    for page in 0..MAX_PAGES {
        let offset = page * PAGE;
        let found = freehire::search(fetcher, &q, PAGE, offset).await?;
        let full = found.jobs.len() as u32 == PAGE;
        let mut reached = false;
        for job in found.jobs {
            if cursor.is_some_and(|c| !later(&job.found_at, c)) {
                reached = true;
                break;
            }
            jobs.push(job);
        }
        if reached || !full || u64::from(offset + PAGE) >= found.total {
            break;
        }
    }
    Ok(jobs)
}

/// Every search's pages until one comes back short. A refusal or rate limit ends every search: the jobs found until
/// then are kept, beside the error.
async fn pull_linkedin(
    fetcher: &HttpFetcher,
    searches: &[linkedin::Search],
    now: &str,
) -> (Vec<FeedJob>, Option<IngestError>) {
    let mut jobs = Vec::new();
    let mut first = true;
    for search in searches {
        for page in 0..LINKEDIN_PAGES {
            if !first {
                tokio::time::sleep(LINKEDIN_PAUSE).await;
            }
            first = false;
            match linkedin::search(fetcher, search, page * linkedin::PAGE, now).await {
                Ok(found) => {
                    let full = found.len() as u32 >= linkedin::PAGE;
                    jobs.extend(found);
                    if !full {
                        break;
                    }
                }
                Err(e) => return (jobs, Some(e)),
            }
        }
    }
    (jobs, None)
}

/// Seconds of postings a LinkedIn search asks for: since its last answer and an hour more, at least an hour and at
/// most a week; the first search looks back as far as freehire's first pull.
fn since(last: Option<&str>, now: DateTime<Utc>) -> u32 {
    const HOUR: i64 = 3600;
    let secs = match last.and_then(parse_time) {
        Some(t) => (now - t).num_seconds() + HOUR,
        None => i64::from(FIRST_PULL_DAYS) * 24 * HOUR,
    };
    secs.clamp(HOUR, 7 * 24 * HOUR) as u32
}

/// A source's failure in a pull, as one sentence of the feed's status: `{source}: …`. Refusals and rate limits say the
/// next pull tries again (there's nothing to paste).
fn pull_error(source: &str, e: &IngestError) -> String {
    source_error(source, e, "the next pull tries again")
}

/// `{source}: …`, a refusal or rate limit ending with `then` instead of the paste advice.
fn source_error(source: &str, e: &IngestError, then: &str) -> String {
    let what = match e {
        IngestError::RateLimited { host, .. } => {
            format!("{host} is limiting requests (HTTP 429); {then}")
        }
        IngestError::Blocked { host, status } => {
            format!("{host} refused the request (HTTP {status}); {then}")
        }
        other => other.to_string(),
    };
    let end = if what.ends_with('.') { "" } else { "." };
    format!("{source}: {what}{end}")
}

/// The ATS as a person reads it.
fn ats(board: &Board) -> &'static str {
    match board.source() {
        "greenhouse" => "Greenhouse",
        "lever" => "Lever",
        "ashby" => "Ashby",
        "workday" => "Workday",
        "smartrecruiters" => "SmartRecruiters",
        "workable" => "Workable",
        "recruitee" => "Recruitee",
        other => other,
    }
}

// ── Checks ──

/// Checks `New` and `Waiting` rows until the day's checks are used up: the rest wait for the next pull. The checks are
/// shared between sources: each next one goes to the source with the fewest checks today among those with jobs waiting,
/// its best overlap first (then the employer's newest). Stops at the first failure that isn't the job's own (`claude`,
/// the network).
async fn drain(app: &Arc<App>) -> Result<()> {
    let mut tried = HashSet::new();
    loop {
        let _lane = app.feed_lane.lock().await;
        let (next, back) = blocking(app, move |app| {
            let next = next_in_queue(app, &tried)?;
            Ok((next, tried))
        })
        .await?;
        tried = back;
        let Some(slug) = next else {
            changed(app);
            return Ok(());
        };
        tried.insert(slug.clone());
        match check(app, &slug).await {
            Err(e) if e.kind != ErrorKind::Cancelled => return Err(e),
            _ => {}
        }
    }
}

/// The queue's next slug (not in `tried`), or `None` when it's empty or the day's checks are used up; then every
/// `New` row waits.
fn next_in_queue(app: &App, tried: &HashSet<String>) -> Result<Option<String>> {
    let s = settings(app)?;
    let store = lock(&app.store);
    let since = midnight();
    let used = store.feed_checked_since(&since)?;
    let rows = store.feed()?;
    let queue: Vec<&FeedRow> = rows
        .iter()
        .filter(|r| matches!(r.state, FeedState::New | FeedState::Waiting))
        .filter(|r| !tried.contains(&r.job.slug))
        .collect();
    if used >= s.daily_checks {
        for r in queue.iter().filter(|r| r.state == FeedState::New) {
            store.set_feed_state(&r.job.slug, FeedState::Waiting, None)?;
        }
        return Ok(None);
    }
    let mut checked: HashMap<Origin, usize> = HashMap::new();
    for r in &rows {
        if r.checked_at.as_deref().is_some_and(|t| !later(&since, t)) {
            *checked.entry(r.job.origin).or_default() += 1;
        }
    }
    let next = Origin::ALL
        .iter()
        .filter(|o| queue.iter().any(|r| r.job.origin == **o))
        .min_by_key(|o| checked.get(o).copied().unwrap_or(0))
        .and_then(|o| {
            queue
                .iter()
                .filter(|r| r.job.origin == *o)
                .min_by(|a, b| queue_order(a, b))
        });
    Ok(next.map(|r| r.job.slug.clone()))
}

/// Checks the jobs now, in order and past the day's cap (search results not in the feed are added first); the feed
/// after.
pub async fn check_feed(app: &Arc<App>, slugs: Vec<String>) -> Result<FeedView> {
    {
        let slugs = slugs.clone();
        blocking(app, move |app| add_found(app, &slugs)).await?;
    }
    changed(app);
    let run = Arc::clone(app);
    tokio::spawn(async move {
        let _lane = run.feed_lane.lock().await;
        for slug in slugs {
            let state = lock(&run.store).feed_row(&slug)?.map(|r| r.state);
            if state != Some(FeedState::Checked) {
                check(&run, &slug).await?;
            }
        }
        Ok::<_, AppError>(())
    })
    .await??;
    blocking(app, feed).await
}

/// Checks one job (the caller holds `feed_lane`): its posting read and stored, Extract, every master scored. A posting
/// that can't be read is skipped with the reason; any other failure leaves it waiting, with the reason unless you
/// cancelled it.
async fn check(app: &Arc<App>, slug: &str) -> Result<()> {
    lock(&app.feed).checking = Some(slug.to_owned());
    changed(app);
    let result = check_job(app, slug).await;
    lock(&app.feed).checking = None;
    tokio::spawn(crate::logo::fetch_logos(Arc::clone(app)));
    if let Err(e) = &result {
        let reason = (e.kind != ErrorKind::Cancelled).then_some(e.message.as_str());
        let store = lock(&app.store);
        if let Some(r) = store.feed_row(slug)?
            && matches!(r.state, FeedState::New | FeedState::Waiting)
        {
            store.set_feed_state(slug, FeedState::Waiting, reason)?;
        }
    }
    changed(app);
    result
}

async fn check_job(app: &Arc<App>, slug: &str) -> Result<()> {
    let row = {
        let slug = slug.to_owned();
        blocking(app, move |app| row(app, &slug)).await?
    };
    let posting = match read_posting(app, &row).await? {
        Ok((_, true)) => {
            lock(&app.store).set_feed_state(
                slug,
                FeedState::Skipped,
                Some("Same posting as another job in the feed, at another place."),
            )?;
            return Ok(());
        }
        Ok((p, false)) => p,
        Err(e @ (IngestError::Network { .. } | IngestError::RateLimited { .. })) => {
            return Err(e.into());
        }
        Err(e) => {
            lock(&app.store).set_feed_state(slug, FeedState::Skipped, Some(&e.to_string()))?;
            return Ok(());
        }
    };
    let req = jobs::requirements(app, &posting).await?;
    let slug = slug.to_owned();
    let was = row.state;
    blocking(app, move |app| {
        let masters = jobs::ranked(app.masters_now()?.as_ref(), &req);
        let s = settings(app)?;
        let best = best(&s, &masters);
        let store = lock(&app.store);
        let now = store.feed_row(&slug)?.ok_or_else(gone)?;
        if now.state == FeedState::Dismissed && was != FeedState::Dismissed {
            // Dismissed while it was checked: its posting goes, as dismissing it would have done.
            if let (Some(id), None) = (&now.posting_id, &now.opened_at) {
                store.delete_posting(id)?;
            }
            return Ok(());
        }
        store.set_feed_checked(&slug, best.map(|m| m.score), best.map(|m| m.id.as_str()))?;
        Ok(())
    })
    .await
}

/// The row's posting: the stored one, else read (`resume_ingest::feed::read_job`) and stored. `Err` inside: it couldn't
/// be read.
/// `true` beside it: another feed row holds the same posting already (one job posted per place with the same text),
/// so this row doesn't take it. A posting you had before otherwise (the same text, pasted) is marked opened, so it
/// stays in Recent jobs.
async fn read_posting(
    app: &Arc<App>,
    row: &FeedRow,
) -> Result<std::result::Result<(PostingRow, bool), IngestError>> {
    if let Some(id) = &row.posting_id
        && let Some(p) = lock(&app.store).posting(id)?
    {
        return Ok(Ok((p, false)));
    }
    let posting = match resume_ingest::feed::read_job(&HttpFetcher::new(), &row.job).await {
        Ok(p) => p,
        Err(e) => return Ok(Err(e)),
    };
    let slug = row.job.slug.clone();
    blocking(app, move |app| {
        let store = lock(&app.store);
        let id = store.put_posting(&posting)?;
        let stored = store.posting(&id)?.ok_or_else(AppError::no_posting)?;
        let held = store
            .feed()?
            .iter()
            .any(|r| r.job.slug != slug && r.posting_id.as_deref() == Some(id.as_str()));
        if !held {
            store.set_feed_posting(&slug, &id)?;
            if stored.created_at != stored.updated_at {
                store.set_feed_opened(&id)?;
            }
        }
        Ok(Ok((stored, held)))
    })
    .await
}

/// You opened `posting` (`jobs::open_job`, after Extract): the feed row holding it, if any, is marked opened, and
/// checked with `masters` when it wasn't yet, so the queue skips it.
pub fn opened(app: &App, posting: &str, masters: &[MasterChoice]) -> Result<()> {
    let s = settings(app)?;
    {
        let store = lock(&app.store);
        store.set_feed_opened(posting)?;
        let Some(row) = store
            .feed()?
            .into_iter()
            .find(|r| r.posting_id.as_deref() == Some(posting))
        else {
            return Ok(());
        };
        if matches!(row.state, FeedState::New | FeedState::Waiting) {
            let best = best(&s, masters);
            store.set_feed_checked(
                &row.job.slug,
                best.map(|m| m.score),
                best.map(|m| m.id.as_str()),
            )?;
        }
    }
    changed(app);
    Ok(())
}

// ── Search, open, dismiss ──

/// One page of jobs matching `text` under the feed's filters: freehire's in its order, then LinkedIn's (when it's on
/// and not backed off) from `cursor` (none: the first page). Nothing is checked. freehire failing fails the page;
/// LinkedIn failing ends LinkedIn's part of the search and says why in `error` (being limited backs it off for pulls
/// too).
pub async fn search_feed(
    app: &Arc<App>,
    text: String,
    cursor: Option<SearchCursor>,
) -> Result<SearchResults> {
    let first = cursor.is_none();
    let (q, searches, paused) = blocking(app, move |app| {
        let s = settings(app)?;
        let profile = profile_now(app, &s)?;
        let paused = linkedin_paused(&s, &self::cursor(app)?, Utc::now());
        let words = text.trim().to_owned();
        let mut q = query(&s, &profile);
        q.q = Some(words.clone()).filter(|t| !t.is_empty());
        q.newest_first = false;
        let searches = if s.linkedin && paused.is_none() && !words.is_empty() {
            linkedin_search(&s, &profile, &words, None)
        } else {
            vec![]
        };
        Ok((q, searches, paused))
    })
    .await?;
    let at = cursor.unwrap_or(SearchCursor {
        freehire: Some(0),
        linkedin: (!searches.is_empty()).then_some(0),
        linkedin_found: 0,
        freehire_total: 0,
    });
    let fetcher = HttpFetcher::new();
    let mut jobs = Vec::new();
    let mut total = at.freehire_total;
    let mut next_freehire = None;
    if let Some(offset) = at.freehire {
        let page = freehire::search(&fetcher, &q, SEARCH_PAGE, offset).await?;
        let end = offset + page.jobs.len() as u32;
        next_freehire =
            (page.jobs.len() as u32 == SEARCH_PAGE && u64::from(end) < page.total).then_some(end);
        total = page.total;
        jobs.extend(page.jobs);
    }
    let mut error = paused;
    let mut next_linkedin = None;
    let mut linkedin_found = at.linkedin_found;
    if let Some(start) = at.linkedin {
        let mut full = false;
        for search in &searches {
            match linkedin::search(&fetcher, search, start, &stamp(Utc::now())).await {
                Ok(found) => {
                    full |= found.len() as u32 >= linkedin::PAGE;
                    linkedin_found += found.len() as u32;
                    jobs.extend(found);
                }
                Err(e) => {
                    error = Some(source_error("LinkedIn", &e, "search again in a while"));
                    if let Some(wait) = linkedin_backoff(&e) {
                        let until = stamp(Utc::now() + wait);
                        blocking(app, move |app| {
                            let mut c = self::cursor(app)?;
                            c.linkedin_retry_at = Some(until);
                            lock(&app.store).put_setting(CURSOR, &c)?;
                            changed(app);
                            Ok(())
                        })
                        .await?;
                    }
                    full = false;
                    break;
                }
            }
        }
        next_linkedin = full.then_some(start + linkedin::PAGE);
    }
    let next = (next_freehire.is_some() || next_linkedin.is_some()).then_some(SearchCursor {
        freehire: next_freehire,
        linkedin: next_linkedin,
        linkedin_found,
        freehire_total: total,
    });
    {
        let mut run = lock(&app.feed);
        if first {
            run.found.clear();
        }
        run.found
            .extend(jobs.iter().map(|j| (j.slug.clone(), j.clone())));
    }
    let checking = lock(&app.feed).checking.clone();
    blocking(app, move |app| {
        let threshold = settings(app)?.threshold;
        let store = lock(&app.store);
        let titles = titles(&store)?;
        let items = jobs
            .iter()
            .map(|job| match store.feed_row(&job.slug)? {
                Some(r) => item(
                    &store,
                    &r,
                    state(&r, threshold, checking.as_deref()),
                    &titles,
                ),
                None => Ok(job_item(job, FeedItemState::New)),
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(SearchResults {
            items,
            total: total + u64::from(linkedin_found),
            next,
            error,
        })
    })
    .await
}

/// The job's posting, read and stored when it isn't yet (a search result joins the feed first), marked opened: from
/// now on it's in Recent jobs and Applications.
pub async fn open_feed_job(app: &Arc<App>, slug: &str) -> Result<JobRow> {
    let row = {
        let slug = slug.to_owned();
        blocking(app, move |app| {
            add_found(app, std::slice::from_ref(&slug))?;
            row(app, &slug)
        })
        .await?
    };
    let (posting, _) = read_posting(app, &row).await??;
    blocking(app, move |app| {
        lock(&app.store).set_feed_opened(&posting.id)?;
        changed(app);
        jobs::job_row(app, &posting, app.masters_now()?.as_ref())
    })
    .await
}

/// Dismisses the job (a search result joins the feed first): its check stops, and its posting is deleted when you
/// never opened it.
pub fn dismiss_feed(app: &App, slug: &str) -> Result<FeedView> {
    add_found(app, &[slug.to_owned()])?;
    let row = row(app, slug)?;
    lock(&app.store).set_feed_state(slug, FeedState::Dismissed, None)?;
    if let Some(id) = &row.posting_id {
        app.cancel(id);
        if row.opened_at.is_none() {
            lock(&app.store).delete_posting(id)?;
        }
    }
    changed(app);
    feed(app)
}

/// Adds the latest search's jobs among `slugs` that the feed doesn't have, as `New` (you asked for them: the local
/// check only orders them).
fn add_found(app: &App, slugs: &[String]) -> Result<()> {
    let missing: Vec<&String> = {
        let store = lock(&app.store);
        let mut missing = vec![];
        for slug in slugs {
            if store.feed_row(slug)?.is_none() {
                missing.push(slug);
            }
        }
        missing
    };
    if missing.is_empty() {
        return Ok(());
    }
    let jobs = {
        let run = lock(&app.feed);
        missing
            .iter()
            .map(|s| run.found.get(*s).cloned().ok_or_else(gone))
            .collect::<Result<Vec<_>>>()?
    };
    let s = settings(app)?;
    let profile = profile_now(app, &s).ok();
    let pull = profile.as_ref().map(|p| query(&s, p));
    let store = lock(&app.store);
    let known = known_urls(&store)?;
    let new: Vec<NewFeedJob> = jobs
        .into_iter()
        .map(|job| NewFeedJob {
            overlap: profile
                .as_ref()
                .zip(pull.as_ref())
                .map_or(0.0, |(p, q)| local_check(&job, p, q, &known).overlap),
            state: FeedState::New,
            reason: None,
            job,
        })
        .collect();
    store.add_feed_jobs(&new)?;
    Ok(())
}

// ── Settings ──

pub fn feed_settings(app: &App) -> Result<FeedSettingsView> {
    let s = settings(app)?;
    let auto = match profile_now(app, &s) {
        Ok(p) => FeedAuto {
            years: p.years,
            seniority: p.seniority,
            employment: p.employment,
            category: p.category,
            countries: p.countries,
        },
        Err(_) => FeedAuto::default(),
    };
    let masters = lock(&app.store)
        .masters()?
        .into_iter()
        .map(|(_, m)| MasterRef {
            id: m.id,
            title: m.title,
        })
        .collect();
    let boards = s
        .watchlist
        .iter()
        .filter_map(|link| Board::parse(link))
        .map(|b| WatchBoard {
            url: b.url(),
            ats: ats(&b).to_owned(),
            name: b.name().to_owned(),
        })
        .collect();
    Ok(FeedSettingsView {
        settings: s.into(),
        auto,
        masters,
        boards,
    })
}

/// Saves the feed's settings (custom lists trimmed and lowercased, watchlist links as their boards' careers links, each
/// once; the Apify token trimmed, empty is none); a new schedule counts from the last pull.
pub fn set_feed_settings(app: &App, dto: FeedSettingsDto) -> Result<FeedSettingsView> {
    let mut s = FeedSettings::from(dto);
    if !(0.0..=100.0).contains(&s.threshold) {
        return Err(AppError::other("Set the fit threshold between 0 and 100%"));
    }
    if s.every_hours == 0 {
        return Err(AppError::other("Pulls need at least an hour between them"));
    }
    for list in [
        &mut s.seniority,
        &mut s.employment,
        &mut s.category,
        &mut s.countries,
    ]
    .into_iter()
    .flatten()
    {
        *list = list
            .iter()
            .map(|v| v.trim().to_lowercase())
            .filter(|v| !v.is_empty())
            .collect();
    }
    let mut watchlist: Vec<String> = Vec::new();
    for link in s
        .watchlist
        .iter()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
    {
        let board = Board::parse(link).ok_or_else(|| {
            AppError::other(format!(
                "{link} isn't a careers page gliide can watch: use a Greenhouse, Lever, Ashby, Workday, \
                 SmartRecruiters, Workable or Recruitee link"
            ))
        })?;
        let url = board.url();
        if !watchlist.contains(&url) {
            watchlist.push(url);
        }
    }
    s.watchlist = watchlist;
    s.apify_token = s
        .apify_token
        .map(|t| t.trim().to_owned())
        .filter(|t| !t.is_empty());
    let old = settings(app)?;
    {
        let store = lock(&app.store);
        if let Some(id) = &s.master
            && store.master(id)?.is_none()
        {
            return Err(AppError::missing("This master was deleted; choose another"));
        }
        store.put_setting(SETTINGS, &s)?;
    }
    {
        let mut run = lock(&app.feed);
        if let Some(t) = run.next_pull {
            run.next_pull = Some(t - hours_of(old.every_hours) + hours_of(s.every_hours));
        }
    }
    changed(app);
    feed_settings(app)
}

impl From<FeedSettings> for FeedSettingsDto {
    fn from(s: FeedSettings) -> Self {
        Self {
            enabled: s.enabled,
            master: s.master,
            seniority: s.seniority,
            employment: s.employment,
            category: s.category,
            countries: s.countries,
            needs_visa: s.needs_visa,
            include_clearance: s.include_clearance,
            include_unknown_location: s.include_unknown_location,
            daily_checks: s.daily_checks,
            threshold: s.threshold,
            every_hours: s.every_hours,
            linkedin: s.linkedin,
            watchlist: s.watchlist,
            apify_token: s.apify_token,
        }
    }
}

impl From<FeedSettingsDto> for FeedSettings {
    fn from(s: FeedSettingsDto) -> Self {
        Self {
            enabled: s.enabled,
            master: s.master,
            seniority: s.seniority,
            employment: s.employment,
            category: s.category,
            countries: s.countries,
            needs_visa: s.needs_visa,
            include_clearance: s.include_clearance,
            include_unknown_location: s.include_unknown_location,
            daily_checks: s.daily_checks,
            threshold: s.threshold,
            every_hours: s.every_hours,
            linkedin: s.linkedin,
            watchlist: s.watchlist,
            apify_token: s.apify_token,
        }
    }
}

// ── Helpers ──

fn settings(app: &App) -> Result<FeedSettings> {
    Ok(lock(&app.store)
        .setting::<FeedSettings>(SETTINGS)?
        .unwrap_or_default())
}

fn cursor(app: &App) -> Result<FeedCursor> {
    Ok(lock(&app.store)
        .setting::<FeedCursor>(CURSOR)?
        .unwrap_or_default())
}

/// Adds `error` to the last pull's (a check that failed after it).
fn add_error(app: &App, error: String) -> Result<()> {
    let mut c = cursor(app)?;
    c.error = Some(match c.error {
        Some(before) => format!("{before} {error}"),
        None => error,
    });
    lock(&app.store).put_setting(CURSOR, &c)?;
    Ok(())
}

fn row(app: &App, slug: &str) -> Result<FeedRow> {
    lock(&app.store).feed_row(slug)?.ok_or_else(gone)
}

fn gone() -> AppError {
    AppError::missing("This job isn't in the feed any more; search for it again")
}

/// What Auto works out from the bank and the feed's master (every master when it names none).
fn profile_now(app: &App, s: &FeedSettings) -> Result<Profile> {
    let (bank, masters) = {
        let store = lock(&app.store);
        (store.bank()?, store.masters()?)
    };
    let Some((_, bank)) = bank else {
        return Err(AppError::no_bank());
    };
    let masters: Vec<Master> = masters.into_iter().map(|(_, m)| m).collect();
    let chosen: Vec<&Master> = masters
        .iter()
        .filter(|m| s.master.as_ref().is_none_or(|id| &m.id == id))
        .collect();
    let chosen = if chosen.is_empty() {
        masters.iter().collect()
    } else {
        chosen
    };
    Ok(profile(&bank, &chosen, today()))
}

/// The employer links (`utm_*` removed) of every posting you have: the local check drops jobs among them.
fn known_urls(store: &Store) -> Result<HashSet<String>> {
    Ok(store
        .postings()?
        .iter()
        .flat_map(|p| [p.posting.url.as_deref(), p.posting.source.as_deref()])
        .flatten()
        .map(freehire::without_utm)
        .collect())
}

/// The best master for the feed: its own master when it names one that was scored, else the best of all.
fn best<'a>(s: &FeedSettings, masters: &'a [MasterChoice]) -> Option<&'a MasterChoice> {
    s.master
        .as_ref()
        .and_then(|id| masters.iter().find(|m| &m.id == id))
        .or_else(|| masters.first())
}

fn titles(store: &Store) -> Result<HashMap<String, String>> {
    Ok(store
        .masters()?
        .into_iter()
        .map(|(_, m)| (m.id, m.title))
        .collect())
}

fn state(row: &FeedRow, threshold: f64, checking: Option<&str>) -> FeedItemState {
    if checking == Some(row.job.slug.as_str()) {
        return FeedItemState::Checking;
    }
    match row.state {
        FeedState::New => FeedItemState::New,
        FeedState::Skipped => FeedItemState::Skipped,
        FeedState::Waiting => FeedItemState::Waiting,
        FeedState::Dismissed => FeedItemState::Dismissed,
        FeedState::Checked if row.score.is_some_and(|s| s >= threshold) => FeedItemState::Fit,
        FeedState::Checked => FeedItemState::NotFit,
    }
}

/// The feed's order of states.
fn rank(s: FeedItemState) -> u8 {
    match s {
        FeedItemState::Fit => 0,
        FeedItemState::NotFit => 1,
        FeedItemState::Checking => 2,
        FeedItemState::Waiting => 3,
        FeedItemState::New => 4,
        FeedItemState::Skipped => 5,
        FeedItemState::Dismissed => 6,
    }
}

/// The check queue's order: more overlap first, then the employer's newest (undated last).
fn queue_order(a: &FeedRow, b: &FeedRow) -> std::cmp::Ordering {
    b.overlap
        .total_cmp(&a.overlap)
        .then_with(|| b.job.posted_at.cmp(&a.job.posted_at))
}

/// The companies the feed shows (skipped and dismissed rows left out), named as their rows name them, each with
/// freehire's company slug when freehire named the company (`logo::fetch_logos`). "Unnamed company" is left out.
pub fn shown_companies(app: &App) -> Result<Vec<(String, Option<String>)>> {
    let store = lock(&app.store);
    let mut out = Vec::new();
    for row in store.feed()? {
        if matches!(row.state, FeedState::Skipped | FeedState::Dismissed) {
            continue;
        }
        let req = requirements(&store, &row)?;
        let company = shown(req.as_ref().map(|r| &r.company), listed_company(&row.job));
        if company == UNNAMED {
            continue;
        }
        let named = freehire::is_name(&row.job.company);
        out.push((company, row.job.company_slug.filter(|_| named)));
    }
    Ok(out)
}

fn item(
    store: &Store,
    row: &FeedRow,
    state: FeedItemState,
    titles: &HashMap<String, String>,
) -> Result<FeedItem> {
    let req = requirements(store, row)?;
    let base = job_item(&row.job, state);
    let role = shown(req.as_ref().map(|r| &r.role), base.role.clone());
    let company = shown(req.as_ref().map(|r| &r.company), base.company.clone());
    Ok(FeedItem {
        score: row.score,
        master: row
            .master_id
            .as_ref()
            .and_then(|id| titles.get(id).cloned()),
        job: row.posting_id.clone(),
        knockouts: req
            .map(|r| r.knockouts.into_iter().map(|k| k.text).collect())
            .unwrap_or_default(),
        reason: row.reason.clone(),
        company_key: crate::logo::key(&company),
        role,
        company,
        ..base
    })
}

/// A role as the feed tells per-place copies apart: the company's key and the title, lowercased.
fn role_key(job: &FeedJob) -> (String, String) {
    (
        crate::logo::key(&job.company),
        job.title
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase(),
    )
}

/// Extract's requirements of a checked row.
fn requirements(store: &Store, row: &FeedRow) -> Result<Option<resume_core::Requirements>> {
    Ok(match (&row.posting_id, row.state) {
        (Some(id), FeedState::Checked) => store.latest_requirements(id)?,
        _ => None,
    })
}

/// Extract's name, as Recent jobs shows it; one left empty keeps the listing's.
fn shown(extracted: Option<&String>, listed: String) -> String {
    extracted
        .filter(|s| !s.trim().is_empty())
        .cloned()
        .unwrap_or(listed)
}

/// The company as freehire lists it, or "Unnamed company" when freehire names it by its hosting site or an id
/// (`egug.fa.us2.oraclecloud.com/CX`).
fn listed_company(job: &FeedJob) -> String {
    if freehire::is_name(&job.company) {
        job.company.clone()
    } else {
        UNNAMED.to_owned()
    }
}

/// What the source lists, as a feed row. A company named by its hosting site or an id reads "Unnamed company" until
/// Extract finds the name.
fn job_item(job: &FeedJob, state: FeedItemState) -> FeedItem {
    let company = listed_company(job);
    FeedItem {
        slug: job.slug.clone(),
        origin: job.origin.label().to_owned(),
        role: job.title.clone(),
        company_key: crate::logo::key(&company),
        company,
        location: job.location.clone().unwrap_or_default(),
        url: job.url.clone(),
        state,
        score: None,
        master: None,
        job: None,
        posted_at: job.posted_at.clone(),
        found_at: job.found_at.clone(),
        knockouts: vec![],
        reason: None,
        salary: job.salary.clone().map(Into::into),
    }
}

/// `a` is later than `b` (RFC 3339; as text when either doesn't parse).
fn later(a: &str, b: &str) -> bool {
    match (parse_time(a), parse_time(b)) {
        (Some(a), Some(b)) => a > b,
        _ => a > b,
    }
}

fn parse_time(s: &str) -> Option<DateTime<Utc>> {
    DateTime::<FixedOffset>::parse_from_rfc3339(s)
        .ok()
        .map(|t| t.with_timezone(&Utc))
}

/// Local midnight as the store writes times (RFC 3339 UTC, microseconds), for the day's checks.
fn midnight() -> String {
    let start = Local::now()
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .and_then(|t| t.and_local_timezone(Local).earliest())
        .map_or_else(Utc::now, |t| t.with_timezone(&Utc));
    stamp(start)
}

fn stamp(t: DateTime<Utc>) -> String {
    t.to_rfc3339_opts(SecondsFormat::Micros, true)
}

fn hours_of(h: u32) -> TimeDelta {
    TimeDelta::hours(i64::from(h))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(s: &str) -> DateTime<Utc> {
        parse_time(s).unwrap()
    }

    /// A rate limit leaves LinkedIn alone for at least an hour (longer when it asks), and says until when rather than
    /// failing; a network failure stays an error and is retried on the next pull.
    #[test]
    fn linkedin_backs_off_after_a_rate_limit() {
        let limited = |secs: Option<u64>| IngestError::RateLimited {
            host: "linkedin.com".into(),
            retry_after: secs.map(Duration::from_secs),
        };
        assert_eq!(linkedin_backoff(&limited(None)), Some(TimeDelta::hours(1)));
        assert_eq!(
            linkedin_backoff(&limited(Some(90))),
            Some(TimeDelta::hours(1))
        );
        assert_eq!(
            linkedin_backoff(&limited(Some(3 * 3600))),
            Some(TimeDelta::hours(3))
        );
        let offline = IngestError::Network {
            host: "linkedin.com".into(),
            message: "dns error".into(),
        };
        assert_eq!(linkedin_backoff(&offline), None);

        let now = at("2026-10-14T05:00:00Z");
        let backed_off = FeedCursor {
            linkedin_retry_at: Some("2026-10-14T05:30:00Z".into()),
            ..FeedCursor::default()
        };
        assert!(!linkedin_due(&backed_off, now));
        assert!(linkedin_due(&backed_off, at("2026-10-14T05:30:00Z")));
        assert!(linkedin_paused(&FeedSettings::default(), &backed_off, now).is_some());
        assert!(
            linkedin_paused(
                &FeedSettings::default(),
                &backed_off,
                at("2026-10-14T06:00:00Z")
            )
            .is_none()
        );
        let off = FeedSettings {
            linkedin: false,
            ..FeedSettings::default()
        };
        assert!(linkedin_paused(&off, &backed_off, now).is_none());

        // At most hourly: a launch soon after the last search leaves LinkedIn alone.
        let searched = FeedCursor {
            linkedin_at: Some("2026-10-14T04:30:00Z".into()),
            ..FeedCursor::default()
        };
        assert!(!linkedin_due(&searched, now));
        assert!(linkedin_due(&searched, at("2026-10-14T05:30:00Z")));
        assert!(linkedin_due(&FeedCursor::default(), now));
    }
}

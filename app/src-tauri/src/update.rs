//! Updates (ARCHITECTURE.md §1, Updates): the newest GitHub release's `latest.json`, through the updater plugin, whose
//! bundle must carry a signature from the key whose public half is `plugins.updater.pubkey` in `tauri.conf.json`.
//! Checked about a minute after launch and then once a day while Settings → About says so (release builds only, so a
//! dev build never offers to replace itself), and on Check now. Restart to update downloads the bundle, checks its
//! signature, swaps the app in place and relaunches it. Every change goes to the UI as the `update` event.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use chrono::{DateTime, SecondsFormat, TimeDelta, Utc};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::dto::{UpdateState, UpdateStatus};
use crate::error::{AppError, Result};
use crate::state::{App, lock};

/// The first check waits this long after launch, so it never competes with the window's first screen.
const FIRST: Duration = Duration::from_secs(60);
/// How often the schedule looks at whether a check is due (so a Mac waking from sleep catches up within the hour).
const TICK: Duration = Duration::from_secs(60 * 60);
/// A check is due this long after the last one.
const EVERY: TimeDelta = TimeDelta::hours(24);

/// The app's update state (`tauri::State<Updates>`).
pub struct Updates {
    status: Mutex<UpdateStatus>,
    /// What the last check found, ready to install.
    found: Mutex<Option<Update>>,
    last: Mutex<Option<DateTime<Utc>>>,
}

impl Updates {
    pub fn new(current: String) -> Self {
        Self {
            status: Mutex::new(UpdateStatus { current, ..UpdateStatus::default() }),
            found: Mutex::new(None),
            last: Mutex::new(None),
        }
    }
}

pub fn status(app: &AppHandle) -> UpdateStatus {
    lock(&app.state::<Updates>().status).clone()
}

/// Changes the status and tells the UI.
fn put(app: &AppHandle, change: impl FnOnce(&mut UpdateStatus)) -> UpdateStatus {
    let updates = app.state::<Updates>();
    let status = {
        let mut s = lock(&updates.status);
        change(&mut s);
        s.clone()
    };
    let _ = app.emit("update", &status);
    status
}

/// Asks for the newest release; the status says what it found (or why it couldn't).
pub async fn check(app: &AppHandle) -> UpdateStatus {
    if lock(&app.state::<Updates>().status).state != UpdateState::Idle {
        return status(app);
    }
    put(app, |s| s.state = UpdateState::Checking);
    let found = match app.updater() {
        Ok(updater) => updater.check().await,
        Err(e) => Err(e),
    };
    let now = Utc::now();
    *lock(&app.state::<Updates>().last) = Some(now);
    let checked_at = Some(now.to_rfc3339_opts(SecondsFormat::Secs, true));
    match found {
        Ok(update) => {
            let (available, notes) = update
                .as_ref()
                .map_or((None, None), |u| (Some(u.version.clone()), u.body.clone().filter(|b| !b.trim().is_empty())));
            *lock(&app.state::<Updates>().found) = update;
            put(app, |s| {
                *s = UpdateStatus { current: s.current.clone(), available, notes, checked_at, ..UpdateStatus::default() };
            })
        }
        Err(e) => put(app, |s| {
            s.state = UpdateState::Idle;
            s.checked_at = checked_at;
            s.error = Some(format!("Couldn’t check for updates: {e}"));
        }),
    }
}

/// Downloads and installs the release the last check found, then relaunches the app (it doesn't return then).
pub async fn install(app: &AppHandle) -> Result<()> {
    let Some(update) = lock(&app.state::<Updates>().found).clone() else {
        return Err(AppError::other("There’s no update to install. Check for updates first."));
    };
    put(app, |s| {
        s.state = UpdateState::Installing;
        s.progress = None;
        s.error = None;
    });
    let mut got: u64 = 0;
    // Tell the UI at each whole percent, not each chunk.
    let mut shown: u64 = 0;
    let done = update
        .download_and_install(
            |chunk, total| {
                got += chunk as u64;
                let Some(total) = total.filter(|t| *t > 0) else { return };
                let percent = got.saturating_mul(100) / total;
                if percent > shown {
                    shown = percent;
                    put(app, |s| s.progress = Some(got as f64 / total as f64));
                }
            },
            || {},
        )
        .await;
    match done {
        Ok(()) => app.restart(),
        Err(e) => {
            let message = format!("Couldn’t install gliide {}: {e}", update.version);
            put(app, |s| {
                s.state = UpdateState::Idle;
                s.progress = None;
                s.error = Some(message.clone());
            });
            Err(AppError::other(message))
        }
    }
}

/// Checks at launch and once a day while `check_updates` is on; never in a dev build.
pub async fn schedule(app: AppHandle) {
    if cfg!(debug_assertions) {
        return;
    }
    tokio::time::sleep(FIRST).await;
    loop {
        let on = app.state::<Arc<App>>().settings().check_updates;
        let due = lock(&app.state::<Updates>().last).is_none_or(|t| Utc::now() - t >= EVERY);
        if on && due {
            check(&app).await;
        }
        tokio::time::sleep(TICK).await;
    }
}

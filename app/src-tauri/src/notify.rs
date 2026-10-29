//! Round banners from macOS, opt-in (local/interviewer/SPEC.md §7): the day before a round, an hour before it, and an
//! hour after it ends while it has no debrief, each only when Settings → Notifications has Interviews and that row
//! on. The app fires them itself while it runs, from a minute's tick: a banner is shown only within
//! [`WINDOW`] of its time (one missed while gliide was closed or the Mac asleep isn't replayed; Up next shows it),
//! once per round and kind (the store's `notified`), never for a cancelled round or an application at Offer or
//! Rejected.
//!
//! On macOS this is UserNotifications itself, not `tauri-plugin-notification`: that plugin's desktop side always
//! answers "granted" (it can't tell an app the user turned off) and posts through the deprecated
//! `NSUserNotificationCenter`, as Terminal in `tauri dev`. `UNUserNotificationCenter` tells what macOS allows, asks
//! once (only from Turn on), shows the banner while gliide is in front too, and reports a click, which opens the
//! round's job. It aborts the process in a binary that isn't inside an `.app` (`tauri dev` runs `target/debug/gliide`),
//! so there every call answers [`NotificationPermission::Unknown`] and nothing is shown.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Local, TimeDelta, Utc};
use resume_llm::prep::{Group, Prep};
use resume_store::{AppStatus, Notified, Round, RoundState, Store};
use tauri::{AppHandle, Manager};

use crate::dto::{NotificationPermission, Notifications};
use crate::error::Result;
use crate::state::{App, blocking, lock};

/// How often the schedule looks for a banner that's due.
const TICK: Duration = Duration::from_secs(60);
/// A banner shows only this long after its time.
pub const WINDOW: TimeDelta = TimeDelta::minutes(15);
/// A round's length when it isn't known.
const DEFAULT_MINUTES: i64 = 60;
/// System Settings → Notifications.
const SETTINGS_URL: &str = "x-apple.systempreferences:com.apple.Notifications-Settings.extension";

/// A banner to show: its request id (`day:<round>`), round and kind, and what it says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Banner {
    pub round: String,
    pub kind: Notified,
    pub title: String,
    pub body: String,
}

impl Banner {
    fn id(&self) -> String {
        format!("{}:{}", self.kind.as_str(), self.round)
    }
}

/// The banner due for `round` at `now`, if any: its kind is checked in `n` (with Interviews on), its time passed less
/// than [`WINDOW`] ago, and `notified` says it hasn't been shown. Day before is the start less 24 hours, Hour before
/// the start less an hour, Debrief an hour after the end (the start plus its minutes, an hour when unknown) while the
/// round has no debrief. None for an unscheduled or cancelled round, or an application at Offer or Rejected.
pub fn due(
    now: DateTime<Utc>,
    round: &Round,
    status: AppStatus,
    n: &Notifications,
    notified: impl Fn(Notified) -> bool,
) -> Option<Notified> {
    if !n.interviews
        || matches!(status, AppStatus::Offer | AppStatus::Rejected)
        || round.state != RoundState::Upcoming
    {
        return None;
    }
    let starts = DateTime::parse_from_rfc3339(round.starts_at.as_deref()?)
        .ok()?
        .with_timezone(&Utc);
    let minutes = round.minutes.map_or(DEFAULT_MINUTES, i64::from);
    let ends = starts + TimeDelta::minutes(minutes);
    [
        (Notified::Day, n.day_before, starts - TimeDelta::hours(24)),
        (Notified::Hour, n.hour_before, starts - TimeDelta::hours(1)),
        (
            Notified::Debrief,
            n.debrief && round.debriefed_at.is_none(),
            ends + TimeDelta::hours(1),
        ),
    ]
    .into_iter()
    .find(|&(kind, on, at)| on && at <= now && now < at + WINDOW && !notified(kind))
    .map(|(kind, ..)| kind)
}

/// What a round's prep has ready, for the day-before banner: its questions, and its behavioral ones with no story of
/// yours (as the Interview tab says "No story yet").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ready {
    pub questions: usize,
    pub without_story: usize,
}

/// The banner's title and body: "Globex · Technical", "Globex technical tomorrow at 2:00 PM. 8 questions ready, 2
/// without a story.", "Globex technical in 1 hour", "How did the Globex technical go?". `time` is the start, local.
pub fn text(
    kind: Notified,
    company: &str,
    round_kind: &str,
    time: &str,
    ready: Option<Ready>,
) -> (String, String) {
    let label = kind_label(round_kind);
    let what = format!("{company} {}", label.to_lowercase());
    let body = match kind {
        Notified::Day => {
            let mut body = format!("{what} tomorrow at {time}.");
            if let Some(r) = ready.filter(|r| r.questions > 0) {
                let questions = match r.questions {
                    1 => "1 question".to_owned(),
                    n => format!("{n} questions"),
                };
                body.push_str(&format!(" {questions} ready"));
                if r.without_story > 0 {
                    body.push_str(&format!(", {} without a story", r.without_story));
                }
                body.push('.');
            }
            body
        }
        Notified::Hour => format!("{what} in 1 hour"),
        Notified::Debrief => format!("How did the {what} go?"),
    };
    (format!("{company} · {label}"), body)
}

/// A round kind's label, as `app/src/lib/rounds.ts` names it.
fn kind_label(kind: &str) -> &'static str {
    match kind {
        "recruiter-screen" => "Recruiter screen",
        "hiring-manager" => "Hiring manager",
        "technical" => "Technical",
        "take-home" => "Take-home",
        "system-design" => "System design",
        "behavioral" => "Behavioral",
        "onsite" => "Onsite",
        "final" => "Final",
        _ => "Interview",
    }
}

/// The banners due now, with their text, from what's stored.
fn banners(store: &Store, n: &Notifications, now: DateTime<Utc>) -> Result<Vec<Banner>> {
    let mut out = Vec::new();
    let mut stories: Option<HashSet<String>> = None;
    for a in store.applications()? {
        if matches!(a.status, AppStatus::Offer | AppStatus::Rejected) {
            continue;
        }
        for r in store.rounds(&a.id)? {
            // A notified row that can't be read counts as shown: never a banner twice.
            let Some(kind) = due(now, &r, a.status, n, |k| {
                store.was_notified(&r.id, k).unwrap_or(true)
            }) else {
                continue;
            };
            let ready = match kind {
                Notified::Day => match store.latest_prep(&r.id)? {
                    Some(p) => {
                        let ids = match &mut stories {
                            Some(ids) => ids,
                            None => {
                                stories.insert(store.stories()?.into_iter().map(|s| s.id).collect())
                            }
                        };
                        serde_json::from_str::<Prep>(&p.json)
                            .ok()
                            .map(|p| ready(&p, ids))
                    }
                    None => None,
                },
                _ => None,
            };
            let time = r
                .starts_at
                .as_deref()
                .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                .map(|t| t.with_timezone(&Local).format("%-I:%M %p").to_string())
                .unwrap_or_default();
            let (title, body) = text(kind, &a.company, &r.kind, &time, ready);
            out.push(Banner {
                round: r.id.clone(),
                kind,
                title,
                body,
            });
        }
    }
    Ok(out)
}

/// A prep's questions, and its behavioral ones whose story is missing (none linked, or removed since).
fn ready(p: &Prep, stories: &HashSet<String>) -> Ready {
    Ready {
        questions: p.questions.len(),
        without_story: p
            .questions
            .iter()
            .filter(|q| {
                q.group == Group::Behavioral
                    && !q.story.as_ref().is_some_and(|s| stories.contains(s))
            })
            .count(),
    }
}

/// Looks for due banners every [`TICK`] while the app runs, and shows them when macOS allows.
pub async fn schedule(handle: AppHandle) {
    let mut tick = tokio::time::interval(TICK);
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        tick.tick().await;
        if let Err(e) = fire(&handle).await {
            eprintln!("gliide: notifications: {}", e.message);
        }
    }
}

async fn fire(handle: &AppHandle) -> Result<()> {
    let Some(app) = handle.try_state::<Arc<App>>().map(|s| Arc::clone(&s)) else {
        return Ok(());
    };
    let n = app.settings().notifications;
    if !n.interviews || !(n.day_before || n.hour_before || n.debrief) {
        return Ok(());
    }
    let due = blocking(&app, move |app| banners(&lock(&app.store), &n, Utc::now())).await?;
    if due.is_empty() || permission().await != NotificationPermission::Granted {
        return Ok(());
    }
    for b in due {
        match show(&b).await {
            // Kept only once shown, so a failed one is tried again within its window.
            Ok(()) => {
                let (round, kind) = (b.round.clone(), b.kind);
                blocking(&app, move |app| {
                    Ok(lock(&app.store).mark_notified(&round, kind)?)
                })
                .await?;
            }
            Err(e) => eprintln!("gliide: notification {} not shown: {e}", b.id()),
        }
    }
    Ok(())
}

/// Where a banner's click goes: the job on its Interview tab, or its debrief; Applications when the posting is gone.
fn click_path(store: &Store, banner_id: &str) -> Option<String> {
    let (kind, round) = banner_id.split_once(':')?;
    let r = store.round(round).ok()??;
    let a = store.application(&r.application_id).ok()??;
    Some(match a.posting_id {
        Some(job) if kind == Notified::Debrief.as_str() => format!("/jobs/{job}?debrief={round}"),
        Some(job) => format!("/jobs/{job}"),
        None => "/applications".to_owned(),
    })
}

/// A banner was clicked: the window comes forward and the page goes where the banner points (event `notification`).
fn clicked(handle: &AppHandle, banner_id: String) {
    use tauri::Emitter;

    let handle = handle.clone();
    tauri::async_runtime::spawn(async move {
        if let Some(window) = handle.get_webview_window("main") {
            let _ = window.unminimize();
            let _ = window.show();
            let _ = window.set_focus();
        }
        let Some(app) = handle.try_state::<Arc<App>>().map(|s| Arc::clone(&s)) else {
            return;
        };
        let path = blocking(&app, move |app| {
            Ok(click_path(&lock(&app.store), &banner_id))
        })
        .await;
        if let Ok(Some(path)) = path {
            let _ = handle.emit("notification", path);
        }
    });
}

/// System Settings → Notifications.
pub fn open_settings(handle: &AppHandle) -> Result<()> {
    use tauri_plugin_opener::OpenerExt;

    handle
        .opener()
        .open_url(SETTINGS_URL, None::<&str>)
        .map_err(|e| crate::error::AppError::other(format!("Couldn’t open System Settings: {e}")))
}

#[cfg(target_os = "macos")]
pub use mac::{permission, request, setup};

#[cfg(target_os = "macos")]
mod mac {
    use std::ptr::NonNull;
    use std::sync::Mutex;
    use std::time::Duration;

    use block2::{DynBlock, RcBlock};
    use objc2::rc::Retained;
    use objc2::runtime::{Bool, NSObject, NSObjectProtocol, ProtocolObject};
    use objc2::{AllocAnyThread, DefinedClass, define_class, msg_send};
    use objc2_foundation::{NSBundle, NSError, NSString};
    use objc2_user_notifications::{
        UNAuthorizationOptions, UNAuthorizationStatus, UNMutableNotificationContent,
        UNNotification, UNNotificationPresentationOptions, UNNotificationRequest,
        UNNotificationResponse, UNNotificationSettings, UNUserNotificationCenter,
        UNUserNotificationCenterDelegate,
    };
    use tauri::AppHandle;
    use tokio::sync::oneshot;

    use super::Banner;
    use crate::dto::NotificationPermission;

    /// How long macOS may take to say what it allows (asking waits for your answer instead).
    const READ_TIMEOUT: Duration = Duration::from_secs(5);

    /// The notification center, when this process is an app bundle; `currentNotificationCenter` raises (and aborts)
    /// in a bare binary, whose main bundle is its folder (`tauri dev` embeds an Info.plist, so it has an identifier).
    fn center() -> Option<Retained<UNUserNotificationCenter>> {
        let bundle = NSBundle::mainBundle();
        let bundled = bundle.bundlePath().to_string().ends_with(".app")
            && bundle.bundleIdentifier().is_some();
        bundled.then(UNUserNotificationCenter::currentNotificationCenter)
    }

    /// A one-shot sender a completion block can own: the block is `Fn`, so the sender is taken once.
    fn once<T>(tx: oneshot::Sender<T>) -> impl Fn(T) {
        let tx = Mutex::new(Some(tx));
        move |v| {
            if let Some(tx) = tx.lock().ok().and_then(|mut t| t.take()) {
                let _ = tx.send(v);
            }
        }
    }

    fn read() -> Option<oneshot::Receiver<UNAuthorizationStatus>> {
        let center = center()?;
        let (tx, rx) = oneshot::channel();
        let send = once(tx);
        let block = RcBlock::new(move |settings: NonNull<UNNotificationSettings>| {
            // SAFETY: UserNotifications passes a valid settings object for the block's duration.
            send(unsafe { settings.as_ref() }.authorizationStatus());
        });
        center.getNotificationSettingsWithCompletionHandler(&block);
        Some(rx)
    }

    /// Whether macOS lets gliide show banners: Granted when authorized (provisionally too), Denied when turned off in
    /// System Settings, Unknown before it's been asked or outside an app bundle.
    pub async fn permission() -> NotificationPermission {
        let Some(rx) = read() else {
            return NotificationPermission::Unknown;
        };
        match tokio::time::timeout(READ_TIMEOUT, rx).await {
            Ok(Ok(UNAuthorizationStatus::Denied)) => NotificationPermission::Denied,
            Ok(Ok(
                UNAuthorizationStatus::Authorized
                | UNAuthorizationStatus::Provisional
                | UNAuthorizationStatus::Ephemeral,
            )) => NotificationPermission::Granted,
            _ => NotificationPermission::Unknown,
        }
    }

    /// Asks macOS (its prompt shows only the first time; after that it answers at once), then says what it allows.
    pub async fn request() -> NotificationPermission {
        let asked = center().map(|center| {
            let (tx, rx) = oneshot::channel();
            let send = once(tx);
            let block = RcBlock::new(move |_granted: Bool, _error: *mut NSError| send(()));
            center.requestAuthorizationWithOptions_completionHandler(
                UNAuthorizationOptions::Alert,
                &block,
            );
            rx
        });
        match asked {
            Some(rx) => {
                let _ = rx.await;
                permission().await
            }
            None => NotificationPermission::Unknown,
        }
    }

    /// Shows `b` now; errs with macOS's reason.
    pub(super) async fn show(b: &Banner) -> Result<(), String> {
        let rx = {
            let center = center().ok_or("not running as an app bundle")?;
            let content = UNMutableNotificationContent::new();
            content.setTitle(&NSString::from_str(&b.title));
            content.setBody(&NSString::from_str(&b.body));
            content.setThreadIdentifier(&NSString::from_str(&b.round));
            let request = UNNotificationRequest::requestWithIdentifier_content_trigger(
                &NSString::from_str(&b.id()),
                &content,
                None,
            );
            let (tx, rx) = oneshot::channel();
            let send = once(tx);
            let block = RcBlock::new(move |error: *mut NSError| {
                // SAFETY: a non-null error is a valid NSError for the block's duration.
                send(unsafe { error.as_ref() }.map(|e| e.localizedDescription().to_string()));
            });
            center.addNotificationRequest_withCompletionHandler(&request, Some(&block));
            rx
        };
        match rx.await {
            Ok(None) => Ok(()),
            Ok(Some(e)) => Err(e),
            Err(_) => Err("macOS didn't answer".to_owned()),
        }
    }

    define_class!(
        // SAFETY: NSObject has no subclassing requirements, and `Delegate` doesn't implement `Drop`.
        #[unsafe(super(NSObject))]
        #[name = "GliideNotificationDelegate"]
        #[ivars = AppHandle]
        struct Delegate;

        unsafe impl NSObjectProtocol for Delegate {}

        unsafe impl UNUserNotificationCenterDelegate for Delegate {
            /// While gliide is in front, its banners show anyway (macOS hides them by default).
            #[unsafe(method(userNotificationCenter:willPresentNotification:withCompletionHandler:))]
            fn will_present(
                &self,
                _center: &UNUserNotificationCenter,
                _notification: &UNNotification,
                handler: &DynBlock<dyn Fn(UNNotificationPresentationOptions)>,
            ) {
                handler.call((UNNotificationPresentationOptions::Banner
                    | UNNotificationPresentationOptions::List,));
            }

            #[unsafe(method(userNotificationCenter:didReceiveNotificationResponse:withCompletionHandler:))]
            fn did_receive(
                &self,
                _center: &UNUserNotificationCenter,
                response: &UNNotificationResponse,
                handler: &DynBlock<dyn Fn()>,
            ) {
                let id = response.notification().request().identifier().to_string();
                super::clicked(self.ivars(), id);
                handler.call(());
            }
        }
    );

    impl Delegate {
        fn new(handle: AppHandle) -> Retained<Self> {
            let this = Self::alloc().set_ivars(handle);
            // SAFETY: NSObject's `init`, on a freshly allocated instance with its ivars set.
            unsafe { msg_send![super(this), init] }
        }
    }

    /// Makes gliide the notification center's delegate (so banners show while it's in front and a click opens the
    /// round's job). The center holds its delegate weakly; this one lives as long as the app.
    pub fn setup(handle: &AppHandle) {
        let Some(center) = center() else {
            return;
        };
        let delegate = Delegate::new(handle.clone());
        center.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
        std::mem::forget(delegate);
    }
}

#[cfg(target_os = "macos")]
use mac::show;

#[cfg(not(target_os = "macos"))]
pub async fn permission() -> NotificationPermission {
    NotificationPermission::Unknown
}

#[cfg(not(target_os = "macos"))]
pub async fn request() -> NotificationPermission {
    NotificationPermission::Unknown
}

#[cfg(not(target_os = "macos"))]
pub fn setup(_handle: &AppHandle) {}

#[cfg(not(target_os = "macos"))]
async fn show(_b: &Banner) -> std::result::Result<(), String> {
    Err("banners are macOS only".to_owned())
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, TimeDelta, Utc};
    use resume_store::{AppStatus, Notified, Round, RoundState};

    use super::{Ready, due, text};
    use crate::dto::Notifications;

    fn at(s: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Utc)
    }

    fn round(starts_at: Option<&str>, minutes: Option<u32>) -> Round {
        Round {
            id: "r1".into(),
            application_id: "a1".into(),
            kind: "technical".into(),
            starts_at: starts_at.map(Into::into),
            minutes,
            with: String::new(),
            place: String::new(),
            notes: String::new(),
            state: RoundState::Upcoming,
            debriefed_at: None,
            updated_at: String::new(),
        }
    }

    fn on() -> Notifications {
        Notifications {
            interviews: true,
            day_before: true,
            hour_before: true,
            debrief: true,
            ..Notifications::default()
        }
    }

    const START: &str = "2026-11-03T14:00:00-05:00";

    fn due_at(now: &str, r: &Round, n: &Notifications) -> Option<Notified> {
        due(at(now), r, AppStatus::Interviewing, n, |_| false)
    }

    #[test]
    fn each_kind_is_due_from_its_time_for_fifteen_minutes() {
        let r = round(Some(START), Some(45));
        let n = on();
        // Day before: 24 h before the start.
        assert_eq!(due_at("2026-11-02T13:59:59-05:00", &r, &n), None);
        assert_eq!(
            due_at("2026-11-02T14:00:00-05:00", &r, &n),
            Some(Notified::Day)
        );
        assert_eq!(
            due_at("2026-11-02T14:14:59-05:00", &r, &n),
            Some(Notified::Day)
        );
        assert_eq!(due_at("2026-11-02T14:15:00-05:00", &r, &n), None);
        // Hour before.
        assert_eq!(
            due_at("2026-11-03T13:00:00-05:00", &r, &n),
            Some(Notified::Hour)
        );
        assert_eq!(due_at("2026-11-03T13:20:00-05:00", &r, &n), None);
        // Debrief: an hour after its 45 minutes.
        assert_eq!(due_at("2026-11-03T15:44:00-05:00", &r, &n), None);
        assert_eq!(
            due_at("2026-11-03T15:45:00-05:00", &r, &n),
            Some(Notified::Debrief)
        );
        assert_eq!(due_at("2026-11-03T16:00:00-05:00", &r, &n), None);
    }

    #[test]
    fn an_unknown_length_is_an_hour() {
        let r = round(Some(START), None);
        assert_eq!(
            due_at("2026-11-03T16:05:00-05:00", &r, &on()),
            Some(Notified::Debrief)
        );
    }

    #[test]
    fn nothing_is_due_once_shown_or_when_off() {
        let r = round(Some(START), None);
        let now = at("2026-11-03T13:05:00-05:00");
        let n = on();
        assert_eq!(
            due(now, &r, AppStatus::Interviewing, &n, |k| k
                == Notified::Hour),
            None
        );
        assert_eq!(
            due(now, &r, AppStatus::Interviewing, &n, |k| k == Notified::Day),
            Some(Notified::Hour)
        );
        let unchecked = Notifications {
            hour_before: false,
            ..on()
        };
        assert_eq!(
            due(now, &r, AppStatus::Interviewing, &unchecked, |_| false),
            None
        );
        // The three rows checked but Interviews off: nothing.
        let off = Notifications {
            interviews: false,
            ..on()
        };
        assert_eq!(due(now, &r, AppStatus::Interviewing, &off, |_| false), None);
    }

    #[test]
    fn never_for_cancelled_unscheduled_debriefed_or_settled() {
        let n = on();
        let now = at("2026-11-03T13:05:00-05:00");
        let cancelled = Round {
            state: RoundState::Cancelled,
            ..round(Some(START), None)
        };
        assert_eq!(
            due(now, &cancelled, AppStatus::Interviewing, &n, |_| false),
            None
        );
        assert_eq!(
            due(now, &round(None, None), AppStatus::Interviewing, &n, |_| {
                false
            }),
            None
        );
        for status in [AppStatus::Offer, AppStatus::Rejected] {
            assert_eq!(
                due(now, &round(Some(START), None), status, &n, |_| false),
                None
            );
        }
        let debriefed = Round {
            debriefed_at: Some("2026-11-03T15:10:00-05:00".into()),
            ..round(Some(START), None)
        };
        let after = at("2026-11-03T16:05:00-05:00");
        assert_eq!(
            due(after, &debriefed, AppStatus::Interviewing, &n, |_| false),
            None
        );
        let no_debrief = Notifications {
            debrief: false,
            ..on()
        };
        assert_eq!(
            due(
                after,
                &round(Some(START), None),
                AppStatus::Interviewing,
                &no_debrief,
                |_| false
            ),
            None
        );
    }

    #[test]
    fn a_round_added_inside_its_window_is_still_due_but_a_missed_one_is_not() {
        let r = round(Some(START), None);
        let n = on();
        let hour = at(START) - TimeDelta::hours(1);
        assert_eq!(
            due(
                hour + TimeDelta::minutes(14),
                &r,
                AppStatus::Interviewing,
                &n,
                |_| false
            ),
            Some(Notified::Hour)
        );
        assert_eq!(
            due(
                hour + TimeDelta::minutes(40),
                &r,
                AppStatus::Interviewing,
                &n,
                |_| false
            ),
            None
        );
    }

    /// From the store: the day-before banner counts the round's prep (a behavioral question without a story of yours,
    /// one linking a story since removed), is due once, and its click opens Applications (the application has no
    /// posting); a cancelled round never.
    #[test]
    fn banners_come_from_the_store_once_each() {
        use resume_llm::prep::{Group, Prep, Question};
        use resume_store::{Application, Prep as Stored, Store};

        let store = Store::open_in_memory().unwrap();
        let application = |id: &str, company: &str, posting: Option<&str>, status| Application {
            id: id.into(),
            posting_id: posting.map(Into::into),
            analysis_id: None,
            date: "2026-10-01".into(),
            company: company.into(),
            role: "Software Engineer".into(),
            url: None,
            status,
            now: None,
            projected: None,
            on_file: None,
            resume_path: None,
            cover_path: None,
            notes: String::new(),
            updated_at: String::new(),
        };
        store
            .put_application(&application("a1", "Globex", None, AppStatus::Interviewing))
            .unwrap();
        store
            .put_application(&application("a2", "Initech", None, AppStatus::Interviewing))
            .unwrap();
        store.put_round(&round(Some(START), None)).unwrap();
        let cancelled = Round {
            id: "r2".into(),
            application_id: "a2".into(),
            state: RoundState::Cancelled,
            ..round(Some(START), None)
        };
        store.put_round(&cancelled).unwrap();
        let question = |id: &str, group, story: Option<&str>| Question {
            id: id.into(),
            group,
            text: String::new(),
            why: String::new(),
            lines: vec![],
            story: story.map(Into::into),
        };
        let prep = Prep {
            focus: String::new(),
            questions: vec![
                question("q1", Group::Resume, None),
                question("q2", Group::Behavioral, None),
                question("q3", Group::Behavioral, Some("gone")),
            ],
            gaps: vec![],
            ask_them: vec![],
        };
        store
            .put_prep(&Stored {
                key: "k".into(),
                round_id: "r1".into(),
                json: serde_json::to_string(&prep).unwrap(),
                model: "sonnet".into(),
                at: String::new(),
            })
            .unwrap();

        let now = at("2026-11-02T14:05:00-05:00");
        let due = super::banners(&store, &on(), now).unwrap();
        assert_eq!(due.len(), 1);
        let b = &due[0];
        assert_eq!((b.round.as_str(), b.kind), ("r1", Notified::Day));
        assert_eq!(b.title, "Globex · Technical");
        assert!(
            b.body.starts_with("Globex technical tomorrow at "),
            "{}",
            b.body
        );
        assert!(
            b.body.ends_with(". 3 questions ready, 2 without a story."),
            "{}",
            b.body
        );
        assert_eq!(
            super::click_path(&store, &b.id()).as_deref(),
            Some("/applications")
        );

        store.mark_notified("r1", Notified::Day).unwrap();
        assert!(super::banners(&store, &on(), now).unwrap().is_empty());
    }

    /// A test binary isn't in an `.app`, as `tauri dev`'s isn't: UserNotifications is never touched (it would abort).
    #[tokio::test]
    async fn outside_an_app_bundle_macos_is_unknown_and_never_asked() {
        use crate::dto::NotificationPermission;

        assert_eq!(super::permission().await, NotificationPermission::Unknown);
        assert_eq!(super::request().await, NotificationPermission::Unknown);
    }

    #[test]
    fn texts_read_as_the_spec_says() {
        let ready = Some(Ready {
            questions: 8,
            without_story: 2,
        });
        assert_eq!(
            text(Notified::Day, "Globex", "technical", "2:00 PM", ready),
            (
                "Globex · Technical".to_owned(),
                "Globex technical tomorrow at 2:00 PM. 8 questions ready, 2 without a story."
                    .to_owned()
            )
        );
        assert_eq!(
            text(Notified::Day, "Globex", "technical", "2:00 PM", None).1,
            "Globex technical tomorrow at 2:00 PM."
        );
        assert_eq!(
            text(
                Notified::Day,
                "Globex",
                "technical",
                "2:00 PM",
                Some(Ready {
                    questions: 1,
                    without_story: 0
                })
            )
            .1,
            "Globex technical tomorrow at 2:00 PM. 1 question ready."
        );
        assert_eq!(
            text(Notified::Hour, "Globex", "technical", "", None).1,
            "Globex technical in 1 hour"
        );
        assert_eq!(
            text(Notified::Debrief, "Initech", "recruiter-screen", "", None),
            (
                "Initech · Recruiter screen".to_owned(),
                "How did the Initech recruiter screen go?".to_owned()
            )
        );
        assert_eq!(
            text(Notified::Hour, "Hooli", "other", "", None).0,
            "Hooli · Interview"
        );
    }
}

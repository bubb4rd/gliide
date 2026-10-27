//! Interview rounds (local/interviewer/SPEC.md §3, §8): an application's rounds; saving one, which moves a Saved or
//! Applied application to Interviewing; removing one and restoring cancelled ones; reading an invite into a round's
//! fields (pasted text: one model call; an `.ics` file: parsed here); and the resume the application sent.

use std::path::Path;
use std::sync::Arc;

use chrono::{DateTime, Local, SecondsFormat};
use resume_core::Master;
use resume_llm::invite::{self, Invite, InviteInput};
use resume_store::{self as store, Application, RoundState as StoreState};

use crate::dto::{
    AppStatus, ApplicationRow, Round, RoundDraft, RoundKind, RoundState, RoundsView, SentFrom,
    SentView, Stage,
};
use crate::error::{AppError, Result};
use crate::export;
use crate::ics;
use crate::state::{App, DEFAULT_TEMPLATE, blocking, lock, model_call};

/// The longest a round may be, in minutes (a full onsite).
const MAX_MINUTES: u32 = 600;

/// The application's rounds: scheduled ones by when they start, then the unscheduled.
pub fn rounds(app: &App, application: &str) -> Result<Vec<Round>> {
    Ok(lock(&app.store)
        .rounds(application)?
        .into_iter()
        .map(round)
        .collect())
}

/// Saves a round from the form: a new one when `draft.id` is `None`, else that round (one removed comes back). A
/// round saved upcoming moves a Saved or Applied application to Interviewing, logged and in `applications.csv`.
pub fn save_round(app: &App, application: &str, draft: RoundDraft) -> Result<RoundsView> {
    let starts_at = match draft.starts_at.as_deref().map(str::trim) {
        Some(s) if !s.is_empty() => Some(
            normalized(s).ok_or_else(|| AppError::other(format!("“{s}” isn’t a date and time")))?,
        ),
        _ => None,
    };
    if draft
        .minutes
        .is_some_and(|m| !(1..=MAX_MINUTES).contains(&m))
    {
        return Err(AppError::other("A round lasts from 1 minute to 10 hours"));
    }
    let store = lock(&app.store);
    let a = application_of(&store, application)?;
    let id = draft.id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    // An edit keeps the round's state; a new round, or one removed and brought back, is upcoming.
    let state = store.round(&id)?.map_or(StoreState::Upcoming, |r| r.state);
    let saved = store.put_round(&store::Round {
        id,
        application_id: a.id.clone(),
        kind: kind_name(draft.kind.unwrap_or(RoundKind::Other)).to_owned(),
        starts_at,
        minutes: draft.minutes,
        with: draft.with.trim().to_owned(),
        place: draft.place.trim().to_owned(),
        notes: draft.notes.trim().to_owned(),
        state,
        updated_at: String::new(),
    })?;
    drop(store);
    let moved_from = (saved.state == StoreState::Upcoming
        && matches!(
            a.status,
            store::AppStatus::Saved | store::AppStatus::Applied
        ))
    .then_some(AppStatus::from(a.status));
    if moved_from.is_some() {
        export::set_status(app, std::slice::from_ref(&a.id), AppStatus::Interviewing)?;
    }
    view(app, &a.id, moved_from)
}

/// Deletes a round; Undo saves it again with its id (`save_round`).
pub fn remove_round(app: &App, round: &str) -> Result<RoundsView> {
    let removed = lock(&app.store)
        .delete_round(round)?
        .ok_or_else(|| AppError::missing("This interview was already removed"))?;
    view(app, &removed.application_id, None)
}

/// Rounds a status change cancelled, upcoming again; the applications they belong to, as they now stand.
pub fn restore_rounds(app: &App, rounds: &[String]) -> Result<Vec<ApplicationRow>> {
    let ids: Vec<&str> = rounds.iter().map(String::as_str).collect();
    let store = lock(&app.store);
    let restored = store.set_round_state(&ids, StoreState::Upcoming)?;
    let mut apps: Vec<Application> = Vec::new();
    for r in &restored {
        if !apps.iter().any(|a| a.id == r.application_id) {
            apps.push(application_of(&store, &r.application_id)?);
        }
    }
    export::rows(&store, &apps)
}

/// The application's rounds and row after a change.
fn view(app: &App, application: &str, moved_from: Option<AppStatus>) -> Result<RoundsView> {
    let store = lock(&app.store);
    let a = application_of(&store, application)?;
    Ok(RoundsView {
        rounds: store.rounds(application)?.into_iter().map(round).collect(),
        application: export::row(&a, store.next_round(application)?),
        moved_from,
    })
}

fn application_of(store: &store::Store, id: &str) -> Result<Application> {
    store
        .application(id)?
        .ok_or_else(|| AppError::missing("This application was deleted"))
}

/// One model call: a pasted invite read into a round's fields, each only when the invite states it. Runs under the
/// application's posting (its progress and `cancel`).
pub async fn read_invite(app: &Arc<App>, application: &str, text: String) -> Result<RoundDraft> {
    if text.trim().is_empty() {
        return Err(AppError::other("Paste the invite’s text first"));
    }
    let id = application.to_owned();
    let a = blocking(app, move |app| application_of(&lock(&app.store), &id)).await?;
    let job = a.posting_id.clone().unwrap_or_else(|| a.id.clone());
    let llm = app.claude().await?;
    let model = app.settings().suggest_model;
    let now = Local::now();
    let (today, weekday, offset) = (
        now.format("%Y-%m-%d").to_string(),
        now.format("%A").to_string(),
        now.format("%:z").to_string(),
    );
    let zone = iana_time_zone::get_timezone().ok();
    let m = model.clone();
    let (invite, out) = model_call(app, &job, Stage::Inviting, async move {
        let input = InviteInput {
            text: &text,
            today: &today,
            weekday: &weekday,
            offset: &offset,
            zone: zone.as_deref(),
            company: &a.company,
            role: &a.role,
        };
        invite::read(&llm, &invite::schema(), &input, &m).await
    })
    .await?;
    app.note_served(&model, &out);
    Ok(draft_of(invite))
}

/// The model's reading as the form takes it: a kind it doesn't name, a start that isn't RFC 3339 or a length out of
/// range dropped.
fn draft_of(i: Invite) -> RoundDraft {
    RoundDraft {
        id: None,
        kind: i.kind.as_deref().and_then(parse_kind),
        starts_at: i.starts_at.as_deref().and_then(normalized),
        minutes: i.minutes.filter(|m| (1..=MAX_MINUTES).contains(m)),
        with: i.with,
        place: i.place,
        notes: i.notes,
    }
}

/// A calendar invite file (`.ics`) read into a round's fields: its first event.
pub fn read_invite_file(path: &str) -> Result<RoundDraft> {
    let bytes = std::fs::read(path)?;
    ics::read(&String::from_utf8_lossy(&bytes))
        .ok_or_else(|| AppError::other("This file holds no calendar event"))
}

/// The resume the application sent: the tailored resume as exported, rendered again with its master's template (the
/// default when the master is gone); else the exported PDF itself.
pub fn sent_resume(app: &App, application: &str) -> Result<SentView> {
    let store = lock(&app.store);
    let a = application_of(&store, application)?;
    if let Some(json) = store.sent(&a.id)? {
        let master = match a.analysis_id.as_deref() {
            Some(id) => match store.analysis_by_id(id)? {
                Some(analysis) => store.master(&analysis.master_id)?.map(|(_, m)| m),
                None => None,
            },
            None => None,
        };
        drop(store);
        let master = master.unwrap_or_else(default_master);
        let pages = app.with_renderer(&master, |r| Ok(r.render(&json, false)?.svgs()))?;
        return Ok(SentView {
            pages,
            from: SentFrom::Sent,
        });
    }
    drop(store);
    let Some(path) = a.resume_path else {
        return Err(AppError::missing("No resume was sent for this application"));
    };
    let name = Path::new(&path)
        .file_name()
        .map_or_else(|| path.clone(), |n| n.to_string_lossy().into_owned());
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(AppError::missing(format!(
                "{name} isn’t where it was exported anymore"
            )));
        }
        Err(e) => return Err(e.into()),
    };
    let pages = resume_render::pdf_svgs(bytes)
        .filter(|pages| !pages.is_empty())
        .ok_or_else(|| AppError::other(format!("{name} couldn’t be read as a PDF")))?;
    Ok(SentView {
        pages,
        from: SentFrom::Pdf,
    })
}

fn default_master() -> Master {
    Master {
        id: String::new(),
        title: String::new(),
        template: DEFAULT_TEMPLATE.to_owned(),
        page_limit: 1,
        sections: vec![],
    }
}

/// `s` as RFC 3339 to the second, in the offset it was given in (`Z` for UTC); `None` when it isn't RFC 3339.
fn normalized(s: &str) -> Option<String> {
    DateTime::parse_from_rfc3339(s.trim())
        .ok()
        .map(|d| d.to_rfc3339_opts(SecondsFormat::Secs, true))
}

/// A stored round as the UI shows it; a kind the app doesn't know reads as Other.
pub fn round(r: store::Round) -> Round {
    Round {
        kind: parse_kind(&r.kind).unwrap_or(RoundKind::Other),
        id: r.id,
        application: r.application_id,
        starts_at: r.starts_at,
        minutes: r.minutes,
        with: r.with,
        place: r.place,
        notes: r.notes,
        state: match r.state {
            StoreState::Upcoming => RoundState::Upcoming,
            StoreState::Cancelled => RoundState::Cancelled,
        },
    }
}

/// The kind's name, as `RoundKind` serializes and the store keeps it.
fn kind_name(kind: RoundKind) -> &'static str {
    match kind {
        RoundKind::RecruiterScreen => "recruiter-screen",
        RoundKind::HiringManager => "hiring-manager",
        RoundKind::Technical => "technical",
        RoundKind::TakeHome => "take-home",
        RoundKind::SystemDesign => "system-design",
        RoundKind::Behavioral => "behavioral",
        RoundKind::Onsite => "onsite",
        RoundKind::Final => "final",
        RoundKind::Other => "other",
    }
}

fn parse_kind(name: &str) -> Option<RoundKind> {
    serde_json::from_value(serde_json::Value::String(name.to_owned())).ok()
}

#[cfg(test)]
mod tests {
    use resume_llm::invite::Invite;

    use super::{draft_of, kind_name, normalized, parse_kind};
    use crate::dto::RoundKind;

    #[test]
    fn kinds_round_trip_by_name() {
        for kind in [
            RoundKind::RecruiterScreen,
            RoundKind::HiringManager,
            RoundKind::Technical,
            RoundKind::TakeHome,
            RoundKind::SystemDesign,
            RoundKind::Behavioral,
            RoundKind::Onsite,
            RoundKind::Final,
            RoundKind::Other,
        ] {
            assert_eq!(parse_kind(kind_name(kind)), Some(kind));
            assert_eq!(
                serde_json::to_value(kind).unwrap(),
                serde_json::json!(kind_name(kind))
            );
        }
        assert_eq!(parse_kind("lunch"), None);
    }

    #[test]
    fn starts_keep_their_offset_to_the_second() {
        assert_eq!(
            normalized(" 2026-11-03T14:00:00.000-05:00 ").as_deref(),
            Some("2026-11-03T14:00:00-05:00")
        );
        assert_eq!(
            normalized("2026-11-03T19:00:00+00:00").as_deref(),
            Some("2026-11-03T19:00:00Z")
        );
        assert_eq!(normalized("2026-11-03 2pm"), None);
        assert_eq!(normalized("2026-11-03"), None);
    }

    #[test]
    fn an_invite_reading_drops_what_isnt_valid() {
        let d = draft_of(Invite {
            kind: Some("lunch".into()),
            starts_at: Some("Tuesday 2pm".into()),
            minutes: Some(900),
            with: "Priya".into(),
            place: String::new(),
            notes: String::new(),
        });
        assert_eq!((d.kind, d.starts_at, d.minutes), (None, None, None));
        assert_eq!(d.with, "Priya");
        let d = draft_of(Invite {
            kind: Some("system-design".into()),
            starts_at: Some("2026-11-03T14:00:00+01:00".into()),
            minutes: Some(45),
            ..Invite::default()
        });
        assert_eq!(d.kind, Some(RoundKind::SystemDesign));
        assert_eq!(d.starts_at.as_deref(), Some("2026-11-03T14:00:00+01:00"));
        assert_eq!(d.minutes, Some(45));
    }
}

//! Debriefs (local/interviewer/SPEC.md §2, §9): what you write down after a round (the questions they asked, from its
//! prep or typed; what you got stuck on; notes), saved with no model call; and one model call that reads it for Dig
//! questions on the bank entries what you got stuck on touches, and notes for the next round's prep. What reading
//! found is kept with the debrief until the debrief changes. A later round's prep reads the earlier rounds' debriefs
//! ([`earlier`]).

use std::sync::Arc;

use chrono::DateTime;
use resume_llm::debrief::{self as reading, DebriefInput, Earlier, Said};
use resume_llm::prep as written;
use resume_store::{self as store, DebriefRow, Store};
use serde::{Deserialize, Serialize};

use crate::dig::question;
use crate::dto::{Asked, Debrief, DebriefDig, DebriefRead, DebriefView, Stage};
use crate::error::{AppError, Result};
use crate::prep::sent_of;
use crate::rounds::application_of;
use crate::state::{App, blocking, lock, model_call};

/// What reading a debrief found, as the store keeps it (`debriefs.read`): the checked read and the model that wrote it.
#[derive(Serialize, Deserialize)]
struct ReadDoc {
    #[serde(flatten)]
    read: reading::Read,
    model: String,
}

/// The round's debrief, `None` while it has none.
pub fn debrief(app: &App, round: &str) -> Result<Option<DebriefView>> {
    lock(&app.store)
        .debrief(round)?
        .map(|d| view(&d))
        .transpose()
}

/// Saves the round's debrief as you wrote it, tidied ([`tidied`]); no model call. What reading it found stays only
/// while the debrief is unchanged.
pub fn save_debrief(app: &App, round: &str, debrief: Debrief) -> Result<DebriefView> {
    let store = lock(&app.store);
    let r = round_of(&store, round)?;
    let prep_ids = prep_ids(&store, &r.id)?;
    let d = tidied(debrief, &prep_ids);
    let row = store.put_debrief(&r.id, &serde_json::to_string(&d)?)?;
    drop(store);
    view(&row)
}

fn round_of(store: &Store, id: &str) -> Result<store::Round> {
    store
        .round(id)?
        .ok_or_else(|| AppError::missing("This interview was removed"))
}

/// The question ids of the round's prep written last; none without a prep.
fn prep_ids(store: &Store, round: &str) -> Result<Vec<String>> {
    Ok(match store.latest_prep(round)? {
        Some(p) => serde_json::from_str::<written::Prep>(&p.json)?
            .questions
            .into_iter()
            .map(|q| q.id)
            .collect(),
        None => Vec::new(),
    })
}

/// `d` as it's saved: each question's text with its spaces tidied, empty ones dropped, a prep id the round's prep
/// (`prep_ids`) doesn't have read as typed, a question already listed (the same prep question, or the same words
/// ignoring case) once; each topic tidied, empty ones dropped, once ignoring case; the notes trimmed.
pub(crate) fn tidied(d: Debrief, prep_ids: &[String]) -> Debrief {
    let mut asked: Vec<Asked> = Vec::new();
    for a in d.asked {
        let text = tidy(&a.text);
        if text.is_empty() {
            continue;
        }
        let prep = a
            .prep
            .map(|p| p.trim().to_owned())
            .filter(|p| prep_ids.contains(p));
        let seen = asked.iter().any(|x| {
            (prep.is_some() && x.prep == prep) || x.text.to_lowercase() == text.to_lowercase()
        });
        if !seen {
            asked.push(Asked { text, prep });
        }
    }
    let mut stuck_on: Vec<String> = Vec::new();
    for t in d.stuck_on {
        let t = tidy(&t);
        if !t.is_empty()
            && !stuck_on
                .iter()
                .any(|x| x.to_lowercase() == t.to_lowercase())
        {
            stuck_on.push(t);
        }
    }
    Debrief {
        asked,
        stuck_on,
        notes: d.notes.trim().to_owned(),
    }
}

fn tidy(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// One model call: the round's saved debrief read against the posting, the resume as sent and your bank, checked
/// (`resume_llm::debrief::checked`) and kept with the debrief. Runs under the application's posting (its progress and
/// `cancel`), as prep does.
pub async fn read_debrief(app: &Arc<App>, round: &str) -> Result<DebriefView> {
    let id = round.to_owned();
    let (r, a, row, d, requirements, bank, sent) = blocking(app, move |app| {
        let store = lock(&app.store);
        let r = round_of(&store, &id)?;
        let row = store
            .debrief(&r.id)?
            .ok_or_else(|| AppError::other("Save the debrief first"))?;
        let d: Debrief = serde_json::from_str(&row.debrief)?;
        if d.asked.is_empty() && d.stuck_on.is_empty() && d.notes.is_empty() {
            return Err(AppError::other(
                "Add what they asked or what you got stuck on first",
            ));
        }
        let a = application_of(&store, &r.application_id)?;
        let requirements = match a.posting_id.as_deref() {
            Some(p) => store.latest_requirements(p)?,
            None => None,
        };
        let (_, bank) = store.bank()?.ok_or_else(AppError::no_bank)?;
        let json = store.sent(&a.id)?;
        drop(store);
        let (sent, _) = sent_of(&a, json, &bank)?;
        Ok((r, a, row, d, requirements, bank, sent))
    })
    .await?;
    let llm = app.claude().await?;
    let model = app.settings().suggest_model;
    let job = a.posting_id.clone().unwrap_or_else(|| a.id.clone());
    let asked: Vec<String> = d.asked.iter().map(|q| q.text.clone()).collect();
    let (m, kind, role, company) = (
        model.clone(),
        r.kind.clone(),
        a.role.clone(),
        a.company.clone(),
    );
    let (req, b, st, ak, notes) = (
        requirements.clone(),
        bank.clone(),
        d.stuck_on.clone(),
        asked.clone(),
        d.notes.clone(),
    );
    let (reply, out) = model_call(app, &job, Stage::Debriefing, async move {
        let input = DebriefInput {
            kind: &kind,
            role: &role,
            company: &company,
            requirements: req.as_ref(),
            sent: &sent,
            bank: &b,
            said: Said {
                asked: &ak,
                stuck_on: &st,
                notes: &notes,
            },
        };
        reading::read(&llm, &reading::schema(), &input, &m).await
    })
    .await?;
    app.note_served(&model, &out);
    let said = Said {
        asked: &asked,
        stuck_on: &d.stuck_on,
        notes: &d.notes,
    };
    let job_names = [a.company.as_str(), a.role.as_str(), r.with.as_str()];
    let (read, dropped) = reading::checked(reply, &bank, &said, &job_names, requirements.as_ref());
    if !dropped.is_empty() {
        eprintln!(
            "gliide: debrief read for round {} dropped {dropped:?}",
            r.id
        );
    }
    let doc = ReadDoc {
        read,
        model: out.served(&model).unwrap_or(&model).to_owned(),
    };
    let json = serde_json::to_string(&doc)?;
    let stored = blocking(app, move |app| {
        lock(&app.store)
            .put_debrief_read(&row.round_id, &row.at, &json)?
            .ok_or_else(|| AppError::other("The debrief changed while it was read; read it again"))
    })
    .await?;
    view(&stored)
}

/// A stored debrief as the UI shows it.
fn view(row: &DebriefRow) -> Result<DebriefView> {
    let read = match &row.read {
        Some(json) => {
            let doc: ReadDoc = serde_json::from_str(json)?;
            Some(DebriefRead {
                digs: doc
                    .read
                    .digs
                    .into_iter()
                    .map(|d| DebriefDig {
                        entry: d.entry,
                        entry_name: d.entry_name,
                        topic: d.topic,
                        questions: d.questions.into_iter().map(question).collect(),
                    })
                    .collect(),
                prep_notes: doc.read.prep_notes,
                model: doc.model,
            })
        }
        None => None,
    };
    Ok(DebriefView {
        round: row.round_id.clone(),
        debrief: serde_json::from_str(&row.debrief)?,
        read,
        at: row.at.clone(),
    })
}

/// The debriefs of round `r`'s application that a prep for `r` reads, oldest first: every other round's with something
/// in it, unless both rounds are scheduled and that one doesn't start before `r`.
pub(crate) fn earlier(store: &Store, r: &store::Round) -> Result<Vec<Earlier>> {
    let rows = store.application_debriefs(&r.application_id)?;
    if rows.iter().all(|d| d.round_id == r.id) {
        return Ok(Vec::new());
    }
    let rounds = store.rounds(&r.application_id)?;
    let start = |s: Option<&str>| s.and_then(|s| DateTime::parse_from_rfc3339(s).ok());
    let this = start(r.starts_at.as_deref());
    let mut out = Vec::new();
    for row in rows {
        let Some(other) = rounds.iter().find(|x| x.id == row.round_id && x.id != r.id) else {
            continue;
        };
        if let (Some(t), Some(o)) = (this, start(other.starts_at.as_deref()))
            && o >= t
        {
            continue;
        }
        let d: Debrief = serde_json::from_str(&row.debrief)?;
        let prep_notes = match &row.read {
            Some(json) => serde_json::from_str::<ReadDoc>(json)?.read.prep_notes,
            None => Vec::new(),
        };
        if d.asked.is_empty()
            && d.stuck_on.is_empty()
            && d.notes.is_empty()
            && prep_notes.is_empty()
        {
            continue;
        }
        out.push(Earlier {
            kind: other.kind.clone(),
            date: other
                .starts_at
                .as_deref()
                .and_then(|s| s.get(..10))
                .unwrap_or_default()
                .to_owned(),
            asked: d.asked.into_iter().map(|a| a.text).collect(),
            stuck_on: d.stuck_on,
            notes: d.notes,
            prep_notes,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use resume_llm::dig::Question;
    use resume_store::{AppStatus, Application, Prep, RoundState};

    use super::*;

    fn asked(text: &str, prep: Option<&str>) -> Asked {
        Asked {
            text: text.into(),
            prep: prep.map(Into::into),
        }
    }

    #[test]
    fn a_debrief_is_tidied_as_saved() {
        let d = tidied(
            Debrief {
                asked: vec![
                    asked("  Walk me through   the API ", Some("q1")),
                    asked("Walk me through the API again", Some("q1")),
                    asked("How would you use Kafka?", Some("q9")),
                    asked("how would you use kafka?", None),
                    asked("   ", None),
                    asked("Why Globex?", None),
                ],
                stuck_on: vec![
                    " Kafka  consumer groups ".into(),
                    "kafka consumer groups".into(),
                    "".into(),
                    "Table sizing".into(),
                ],
                notes: "\n Final is with the manager.\n".into(),
            },
            &["q1".into(), "q2".into()],
        );
        assert_eq!(
            d.asked,
            [
                asked("Walk me through the API", Some("q1")),
                asked("How would you use Kafka?", None),
                asked("Why Globex?", None),
            ]
        );
        assert_eq!(d.stuck_on, ["Kafka consumer groups", "Table sizing"]);
        assert_eq!(d.notes, "Final is with the manager.");
    }

    fn app() -> App {
        App::new(Store::open_in_memory().unwrap(), Box::new(|_| {})).unwrap()
    }

    fn round(id: &str, starts_at: Option<&str>) -> store::Round {
        store::Round {
            id: id.into(),
            application_id: "a".into(),
            kind: "technical".into(),
            starts_at: starts_at.map(Into::into),
            minutes: Some(60),
            with: "Priya Shah".into(),
            place: String::new(),
            notes: String::new(),
            state: RoundState::Upcoming,
            debriefed_at: None,
            updated_at: String::new(),
        }
    }

    fn seed(app: &App) {
        let store = lock(&app.store);
        store
            .put_application(&Application {
                id: "a".into(),
                posting_id: None,
                analysis_id: None,
                date: "2026-10-01".into(),
                company: "Globex".into(),
                role: "Software Engineer".into(),
                url: None,
                status: AppStatus::Interviewing,
                now: None,
                projected: None,
                on_file: None,
                resume_path: None,
                cover_path: None,
                notes: String::new(),
                updated_at: String::new(),
            })
            .unwrap();
        store
            .put_round(&round("r1", Some("2026-10-20T14:00:00-04:00")))
            .unwrap();
        store
            .put_round(&round("r2", Some("2026-10-27T14:00:00-04:00")))
            .unwrap();
        let prep = written::Prep {
            focus: String::new(),
            questions: vec![written::Question {
                id: "q1".into(),
                group: written::Group::Resume,
                text: "Walk me through the API".into(),
                why: String::new(),
                lines: vec![],
                story: None,
            }],
            gaps: vec![],
            ask_them: vec![],
        };
        store
            .put_prep(&Prep {
                key: "k".into(),
                round_id: "r1".into(),
                json: serde_json::to_string(&prep).unwrap(),
                model: "sonnet".into(),
                at: String::new(),
            })
            .unwrap();
    }

    fn read_doc(prep_notes: &[&str]) -> String {
        serde_json::to_string(&ReadDoc {
            read: reading::Read {
                digs: vec![reading::Dig {
                    topic: "Kafka consumer groups".into(),
                    entry: "acme".into(),
                    entry_name: "Acme".into(),
                    questions: vec![Question {
                        id: "b1".into(),
                        about: "acme".into(),
                        about_label: "The entry".into(),
                        text: "Did anything at Acme read from Kafka?".into(),
                        why: "They pressed on it".into(),
                    }],
                }],
                prep_notes: prep_notes.iter().map(|n| (*n).to_owned()).collect(),
            },
            model: "claude-sonnet".into(),
        })
        .unwrap()
    }

    #[test]
    fn a_debrief_saves_reads_back_and_drops_a_stale_read() {
        let app = app();
        seed(&app);
        assert_eq!(debrief(&app, "r1").unwrap(), None);
        assert!(save_debrief(&app, "gone", Debrief::default()).is_err());

        let d = Debrief {
            asked: vec![
                asked("Walk me through the API", Some("q1")),
                asked("Why us?", Some("q7")),
            ],
            stuck_on: vec!["Kafka consumer groups".into()],
            notes: "Final next week.".into(),
        };
        let saved = save_debrief(&app, "r1", d.clone()).unwrap();
        assert_eq!(saved.round, "r1");
        assert_eq!(saved.debrief.asked[1].prep, None);
        assert_eq!(saved.read, None);
        assert_eq!(debrief(&app, "r1").unwrap().unwrap(), saved);
        let r1 = crate::rounds::rounds(&app, "a").unwrap();
        assert_eq!(r1[0].debriefed_at.as_deref(), Some(saved.at.as_str()));

        // What reading found shows with it, and stays while the debrief doesn't change.
        lock(&app.store)
            .put_debrief_read("r1", &saved.at, &read_doc(&["Kafka came up twice."]))
            .unwrap()
            .unwrap();
        let read = debrief(&app, "r1").unwrap().unwrap().read.unwrap();
        assert_eq!(read.model, "claude-sonnet");
        assert_eq!(read.prep_notes, ["Kafka came up twice."]);
        assert_eq!(read.digs[0].topic, "Kafka consumer groups");
        assert_eq!(read.digs[0].questions[0].id, "b1");
        assert!(save_debrief(&app, "r1", d.clone()).unwrap().read.is_some());
        let changed = save_debrief(
            &app,
            "r1",
            Debrief {
                notes: "Final on Friday.".into(),
                ..d
            },
        )
        .unwrap();
        assert_eq!(changed.read, None);
    }

    #[test]
    fn a_later_rounds_prep_reads_the_earlier_debriefs() {
        let app = app();
        seed(&app);
        let store = lock(&app.store);
        let r1 = store.round("r1").unwrap().unwrap();
        let r2 = store.round("r2").unwrap().unwrap();
        assert!(earlier(&store, &r2).unwrap().is_empty());
        drop(store);
        let saved = save_debrief(
            &app,
            "r1",
            Debrief {
                asked: vec![asked("Walk me through the API", Some("q1"))],
                stuck_on: vec!["Kafka consumer groups".into()],
                notes: "Final next week.".into(),
            },
        )
        .unwrap();
        save_debrief(
            &app,
            "r2",
            Debrief {
                notes: "Later.".into(),
                ..Debrief::default()
            },
        )
        .unwrap();
        let store = lock(&app.store);
        store
            .put_debrief_read("r1", &saved.at, &read_doc(&["Kafka came up twice."]))
            .unwrap();
        assert_eq!(
            earlier(&store, &r2).unwrap(),
            [Earlier {
                kind: "technical".into(),
                date: "2026-10-20".into(),
                asked: vec!["Walk me through the API".into()],
                stuck_on: vec!["Kafka consumer groups".into()],
                notes: "Final next week.".into(),
                prep_notes: vec!["Kafka came up twice.".into()],
            }]
        );
        // Not its own, nor a later round's.
        assert!(earlier(&store, &r1).unwrap().is_empty());
        // An unscheduled round reads every other.
        let open = store.put_round(&round("r3", None)).unwrap();
        assert_eq!(earlier(&store, &open).unwrap().len(), 2);
    }
}

//! A round's prep and typed practice (local/interviewer/SPEC.md §5, §9): the prep (one model call, kept until what it
//! was written from changes; each likely question linked to the story of yours that answers it), an answer to one of
//! its questions read back (with the question's story, and the answer as a story to keep), the answers practiced so
//! far, and a claim an answer made kept as a fact of its bank entry.

use std::sync::Arc;

use chrono::{Local, SecondsFormat, Utc};
use resume_core::bank::{Bank, FactKind, FactSource};
use resume_core::facts::add_fact;
use resume_core::posting::text_hash;
use resume_core::story::Story;
use resume_core::{Requirements, Resume};
use resume_llm::debrief::Earlier;
use resume_llm::nudge::{self, LastAnswer, NudgeInput};
use resume_llm::practice::{self, PracticeInput};
use resume_llm::prep::{self as written, PrepInput, SentResume};
use resume_store::{self as store, Application, PracticeRow};

use crate::dto::{
    Feedback, Mismatch, NewClaim, Nudge, PracticeTurn, PrepGap, PrepQuestion, PrepView,
    QuestionGroup, Stage, StarPart, StorySource,
};
use crate::error::{AppError, Result};
use crate::rounds::{application_of, sent_pdf};
use crate::state::{App, blocking, lock, model_call};
use crate::stories::draft_of;

/// What a prep or a practiced answer is read against: the round, its application and posting, the resume as sent.
struct Context {
    round: store::Round,
    application: Application,
    /// The model call's job: the posting, else the application.
    job: String,
    requirements: Option<Requirements>,
    sent: SentResume,
    /// Your stories, newest first.
    stories: Vec<Story>,
    /// This application's earlier rounds' debriefs, oldest first.
    debriefs: Vec<Earlier>,
    /// What the prep was written from, hashed (the round included).
    key: String,
}

fn context(app: &App, round: &str) -> Result<Context> {
    let store = lock(&app.store);
    let r = store
        .round(round)?
        .ok_or_else(|| AppError::missing("This interview was removed"))?;
    let a = application_of(&store, &r.application_id)?;
    let posting = match a.posting_id.as_deref() {
        Some(id) => store.posting(id)?,
        None => None,
    };
    let requirements = match a.posting_id.as_deref() {
        Some(id) => store.latest_requirements(id)?,
        None => None,
    };
    let (_, bank) = store.bank()?.ok_or_else(AppError::no_bank)?;
    let json = store.sent(&a.id)?;
    let stories = store.stories()?;
    let debriefs = crate::debrief::earlier(&store, &r)?;
    drop(store);
    let (sent, sent_hash) = sent_of(&a, json, &bank)?;
    let posting_hash = posting.map(|p| p.posting.hash).unwrap_or_default();
    // Not the bank's revision, nor your stories: a fact kept or a story saved from practice would otherwise rewrite the
    // prep (and renumber its questions) the next time the tab opens. Write again takes the bank and stories as they
    // are now. Earlier rounds' debriefs count when there are some: a debrief saved or read means a fresh prep.
    let mut from = format!(
        "{}\n{}\n{}\n{posting_hash}\n{sent_hash}\n{}\n{}\n{}",
        written::PROMPT,
        written::SCHEMA,
        r.id,
        r.kind,
        r.with,
        r.notes
    );
    if !debriefs.is_empty() {
        from.push('\n');
        from.push_str(written::DEBRIEFS_PROMPT);
        from.push('\n');
        from.push_str(&serde_json::to_string(&debriefs)?);
    }
    let key = text_hash(&from);
    Ok(Context {
        job: a.posting_id.clone().unwrap_or_else(|| a.id.clone()),
        round: r,
        application: a,
        requirements,
        sent,
        stories,
        debriefs,
        key,
    })
}

/// The resume the application sent as Prep, Practice and Debrief read it, and its hash: the tailored resume as
/// exported (`json`) tagged with its entries' facts from `bank`; else the exported PDF's text.
pub(crate) fn sent_of(
    a: &Application,
    json: Option<String>,
    bank: &Bank,
) -> Result<(SentResume, String)> {
    Ok(match json {
        Some(json) => {
            let resume: Resume = serde_json::from_str(&json)?;
            (SentResume::tagged(&resume, bank), text_hash(&json))
        }
        None => {
            let (name, bytes) = sent_pdf(a)?;
            let text = resume_render::pdf_text(bytes)
                .filter(|t| !t.trim().is_empty())
                .ok_or_else(|| AppError::other(format!("{name} couldn’t be read as a PDF")))?;
            (SentResume::pdf(&text), text_hash(&text))
        }
    })
}

/// The round's prep: the one kept for what it would be written from now, unless `again`; else one model call,
/// checked against the resume as sent, the bank and your stories, and kept.
pub async fn prep(app: &Arc<App>, round: &str, again: bool) -> Result<PrepView> {
    let id = round.to_owned();
    let (cx, kept) = blocking(app, move |app| {
        let cx = context(app, &id)?;
        let kept = if again {
            None
        } else {
            lock(&app.store).prep(&cx.key)?
        };
        Ok((cx, kept))
    })
    .await?;
    if let Some(p) = kept {
        return view(&p, &cx.stories);
    }
    let llm = app.agent().await?;
    let model = app.settings().current().suggest_model.clone();
    let today = Local::now().format("%Y-%m-%d").to_string();
    let Context {
        round,
        application,
        job,
        requirements,
        sent,
        stories,
        debriefs,
        key,
    } = cx;
    let (m, kind, with, notes, req, s, st) = (
        model.clone(),
        round.kind.clone(),
        round.with.clone(),
        round.notes.clone(),
        requirements.clone(),
        sent.clone(),
        stories.clone(),
    );
    let (role, company) = (application.role.clone(), application.company.clone());
    let (reply, out) = model_call(app, &job, Stage::Prepping, async move {
        let input = PrepInput {
            kind: &kind,
            with: &with,
            notes: &notes,
            role: &role,
            company: &company,
            requirements: req.as_ref(),
            sent: &s,
            stories: &st,
            debriefs: &debriefs,
            today: &today,
        };
        written::write(&llm, &written::schema(), &input, &m).await
    })
    .await?;
    app.note_served(&model, &out);
    let (p, dropped) = written::checked(reply, &sent, requirements.as_ref(), &stories);
    if !dropped.is_empty() {
        eprintln!("gliide: prep for round {} dropped {dropped:?}", round.id);
    }
    let served = out.served(&model).unwrap_or(&model).to_owned();
    let row = store::Prep {
        key,
        round_id: round.id,
        json: serde_json::to_string(&p)?,
        model: served,
        at: String::new(),
    };
    let stored = blocking(app, move |app| Ok(lock(&app.store).put_prep(&row)?)).await?;
    view(&stored, &stories)
}

/// A stored prep as the Interview tab shows it; a question's story you've removed since reads as none.
fn view(p: &store::Prep, stories: &[Story]) -> Result<PrepView> {
    let w: written::Prep = serde_json::from_str(&p.json)?;
    Ok(PrepView {
        round: p.round_id.clone(),
        focus: w.focus,
        questions: w
            .questions
            .into_iter()
            .map(|q| PrepQuestion {
                id: q.id,
                group: match q.group {
                    written::Group::Resume => QuestionGroup::Resume,
                    written::Group::Skills => QuestionGroup::Skills,
                    written::Group::Behavioral => QuestionGroup::Behavioral,
                    written::Group::Logistics => QuestionGroup::Logistics,
                },
                text: q.text,
                why: q.why,
                lines: q.lines,
                story: q.story.filter(|id| stories.iter().any(|s| s.id == *id)),
            })
            .collect(),
        gaps: w
            .gaps
            .into_iter()
            .map(|g| PrepGap {
                term: g.term,
                required: g.required,
                answer: g.answer,
                lines: g.lines,
            })
            .collect(),
        ask_them: w.ask_them,
        model: p.model.clone(),
        at: p.at.clone(),
    })
}

/// One model call: `answer` to the round's prep question `question` read back (with the story the question links,
/// when it links one), checked against the resume as sent, the bank and the answer, and kept as a practice turn.
pub async fn practice(
    app: &Arc<App>,
    round: &str,
    question: String,
    answer: String,
) -> Result<PracticeTurn> {
    let answer = answer.trim().to_owned();
    if answer.is_empty() {
        return Err(AppError::other("Type your answer first"));
    }
    let id = round.to_owned();
    let (cx, prep) = blocking(app, move |app| {
        let cx = context(app, &id)?;
        let prep = lock(&app.store)
            .latest_prep(&id)?
            .ok_or_else(|| AppError::missing("Write this round’s prep first"))?;
        Ok((cx, prep))
    })
    .await?;
    let prep: written::Prep = serde_json::from_str(&prep.json)?;
    let q = prep
        .questions
        .into_iter()
        .find(|q| q.id == question)
        .ok_or_else(|| AppError::missing("This question is no longer in the prep"))?;
    let told: Option<Story> = q
        .story
        .as_deref()
        .and_then(|id| cx.stories.iter().find(|s| s.id == id))
        .cloned();
    let terms: Vec<String> = cx
        .requirements
        .as_ref()
        .map(|r| r.skills.iter().map(|k| k.term.clone()).collect())
        .unwrap_or_default();
    let llm = app.agent().await?;
    let model = app.settings().current().suggest_model.clone();
    let (m, s, a, qt, st) = (
        model.clone(),
        cx.sent.clone(),
        answer.clone(),
        q.clone(),
        told.clone(),
    );
    let (reply, out) = model_call(app, &cx.job, Stage::Practicing, async move {
        let input = PracticeInput {
            question: &qt.text,
            why: &qt.why,
            lines: &qt.lines,
            answer: &a,
            terms: &terms,
            sent: &s,
            story: st.as_ref(),
        };
        practice::read(&llm, &practice::schema(), &input, &m).await
    })
    .await?;
    app.note_served(&model, &out);
    let (f, dropped) = practice::checked(reply, &cx.sent, &q.lines, &answer, told.as_ref());
    if !dropped.is_empty() {
        eprintln!(
            "gliide: practice on round {} dropped {dropped:?}",
            cx.round.id
        );
    }
    let row = PracticeRow {
        id: uuid::Uuid::new_v4().to_string(),
        round_id: Some(cx.round.id),
        story_id: told.map(|s| s.id),
        question: q.id,
        asked: q.text,
        answer,
        feedback: serde_json::to_string(&f)?,
        at: String::new(),
    };
    let stored = blocking(app, move |app| Ok(lock(&app.store).add_practice(&row)?)).await?;
    turn(stored)
}

/// One model call: nudges for prep question `question` of the round while you answer it (`draft`, what you've typed;
/// may be empty), read with your last answer to it and what its reading found, checked against the resume as sent,
/// the bank and your words. Not kept: they're for the draft in front of you.
pub async fn nudge(
    app: &Arc<App>,
    round: &str,
    question: String,
    draft: String,
) -> Result<Vec<Nudge>> {
    let (id, wanted) = (round.to_owned(), question.clone());
    let (cx, prep, last) = blocking(app, move |app| {
        let cx = context(app, &id)?;
        let store = lock(&app.store);
        let prep = store
            .latest_prep(&id)?
            .ok_or_else(|| AppError::missing("Write this round’s prep first"))?;
        let last = store
            .practice(&id)?
            .into_iter()
            .rfind(|p| p.question == wanted);
        Ok((cx, prep, last))
    })
    .await?;
    let prep: written::Prep = serde_json::from_str(&prep.json)?;
    let q = prep
        .questions
        .into_iter()
        .find(|q| q.id == question)
        .ok_or_else(|| AppError::missing("This question is no longer in the prep"))?;
    let told: Option<Story> = q
        .story
        .as_deref()
        .and_then(|id| cx.stories.iter().find(|s| s.id == id))
        .cloned();
    let terms: Vec<String> = cx
        .requirements
        .as_ref()
        .map(|r| r.skills.iter().map(|k| k.term.clone()).collect())
        .unwrap_or_default();
    let last = match last {
        Some(p) => Some((p.answer, serde_json::from_str::<practice::Feedback>(&p.feedback)?)),
        None => None,
    };
    let llm = app.agent().await?;
    let model = app.settings().current().suggest_model.clone();
    let (m, s, d, qt, st, la) = (
        model.clone(),
        cx.sent.clone(),
        draft.clone(),
        q.clone(),
        told.clone(),
        last.clone(),
    );
    let (reply, out) = model_call(app, &cx.job, Stage::Nudging, async move {
        let input = NudgeInput {
            question: &qt.text,
            why: &qt.why,
            lines: &qt.lines,
            draft: &d,
            last: la.as_ref().map(|(answer, f)| LastAnswer {
                answer,
                situation: f.situation,
                task: f.task,
                action: f.action,
                result: f.result,
                note: f.star_note.as_deref(),
            }),
            terms: &terms,
            sent: &s,
            story: st.as_ref(),
        };
        nudge::read(&llm, &nudge::schema(), &input, &m).await
    })
    .await?;
    app.note_served(&model, &out);
    let last_answer = last.as_ref().map(|(a, _)| a.as_str());
    let (nudges, dropped) = nudge::checked(reply, &cx.sent, &q.lines, &draft, last_answer, told.as_ref());
    if !dropped.is_empty() {
        eprintln!("gliide: nudge on round {} dropped {dropped:?}", cx.round.id);
    }
    Ok(nudges
        .into_iter()
        .map(|n| Nudge {
            text: n.text,
            lines: n.lines,
            example: n.example,
        })
        .collect())
}

/// The round's practiced answers, oldest first.
pub fn practice_turns(app: &App, round: &str) -> Result<Vec<PracticeTurn>> {
    lock(&app.store)
        .practice(round)?
        .into_iter()
        .map(turn)
        .collect()
}

/// A stored practice row as the thread shows it; its story draft is drawn from this turn (`from` its id).
pub(crate) fn turn(p: PracticeRow) -> Result<PracticeTurn> {
    let f: practice::Feedback = serde_json::from_str(&p.feedback)?;
    let star = |s: practice::StarPart| match s {
        practice::StarPart::Yes => StarPart::Yes,
        practice::StarPart::Thin => StarPart::Thin,
        practice::StarPart::No => StarPart::No,
    };
    let story = f.story.map(|s| draft_of(s, StorySource::Practice, &p.id));
    Ok(PracticeTurn {
        round: p.round_id,
        story: p.story_id,
        question: p.question,
        asked: p.asked,
        answer: p.answer,
        feedback: Feedback {
            lines: f.lines,
            situation: star(f.situation),
            task: star(f.task),
            action: star(f.action),
            result: star(f.result),
            star_note: f.star_note,
            mismatches: f
                .mismatches
                .into_iter()
                .map(|m| Mismatch {
                    said: m.said,
                    resume: m.resume,
                    line: m.line,
                })
                .collect(),
            new_claims: f
                .new_claims
                .into_iter()
                .map(|c| NewClaim {
                    text: c.text,
                    entry: c.entry,
                    entry_name: c.entry_name,
                })
                .collect(),
            tighter: f.tighter,
            follow_up: f.follow_up,
            story,
        },
        id: p.id,
        at: p.at,
    })
}

/// Keeps `text`, a claim a practiced answer made, as a fact of bank entry `entry` (your word is a source): said for
/// the round's posting and company; for a story told alone (`round` `None`), for none. Writes a new bank revision.
pub fn keep_claim(app: &App, round: Option<&str>, entry: &str, text: &str) -> Result<()> {
    let store = lock(&app.store);
    let a = match round {
        Some(round) => {
            let r = store
                .round(round)?
                .ok_or_else(|| AppError::missing("This interview was removed"))?;
            Some(application_of(&store, &r.application_id)?)
        }
        None => None,
    };
    let (_, mut bank): (_, Bank) = store.bank()?.ok_or_else(AppError::no_bank)?;
    let source = FactSource {
        kind: FactKind::Practice,
        at: Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
        posting: a.as_ref().and_then(|a| a.posting_id.clone()),
        company: a
            .as_ref()
            .map(|a| a.company.trim())
            .filter(|c| !c.is_empty())
            .map(str::to_owned),
    };
    add_fact(&mut bank, entry, text, &[], source).map_err(|e| AppError::other(e.to_string()))?;
    store.put_bank(&bank)?;
    Ok(())
}

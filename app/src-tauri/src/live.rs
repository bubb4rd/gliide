//! A round's live interview (ARCHITECTURE.md §3, Live): the prep's questions asked in order, verbatim, with no call;
//! after each answer one model call decides a follow-up on what you said, or none, at most
//! [`MAX_FOLLOW_UPS`] a question; then the next question, and past the last the run ends. Your answer is kept before
//! the call, so a failed or stopped call leaves the run `waiting` for the step again. Ended, each question's exchange
//! is read as Practice reads an answer and kept as practice of the round, from this run (the readback).

use std::sync::Arc;

use resume_llm::live::{self, LiveInput, MAX_FOLLOW_UPS};
use resume_llm::practice::{self, PracticeInput};
use resume_llm::prep as written;
use resume_store::{self as store, LiveKind, LiveRunRow, LiveTurnRow, PracticeRow, Store};

use crate::dto::{
    Feature, LiveRun, LiveSession, LiveTakenBack, LiveTurn, LiveTurnKind, PracticeTurn, Spoken,
    Stage,
};
use crate::error::{AppError, Result};
use crate::prep::{context, turn};
use crate::state::{App, blocking, lock, model_call, single_flight};

/// Starts a live interview of the round: its first turn asks the prep's first question.
pub fn start(app: &App, round: &str) -> Result<LiveSession> {
    let store = lock(&app.store);
    let prep = prep_of(&store, round)?;
    let first = prep
        .questions
        .first()
        .ok_or_else(|| AppError::missing("This round’s prep has no questions"))?;
    let run = store.add_live_run(round, prep.questions.len() as u32)?;
    store.add_live_turn(&run.id, &ask(first))?;
    session_in(&store, &run.id)
}

/// Live run `run` as it stands, with what its readback has read so far.
pub fn session(app: &App, run: &str) -> Result<LiveSession> {
    session_in(&lock(&app.store), run)
}

/// The round's live runs, the newest first.
pub fn runs(app: &App, round: &str) -> Result<Vec<LiveRun>> {
    Ok(lock(&app.store)
        .live_runs(round)?
        .into_iter()
        .map(|r| LiveRun {
            id: r.id,
            started_at: r.started_at,
            ended_at: r.ended_at,
            active_ms: r.active_ms,
            asked: r.asked,
            follows: r.follows,
        })
        .collect())
}

/// `answer` to the current turn, kept first, then the step after it (see [`step`]). The step failing (or `cancel`
/// stopping its call) leaves the answer kept and the run `waiting`.
pub async fn answer(
    app: &Arc<App>,
    run: &str,
    answer: String,
    spoken: Option<Spoken>,
) -> Result<LiveSession> {
    let answer = answer.trim().to_owned();
    if answer.is_empty() {
        return Err(AppError::other("Type your answer first"));
    }
    let id = run.to_owned();
    blocking(app, move |app| {
        let store = lock(&app.store);
        let (row, turns) = run_of(&store, &id)?;
        if row.ended_at.is_some() {
            return Err(AppError::other("This interview has ended"));
        }
        let seq = current(&turns)
            .ok_or_else(|| AppError::other("Wait for the next question"))?
            .seq;
        let spoken = spoken.map(|s| store::Spoken {
            ms: s.ms,
            words: s.words,
        });
        Ok(store.answer_live_turn(&id, seq, &answer, spoken)?)
    })
    .await?;
    step(app, run).await
}

/// The step owed after the last answer, again (the run as it is when none is owed).
pub async fn retry(app: &Arc<App>, run: &str) -> Result<LiveSession> {
    step(app, run).await
}

/// Takes back your last answer: its turn unanswered again (a turn asked after it removed) and its text, to put back
/// in the box. The caller stops a call running for it (`cancel`) first.
pub fn take_back(app: &App, run: &str) -> Result<LiveTakenBack> {
    let store = lock(&app.store);
    let (row, _) = run_of(&store, run)?;
    if row.ended_at.is_some() {
        return Err(AppError::other("This interview has ended"));
    }
    let answer = store
        .unanswer_live_turn(run)?
        .ok_or_else(|| AppError::other("There’s no answer to take back"))?;
    Ok(LiveTakenBack {
        session: session_in(&store, run)?,
        answer,
    })
}

/// Next question: the current turn skipped (or, while `waiting`, the follow-up owed not asked), then the next prep
/// question, or the end past the last. The caller stops a call running for it (`cancel`) first.
pub fn skip(app: &App, run: &str) -> Result<LiveSession> {
    let store = lock(&app.store);
    let (row, turns) = run_of(&store, run)?;
    if row.ended_at.is_some() {
        return Err(AppError::other("This interview has ended"));
    }
    match current(&turns) {
        Some(t) => store.skip_live_turn(run, t.seq)?,
        None if owed(&row, &turns) => {}
        None => return Err(AppError::other("There’s no question to skip")),
    }
    let prep = prep_of(&store, &row.round_id)?;
    next(&store, run, &prep)?;
    session_in(&store, run)
}

/// Ends the run with the clock as the UI kept it. `answer` (Send and end), when it says something, is kept on the
/// current turn first, with no call; else a turn unanswered is dropped. A run that ended by itself keeps when it ended
/// and takes the clock.
pub fn end(
    app: &App,
    run: &str,
    active_ms: u32,
    answer: Option<String>,
    spoken: Option<Spoken>,
) -> Result<LiveSession> {
    let store = lock(&app.store);
    if let Some(answer) = answer.as_deref().map(str::trim).filter(|a| !a.is_empty()) {
        let (row, turns) = run_of(&store, run)?;
        if row.ended_at.is_some() {
            return Err(AppError::other("This interview has ended"));
        }
        let seq = current(&turns)
            .ok_or_else(|| AppError::other("There’s no question to answer"))?
            .seq;
        let spoken = spoken.map(|s| store::Spoken {
            ms: s.ms,
            words: s.words,
        });
        store.answer_live_turn(run, seq, answer, spoken)?;
    }
    store.end_live_run(run, Some(active_ms))?;
    session_in(&store, run)
}

/// One model call: the ended run's exchange on prep question `question` (your answer, then each follow-up and your
/// reply) read as Practice reads an answer, checked against the resume as sent, the bank and all you said, and kept
/// as practice of the round from this run. Read already, the row kept, with no call; a caller arriving while the call
/// runs gets the row it keeps.
pub async fn read(app: &Arc<App>, run: &str, question: String) -> Result<PracticeTurn> {
    let key = format!("live read {run} {question}");
    single_flight(app, &key, async |_| {
        let (id, wanted) = (run.to_owned(), question.clone());
        let (row, turns, prep, done) = blocking(app, move |app| {
            let store = lock(&app.store);
            let (row, turns) = run_of(&store, &id)?;
            let done = store
                .live_practice(&id)?
                .into_iter()
                .find(|p| p.question == wanted);
            let prep = prep_of(&store, &row.round_id)?;
            Ok((row, turns, prep, done))
        })
        .await?;
        if let Some(p) = done {
            return turn(p);
        }
        if row.ended_at.is_none() {
            return Err(AppError::other("End the interview first"));
        }
        let asked = turns
            .iter()
            .find(|t| t.kind == LiveKind::Ask && t.question == question)
            .filter(|t| t.answer.is_some())
            .ok_or_else(|| AppError::missing("This question wasn’t answered in this interview"))?;
        let answer = asked.answer.clone().unwrap_or_default();
        let mine: Vec<&LiveTurnRow> = turns
            .iter()
            .filter(|t| t.question == question && t.answer.is_some())
            .collect();
        let follow_ups = follow_ups(&turns, &question);
        let spoken = mine
            .iter()
            .filter_map(|t| t.spoken)
            .reduce(|a, b| store::Spoken {
                ms: a.ms.saturating_add(b.ms),
                words: a.words.saturating_add(b.words),
            });
        let q = question_of(&prep, &question, &asked.asked);
        let round = row.round_id.clone();
        let cx = blocking(app, move |app| context(app, &round)).await?;
        let told = q
            .story
            .as_deref()
            .and_then(|id| cx.stories.iter().find(|s| s.id == id))
            .cloned();
        let terms = cx.terms();
        let route = app.route(Feature::Practice);
        let llm = app.llm(&route, Feature::Practice).await?;
        let model = route.model;
        let (m, s, a, qt, st, fu) = (
            model.clone(),
            cx.sent.clone(),
            answer.clone(),
            q.clone(),
            told.clone(),
            follow_ups.clone(),
        );
        let (reply, out) = model_call(app, &cx.job, Stage::Practicing, async move {
            let input = PracticeInput {
                question: &qt.text,
                why: &qt.why,
                lines: &qt.lines,
                answer: &a,
                follow_ups: &fu,
                terms: &terms,
                sent: &s,
                story: st.as_ref(),
            };
            practice::read(&llm, &practice::schema(), &input, &m).await
        })
        .await?;
        app.note_served(&model, &out);
        let said = practice::said(&answer, &follow_ups);
        let (f, dropped) = practice::checked(reply, &cx.sent, &q.lines, &said, told.as_ref());
        if !dropped.is_empty() {
            eprintln!(
                "gliide: live read on round {} dropped {dropped:?}",
                cx.round.id
            );
        }
        let p = PracticeRow {
            id: uuid::Uuid::new_v4().to_string(),
            round_id: Some(cx.round.id),
            story_id: told.map(|s| s.id),
            question: question.clone(),
            asked: asked.asked.clone(),
            answer,
            feedback: serde_json::to_string(&f)?,
            spoken,
            live_id: Some(run.to_owned()),
            at: String::new(),
        };
        let stored = blocking(app, move |app| Ok(lock(&app.store).add_practice(&p)?)).await?;
        turn(stored)
    })
    .await
}

/// The step owed after the last answer: for fewer than [`MAX_FOLLOW_UPS`] follow-ups on its question, one call for a
/// follow-up; with none (or after the last follow-up's reply, with no call), the next prep question; past the last,
/// the run ends. One at a time per run: a caller arriving while one runs gets the run it leaves. None owed (taken
/// back, skipped or ended meanwhile), nothing changes.
async fn step(app: &Arc<App>, run: &str) -> Result<LiveSession> {
    let key = format!("live step {run}");
    single_flight(app, &key, async |joined| {
        if !joined {
            advance(app, run).await?;
        }
        let id = run.to_owned();
        blocking(app, move |app| session(app, &id)).await
    })
    .await
}

async fn advance(app: &Arc<App>, run: &str) -> Result<()> {
    let id = run.to_owned();
    let (row, turns, prep) = blocking(app, move |app| {
        let store = lock(&app.store);
        let (row, turns) = run_of(&store, &id)?;
        let prep = prep_of(&store, &row.round_id)?;
        Ok((row, turns, prep))
    })
    .await?;
    let Some(last) = turns.last().filter(|_| owed(&row, &turns)) else {
        return Ok(());
    };
    let seq = last.seq;
    let follows = turns
        .iter()
        .filter(|t| t.question == last.question && t.kind == LiveKind::Follow)
        .count();
    let follow = if follows < MAX_FOLLOW_UPS {
        follow_up(app, &row, &turns, &prep).await?
    } else {
        None
    };
    let id = run.to_owned();
    blocking(app, move |app| {
        let store = lock(&app.store);
        // Taken back, skipped or ended while the call ran: the step isn't owed any more.
        let (row, turns) = run_of(&store, &id)?;
        if !owed(&row, &turns) || turns.last().map(|t| t.seq) != Some(seq) {
            return Ok(());
        }
        match follow {
            Some(f) => {
                store.add_live_turn(&id, &f)?;
            }
            None => next(&store, &id, &prep)?,
        }
        Ok(())
    })
    .await
}

/// One model call: a follow-up on your last answer to its question (read with the exchange on it so far), or none.
async fn follow_up(
    app: &Arc<App>,
    row: &LiveRunRow,
    turns: &[LiveTurnRow],
    prep: &written::Prep,
) -> Result<Option<LiveTurnRow>> {
    let Some(last) = turns.last() else {
        return Ok(None);
    };
    let question = last.question.clone();
    let asked = turns
        .iter()
        .find(|t| t.kind == LiveKind::Ask && t.question == question)
        .unwrap_or(last);
    let answer = asked.answer.clone().unwrap_or_default();
    let follow_ups = follow_ups(turns, &question);
    let reply = last.answer.clone().unwrap_or_default();
    let q = question_of(prep, &question, &asked.asked);
    let round = row.round_id.clone();
    let cx = blocking(app, move |app| context(app, &round)).await?;
    let told = q
        .story
        .as_deref()
        .and_then(|id| cx.stories.iter().find(|s| s.id == id))
        .cloned();
    let terms = cx.terms();
    let route = app.route(Feature::Live);
    let llm = app.llm(&route, Feature::Live).await?;
    let model = route.model;
    let (m, s) = (model.clone(), cx.sent.clone());
    let (reply_f, out) = model_call(app, &cx.job, Stage::Interviewing, async move {
        let input = LiveInput {
            question: &q.text,
            why: &q.why,
            lines: &q.lines,
            answer: &answer,
            follow_ups: &follow_ups,
            terms: &terms,
            sent: &s,
            story: told.as_ref(),
        };
        live::read(&llm, &live::schema(), &input, &m).await
    })
    .await?;
    app.note_served(&model, &out);
    let f = live::checked(reply_f, &reply);
    Ok(f.follow_up.map(|text| LiveTurnRow {
        seq: 0,
        question,
        kind: LiveKind::Follow,
        asked: text,
        picked: f.picked,
        answer: None,
        skipped: false,
        spoken: None,
        at: String::new(),
    }))
}

/// Asks the prep question after the last one asked, or ends the run past the last (or past the count it started
/// with).
fn next(store: &Store, run: &str, prep: &written::Prep) -> Result<()> {
    let (row, turns) = run_of(store, run)?;
    let asked = turns.iter().filter(|t| t.kind == LiveKind::Ask).count();
    match prep
        .questions
        .get(asked)
        .filter(|_| asked < row.questions as usize)
    {
        Some(q) => {
            store.add_live_turn(run, &ask(q))?;
        }
        None => store.end_live_run(run, None)?,
    }
    Ok(())
}

/// A turn asking prep question `q` verbatim.
fn ask(q: &written::Question) -> LiveTurnRow {
    LiveTurnRow {
        seq: 0,
        question: q.id.clone(),
        kind: LiveKind::Ask,
        asked: q.text.clone(),
        picked: None,
        answer: None,
        skipped: false,
        spoken: None,
        at: String::new(),
    }
}

/// The follow-ups answered on `question`, each with your reply, in order.
fn follow_ups(turns: &[LiveTurnRow], question: &str) -> Vec<(String, String)> {
    turns
        .iter()
        .filter(|t| t.kind == LiveKind::Follow && t.question == question)
        .filter_map(|t| t.answer.clone().map(|a| (t.asked.clone(), a)))
        .collect()
}

/// Prep question `id`; one no longer in the prep (written again since) as it was asked, about no lines.
fn question_of(prep: &written::Prep, id: &str, asked: &str) -> written::Question {
    prep.questions
        .iter()
        .find(|q| q.id == id)
        .cloned()
        .unwrap_or_else(|| written::Question {
            id: id.to_owned(),
            group: written::Group::Resume,
            text: asked.to_owned(),
            why: String::new(),
            lines: Vec::new(),
            story: None,
        })
}

fn prep_of(store: &Store, round: &str) -> Result<written::Prep> {
    let prep = store
        .latest_prep(round)?
        .ok_or_else(|| AppError::missing("Write this round’s prep first"))?;
    Ok(serde_json::from_str(&prep.json)?)
}

fn run_of(store: &Store, run: &str) -> Result<(LiveRunRow, Vec<LiveTurnRow>)> {
    store
        .live_run(run)?
        .ok_or_else(|| AppError::missing("This interview was removed"))
}

/// The turn waiting for your answer: the last, unanswered and not skipped.
fn current(turns: &[LiveTurnRow]) -> Option<&LiveTurnRow> {
    turns.last().filter(|t| t.answer.is_none() && !t.skipped)
}

/// The step after the last answer is owed: not ended, and the last turn answered.
fn owed(row: &LiveRunRow, turns: &[LiveTurnRow]) -> bool {
    row.ended_at.is_none() && turns.last().is_some_and(|t| t.answer.is_some())
}

fn session_in(store: &Store, run: &str) -> Result<LiveSession> {
    let (row, turns) = run_of(store, run)?;
    let order = |q: &str| {
        turns
            .iter()
            .position(|t| t.kind == LiveKind::Ask && t.question == q)
            .unwrap_or(usize::MAX)
    };
    let mut read = store.live_practice(run)?;
    read.sort_by_key(|p| order(&p.question));
    let read = read.into_iter().map(turn).collect::<Result<Vec<_>>>()?;
    let waiting = owed(&row, &turns);
    Ok(LiveSession {
        id: row.id,
        round: row.round_id,
        started_at: row.started_at,
        ended_at: row.ended_at,
        active_ms: row.active_ms,
        questions: row.questions,
        turns: turns
            .into_iter()
            .map(|t| LiveTurn {
                seq: t.seq,
                question: t.question,
                kind: match t.kind {
                    LiveKind::Ask => LiveTurnKind::Ask,
                    LiveKind::Follow => LiveTurnKind::Follow,
                },
                asked: t.asked,
                picked: t.picked,
                answer: t.answer,
                skipped: t.skipped,
                spoken: t.spoken.map(|s| Spoken {
                    ms: s.ms,
                    words: s.words,
                }),
                at: t.at,
            })
            .collect(),
        waiting,
        read,
    })
}

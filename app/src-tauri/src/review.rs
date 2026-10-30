//! The open review: Suggest's changes for one posting × master × mode, your draft over them (an event log with an
//! undo cursor, kept in the store), and the fit check rebuilt after every step with the warm renderer.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Instant;

use chrono::{SecondsFormat, Utc};
use resume_core::bank::{Bank, BankEntry, FactKind, FactSource};
use resume_core::change::{Change, Draft, DraftEvent, Mode as CoreMode, Op, Origin, State};
use resume_core::changes::{changes_of, draft_changes};
use resume_core::facts::{FactError, add_fact};
use resume_core::matcher::{Page, find_literal};
use resume_core::resume::SectionKind;
use resume_core::suggestions::Suggestions;
use resume_core::validate::Grounding;
use resume_core::variants::add_variant;
use resume_core::{Master, Requirements, Resume, resolve};
use resume_llm::instruct::{self, EditKind, InstructInput};
use resume_llm::rewrite::{self, RewriteInput};
use resume_llm::suggest::{self, SuggestInput};
use resume_pipeline::{suggest_key, today};
use resume_store::{DraftLog, NewAnalysis, PostingRow};
use resume_tailor::{Tailored, tailor};

use crate::dto::{
    Accepted, ChipState, CustomEdit, Decision, Displaced, EditTargets, Instructed, KeyDecision,
    MasterChoice, Proposal, ReviewView, Rewritten, Skipped, Stage, Target,
};
use crate::entry::variant_error;
use crate::error::{AppError, Result};
use crate::jobs;
use crate::state::{App, blocking, lock, model_call, single_flight};
use crate::view;

pub struct Review {
    /// The analysis id.
    pub id: String,
    pub posting_id: String,
    pub posting: PostingRow,
    pub bank: Bank,
    pub master: Master,
    pub req: Requirements,
    /// The `claude --model` alias or name the suggestions were made with.
    pub model: String,
    pub suggestions: Suggestions,
    /// `changes_of` the suggestions: before your wording and your own edits.
    pub changes: Vec<Change>,
    pub log: DraftLog,
    /// The master resolved: the lines your own edits can target.
    pub base: Resume,
    /// Every master's score now, best first.
    pub masters: Vec<MasterChoice>,
    /// The last rebuild.
    pub last: Option<Last>,
}

/// What the last rebuild worked from and made.
pub struct Last {
    pub t: Tailored,
    /// `draft_changes` output: suggestions with your wording, then your own.
    pub changes: Vec<Change>,
    pub draft: Draft,
}

/// Opens the review of `job`: Suggest's output for the master, drawing on the bank as Settings says (an existing
/// analysis for the same input and model keeps its draft; `fresh` calls Suggest again, carrying your decisions over by
/// key), then the fit check. A call that a newer `start_review` overtook rejects as cancelled and leaves the newer
/// review open.
pub async fn start_review(
    app: &Arc<App>,
    job: &str,
    master: Option<String>,
    fresh: bool,
) -> Result<ReviewView> {
    let ticket = app.review_ticket();
    let (review, view) = make_review(app, job, master, fresh).await?;
    let mut slot = lock(&app.review);
    if !app.is_newest_review(ticket) {
        return Err(AppError::cancelled());
    }
    *slot = Some(review);
    Ok(view)
}

/// Makes and saves the review of `job` with its last master (Suggest's output, the fit score) without opening it: a
/// job you opened and left goes on, so its review is ready when you come back.
pub async fn prepare_review(app: &Arc<App>, job: &str) -> Result<()> {
    make_review(app, job, None, false).await.map(drop)
}

/// [`start_review`]'s review, made but not opened. Suggest runs once per input and model: a call asking for one
/// running already waits for it and reads its answer from the store.
async fn make_review(
    app: &Arc<App>,
    job: &str,
    master: Option<String>,
    fresh: bool,
) -> Result<(Review, ReviewView)> {
    let row = jobs::posting(app, job)?;
    let req = jobs::requirements(app, &row).await?;
    let settings = app.settings();
    let latest = lock(&app.store).latest_analysis(job)?;
    // Every review follows the setting, so entries you add to the bank reach the jobs you already have.
    let mode: CoreMode = settings.suggestions_from.into();
    let master_id = match master.or(latest.map(|a| a.master_id)) {
        Some(id) => id,
        None => {
            let req = req.clone();
            blocking(app, move |app| {
                Ok(jobs::ranked(app.masters_now()?.as_ref(), &req)
                    .first()
                    .map(|m| m.id.clone()))
            })
            .await?
            .ok_or_else(AppError::no_bank)?
        }
    };
    let ((bank_rev, bank), (master_rev, master)) = {
        let store = lock(&app.store);
        let bank = store.bank()?.ok_or_else(AppError::no_bank)?;
        let master = store
            .master(&master_id)?
            .ok_or_else(|| AppError::missing("This master was deleted"))?;
        (bank, master)
    };
    let resume = resolve(&bank, &master)?;
    let candidates = resume_pipeline::candidates(&bank, &master, &req, mode);
    let key = suggest_key(
        &SuggestInput {
            posting: &row.posting,
            requirements: &req,
            resume: &resume,
            bank: &bank,
            candidates: &candidates,
        },
        mode,
    );
    let model = settings.suggest_model;
    let analysis = single_flight(app, &format!("suggest {model} {key}"), async |joined| {
        // Joined: the call waited for made the answer, fresh or not.
        let cached = if fresh && !joined {
            None
        } else {
            lock(&app.store).analysis(&key, &model)?
        };
        if let Some(a) = cached {
            lock(&app.store).touch_analysis(&a.id)?;
            return Ok(a);
        }
        let llm = app.claude().await?;
        let (p, rq, rs, bk, cs, m) = (
            row.posting.clone(),
            req.clone(),
            resume.clone(),
            bank.clone(),
            candidates.clone(),
            model.clone(),
        );
        let (s, out) = model_call(app, job, Stage::Suggesting, async move {
            let input = SuggestInput {
                posting: &p,
                requirements: &rq,
                resume: &rs,
                bank: &bk,
                candidates: &cs,
            };
            suggest::suggest(&llm, &suggest::schema(), &input, &m).await
        })
        .await?;
        app.note_served(&model, &out);
        let store = lock(&app.store);
        // Your draft carries over from this input's last answer, else from the review of this master you last opened,
        // whichever mode it drew on: a changed setting or bank keeps your decisions and edits.
        let previous = match store.analysis(&key, &model)? {
            Some(a) => Some(a),
            None => store
                .analyses(job)?
                .into_iter()
                .find(|a| a.master_id == master.id),
        };
        let a = store.put_analysis(&NewAnalysis {
            posting_id: job,
            bank_rev: &bank_rev,
            master_id: &master.id,
            master_rev: &master_rev,
            mode,
            key: &key,
            model: &model,
            suggestions: &s,
            latency_ms: out.latency().as_millis() as u64,
        })?;
        if let Some(prev) = previous {
            let log = store.draft(&prev.id)?;
            for e in &log.events[..log.cursor.min(log.events.len())] {
                store.push_draft_event(&a.id, e)?;
            }
        }
        Ok(a)
    })
    .await?;
    blocking(app, move |app| {
        let masters = jobs::ranked(app.masters_now()?.as_ref(), &req);
        let log = lock(&app.store).draft(&analysis.id)?;
        let changes = changes_of(&analysis.suggestions, &bank, &master, &req, mode);
        let mut review = Review {
            id: analysis.id,
            posting_id: row.id.clone(),
            posting: row,
            bank,
            master,
            req,
            model: analysis.model,
            suggestions: analysis.suggestions,
            changes,
            log,
            base: resume,
            masters,
            last: None,
        };
        // Built aside: a failed rebuild leaves the open review as it was.
        let view = rebuild(app, &mut review)?;
        Ok((review, view))
    })
    .await
}

/// The fit check over the draft as it stands: the tailored resume, its scores, every change's state, and the
/// highlighted pages. Saves the projected score for the posting's fit score.
pub fn rebuild(app: &App, r: &mut Review) -> Result<ReviewView> {
    let start = Instant::now();
    let draft = r.log.draft();
    let changes = draft_changes(&r.changes, &draft);
    let (t, pages) = app.with_renderer(&r.master, |renderer| {
        let t = tailor(
            &r.bank,
            &r.master,
            &r.req,
            &changes,
            &draft,
            renderer,
            today(),
        )?;
        let preview = renderer.render(&serde_json::to_string(&t.resume)?, true)?;
        Ok((t, preview.svgs()))
    })?;
    let tailor_ms = start.elapsed().as_millis() as u32;
    let status = {
        let store = lock(&app.store);
        store.set_projected(&r.id, t.projected.total)?;
        jobs::status_of(&store, &r.posting_id)?
    };
    let view = view::review(r, &changes, &draft, &t, pages, tailor_ms, status);
    r.last = Some(Last { t, changes, draft });
    Ok(view)
}

/// Runs `f` on the open review `id` (on the blocking pool), then rebuilds it.
async fn edit(
    app: &Arc<App>,
    id: &str,
    f: impl FnOnce(&App, &mut Review) -> Result<()> + Send + 'static,
) -> Result<ReviewView> {
    let id = id.to_owned();
    blocking(app, move |app| {
        let mut slot = lock(&app.review);
        let r = open(&mut slot, &id)?;
        f(app, r)?;
        rebuild(app, r)
    })
    .await
}

fn open<'a>(slot: &'a mut Option<Review>, id: &str) -> Result<&'a mut Review> {
    slot.as_mut()
        .filter(|r| r.id == id)
        .ok_or_else(AppError::replaced)
}

/// Appends `e` to the review's draft log (dropping what undo left after the cursor).
fn push(app: &App, r: &mut Review, e: DraftEvent) -> Result<()> {
    r.log = lock(&app.store).push_draft_event(&r.id, &e)?;
    Ok(())
}

/// A copy of `r` without its last rebuild, to work on outside the slot.
fn detached(r: &Review) -> Review {
    Review {
        id: r.id.clone(),
        posting_id: r.posting_id.clone(),
        posting: r.posting.clone(),
        bank: r.bank.clone(),
        master: r.master.clone(),
        req: r.req.clone(),
        model: r.model.clone(),
        suggestions: r.suggestions.clone(),
        changes: r.changes.clone(),
        log: r.log.clone(),
        base: r.base.clone(),
        masters: r.masters.clone(),
        last: None,
    }
}

/// `events` as one undo step: nothing for none, the event itself for one, else a `Batch`.
fn push_step(app: &App, r: &mut Review, mut events: Vec<DraftEvent>) -> Result<()> {
    match events.len() {
        0 => Ok(()),
        1 => push(app, r, events.remove(0)),
        _ => push(app, r, DraftEvent::Batch { events }),
    }
}

/// The change `key` as the last rebuild had it (with your wording; your own included).
fn change<'a>(r: &'a Review, key: &str) -> Result<&'a Change> {
    r.last
        .as_ref()
        .and_then(|l| l.changes.iter().find(|c| c.key == key))
        .ok_or_else(|| AppError::missing("This change is gone; reopen the review"))
}

/// Saves change `key`'s line as a variant of its bullet in the bank: every master and later review can show it, and
/// Suggest prefers it over new words. This review goes on as it is, its copy of the bank holding the variant too.
pub async fn save_variant(app: &Arc<App>, review: &str, key: String) -> Result<ReviewView> {
    edit(app, review, move |app, r| {
        let last = r.last.as_ref().ok_or_else(AppError::replaced)?;
        let c = change(r, &key)?;
        let bullet = view::savable(r, c, &last.t)
            .ok_or_else(|| {
                AppError::other("Only a bullet's new wording can be saved as a variant")
            })?
            .to_owned();
        let text = view::line_text(c, &last.t);
        {
            let store = lock(&app.store);
            let (_, mut bank) = store.bank()?.ok_or_else(AppError::no_bank)?;
            add_variant(&mut bank, &bullet, &text).map_err(variant_error)?;
            store.put_bank(&bank)?;
        }
        add_variant(&mut r.bank, &bullet, &text).map_err(variant_error)?;
        Ok(())
    })
    .await
}

/// Keeps "I have `skill` experience, at `entry`" (your answer to the chat's "Do you have … experience?") as a fact of
/// that bank entry: every later review and call backs `skill` there without asking again. This review's copy of the
/// bank holds it too, so the ask that goes again next can use it.
pub async fn keep_fact(
    app: &Arc<App>,
    review: &str,
    entry: String,
    skill: String,
) -> Result<ReviewView> {
    edit(app, review, move |app, r| {
        let skill = skill.trim().to_owned();
        if skill.is_empty() {
            return Err(AppError::other("Say which skill you have"));
        }
        let text = format!("Used {skill}");
        let source = FactSource {
            kind: FactKind::Review,
            at: Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
            posting: Some(r.posting_id.clone()),
            company: Some(crate::cover::company(r).trim())
                .filter(|c| !c.is_empty())
                .map(str::to_owned),
        };
        let skills = [skill];
        let fact_error = |e: FactError| AppError::other(e.to_string());
        {
            let store = lock(&app.store);
            let (_, mut bank) = store.bank()?.ok_or_else(AppError::no_bank)?;
            add_fact(&mut bank, &entry, &text, &skills, source.clone()).map_err(fact_error)?;
            store.put_bank(&bank)?;
        }
        add_fact(&mut r.bank, &entry, &text, &skills, source).map_err(fact_error)?;
        Ok(())
    })
    .await
}

pub async fn decide(
    app: &Arc<App>,
    review: &str,
    key: String,
    decision: Option<Decision>,
) -> Result<ReviewView> {
    edit(app, review, move |app, r| {
        change(r, &key)?;
        push(
            app,
            r,
            DraftEvent::Decide {
                key,
                decision: decision.map(Into::into),
            },
        )
    })
    .await
}

/// Your wording for change `key`; `None` restores the suggestion's. Your own edits are reworded in place.
pub async fn set_wording(
    app: &Arc<App>,
    review: &str,
    key: String,
    text: Option<String>,
) -> Result<ReviewView> {
    edit(app, review, move |app, r| {
        let text = text.map(|t| t.trim().to_owned()).filter(|t| !t.is_empty());
        let c = change(r, &key)?.clone();
        if r.log.draft().custom.iter().any(|x| x.key == key) {
            let Some(text) = text else {
                return Ok(());
            };
            let change = reworded(r, &c, &text)?;
            return push(app, r, DraftEvent::AddCustom { change });
        }
        push(app, r, DraftEvent::Word { key, text })
    })
    .await
}

pub async fn add_edit(app: &Arc<App>, review: &str, e: CustomEdit) -> Result<ReviewView> {
    edit(app, review, move |app, r| {
        let change = custom(r, e, &[])?;
        push(app, r, DraftEvent::AddCustom { change })
    })
    .await
}

pub async fn remove_edit(app: &Arc<App>, review: &str, key: String) -> Result<ReviewView> {
    edit(app, review, move |app, r| {
        if !r.log.draft().custom.iter().any(|c| c.key == key) {
            return Err(AppError::missing("Only your own edits can be removed"));
        }
        push(app, r, DraftEvent::RemoveCustom { key })
    })
    .await
}

pub async fn undo(app: &Arc<App>, review: &str) -> Result<ReviewView> {
    edit(app, review, |app, r| {
        if r.log.cursor > 0 {
            r.log = lock(&app.store).set_draft_cursor(&r.id, r.log.cursor - 1)?;
        }
        Ok(())
    })
    .await
}

pub async fn redo(app: &Arc<App>, review: &str) -> Result<ReviewView> {
    edit(app, review, |app, r| {
        if r.log.cursor < r.log.events.len() {
            r.log = lock(&app.store).set_draft_cursor(&r.id, r.log.cursor + 1)?;
        }
        Ok(())
    })
    .await
}

/// A requirement skill clicked: off the page when it's on (the change adding it off, or a removal of yours), on the
/// page when it isn't (the suggestion adding it on, else an addition of yours to the skills line that fits best).
pub async fn toggle_skill(app: &Arc<App>, review: &str, term: String) -> Result<ReviewView> {
    edit(app, review, move |app, r| {
        let last = r.last.as_ref().ok_or_else(AppError::replaced)?;
        let facts = view::skills(r, last);
        let f = facts.iter().find(|f| f.m.term == term).ok_or_else(|| {
            AppError::missing(format!("“{term}” isn't a skill this posting asks for"))
        })?;
        if let Some(reason) = &f.locked {
            return Err(AppError::other(reason.clone()));
        }
        let mut events = Vec::new();
        match f.state {
            ChipState::Add => {
                for c in &f.adders {
                    events.push(if is_custom(last, &c.key) {
                        DraftEvent::RemoveCustom { key: c.key.clone() }
                    } else {
                        DraftEvent::Decide {
                            key: c.key.clone(),
                            decision: Some(resume_core::change::Decision::Off),
                        }
                    });
                }
            }
            ChipState::Have => {
                if let Some((group, item)) = &f.item {
                    let section = group_section(&r.base, group).unwrap_or_default();
                    let mut c = Change::new(
                        Op::SkillRemove {
                            group: group.clone(),
                            item: item.clone(),
                        },
                        Origin::Yours,
                        item.clone(),
                        vec![],
                        "You said it isn't yours".to_owned(),
                        1,
                        section,
                    );
                    c.key = new_key();
                    events.push(DraftEvent::AddCustom { change: c });
                }
            }
            ChipState::Missing | ChipState::NotMine => {
                if let Some(c) = &f.removal {
                    events.push(DraftEvent::RemoveCustom { key: c.key.clone() });
                } else if !f.removers.is_empty() {
                    for c in &f.removers {
                        events.push(DraftEvent::Decide {
                            key: c.key.clone(),
                            decision: Some(resume_core::change::Decision::Off),
                        });
                    }
                } else if let Some(c) = &f.suggested {
                    events.push(DraftEvent::Decide {
                        key: c.key.clone(),
                        decision: Some(resume_core::change::Decision::On),
                    });
                } else {
                    let group = best_line(r, last, f.kind)
                        .ok_or_else(|| AppError::missing("This master has no skills line"))?;
                    let change = custom(
                        r,
                        CustomEdit::Skill {
                            group,
                            item: f.m.term.clone(),
                        },
                        &[],
                    )?;
                    events.push(DraftEvent::AddCustom { change });
                }
            }
        }
        // One click is one undo step.
        push_step(app, r, events)
    })
    .await
}

/// Where your own new edits can go: the master's entries and skills lines.
pub fn edit_targets(app: &App, review: &str) -> Result<EditTargets> {
    let mut slot = lock(&app.review);
    let r = open(&mut slot, review)?;
    let mut t = EditTargets {
        entries: vec![],
        groups: vec![],
    };
    for s in &r.base.sections {
        for e in &s.entries {
            t.entries.push(Target {
                id: e.id.clone(),
                label: view::entry_label(e),
            });
        }
        for g in &s.groups {
            t.groups.push(Target {
                id: g.id.clone(),
                label: g.label.clone(),
            });
        }
    }
    Ok(t)
}

/// What a rewrite needs, taken from the review before the model call.
struct RewriteJob {
    posting_id: String,
    line: String,
    facts: String,
    terms: Vec<String>,
    budget: usize,
}

/// One model call: change `key`'s line rewritten per `note`, checked by the grounding validator, and set as your
/// wording.
pub async fn rewrite(app: &Arc<App>, review: &str, key: String, note: String) -> Result<Rewritten> {
    let id = review.to_owned();
    let k = key.clone();
    let job = blocking(app, move |app| {
        let mut slot = lock(&app.review);
        let r = open(&mut slot, &id)?;
        rewrite_job(r, &k)
    })
    .await?;
    let text = rewritten(app, job, note).await?;
    let id = review.to_owned();
    blocking(app, move |app| {
        let mut slot = lock(&app.review);
        let r = open(&mut slot, &id)?;
        let c = change(r, &key)?.clone();
        let flags = grounding(r, &op_with_text(&c.op, &text), &c.before, &[]);
        if r.log.draft().custom.iter().any(|x| x.key == key) {
            let change = reworded(r, &c, &text)?;
            push(app, r, DraftEvent::AddCustom { change })?;
        } else {
            push(
                app,
                r,
                DraftEvent::Word {
                    key,
                    text: Some(text),
                },
            )?;
        }
        Ok(Rewritten {
            review: rebuild(app, r)?,
            flags,
        })
    })
    .await
}

/// One model call: a line no change touches (`line`, or the summary when `None`) rewritten per `note`, checked by
/// the grounding validator, and added as your edit.
pub async fn rewrite_line(
    app: &Arc<App>,
    review: &str,
    line: Option<String>,
    note: String,
) -> Result<Rewritten> {
    let id = review.to_owned();
    let l = line.clone();
    let job = blocking(app, move |app| {
        let mut slot = lock(&app.review);
        let r = open(&mut slot, &id)?;
        line_job(r, l.as_deref())
    })
    .await?;
    let text = rewritten(app, job, note).await?;
    let id = review.to_owned();
    blocking(app, move |app| {
        let mut slot = lock(&app.review);
        let r = open(&mut slot, &id)?;
        let edit = match line {
            Some(line) => CustomEdit::Line { line, text },
            None => CustomEdit::Summary { text },
        };
        let change = custom(r, edit, &[])?;
        let flags = change.flags.clone();
        push(app, r, DraftEvent::AddCustom { change })?;
        Ok(Rewritten {
            review: rebuild(app, r)?,
            flags,
        })
    })
    .await
}

/// The Rewrite model call for `job`: the new line, per `note`.
async fn rewritten(app: &Arc<App>, job: RewriteJob, note: String) -> Result<String> {
    let llm = app.claude().await?;
    let model = app.settings().suggest_model;
    let (line, facts, terms, budget) = (job.line, job.facts, job.terms, job.budget);
    let m = model.clone();
    let (text, out) = model_call(app, &job.posting_id, Stage::Rewriting, async move {
        let input = RewriteInput {
            line: &line,
            facts: &facts,
            terms: &terms,
            note: &note,
            budget,
        };
        rewrite::rewrite(&llm, &rewrite::schema(), &input, &m).await
    })
    .await?;
    app.note_served(&model, &out);
    Ok(text)
}

fn rewrite_job(r: &Review, key: &str) -> Result<RewriteJob> {
    let last = r.last.as_ref().ok_or_else(AppError::replaced)?;
    let c = change(r, key)?;
    if !view::rewritable(&c.op) {
        return Err(AppError::other(
            "Only bullets and the summary can be rewritten",
        ));
    }
    let line = view::line_text(c, &last.t);
    let terms = r.req.skills.iter().map(|s| s.term.clone()).collect();
    let (facts, budget) = match &c.op {
        Op::Summary { .. } => (suggest::tagged_text(&r.base), rewrite::SUMMARY_CHARS),
        Op::Add { entry, .. } => (
            entry_of(&r.bank, entry)
                .map(rewrite::entry_facts)
                .unwrap_or_default(),
            rewrite::budget(&line),
        ),
        op => {
            let target = view::target_line(op).unwrap_or_default();
            (
                entry_holding(&r.bank, target)
                    .map(rewrite::entry_facts)
                    .unwrap_or_default(),
                rewrite::budget(&c.before),
            )
        }
    };
    Ok(RewriteJob {
        posting_id: r.posting_id.clone(),
        line,
        facts,
        terms,
        budget,
    })
}

/// What a rewrite of a line no change touches needs: the master's line (the summary when `line` is `None`) and the
/// facts it may draw on.
fn line_job(r: &Review, line: Option<&str>) -> Result<RewriteJob> {
    let (text, facts, budget) = match line {
        None => {
            let (text, _) = summary_of(&r.base);
            if text.trim().is_empty() {
                return Err(AppError::missing("This master has no summary to rewrite"));
            }
            (text, suggest::tagged_text(&r.base), rewrite::SUMMARY_CHARS)
        }
        Some(line) => {
            if line.contains('#') {
                return Err(AppError::other(
                    "Only bullets and the summary can be rewritten",
                ));
            }
            let (text, _) = line_of(&r.base, line)
                .ok_or_else(|| AppError::missing("This line isn't on the master"))?;
            let facts = entry_holding(&r.bank, line)
                .map(rewrite::entry_facts)
                .unwrap_or_default();
            let budget = rewrite::budget(&text);
            (text, facts, budget)
        }
    };
    Ok(RewriteJob {
        posting_id: r.posting_id.clone(),
        line: text,
        facts,
        terms: r.req.skills.iter().map(|s| s.term.clone()).collect(),
        budget,
    })
}

/// What an instruction needs, taken from the review before the model call.
struct InstructJob {
    posting_id: String,
    /// The tailored resume as it reads now.
    resume: Resume,
    terms: Vec<(String, bool)>,
    /// The review as it was (its bank is the call's): the edits land on it should another be open by the time the
    /// model answers.
    aside: Review,
}

/// One model call: a free-form instruction turned into a few edits within `scope` (ids of entries, bullets, skills
/// lines or `summary`; empty for anywhere). `confirmed` names skills you say you have though the resume doesn't show
/// them: the model may use them. What you confirmed and what the instruction says about your experience are your word,
/// so this call's edits aren't flagged for the names and numbers they hold. Each lands as your own edit turned off,
/// pending until you accept it, all in one undo step; an edit that can't be placed joins what the model skipped. The
/// edits land on the review's draft also when you left the job meanwhile (another review open, or none).
pub async fn instruct(
    app: &Arc<App>,
    review: &str,
    instruction: String,
    scope: Vec<String>,
    confirmed: Vec<String>,
) -> Result<Instructed> {
    let instruction = instruction.trim().to_owned();
    if instruction.is_empty() {
        return Err(AppError::other("Write an instruction first"));
    }
    let confirmed = normalized(confirmed);
    // Your word for this call's grounding: the skills you confirmed and the instruction itself.
    let vouched: Vec<String> = confirmed.iter().cloned().chain([instruction.clone()]).collect();
    let id = review.to_owned();
    let InstructJob {
        posting_id,
        resume,
        terms,
        aside,
    } = blocking(app, move |app| {
        let mut slot = lock(&app.review);
        let r = open(&mut slot, &id)?;
        let last = r.last.as_ref().ok_or_else(AppError::replaced)?;
        Ok(InstructJob {
            posting_id: r.posting_id.clone(),
            terms: instruct::terms(&r.req, &last.t.resume),
            resume: last.t.resume.clone(),
            aside: detached(r),
        })
    })
    .await?;
    let llm = app.claude().await?;
    let model = app.settings().suggest_model;
    let s = scope.clone();
    let c = confirmed.clone();
    let m = model.clone();
    let ((reply, out), mut aside) = model_call(app, &posting_id, Stage::Instructing, async move {
        let input = InstructInput {
            resume: &resume,
            bank: &aside.bank,
            terms: &terms,
            scope: &s,
            confirmed: &c,
            instruction: &instruction,
        };
        let answer = instruct::instruct(&llm, &instruct::schema(), &input, &m).await?;
        Ok((answer, aside))
    })
    .await?;
    app.note_served(&model, &out);
    let model = out.served(&model).map_or(model, str::to_owned);
    blocking(app, move |app| {
        let mut slot = lock(&app.review);
        let r = match slot.as_mut().filter(|r| r.id == aside.id) {
            Some(r) => r,
            // You left the job: its review, as the store has its draft now, takes the edits.
            None => {
                aside.log = lock(&app.store).draft(&aside.id)?;
                rebuild(app, &mut aside)?;
                &mut aside
            }
        };
        let last = r.last.as_ref().ok_or_else(AppError::replaced)?;
        let mut placed = place(&r.base, &last.t.resume, &scope, reply.edits, |e| {
            custom(r, e, &vouched)
        });
        if let Some(step) = placed.step {
            push(app, r, step)?;
        }
        let mut skipped: Vec<Skipped> = reply
            .skipped
            .into_iter()
            .map(|s| {
                let term = s.term.trim();
                Skipped {
                    what: s.what.trim().to_owned(),
                    why: s.why.trim().to_owned(),
                    term: (!term.is_empty()).then(|| term.to_owned()),
                }
            })
            .collect();
        skipped.append(&mut placed.skipped);
        Ok(Instructed {
            review: rebuild(app, r)?,
            reply: reply.reply,
            model,
            proposals: placed.proposals,
            skipped,
        })
    })
    .await
}

/// `names` trimmed, without empty entries or case-insensitive repeats, in their order.
fn normalized(names: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::with_capacity(names.len());
    for n in names {
        let n = n.trim();
        if !n.is_empty() && !out.iter().any(|o| o.to_lowercase() == n.to_lowercase()) {
            out.push(n.to_owned());
        }
    }
    out
}

/// The model's edits placed on the review: the step that lands them pending (one `Batch` of `AddCustom` and
/// `Decide Off` per edit; `None` when none could be placed), what each proposes, and the edits that couldn't be.
struct Placed {
    step: Option<DraftEvent>,
    proposals: Vec<Proposal>,
    skipped: Vec<Skipped>,
}

/// Places `edits` within `scope` (empty: anywhere) on the master `base`, as `now` (the tailored resume) reads:
/// each becomes a [`CustomEdit`], made a change by `make`. An edit is skipped when its target isn't on the master,
/// lies outside the scope, is a note line, or already reads that way, or when an earlier edit took its line.
fn place(
    base: &Resume,
    now: &Resume,
    scope: &[String],
    edits: Vec<instruct::Edit>,
    mut make: impl FnMut(CustomEdit) -> Result<Change>,
) -> Placed {
    let anywhere = scope.is_empty();
    let scoped = |id: &str| anywhere || scope.iter().any(|s| s == id);
    let mut taken: Vec<String> = Vec::new();
    let mut events = Vec::new();
    let mut proposals = Vec::new();
    let mut skipped = Vec::new();
    for e in edits {
        let text = e.text.trim().to_owned();
        let id = e.id.trim().to_owned();
        let what = if text.is_empty() {
            id.clone()
        } else {
            text.clone()
        };
        let unplaced = || {
            format!(
                "Couldn't place an edit for {}",
                if id.is_empty() { "the summary" } else { &id }
            )
        };
        let placed: std::result::Result<(CustomEdit, Option<String>, String), String> = (|| {
            if text.is_empty() {
                return Err(unplaced());
            }
            match e.kind {
                EditKind::Line => {
                    if id.contains('#') {
                        return Err(format!("{id} is a note line; only bullets get new wording"));
                    }
                    let (before, _) = line_of(base, &id).ok_or_else(unplaced)?;
                    let entry = base
                        .sections
                        .iter()
                        .flat_map(|s| &s.entries)
                        .find(|en| en.bullets.iter().any(|b| b.id == id))
                        .map(|en| en.id.as_str())
                        .unwrap_or_default();
                    if !(scoped(&id) || (!entry.is_empty() && scoped(entry))) {
                        return Err(format!("{id} is outside what you picked"));
                    }
                    let was = bullet_text(now, &id).unwrap_or(before);
                    if was.trim() == text {
                        return Err(format!("{id} already reads that way"));
                    }
                    Ok((
                        CustomEdit::Line {
                            line: id.clone(),
                            text: text.clone(),
                        },
                        Some(was),
                        id.clone(),
                    ))
                }
                EditKind::Summary => {
                    let (_, section) = summary_of(base);
                    if !(scoped("summary") || scoped(&section)) {
                        return Err("The summary is outside what you picked".to_owned());
                    }
                    let (was, _) = summary_of(now);
                    if was.trim() == text {
                        return Err("The summary already reads that way".to_owned());
                    }
                    Ok((
                        CustomEdit::Summary { text: text.clone() },
                        (!was.trim().is_empty()).then_some(was),
                        "summary".to_owned(),
                    ))
                }
                EditKind::Bullet => {
                    entry_section(base, &id).ok_or_else(unplaced)?;
                    if !scoped(&id) {
                        return Err(format!("{id} is outside what you picked"));
                    }
                    let has = now
                        .sections
                        .iter()
                        .flat_map(|s| &s.entries)
                        .filter(|en| en.id == id)
                        .flat_map(|en| &en.bullets)
                        .any(|b| b.text.trim() == text);
                    if has {
                        return Err(format!("{id} already has that bullet"));
                    }
                    Ok((
                        CustomEdit::Bullet {
                            entry: id.clone(),
                            text: text.clone(),
                        },
                        None,
                        format!("{id}+{}", text.to_lowercase()),
                    ))
                }
                EditKind::Skill => {
                    group_section(base, &id).ok_or_else(unplaced)?;
                    if !scoped(&id) {
                        return Err(format!("{id} is outside what you picked"));
                    }
                    let has = now
                        .sections
                        .iter()
                        .flat_map(|s| &s.groups)
                        .filter(|g| g.id == id)
                        .flat_map(|g| &g.items)
                        .any(|i| i.trim().eq_ignore_ascii_case(&text));
                    if has {
                        return Err(format!("{id} already lists {text}"));
                    }
                    Ok((
                        CustomEdit::Skill {
                            group: id.clone(),
                            item: text.clone(),
                        },
                        None,
                        format!("{id}+{}", text.to_lowercase()),
                    ))
                }
            }
        })(
        );
        let (edit, was, target) = match placed {
            Ok(p) => p,
            Err(why) => {
                skipped.push(Skipped {
                    what,
                    why,
                    term: None,
                });
                continue;
            }
        };
        if taken.contains(&target) {
            skipped.push(Skipped {
                what,
                why: "Another edit already took this line".to_owned(),
                term: None,
            });
            continue;
        }
        let change = match make(edit.clone()) {
            Ok(c) => c,
            Err(err) => {
                skipped.push(Skipped {
                    what,
                    why: err.message,
                    term: None,
                });
                continue;
            }
        };
        taken.push(target);
        proposals.push(Proposal {
            key: change.key.clone(),
            edit,
            why: e.why.trim().to_owned(),
            was,
            flags: change.flags.clone(),
        });
        let key = change.key.clone();
        events.push(DraftEvent::AddCustom { change });
        events.push(DraftEvent::Decide {
            key,
            decision: Some(resume_core::change::Decision::Off),
        });
    }
    Placed {
        step: (!events.is_empty()).then_some(DraftEvent::Batch { events }),
        proposals,
        skipped,
    }
}

/// A bullet's text on `resume`.
fn bullet_text(resume: &Resume, id: &str) -> Option<String> {
    resume
        .sections
        .iter()
        .flat_map(|s| &s.entries)
        .flat_map(|e| &e.bullets)
        .find(|b| b.id == id)
        .map(|b| b.text.clone())
}

/// Changes `keys` on, in one undo step; every other change on the page that holds the same line (the same bullet or
/// note line, or the summary) goes off to make room.
pub async fn accept(app: &Arc<App>, review: &str, keys: Vec<String>) -> Result<Accepted> {
    let id = review.to_owned();
    blocking(app, move |app| {
        let mut slot = lock(&app.review);
        let r = open(&mut slot, &id)?;
        let last = r.last.as_ref().ok_or_else(AppError::replaced)?;
        let displaced = displaced(&last.changes, &last.t.states, &keys)?;
        let on = keys.into_iter().map(|key| DraftEvent::Decide {
            key,
            decision: Some(resume_core::change::Decision::On),
        });
        let off = displaced.iter().map(|d| DraftEvent::Decide {
            key: d.key.clone(),
            decision: Some(resume_core::change::Decision::Off),
        });
        push_step(app, r, on.chain(off).collect())?;
        Ok(Accepted {
            review: rebuild(app, r)?,
            displaced,
        })
    })
    .await
}

/// The changes on the page (in or forced) other than `keys` that hold a line one of `keys` targets, each with the
/// key that takes it. An unknown key fails.
fn displaced(
    changes: &[Change],
    states: &BTreeMap<String, State>,
    keys: &[String],
) -> Result<Vec<Displaced>> {
    let mut out: Vec<Displaced> = Vec::new();
    for key in keys {
        let a = changes
            .iter()
            .find(|c| &c.key == key)
            .ok_or_else(|| AppError::missing("This change is gone; reopen the review"))?;
        for c in changes {
            let on = matches!(states.get(&c.key), Some(State::In | State::Forced { .. }));
            if on
                && !keys.contains(&c.key)
                && same_line(&a.op, &c.op)
                && !out.iter().any(|d| d.key == c.key)
            {
                out.push(Displaced {
                    key: c.key.clone(),
                    by: key.clone(),
                });
            }
        }
    }
    Ok(out)
}

/// Both ops hold one line: the same bullet or note line, or both the summary.
fn same_line(a: &Op, b: &Op) -> bool {
    match (view::target_line(a), view::target_line(b)) {
        (Some(x), Some(y)) => x == y,
        _ => matches!((a, b), (Op::Summary { .. }, Op::Summary { .. })),
    }
}

/// Changes `keys` turned down, in one undo step: your own edits removed, suggestions turned off.
pub async fn reject(app: &Arc<App>, review: &str, keys: Vec<String>) -> Result<ReviewView> {
    edit(app, review, move |app, r| {
        let last = r.last.as_ref().ok_or_else(AppError::replaced)?;
        let mut events = Vec::with_capacity(keys.len());
        for key in keys {
            change(r, &key)?;
            events.push(if is_custom(last, &key) {
                DraftEvent::RemoveCustom { key }
            } else {
                DraftEvent::Decide {
                    key,
                    decision: Some(resume_core::change::Decision::Off),
                }
            });
        }
        push_step(app, r, events)
    })
    .await
}

/// A rewind of the chat, in one undo step: `keys` (the proposals of the asks taken back) off the page (your own edits
/// removed, suggestions turned off), and `restore` (changes accepting them turned off) back to their default.
pub async fn withdraw(
    app: &Arc<App>,
    review: &str,
    keys: Vec<String>,
    restore: Vec<String>,
) -> Result<ReviewView> {
    edit(app, review, move |app, r| {
        let last = r.last.as_ref().ok_or_else(AppError::replaced)?;
        let mut events = Vec::with_capacity(keys.len() + restore.len());
        for key in restore.into_iter().filter(|k| !keys.contains(k)) {
            change(r, &key)?;
            events.push(DraftEvent::Decide {
                key,
                decision: None,
            });
        }
        for key in keys {
            change(r, &key)?;
            events.push(if is_custom(last, &key) {
                DraftEvent::RemoveCustom { key }
            } else {
                DraftEvent::Decide {
                    key,
                    decision: Some(resume_core::change::Decision::Off),
                }
            });
        }
        push_step(app, r, events)
    })
    .await
}

/// Several decisions as one undo step.
pub async fn settle(
    app: &Arc<App>,
    review: &str,
    decisions: Vec<KeyDecision>,
) -> Result<ReviewView> {
    edit(app, review, move |app, r| {
        let mut events = Vec::with_capacity(decisions.len());
        for d in decisions {
            change(r, &d.key)?;
            events.push(DraftEvent::Decide {
                key: d.key,
                decision: d.decision.map(Into::into),
            });
        }
        push_step(app, r, events)
    })
    .await
}

/// Your own edit as a change: its target checked, the requirement terms it adds as hits, and the grounding
/// validator's findings (yours stay in; the findings become its warning), with what you `vouched` for in a chat ask
/// counted as sourced.
fn custom(r: &Review, e: CustomEdit, vouched: &[String]) -> Result<Change> {
    let need = |t: &str| {
        let t = t.trim();
        if t.is_empty() {
            Err(AppError::other("Write the text first"))
        } else {
            Ok(t.to_owned())
        }
    };
    let (op, before, section) = match e {
        CustomEdit::Bullet { entry, text } => {
            let section = entry_section(&r.base, &entry)
                .ok_or_else(|| AppError::missing("This entry isn't on the master"))?;
            (
                Op::Add {
                    entry,
                    text: need(&text)?,
                },
                String::new(),
                section,
            )
        }
        CustomEdit::Skill { group, item } => {
            let section = group_section(&r.base, &group)
                .ok_or_else(|| AppError::missing("This skills line isn't on the master"))?;
            (
                Op::SkillAdd {
                    group,
                    item: need(&item)?,
                },
                String::new(),
                section,
            )
        }
        CustomEdit::Line { line, text } => {
            let (before, section) = line_of(&r.base, &line)
                .ok_or_else(|| AppError::missing("This line isn't on the master"))?;
            (
                Op::Set {
                    line,
                    text: need(&text)?,
                },
                before,
                section,
            )
        }
        CustomEdit::Summary { text } => {
            let (before, section) = summary_of(&r.base);
            (Op::Summary { text: need(&text)? }, before, section)
        }
    };
    Ok(yours(r, op, before, section, new_key(), vouched))
}

/// Your own edit `c` with new text, keeping its key.
fn reworded(r: &Review, c: &Change, text: &str) -> Result<Change> {
    match &c.op {
        Op::Set { .. } | Op::Add { .. } | Op::Summary { .. } | Op::SkillAdd { .. } => Ok(yours(
            r,
            op_with_text(&c.op, text),
            c.before.clone(),
            c.section.clone(),
            c.key.clone(),
            &[],
        )),
        _ => Err(AppError::other("This edit has no wording to change")),
    }
}

fn yours(
    r: &Review,
    op: Op,
    before: String,
    section: String,
    key: String,
    vouched: &[String],
) -> Change {
    let text = match &op {
        Op::Set { text, .. } | Op::Add { text, .. } | Op::Summary { text } => text.as_str(),
        Op::SkillAdd { item, .. } => item.as_str(),
        _ => "",
    };
    let hits = new_terms(&r.req, text, &before);
    let flags = grounding(r, &op, &before, vouched);
    let mut c = Change::new(
        op,
        Origin::Yours,
        before,
        hits,
        "Your edit".to_owned(),
        1,
        section,
    );
    c.key = key;
    c.flags = flags;
    c
}

/// `op` with its text replaced (the line a line edit makes, a new bullet's, a skill's, the summary's).
fn op_with_text(op: &Op, text: &str) -> Op {
    let text = text.to_owned();
    match op {
        Op::Rewrite { bullet, .. } | Op::UseVariant { bullet, .. } => Op::Set {
            line: bullet.clone(),
            text,
        },
        Op::Swap { line, .. } | Op::Set { line, .. } => Op::Set {
            line: line.clone(),
            text,
        },
        Op::NoteAdd { note, .. } => Op::Set {
            line: note.clone(),
            text,
        },
        Op::Add { entry, .. } => Op::Add {
            entry: entry.clone(),
            text,
        },
        Op::SkillAdd { group, .. } => Op::SkillAdd {
            group: group.clone(),
            item: text,
        },
        Op::Summary { .. } => Op::Summary { text },
        other => other.clone(),
    }
}

/// The grounding validator on a line you wrote or had rewritten: numbers against its entry's lines (the whole bank
/// for a summary), names against the bank and the skills your evidence backs. `vouched` is your word in a chat ask
/// (skills you confirmed, the instruction's text): its names count as known and its numbers as sourced.
fn grounding(r: &Review, op: &Op, before: &str, vouched: &[String]) -> Vec<String> {
    let g = Grounding::new(&r.bank, &r.suggestions, &r.req).confirmed(vouched);
    let check = |text: &str, lines: &[String], extra: &[&str]| {
        let sources: Vec<&str> = lines
            .iter()
            .chain(vouched)
            .map(String::as_str)
            .chain(extra.iter().copied())
            .collect();
        g.check(text, &sources)
    };
    match op {
        Op::SkillAdd { item, .. } => g.check_item(item),
        Op::Summary { text } => check(text, &[], &[g.bank_text()]),
        Op::Add { entry, text } => check(
            text,
            &entry_of(&r.bank, entry).map(entry_lines).unwrap_or_default(),
            &[],
        ),
        Op::Set { line, text } => check(
            text,
            &entry_holding(&r.bank, line).map(entry_lines).unwrap_or_default(),
            &[before],
        ),
        _ => vec![],
    }
}

/// Requirement terms `text` states that `before` didn't.
fn new_terms(req: &Requirements, text: &str, before: &str) -> Vec<String> {
    let (page, was) = (Page::of_text(text), Page::of_text(before));
    req.skills
        .iter()
        .filter(|k| {
            find_literal(&page, &k.term, &k.aliases).is_some()
                && find_literal(&was, &k.term, &k.aliases).is_none()
        })
        .map(|k| k.term.clone())
        .collect()
}

/// Every line of a bank entry: header, subtitle, notes, bullets with their variants and skills, facts with theirs.
fn entry_lines(e: &BankEntry) -> Vec<String> {
    let mut out = vec![
        format!("{}, {} · {}", e.name, e.place, e.dates),
        e.subtitle.clone(),
    ];
    out.extend(e.notes.iter().map(|n| format!("{}: {}", n.label, n.text)));
    for b in &e.bullets {
        out.push(b.text.clone());
        out.extend(b.variants.iter().cloned());
        out.extend(b.skills.iter().cloned());
    }
    out.extend(e.fact_lines().map(str::to_owned));
    out
}

fn entry_of<'a>(bank: &'a Bank, id: &str) -> Option<&'a BankEntry> {
    bank.sections
        .iter()
        .flat_map(|s| &s.entries)
        .find(|e| e.id == id)
}

/// The bank entry a line belongs to: a bullet's entry, or a note line's (`entry#n`).
fn entry_holding<'a>(bank: &'a Bank, line: &str) -> Option<&'a BankEntry> {
    if let Some((entry, n)) = line.rsplit_once('#')
        && n.parse::<usize>().is_ok()
    {
        return entry_of(bank, entry);
    }
    bank.sections
        .iter()
        .flat_map(|s| &s.entries)
        .find(|e| e.bullets.iter().any(|b| b.id == line))
}

fn entry_section(base: &Resume, entry: &str) -> Option<String> {
    base.sections
        .iter()
        .find(|s| s.entries.iter().any(|e| e.id == entry))
        .map(|s| s.id.clone())
}

pub fn group_section(base: &Resume, group: &str) -> Option<String> {
    base.sections
        .iter()
        .find(|s| s.groups.iter().any(|g| g.id == group))
        .map(|s| s.id.clone())
}

/// A bullet's or note line's (`entry#n`) text on the master, and its section.
fn line_of(base: &Resume, line: &str) -> Option<(String, String)> {
    for s in &base.sections {
        for e in &s.entries {
            if let Some(b) = e.bullets.iter().find(|b| b.id == line) {
                return Some((b.text.clone(), s.id.clone()));
            }
            for (i, n) in e.notes.iter().enumerate() {
                if line == format!("{}#{}", e.id, i + 1) {
                    return Some((n.text.clone(), s.id.clone()));
                }
            }
        }
    }
    None
}

/// The master's summary line (the summary section's first entry's first note, else bullet) and its section id;
/// `("", "summary")` when it has none, as `changes_of` names it.
fn summary_of(base: &Resume) -> (String, String) {
    let Some(s) = base
        .sections
        .iter()
        .find(|s| s.kind == Some(SectionKind::Summary))
    else {
        return (String::new(), "summary".to_owned());
    };
    let text = s
        .entries
        .first()
        .and_then(|e| {
            e.notes
                .first()
                .map(|n| n.text.clone())
                .or_else(|| e.bullets.first().map(|b| b.text.clone()))
        })
        .unwrap_or_default();
    (text, s.id.clone())
}

/// The skills line a new skill of `kind` goes on: the one a suggested addition of a same-kind skill used; else the
/// line already stating the most requirement skills of that kind (a tool goes beside Git, not beside Python); else
/// the last skills line, the catch-all (`Tools & Technologies`) more often than the first (`Programming Languages`).
fn best_line(
    r: &Review,
    last: &Last,
    kind: resume_core::requirements::SkillKind,
) -> Option<String> {
    let same_kind = last.changes.iter().find_map(|c| match &c.op {
        Op::SkillAdd { group, .. } if c.origin == Origin::Suggested => {
            let term = c.hits.first()?;
            r.req
                .skills
                .iter()
                .any(|k| &k.term == term && k.kind == kind)
                .then(|| group.clone())
        }
        _ => None,
    });
    let groups: Vec<_> = r.base.sections.iter().flat_map(|s| &s.groups).collect();
    let kin = |item: &str| {
        r.req.skills.iter().any(|k| {
            k.kind == kind
                && std::iter::once(&k.term)
                    .chain(&k.aliases)
                    .any(|t| t.eq_ignore_ascii_case(item.trim()))
        })
    };
    let holding = groups
        .iter()
        .map(|g| (g.items.iter().filter(|i| kin(i)).count(), g))
        .filter(|(n, _)| *n > 0)
        // `max_by_key` keeps the last of equals: ties go to the later, broader line.
        .max_by_key(|(n, _)| *n)
        .map(|(_, g)| g.id.clone());
    same_kind
        .or(holding)
        .or_else(|| groups.last().map(|g| g.id.clone()))
}

fn is_custom(last: &Last, key: &str) -> bool {
    last.draft.custom.iter().any(|c| c.key == key)
}

/// A fresh key for one of your own edits: 16 hex digits, like a suggestion's.
fn new_key() -> String {
    uuid::Uuid::new_v4().simple().to_string()[..16].to_owned()
}

impl From<Decision> for resume_core::change::Decision {
    fn from(d: Decision) -> Self {
        match d {
            Decision::On => Self::On,
            Decision::Off => Self::Off,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use resume_core::change::{Change, Decision as CoreDecision, DraftEvent, Op, Origin, State};
    use resume_core::{Resume, migrate_v1, resolve};
    use resume_llm::instruct::{Edit, EditKind};

    use super::{displaced, place};
    use crate::dto::{CustomEdit, Displaced};
    use crate::error::AppError;

    /// The synthetic jane-doe master, resolved.
    fn jane() -> Resume {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/banks/jane-doe.v1.json"
        );
        let v1: Resume = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        let m = migrate_v1(&v1);
        resolve(&m.bank, &m.master).unwrap()
    }

    fn edit(kind: EditKind, id: &str, text: &str) -> Edit {
        Edit {
            kind,
            id: id.to_owned(),
            text: text.to_owned(),
            why: "why".to_owned(),
        }
    }

    fn change(key: &str, op: Op) -> Change {
        let mut c = Change::new(
            op,
            Origin::Yours,
            String::new(),
            vec![],
            String::new(),
            1,
            String::new(),
        );
        c.key = key.to_owned();
        c
    }

    fn set(key: &str, line: &str) -> Change {
        change(
            key,
            Op::Set {
                line: line.to_owned(),
                text: "x".to_owned(),
            },
        )
    }

    /// A change maker that numbers its keys and flags the bullet edit, as `custom` would.
    fn maker() -> impl FnMut(CustomEdit) -> super::Result<Change> {
        let mut n = 0;
        move |e| {
            n += 1;
            let mut c = change(
                &format!("k{n}"),
                Op::Summary {
                    text: String::new(),
                },
            );
            if matches!(e, CustomEdit::Bullet { .. }) {
                c.flags = vec!["“9” isn't in your resume".to_owned()];
            }
            Ok(c)
        }
    }

    #[test]
    fn place_maps_each_kind_and_lands_one_pending_batch() {
        let base = jane();
        let mut now = base.clone();
        now.sections[0].entries[0].bullets[0].text = "Shipped a Rust service".to_owned();
        let edits = vec![
            edit(
                EditKind::Line,
                "acme.1",
                " Built a Rust and Postgres service ",
            ),
            edit(EditKind::Summary, "", "Software engineer"),
            edit(EditKind::Bullet, "acme", "Wrote SQL"),
            edit(EditKind::Skill, "languages", "Go"),
        ];
        let p = place(&base, &now, &[], edits, maker());
        assert_eq!(p.skipped, vec![]);
        let edits: Vec<_> = p.proposals.iter().map(|p| p.edit.clone()).collect();
        assert_eq!(
            edits,
            [
                CustomEdit::Line {
                    line: "acme.1".into(),
                    text: "Built a Rust and Postgres service".into()
                },
                CustomEdit::Summary {
                    text: "Software engineer".into()
                },
                CustomEdit::Bullet {
                    entry: "acme".into(),
                    text: "Wrote SQL".into()
                },
                CustomEdit::Skill {
                    group: "languages".into(),
                    item: "Go".into()
                },
            ]
        );
        // `was` is the page as it reads now, not the master.
        assert_eq!(
            p.proposals[0].was.as_deref(),
            Some("Shipped a Rust service")
        );
        assert_eq!(p.proposals[1].was, None);
        assert_eq!(p.proposals[2].was, None);
        assert_eq!(p.proposals[2].flags.len(), 1);
        let Some(DraftEvent::Batch { events }) = p.step else {
            panic!("one batch expected, got {:?}", p.step);
        };
        assert_eq!(events.len(), 8);
        for (i, pair) in events.chunks(2).enumerate() {
            let key = format!("k{}", i + 1);
            assert!(matches!(&pair[0], DraftEvent::AddCustom { change } if change.key == key));
            assert!(matches!(
                &pair[1],
                DraftEvent::Decide { key: k, decision: Some(CoreDecision::Off) } if *k == key
            ));
            assert_eq!(p.proposals[i].key, key);
        }
    }

    #[test]
    fn place_skips_what_it_cant_place() {
        let base = jane();
        let before = base.sections[0].entries[0].bullets[1].text.clone();
        let scope = ["acme".to_owned(), "languages".to_owned()];
        let edits = vec![
            edit(EditKind::Line, "acme.9", "New words"),
            edit(EditKind::Line, "tidepool.1", "New words"),
            edit(EditKind::Line, "state-u#1", "New words"),
            edit(EditKind::Line, "acme.2", &before),
            edit(EditKind::Summary, "", "Software engineer"),
            edit(EditKind::Bullet, "tidepool", "Wrote SQL"),
            edit(EditKind::Skill, "languages", "rust"),
            edit(EditKind::Skill, "nowhere", "Go"),
            edit(EditKind::Line, "acme.3", "   "),
            edit(EditKind::Line, "acme.1", "First"),
            edit(EditKind::Line, "acme.1", "Second"),
        ];
        let p = place(&base, &base, &scope, edits, maker());
        let why: Vec<_> = p.skipped.iter().map(|s| s.why.as_str()).collect();
        assert_eq!(
            why,
            [
                "Couldn't place an edit for acme.9",
                "tidepool.1 is outside what you picked",
                "state-u#1 is a note line; only bullets get new wording",
                "acme.2 already reads that way",
                "The summary is outside what you picked",
                "tidepool is outside what you picked",
                "languages already lists rust",
                "Couldn't place an edit for nowhere",
                "Couldn't place an edit for acme.3",
                "Another edit already took this line",
            ]
        );
        assert_eq!(p.skipped[0].what, "New words");
        assert_eq!(p.proposals.len(), 1);
        assert!(matches!(&p.step, Some(DraftEvent::Batch { events }) if events.len() == 2));

        // A bullet id in scope allows that bullet only; nothing placed lands nothing.
        let p = place(
            &base,
            &base,
            &["acme.1".to_owned()],
            vec![edit(EditKind::Line, "acme.2", "New words")],
            maker(),
        );
        assert!(p.step.is_none() && p.proposals.is_empty());

        // A change the maker refuses is skipped with its message.
        let p = place(
            &base,
            &base,
            &[],
            vec![edit(EditKind::Skill, "languages", "Go")],
            |_| Err(AppError::missing("This skills line isn't on the master")),
        );
        assert_eq!(p.skipped[0].why, "This skills line isn't on the master");
        assert!(p.step.is_none());
    }

    #[test]
    fn accept_displaces_only_changes_on_the_page_holding_the_same_line() {
        let changes = vec![
            set("mine", "acme.1"),
            change(
                "rewrite",
                Op::Rewrite {
                    bullet: "acme.1".into(),
                    text: "x".into(),
                },
            ),
            change(
                "cut",
                Op::Cut {
                    bullet: "acme.1".into(),
                },
            ),
            set("off", "acme.1"),
            set("other", "acme.2"),
            change("sum-a", Op::Summary { text: "a".into() }),
            change("sum-b", Op::Summary { text: "b".into() }),
            change(
                "skill",
                Op::SkillAdd {
                    group: "languages".into(),
                    item: "Go".into(),
                },
            ),
        ];
        let states: BTreeMap<String, State> = [
            ("mine", State::Off),
            ("rewrite", State::In),
            ("cut", State::Forced { warning: None }),
            ("off", State::Off),
            ("other", State::In),
            ("sum-a", State::Off),
            ("sum-b", State::In),
            ("skill", State::In),
        ]
        .into_iter()
        .map(|(k, s)| (k.to_owned(), s))
        .collect();
        let d = |key: &str, by: &str| Displaced {
            key: key.into(),
            by: by.into(),
        };
        assert_eq!(
            displaced(
                &changes,
                &states,
                &["mine".into(), "sum-a".into(), "skill".into()]
            )
            .unwrap(),
            [d("rewrite", "mine"), d("cut", "mine"), d("sum-b", "sum-a")]
        );
        // Accepted together, two changes on one line don't displace each other.
        assert_eq!(
            displaced(&changes, &states, &["mine".into(), "rewrite".into()]).unwrap(),
            [d("cut", "mine")]
        );
        assert!(displaced(&changes, &states, &["gone".into()]).is_err());
    }
}

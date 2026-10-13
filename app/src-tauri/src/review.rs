//! The open review: Suggest's changes for one posting × master × mode, your draft over them (an event log with an
//! undo cursor, kept in the store), and the fit check rebuilt after every step with the warm renderer.

use std::sync::Arc;
use std::time::Instant;

use resume_core::bank::{Bank, BankEntry};
use resume_core::change::{Change, Draft, DraftEvent, Mode as CoreMode, Op, Origin};
use resume_core::changes::{changes_of, draft_changes};
use resume_core::matcher::{Page, find_literal};
use resume_core::resume::SectionKind;
use resume_core::suggestions::Suggestions;
use resume_core::validate::Grounding;
use resume_core::{Master, Requirements, Resume, resolve};
use resume_llm::rewrite::{self, RewriteInput};
use resume_llm::suggest::{self, SuggestInput};
use resume_pipeline::{suggest_key, today};
use resume_store::{DraftLog, NewAnalysis, PostingRow};
use resume_tailor::{Tailored, tailor};

use crate::dto::{
    ChipState, CustomEdit, Decision, EditTargets, MasterChoice, Mode, ReviewView, Rewritten, Stage,
    Target,
};
use crate::error::{AppError, Result};
use crate::jobs;
use crate::state::{App, blocking, lock, model_call};
use crate::view;

/// About how long Suggest takes on sonnet, for the progress bar.
const SUGGEST_MS: u32 = 15_000;
/// About how long a one-line rewrite takes.
const REWRITE_MS: u32 = 8_000;

pub struct Review {
    /// The analysis id.
    pub id: String,
    pub posting_id: String,
    pub posting: PostingRow,
    pub bank: Bank,
    pub master: Master,
    pub mode: CoreMode,
    pub req: Requirements,
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

/// Opens the review of `job`: Suggest's output for the master and mode (an existing analysis for the same input and
/// model keeps its draft; `fresh` calls Suggest again, carrying your decisions over by key), then the fit check. A
/// call that a newer `start_review` overtook rejects as cancelled and leaves the newer review open.
pub async fn start_review(
    app: &Arc<App>,
    job: &str,
    master: Option<String>,
    mode: Option<Mode>,
    fresh: bool,
) -> Result<ReviewView> {
    let ticket = app.review_ticket();
    let row = jobs::posting(app, job)?;
    let req = jobs::requirements(app, &row).await?;
    let settings = app.settings();
    let latest = lock(&app.store).latest_analysis(job)?;
    let mode: CoreMode = mode
        .map(Into::into)
        .or(latest.as_ref().map(|a| a.mode))
        .unwrap_or_else(|| settings.default_mode.into());
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
    let cached = if fresh {
        None
    } else {
        lock(&app.store).analysis(&key, &model)?
    };
    let analysis = match cached {
        Some(a) => {
            lock(&app.store).touch_analysis(&a.id)?;
            a
        }
        None => {
            let llm = app.claude().await?;
            let (p, rq, rs, bk, cs, m) = (
                row.posting.clone(),
                req.clone(),
                resume.clone(),
                bank.clone(),
                candidates.clone(),
                model.clone(),
            );
            let (s, out) = model_call(app, job, Stage::Suggesting, Some(SUGGEST_MS), async move {
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
            let store = lock(&app.store);
            let previous = match store.analysis(&key, &model)? {
                Some(a) => Some(a),
                None => store
                    .analyses(job)?
                    .into_iter()
                    .find(|a| a.master_id == master.id && a.mode == mode),
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
            a
        }
    };
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
            mode,
            req,
            suggestions: analysis.suggestions,
            changes,
            log,
            base: resume,
            masters,
            last: None,
        };
        // Built aside: a failed rebuild leaves the open review as it was.
        let view = rebuild(app, &mut review)?;
        let mut slot = lock(&app.review);
        if !app.is_newest_review(ticket) {
            return Err(AppError::cancelled());
        }
        *slot = Some(review);
        Ok(view)
    })
    .await
}

/// The fit check over the draft as it stands: the tailored resume, its scores, every change's state, and the
/// highlighted pages. Saves the projected score for the Jobs list.
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

/// The change `key` as the last rebuild had it (with your wording; your own included).
fn change<'a>(r: &'a Review, key: &str) -> Result<&'a Change> {
    r.last
        .as_ref()
        .and_then(|l| l.changes.iter().find(|c| c.key == key))
        .ok_or_else(|| AppError::missing("This change is gone; reopen the review"))
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
        let change = custom(r, e)?;
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
                    )?;
                    events.push(DraftEvent::AddCustom { change });
                }
            }
        }
        // One click is one undo step.
        match events.len() {
            0 => {}
            1 => push(app, r, events.remove(0))?,
            _ => push(app, r, DraftEvent::Batch { events })?,
        }
        Ok(())
    })
    .await
}

/// Where your own edits can go: the master's entries, skills lines and lines.
pub fn edit_targets(app: &App, review: &str) -> Result<EditTargets> {
    let mut slot = lock(&app.review);
    let r = open(&mut slot, review)?;
    let mut t = EditTargets {
        entries: vec![],
        groups: vec![],
        lines: vec![],
    };
    for s in &r.base.sections {
        for e in &s.entries {
            let label = [e.name.as_str(), e.subtitle.as_str()]
                .into_iter()
                .filter(|p| !p.is_empty())
                .collect::<Vec<_>>()
                .join(" · ");
            t.entries.push(Target {
                id: e.id.clone(),
                label,
            });
            for (i, n) in e.notes.iter().enumerate() {
                t.lines.push(Target {
                    id: format!("{}#{}", e.id, i + 1),
                    label: if n.label.is_empty() {
                        n.text.clone()
                    } else {
                        format!("{}: {}", n.label, n.text)
                    },
                });
            }
            for b in &e.bullets {
                t.lines.push(Target {
                    id: b.id.clone(),
                    label: b.text.clone(),
                });
            }
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
    let llm = app.claude().await?;
    let model = app.settings().suggest_model;
    let (line, facts, terms, budget) = (job.line, job.facts, job.terms, job.budget);
    let text = model_call(
        app,
        &job.posting_id,
        Stage::Rewriting,
        Some(REWRITE_MS),
        async move {
            let input = RewriteInput {
                line: &line,
                facts: &facts,
                terms: &terms,
                note: &note,
                budget,
            };
            rewrite::rewrite(&llm, &rewrite::schema(), &input, &model)
                .await
                .map(|(text, _)| text)
        },
    )
    .await?;
    let id = review.to_owned();
    blocking(app, move |app| {
        let mut slot = lock(&app.review);
        let r = open(&mut slot, &id)?;
        let c = change(r, &key)?.clone();
        let flags = grounding(r, &op_with_text(&c.op, &text), &c.before);
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

/// Your own edit as a change: its target checked, the requirement terms it adds as hits, and the grounding
/// validator's findings (yours stay in; the findings become its warning).
fn custom(r: &Review, e: CustomEdit) -> Result<Change> {
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
    Ok(yours(r, op, before, section, new_key()))
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
        )),
        _ => Err(AppError::other("This edit has no wording to change")),
    }
}

fn yours(r: &Review, op: Op, before: String, section: String, key: String) -> Change {
    let text = match &op {
        Op::Set { text, .. } | Op::Add { text, .. } | Op::Summary { text } => text.as_str(),
        Op::SkillAdd { item, .. } => item.as_str(),
        _ => "",
    };
    let hits = new_terms(&r.req, text, &before);
    let flags = grounding(r, &op, &before);
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
/// for a summary), names against the bank and the skills your evidence backs.
fn grounding(r: &Review, op: &Op, before: &str) -> Vec<String> {
    let g = Grounding::new(&r.bank, &r.suggestions, &r.req);
    match op {
        Op::SkillAdd { item, .. } => g.check_item(item),
        Op::Summary { text } => g.check(text, &[g.bank_text()]),
        Op::Add { entry, text } => {
            let lines = entry_of(&r.bank, entry)
                .map(entry_lines)
                .unwrap_or_default();
            let lines: Vec<&str> = lines.iter().map(String::as_str).collect();
            g.check(text, &lines)
        }
        Op::Set { line, text } => {
            let mut lines = entry_holding(&r.bank, line)
                .map(entry_lines)
                .unwrap_or_default();
            lines.push(before.to_owned());
            let lines: Vec<&str> = lines.iter().map(String::as_str).collect();
            g.check(text, &lines)
        }
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

/// Every line of a bank entry: header, subtitle, notes, bullets with their variants and skills.
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

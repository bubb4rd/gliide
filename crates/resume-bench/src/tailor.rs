//! Tailor: Suggest on a posting with its gold requirements, then the app's step after it
//! (`resume_pipeline::shorten`: a rewrite that wraps long is asked once to fit), its turns and time counted with the
//! run's. A valid run goes through `changes_of` and `resume_tailor::tailor` as the app's review does.

use std::time::Instant;

use resume_core::change::{Change, Draft, Mode, State};
use resume_core::changes::{changes_of, draft_changes};
use resume_core::suggestions::Suggestions;
use resume_core::{Bank, Master, Requirements, Resume, resolve};
use resume_llm::suggest::{self, SuggestInput};
use resume_llm::{Llm, Route, rewrite};
use resume_pipeline::{candidates, mode_name, shorten, template, today};
use resume_render::{FontOptions, Renderer};
use resume_tailor::{Tailored, tailor};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::run::{Called, call};
use crate::{BenchError, Fixtures};

/// The master a mode tailors: the trimmed one for bank-aware (so a bank entry is left to suggest), the full `swe`
/// master for master-only.
pub fn master_of(fx: &Fixtures, mode: Mode) -> &Master {
    match mode {
        Mode::BankAware => &fx.trim,
        Mode::MasterOnly => &fx.master,
    }
}

/// Suggest on posting `item` in `mode`, then the fit step: the call and what it read (its `meta`).
pub async fn suggest(
    llm: &impl Llm,
    route: &Route,
    fx: &Fixtures,
    mode: Mode,
    item: &str,
) -> Result<(Called, Value), BenchError> {
    let (f, req) = fx.item(item)?;
    let master = master_of(fx, mode);
    let resume = resolve(&fx.bank, master)?;
    let mut renderer = Renderer::new(&template(master)?, FontOptions::default());
    let candidates = candidates(&fx.bank, master, req, mode);
    let input = SuggestInput {
        posting: &f.posting,
        requirements: req,
        resume: &resume,
        bank: &fx.bank,
        candidates: &candidates,
    };
    let mut c = call(suggest::suggest(
        llm,
        &suggest::schema(),
        &input,
        &route.model,
    ))
    .await;
    let (shortened, problems) =
        fit(&mut c, llm, route, &resume, &fx.bank, req, &mut renderer).await;
    let mut meta = json!({
        "mode": mode_name(mode),
        "master": master.id,
        "candidates": candidates,
        "requirements": "gold",
        "input_chars": suggest::input(&input).chars().count(),
        "shortened": shortened,
    });
    if !problems.is_empty() {
        meta["fit_problems"] = json!(problems);
    }
    Ok((c, meta))
}

/// The fit step on a valid run: each rewrite that wraps long, asked once to fit; the turns and time join the run's,
/// a rewrite that now fits replaces the one in its output. How many it replaced, and what went wrong on the way (a
/// failed measure or rewrite leaves the run as Suggest wrote it).
async fn fit(
    c: &mut Called,
    llm: &impl Llm,
    route: &Route,
    resume: &Resume,
    bank: &Bank,
    req: &Requirements,
    renderer: &mut Renderer,
) -> (usize, Vec<String>) {
    let Some(Ok(mut s)) = c.output.as_ref().map(Suggestions::deserialize) else {
        return (0, Vec::new());
    };
    let started = Instant::now();
    let long = match shorten::long_rewrites(&s, resume, bank, req, renderer) {
        Ok(long) => long,
        Err(e) => return (0, vec![format!("measure failed: {e}")]),
    };
    let mut replaced = 0;
    let mut problems = Vec::new();
    for job in &long {
        match rewrite::rewrite(llm, &rewrite::schema(), &job.input(), &route.model).await {
            Ok((text, out)) => {
                c.attempts.extend(out.attempts);
                if shorten::apply(&mut s, job, &text, renderer).unwrap_or(false) {
                    replaced += 1;
                }
            }
            Err(e) => problems.push(format!("shorten failed: {e}")),
        }
    }
    c.latency += started.elapsed();
    if replaced > 0 {
        c.output = Some(serde_json::to_value(&s).expect("Suggestions serialize"));
    }
    (replaced, problems)
}

/// A Suggest output as the app's review sees it with an empty draft: its changes and `tailor` on them.
pub struct Reviewed {
    pub changes: Vec<Change>,
    pub tailored: Tailored,
}

/// Why the review leaves a change out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LeftOut {
    /// It has grounding flags, so it starts left out.
    Validator,
    /// It doesn't apply to the master.
    Apply,
    /// The fit check: it takes more lines than the line it replaces, as the template wraps it.
    Lines,
    /// The fit check: the page runs over.
    Page,
}

impl Reviewed {
    /// Why change `c` is left out; `None` when it is in.
    pub fn left_out(&self, c: &Change) -> Option<LeftOut> {
        let Some(State::LeftOut { reason }) = self.tailored.states.get(&c.key) else {
            return None;
        };
        Some(if !c.flags.is_empty() {
            LeftOut::Validator
        } else if reason.starts_with("doesn't apply") {
            LeftOut::Apply
        } else if reason.starts_with("takes ") {
            LeftOut::Lines
        } else {
            LeftOut::Page
        })
    }
}

/// `s` through the review of `mode`'s master ([`master_of`]) for requirements `req`.
pub fn review(
    fx: &Fixtures,
    mode: Mode,
    req: &Requirements,
    s: &Suggestions,
    renderer: &mut Renderer,
) -> Result<Reviewed, BenchError> {
    let master = master_of(fx, mode);
    let changes = draft_changes(
        &changes_of(s, &fx.bank, master, req, mode),
        &Draft::default(),
    );
    let tailored = tailor(
        &fx.bank,
        master,
        req,
        &changes,
        &Draft::default(),
        renderer,
        today(),
    )?;
    Ok(Reviewed { changes, tailored })
}

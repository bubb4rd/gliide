//! The fit check (ARCHITECTURE.md §5, §6): which changes are in, the tailored resume they make on the page limit, and
//! its three scores.
//!
//! **States.** A change starts in, except: one you turned off is off; one the grounding validator flagged is left out
//! (with the flags as its reason) and one from your bank (`include`, `swap_entry`) is off while the mode experiment
//! runs. One you turned on is in, or forced when it would be left out. Yours and forced changes are never left out by
//! the fit check; when they run long they stay, with a warning.
//!
//! **Fit, as a fit loop with one measure pass.** One measure compile lays out every in-change's new text at
//! the width of the line it targets: a rewrite, variant, set, swap or note addition over its line's original count is
//! left out ("takes 3 lines, not 2"); a skills line keeps its line count by leaving out its additions, lowest priority
//! first. Then `apply` and compile; while the resume runs over its page limit, the lowest-priority included bank entry
//! is left out, then new bullet, then skill addition on a line that grew, then the summary, at most [`MAX_ROUNDS`]
//! compiles.
//!
//! **Scores.** *Now* is the master as rendered; *projected* the draft, fitted; *ceiling* every change the validator
//! didn't flag (your decisions aside; bank changes included, which only bank-aware mode proposes), fitted. Each is
//! scored with its compile's page count and PDF read-back ([`Renderer::pdf_text`]), so the format check sees what an
//! ATS reads from the exported file.
//!
//! **Points** are a change's marginal value against the draft as fitted: for a change that is in, projected minus
//! the score with only that change taken out; for one that isn't, the score with only that change put in minus
//! projected. Each is measured, not estimated: `apply`, one compile for the page count and PDF read-back, `score`;
//! the fit check doesn't re-run, so a change that would push the page over shows its cost in the Format component.

use std::collections::BTreeMap;

use resume_core::apply::{ApplyError, apply};
use resume_core::change::{Change, Decision, Draft, Op, Origin, State};
use resume_core::{
    Bank, Master, PageFacts, Requirements, ResolveError, Resume, Score, resolve, score,
};
use resume_render::{Candidate, LineKind, Metrics, RenderError, Rendered, Renderer};

/// Compiles the page loop may run before it settles for what it has.
pub const MAX_ROUNDS: usize = 16;

#[derive(Debug, thiserror::Error)]
pub enum TailorError {
    #[error(transparent)]
    Resolve(#[from] ResolveError),
    #[error(transparent)]
    Apply(#[from] ApplyError),
    #[error(transparent)]
    Render(#[from] RenderError),
    #[error("resume JSON: {0}")]
    Json(#[from] serde_json::Error),
}

/// A tailored resume and what the fit check decided.
pub struct Tailored {
    /// `apply(master, changes that are in)`, with the build fields the template highlights.
    pub resume: Resume,
    /// Its plain compile (no highlight): the PDF, metrics and page count.
    pub rendered: Rendered,
    /// Every change's state, by key.
    pub states: BTreeMap<String, State>,
    /// Changes kept although they run long (yours and forced), by key: why.
    pub warnings: BTreeMap<String, String>,
    /// The line each line edit makes (rewrite, variant, set, swap, note addition), by key: what the review shows.
    pub after: BTreeMap<String, String>,
    pub now: Score,
    pub projected: Score,
    pub ceiling: Score,
    /// The ceiling's states, by key.
    pub ceiling_states: BTreeMap<String, State>,
    /// Points per change, by key (see the crate docs); absent when the change can't be toggled alone.
    pub points: BTreeMap<String, f64>,
    /// Compiles run: measure passes, page-loop renders, point renders.
    pub compiles: Compiles,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Compiles {
    pub measure: usize,
    pub fit: usize,
    pub points: usize,
}

/// Tailors `master` with `changes` (`draft_changes` output: suggestions with your wording, then your own) under
/// `draft`'s decisions, through `renderer`'s template, scoring against `req` on `today` (`(year, month)`).
pub fn tailor(
    bank: &Bank,
    master: &Master,
    req: &Requirements,
    changes: &[Change],
    draft: &Draft,
    renderer: &mut Renderer,
    today: (u32, u32),
) -> Result<Tailored, TailorError> {
    let base = resolve(bank, master)?;
    let cx = Cx {
        bank,
        master,
        base: &base,
        req,
        today,
        ats_safe: renderer.manifest().ats_safe,
    };
    let mut compiles = Compiles::default();
    let now_page = renderer.render(&serde_json::to_string(&base)?, false)?;
    compiles.fit += 1;
    let now = cx.score(&base, &now_page, renderer)?;

    let edits: Vec<LineEdit> = changes.iter().map(|c| cx.line_edit(c)).collect();
    let after = changes
        .iter()
        .zip(&edits)
        .filter_map(|(c, e)| match e {
            LineEdit::Lines(lines) => Some((
                c.key.clone(),
                lines
                    .iter()
                    .map(|(new, _)| new.text.as_str())
                    .collect::<Vec<_>>()
                    .join(" / "),
            )),
            _ => None,
        })
        .collect();

    let start = changes
        .iter()
        .map(|c| Slot::draft(c, draft.decisions.get(&c.key).copied()))
        .collect();
    let drafted = cx.fit(changes, &edits, start, renderer, &mut compiles)?;
    let projected = cx.score(&drafted.resume, &drafted.rendered, renderer)?;

    let start = changes.iter().map(Slot::ceiling).collect();
    let ceiling_fit = cx.fit(changes, &edits, start, renderer, &mut compiles)?;
    let ceiling = cx.score(&ceiling_fit.resume, &ceiling_fit.rendered, renderer)?;

    let points = cx.points(
        changes,
        &drafted.slots,
        projected.total,
        renderer,
        &mut compiles,
    )?;

    let keyed = |slots: &[Slot]| -> BTreeMap<String, State> {
        changes
            .iter()
            .zip(slots)
            .map(|(c, s)| (c.key.clone(), s.state.clone()))
            .collect()
    };
    Ok(Tailored {
        states: keyed(&drafted.slots),
        warnings: changes
            .iter()
            .zip(&drafted.slots)
            .filter_map(|(c, s)| Some((c.key.clone(), s.warning.clone()?)))
            .collect(),
        ceiling_states: keyed(&ceiling_fit.slots),
        resume: drafted.resume,
        rendered: drafted.rendered,
        after,
        now,
        projected,
        ceiling,
        points,
        compiles,
    })
}

/// Where a change stands while the fit check runs.
#[derive(Debug, Clone)]
struct Slot {
    state: State,
    /// Yours, or turned on by you: the fit check never leaves it out.
    immune: bool,
    yours: bool,
    /// Why an immune change runs long.
    warning: Option<String>,
}

impl Slot {
    /// The state your draft starts a change in (ARCHITECTURE.md §5 diagram).
    fn draft(c: &Change, decision: Option<Decision>) -> Self {
        let yours = c.origin == Origin::Yours;
        let flags = (!c.flags.is_empty()).then(|| c.flags.join("; "));
        let (state, immune) = match decision {
            Some(Decision::Off) => (State::Off, false),
            Some(Decision::On) if flags.is_some() && !yours => {
                (State::Forced { warning: None }, true)
            }
            Some(Decision::On) => (State::In, true),
            None if yours => (State::In, true),
            None => match flags.clone() {
                Some(reason) => (State::LeftOut { reason }, false),
                None if from_bank(&c.op) => (State::Off, false),
                None => (State::In, false),
            },
        };
        Self {
            state,
            immune,
            yours,
            // Yours stays in although flagged: the flags become its warning.
            warning: if yours { flags } else { None },
        }
    }

    /// The ceiling's start: every change in except those the validator flagged; yours as in the draft.
    fn ceiling(c: &Change) -> Self {
        Self::draft(
            c,
            (c.flags.is_empty() || c.origin == Origin::Yours).then_some(Decision::On),
        )
        .unpinned()
    }

    /// Suggestions in the ceiling are subject to the fit check: only yours stay immune.
    fn unpinned(mut self) -> Self {
        self.immune = self.yours;
        self
    }

    fn is_in(&self) -> bool {
        matches!(self.state, State::In | State::Forced { .. })
    }

    /// What the fit check does to a change that's over a budget: leaves it out, or keeps an immune one with a
    /// warning (forced, unless it's yours).
    fn over(&mut self, reason: String) {
        if !self.immune {
            self.state = State::LeftOut { reason };
            return;
        }
        if !self.yours {
            self.state = State::Forced {
                warning: Some(reason.clone()),
            };
        }
        self.warning.get_or_insert(reason);
    }
}

/// `include` and `swap_entry`: changes that bring in experience from outside the master.
fn from_bank(op: &Op) -> bool {
    matches!(op, Op::Include { .. } | Op::SwapEntry { .. })
}

/// What a change does to the lines with a budget.
enum LineEdit {
    /// Not a line edit (cuts, additions, skills, bank, order, summary).
    None,
    /// The lines it changes: its text and the master text it replaces, as measure candidates (ids set at measure).
    Lines(Vec<(Candidate, Candidate)>),
    /// It doesn't apply to the master on its own.
    Failed(String),
}

struct Fitted {
    resume: Resume,
    rendered: Rendered,
    slots: Vec<Slot>,
}

struct Cx<'a> {
    bank: &'a Bank,
    master: &'a Master,
    /// The master, resolved.
    base: &'a Resume,
    req: &'a Requirements,
    today: (u32, u32),
    ats_safe: bool,
}

impl Cx<'_> {
    /// `resume`'s score as `rendered` puts it on the page: its page count and its PDF's text read back.
    fn score(
        &self,
        resume: &Resume,
        rendered: &Rendered,
        renderer: &mut Renderer,
    ) -> Result<Score, TailorError> {
        let facts = PageFacts {
            pages: rendered.metrics.pages,
            page_limit: self.master.page_limit,
            ats_safe: self.ats_safe,
            pdf_text: Some(renderer.pdf_text(rendered)?),
        };
        Ok(score(resume, self.req, &facts, self.today))
    }

    fn apply<'c>(
        &self,
        changes: impl IntoIterator<Item = &'c Change>,
    ) -> Result<Resume, ApplyError> {
        let changes: Vec<&Change> = changes.into_iter().collect();
        apply(self.bank, self.master, &changes)
    }

    /// The changes that are in, applied. A change that doesn't apply beside the others (a swap whose words a rewrite
    /// replaced) is left out with the reason, whoever asked for it, and the rest applied again.
    fn apply_in(&self, changes: &[Change], slots: &mut [Slot]) -> Result<Resume, TailorError> {
        loop {
            let err = match self.apply(in_changes(changes, slots)) {
                Ok(resume) => return Ok(resume),
                Err(e) => e,
            };
            let Some(i) = err
                .key()
                .and_then(|k| changes.iter().position(|c| c.key == k))
                .filter(|&i| slots[i].is_in())
            else {
                return Err(err.into());
            };
            slots[i].state = State::LeftOut {
                reason: format!("doesn't apply: {err}"),
            };
        }
    }

    /// The lines `c` changes, applied to the master alone, beside the master text each replaces.
    fn line_edit(&self, c: &Change) -> LineEdit {
        if !matches!(
            c.op,
            Op::Rewrite { .. }
                | Op::UseVariant { .. }
                | Op::Set { .. }
                | Op::Swap { .. }
                | Op::NoteAdd { .. }
        ) {
            return LineEdit::None;
        }
        match self.apply([c]) {
            Ok(applied) => LineEdit::Lines(changed_lines(&applied, self.base)),
            Err(e) => LineEdit::Failed(e.to_string()),
        }
    }

    /// The fit check from `slots`: line budgets from one measure compile, then the page loop.
    fn fit(
        &self,
        changes: &[Change],
        edits: &[LineEdit],
        mut slots: Vec<Slot>,
        renderer: &mut Renderer,
        compiles: &mut Compiles,
    ) -> Result<Fitted, TailorError> {
        let mut cands = Measure::default();
        // Line edits: every changed line's new text and its master text.
        let mut line_checks = Vec::new();
        for (i, edit) in edits.iter().enumerate() {
            if !slots[i].is_in() {
                continue;
            }
            match edit {
                LineEdit::None => {}
                // It can't be in, whoever asked for it.
                LineEdit::Failed(e) => {
                    slots[i].state = State::LeftOut {
                        reason: format!("doesn't apply: {e}"),
                    };
                }
                LineEdit::Lines(lines) => {
                    let pairs: Vec<_> = lines
                        .iter()
                        .map(|(new, old)| (cands.push(new.clone()), cands.push(old.clone())))
                        .collect();
                    line_checks.push((i, pairs));
                }
            }
        }
        // Skills lines: the line with each prefix of its movable additions, most important first.
        let group_checks = self.group_checks(changes, &slots, &mut cands)?;

        let lines = cands.run(renderer)?;
        compiles.measure += usize::from(!cands.0.is_empty());
        for (i, pairs) in line_checks {
            if let Some((n, m)) = pairs
                .iter()
                .map(|&(new, old)| (lines[new], lines[old]))
                .find(|(n, m)| n > m)
            {
                slots[i].over(format!("takes {n} lines, not {m}"));
            }
        }
        for g in group_checks {
            let fits = g.prefixes.iter().rposition(|&p| lines[p] <= lines[g.orig]);
            let budget = lines[g.orig];
            match fits {
                Some(k) => {
                    for &i in &g.movable[k..] {
                        let n = lines[g.prefixes[k + 1]];
                        slots[i].over(format!(
                            "the {} line would take {n} lines, not {budget}",
                            g.label
                        ));
                    }
                }
                None => {
                    let n = lines[g.prefixes[0]];
                    for &i in &g.movable {
                        slots[i].over(format!(
                            "the {} line would take {n} lines, not {budget}",
                            g.label
                        ));
                    }
                    for &i in &g.kept {
                        slots[i].over(format!(
                            "the {} line takes {n} lines, not {budget}",
                            g.label
                        ));
                    }
                }
            }
        }

        // The page loop.
        let mut rounds = 0;
        loop {
            let resume = self.apply_in(changes, &mut slots)?;
            let rendered = renderer.render(&serde_json::to_string(&resume)?, false)?;
            rounds += 1;
            compiles.fit += 1;
            let pages = rendered.metrics.pages;
            let next = (pages > self.master.page_limit && rounds < MAX_ROUNDS)
                .then(|| next_out(changes, &slots, &rendered.metrics))
                .flatten();
            let Some(i) = next else {
                if pages > self.master.page_limit {
                    for (c, s) in changes.iter().zip(&mut slots) {
                        if s.is_in() && s.immune && tier(c, &rendered.metrics).is_some() {
                            s.over(format!("the resume runs to {pages} pages"));
                        }
                    }
                }
                // Your skills on a line that grew (the measure pass only checks lines with movable additions).
                for (c, s) in changes.iter().zip(&mut slots) {
                    if let Op::SkillAdd { group, .. } = &c.op
                        && s.is_in()
                        && s.immune
                        && let Some(g) = rendered
                            .metrics
                            .groups
                            .iter()
                            .find(|g| &g.id == group && g.lines > g.orig)
                    {
                        s.over(format!(
                            "the skills line takes {} lines, not {}",
                            g.lines, g.orig
                        ));
                    }
                }
                return Ok(Fitted {
                    resume,
                    rendered,
                    slots,
                });
            };
            slots[i].over("the page would overflow".into());
        }
    }

    /// For every skills line with movable additions that are in: the measure candidates of the line with the first
    /// `k` of them (by priority) for each `k`, and of the master's line.
    fn group_checks(
        &self,
        changes: &[Change],
        slots: &[Slot],
        cands: &mut Measure,
    ) -> Result<Vec<GroupCheck>, TailorError> {
        let mut groups: Vec<&str> = Vec::new();
        for (c, s) in changes.iter().zip(slots) {
            if let Op::SkillAdd { group, .. } = &c.op
                && s.is_in()
                && !s.immune
                && !groups.contains(&group.as_str())
            {
                groups.push(group);
            }
        }
        let mut out = Vec::new();
        for g in groups {
            let Some(orig) = find_group(self.base, g) else {
                continue;
            };
            let touches = |c: &Change| match &c.op {
                Op::SkillAdd { group, .. }
                | Op::SkillRemove { group, .. }
                | Op::SkillRename { group, .. } => group == g,
                Op::SkillMove { from, to, .. } => from == g || to == g,
                _ => false,
            };
            let mut movable = Vec::new();
            let mut kept = Vec::new();
            let mut fixed = Vec::new();
            for (i, (c, s)) in changes.iter().zip(slots).enumerate() {
                if !s.is_in() || !touches(c) {
                    continue;
                }
                match (&c.op, s.immune) {
                    (Op::SkillAdd { .. }, false) => movable.push(i),
                    (Op::SkillAdd { .. }, true) => {
                        kept.push(i);
                        fixed.push(i);
                    }
                    _ => fixed.push(i),
                }
            }
            // Most important first; the stable sort keeps the suggestion order among equals.
            movable.sort_by_key(|&i| changes[i].priority);
            let prefixes: Result<Vec<usize>, ApplyError> = (0..=movable.len())
                .map(|k| {
                    let set = fixed.iter().chain(&movable[..k]).map(|&i| &changes[i]);
                    let applied = self.apply(set)?;
                    let line =
                        find_group(&applied, g).map_or_else(String::new, |x| x.items.join(", "));
                    Ok(cands.push(group_candidate(&orig.label, line)))
                })
                .collect();
            // A set that doesn't apply is left to the page loop, which leaves out the change that fails.
            let Ok(prefixes) = prefixes else {
                continue;
            };
            out.push(GroupCheck {
                label: orig.label.clone(),
                orig: cands.push(group_candidate(&orig.label, orig.items.join(", "))),
                prefixes,
                movable,
                kept,
            });
        }
        Ok(out)
    }

    /// Points per change against the fitted draft `slots` (see the crate docs).
    fn points(
        &self,
        changes: &[Change],
        slots: &[Slot],
        projected: f64,
        renderer: &mut Renderer,
        compiles: &mut Compiles,
    ) -> Result<BTreeMap<String, f64>, TailorError> {
        let mut out = BTreeMap::new();
        for (i, c) in changes.iter().enumerate() {
            let on = slots[i].is_in();
            let set = changes
                .iter()
                .zip(slots)
                .enumerate()
                .filter(|&(j, (_, s))| if j == i { !on } else { s.is_in() })
                .map(|(_, (c, _))| c);
            let Ok(resume) = self.apply(set) else {
                continue;
            };
            let rendered = renderer.render(&serde_json::to_string(&resume)?, false)?;
            compiles.points += 1;
            let other = self.score(&resume, &rendered, renderer)?.total;
            let gain = if on {
                projected - other
            } else {
                other - projected
            };
            out.insert(c.key.clone(), (gain * 10.0).round() / 10.0);
        }
        Ok(out)
    }
}

fn in_changes<'c>(changes: &'c [Change], slots: &[Slot]) -> impl Iterator<Item = &'c Change> {
    changes
        .iter()
        .zip(slots)
        .filter(|(_, s)| s.is_in())
        .map(|(c, _)| c)
}

/// The change the page loop leaves out next: the lowest-priority movable one of the first tier that has any (the
/// later one among equals).
fn next_out(changes: &[Change], slots: &[Slot], metrics: &Metrics) -> Option<usize> {
    let mut best: Option<(u8, u8, usize)> = None;
    for (i, (c, s)) in changes.iter().zip(slots).enumerate() {
        if !s.is_in() || s.immune {
            continue;
        }
        let Some(t) = tier(c, metrics) else {
            continue;
        };
        // Lower tier first, then the higher priority number (less important), then the later change.
        let rank = (t, c.priority, i);
        if best.is_none_or(|(bt, bp, _)| t < bt || (t == bt && c.priority >= bp)) {
            best = Some(rank);
        }
    }
    best.map(|(_, _, i)| i)
}

/// Which page-loop tier a change is in: 0 bank entry, 1 new bullet, 2 skill addition on a skills line that grew,
/// 3 summary; none for changes that don't add lines.
fn tier(c: &Change, metrics: &Metrics) -> Option<u8> {
    match &c.op {
        Op::Include { .. } | Op::SwapEntry { .. } => Some(0),
        Op::Add { .. } => Some(1),
        Op::SkillAdd { group, .. } => metrics
            .groups
            .iter()
            .any(|g| &g.id == group && g.lines > g.orig)
            .then_some(2),
        Op::Summary { .. } => Some(3),
        _ => None,
    }
}

struct GroupCheck {
    label: String,
    /// Candidate index of the master's line.
    orig: usize,
    /// Candidate index of the line with the first `k` movable additions, for each `k` from 0.
    prefixes: Vec<usize>,
    /// Movable additions, most important first.
    movable: Vec<usize>,
    /// Immune additions on the line.
    kept: Vec<usize>,
}

/// Candidates of one measure compile.
#[derive(Default)]
struct Measure(Vec<Candidate>);

impl Measure {
    /// Adds `c`; its index in the result.
    fn push(&mut self, mut c: Candidate) -> usize {
        c.id = self.0.len().to_string();
        self.0.push(c);
        self.0.len() - 1
    }

    fn run(&self, renderer: &mut Renderer) -> Result<Vec<u32>, RenderError> {
        Ok(renderer
            .measure(&self.0)?
            .into_iter()
            .map(|(_, n)| n)
            .collect())
    }
}

fn group_candidate(label: &str, text: String) -> Candidate {
    Candidate {
        id: String::new(),
        kind: LineKind::Group,
        depth: 0,
        text,
        label: label.to_owned(),
    }
}

fn find_group<'r>(r: &'r Resume, id: &str) -> Option<&'r resume_core::resume::Group> {
    r.sections
        .iter()
        .flat_map(|s| &s.groups)
        .find(|g| g.id == id)
}

/// Bullets and note lines `applied` marks changed (added bullets aside), as (new, master) measure candidates.
fn changed_lines(applied: &Resume, base: &Resume) -> Vec<(Candidate, Candidate)> {
    let mut out = Vec::new();
    for e in applied.sections.iter().flat_map(|s| &s.entries) {
        let depth = u32::from(!e.notes.is_empty());
        let bullet = |text: &str| Candidate {
            id: String::new(),
            kind: LineKind::Bullet,
            depth,
            text: text.to_owned(),
            label: String::new(),
        };
        for b in &e.bullets {
            if let (true, false, Some(orig)) = (b.changed, b.new, &b.orig) {
                out.push((bullet(&b.text), bullet(orig)));
            }
        }
        let master = base
            .sections
            .iter()
            .flat_map(|s| &s.entries)
            .find(|m| m.id == e.id);
        for (j, n) in e.notes.iter().enumerate() {
            let Some(old) = master.and_then(|m| m.notes.get(j)).filter(|_| n.changed) else {
                continue;
            };
            let note = |label: &str, text: &str| Candidate {
                id: String::new(),
                kind: LineKind::Note,
                depth: 0,
                text: text.to_owned(),
                label: label.to_owned(),
            };
            out.push((note(&n.label, &n.text), note(&old.label, &old.text)));
        }
    }
    out
}

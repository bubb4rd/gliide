//! The review as the UI draws it: changes grouped by section, each with the line it makes or one sentence, skill
//! chips, and scores with their components.

use std::collections::{BTreeMap, HashSet};

use resume_core::change::{Change, Decision as CoreDecision, Draft, Op, Origin, State};
use resume_core::matcher::{Page, SkillMatch, find_literal};
use resume_core::requirements::SkillKind;
use resume_core::resume::{Entry, Resume, SectionKind};
use resume_core::score::Score;
use resume_core::suggestions::Verdict as CoreVerdict;
use resume_core::validate::same_text;
use resume_render::LineCount;
use resume_tailor::Tailored;

use crate::dto::{
    AppStatus, ChangeGroup, ChangeState, ChangeView, ChipState, ComponentView, GapView, LineView,
    MasterChoice, ReviewView, ScoreView, SkillChip, Verdict,
};
use crate::jobs;
use crate::review::{Last, Review};

/// The review view of `r` after a rebuild: `changes` (with your wording and edits), `draft`, the fit check `t`, the
/// highlighted `pages`.
pub fn review(
    r: &Review,
    changes: &[Change],
    draft: &Draft,
    t: &Tailored,
    pages: Vec<String>,
    tailor_ms: u32,
    status: Option<AppStatus>,
) -> ReviewView {
    let skills = skill_facts(r, changes, draft, t)
        .into_iter()
        .map(|f| SkillChip {
            term: f.m.term.clone(),
            priority: f.m.priority,
            required: f.m.required,
            state: f.state,
            locked: f.locked,
        })
        .collect();
    let master = r
        .masters
        .iter()
        .find(|m| m.id == r.master.id)
        .cloned()
        .unwrap_or_else(|| MasterChoice {
            id: r.master.id.clone(),
            title: r.master.title.clone(),
            score: t.now.total,
        });
    ReviewView {
        id: r.id.clone(),
        job: jobs::row_of(&r.posting, Some(&r.req), Some(t.projected.total), status),
        head: jobs::head(&r.posting.posting, Some(&r.req)),
        verdict: match r.suggestions.verdict {
            CoreVerdict::Apply => Verdict::Apply,
            CoreVerdict::Tailor => Verdict::Tailor,
            CoreVerdict::Stretch => Verdict::Stretch,
            CoreVerdict::Skip => Verdict::Skip,
        },
        headline: r.suggestions.headline.clone(),
        master,
        masters: r.masters.clone(),
        mode: r.mode.into(),
        now: score_view(&t.now),
        projected: score_view(&t.projected),
        ceiling: score_view(&t.ceiling),
        skills,
        groups: groups(r, changes, draft, t),
        lines: lines(r, changes, t),
        gaps: r
            .suggestions
            .gaps
            .iter()
            .map(|g| GapView {
                skill: g.gap.clone(),
                fix: g.fix.clone(),
            })
            .collect(),
        knockouts: r.req.knockouts.iter().map(|k| k.text.clone()).collect(),
        pages,
        page_count: t.rendered.metrics.pages,
        page_limit: r.master.page_limit,
        can_undo: r.log.cursor > 0,
        can_redo: r.log.cursor < r.log.events.len(),
        tailor_ms,
    }
}

fn score_view(s: &Score) -> ScoreView {
    ScoreView {
        total: s.total,
        components: s
            .components
            .iter()
            .map(|c| ComponentView {
                id: c.id.to_owned(),
                label: match c.id {
                    "hard_tool" => "Hard skills and tools",
                    "domain_soft" => "Domain and soft",
                    "title" => "Job title",
                    "experience" => "Experience",
                    "education" => "Education",
                    "format" => "Format",
                    other => other,
                }
                .to_owned(),
                weight: c.weight,
                present: c.present,
                value: c.value,
                detail: c.detail.clone(),
            })
            .collect(),
    }
}

// ── Skills ──────────────────────────────────────────────────────────────────────────────────────────────────────

/// A requirement skill against the review: its chip, and the changes a click would act on.
pub struct SkillFact<'a> {
    /// On the projected page.
    pub m: &'a SkillMatch,
    pub kind: SkillKind,
    pub state: ChipState,
    pub locked: Option<String>,
    /// Changes that are in and put it on the page.
    pub adders: Vec<&'a Change>,
    /// A change that isn't in and would put it on the page (a skills-line addition first).
    pub suggested: Option<&'a Change>,
    /// Your removal of it from a skills line, in.
    pub removal: Option<&'a Change>,
    /// Suggested changes that are in and take it off the page (a skill removal, a cut, words cut).
    pub removers: Vec<&'a Change>,
    /// A skills-line item on the page that states it: `(group, item)`.
    pub item: Option<(String, String)>,
}

/// [`skill_facts`] from the last rebuild.
pub fn skills<'a>(r: &'a Review, last: &'a Last) -> Vec<SkillFact<'a>> {
    skill_facts(r, &last.changes, &last.draft, &last.t)
}

/// Every requirement skill in the posting's order: `have` on the master's page, `add` on the page only because a
/// change that is in adds it, `not-mine` when your toggle took it off (or turned off what would add it), else
/// `missing`.
pub fn skill_facts<'a>(
    r: &'a Review,
    changes: &'a [Change],
    draft: &Draft,
    t: &'a Tailored,
) -> Vec<SkillFact<'a>> {
    let prose = Page::of_resume(&without_skills(&t.resume));
    let is_in = |c: &Change| matches!(t.states.get(&c.key), Some(State::In | State::Forced { .. }));
    let custom = |c: &Change| draft.custom.iter().any(|x| x.key == c.key);
    t.projected
        .skills
        .iter()
        .map(|m| {
            let (aliases, kind) = r
                .req
                .skills
                .iter()
                .find(|k| k.term == m.term)
                .map_or((&[][..], SkillKind::Hard), |k| (&k.aliases[..], k.kind));
            let states =
                |text: &str| find_literal(&Page::of_text(text), &m.term, aliases).is_some();
            let adds = |c: &Change| {
                c.hits.iter().any(|h| same_term(h, &m.term))
                    || match &c.op {
                        Op::SkillAdd { item: text, .. }
                        | Op::SkillRename { to: text, .. }
                        | Op::NoteAdd { term: text, .. } => states(text),
                        _ => false,
                    }
            };
            let now = t
                .now
                .skills
                .iter()
                .find(|n| n.term == m.term)
                .is_some_and(|n| n.status.literal());
            let on = m.status.literal();
            let adders: Vec<&Change> = changes.iter().filter(|c| is_in(c) && adds(c)).collect();
            let mut not_in = changes.iter().filter(|c| !is_in(c) && adds(c));
            let suggested = not_in
                .clone()
                .find(|c| matches!(c.op, Op::SkillAdd { .. }))
                .or_else(|| not_in.next());
            let removal = changes.iter().find(|c| {
                is_in(c)
                    && custom(c)
                    && matches!(&c.op, Op::SkillRemove { item, .. } if states(item))
            });
            let removers = changes
                .iter()
                .filter(|c| {
                    is_in(c)
                        && c.origin == Origin::Suggested
                        && match &c.op {
                            Op::SkillRemove { item, .. } => states(item),
                            Op::Cut { .. } => states(&c.before),
                            Op::Swap { from, to, .. } => to.trim().is_empty() && states(from),
                            _ => false,
                        }
                })
                .collect();
            let item = t
                .resume
                .sections
                .iter()
                .flat_map(|s| &s.groups)
                .find_map(|g| {
                    g.items
                        .iter()
                        .find(|i| states(i))
                        .map(|i| (g.id.clone(), i.clone()))
                });
            let turned_off = changes
                .iter()
                .any(|c| adds(c) && draft.decisions.get(&c.key) == Some(&CoreDecision::Off));
            let state = match (on, now) {
                (true, true) => ChipState::Have,
                (true, false) => ChipState::Add,
                (false, true) => ChipState::NotMine,
                (false, false) if turned_off => ChipState::NotMine,
                (false, false) => ChipState::Missing,
            };
            let in_prose = find_literal(&prose, &m.term, aliases).is_some();
            let locked = match state {
                ChipState::Have if in_prose => {
                    Some("Stated in a bullet: edit the bullet".to_owned())
                }
                ChipState::Have if item.is_none() => {
                    Some("On your resume outside the skills lines".to_owned())
                }
                ChipState::Add if adders.is_empty() => {
                    Some("Added by another change: turn that off".to_owned())
                }
                _ => None,
            };
            SkillFact {
                m,
                kind,
                state,
                locked,
                adders,
                suggested,
                removal,
                removers,
                item,
            }
        })
        .collect()
}

/// Two requirement terms are the same: hits copy a requirement's term, case and spacing aside.
fn same_term(a: &str, b: &str) -> bool {
    a.trim().to_lowercase() == b.trim().to_lowercase()
}

/// `r` with its skills lines emptied: the prose a skill can be stated in.
fn without_skills(r: &Resume) -> Resume {
    let mut r = r.clone();
    for s in &mut r.sections {
        for g in &mut s.groups {
            g.items.clear();
        }
    }
    r
}

// ── Changes ─────────────────────────────────────────────────────────────────────────────────────────────────────

/// Changes by section, in the resume's section order (a new summary first), then your bank's.
fn groups(r: &Review, changes: &[Change], draft: &Draft, t: &Tailored) -> Vec<ChangeGroup> {
    let mut order: Vec<&str> = t.resume.sections.iter().map(|s| s.id.as_str()).collect();
    for s in &r.base.sections {
        if !order.contains(&s.id.as_str()) {
            order.push(&s.id);
        }
    }
    let mut by: BTreeMap<&str, Vec<ChangeView>> = BTreeMap::new();
    for c in changes {
        by.entry(c.section.as_str())
            .or_default()
            .push(change_view(r, c, draft, t));
    }
    let mut ids: Vec<&str> = order.into_iter().filter(|s| by.contains_key(s)).collect();
    for s in by.keys() {
        if !ids.contains(s) && *s != "bank" {
            ids.insert(usize::from(*s != "summary") * ids.len(), s);
        }
    }
    if by.contains_key("bank") {
        ids.push("bank");
    }
    ids.into_iter()
        .filter_map(|id| {
            let changes = by.remove(id)?;
            let points = changes
                .iter()
                .filter(|c| matches!(c.state, ChangeState::In | ChangeState::Forced))
                .filter_map(|c| c.points)
                .sum::<f64>();
            Some(ChangeGroup {
                id: id.to_owned(),
                label: section_label(r, t, id),
                // `+ 0.0` turns a rounded −0 into 0.
                points: (points * 10.0).round() / 10.0 + 0.0,
                changes,
            })
        })
        .collect()
}

fn section_label(r: &Review, t: &Tailored, id: &str) -> String {
    if id == "bank" {
        return "From your bank".to_owned();
    }
    let title = t
        .resume
        .sections
        .iter()
        .chain(&r.base.sections)
        .find(|s| s.id == id)
        .map_or(id, |s| s.title.as_str());
    sentence_case(title)
}

/// An entry as the picker and "Other lines" name it: `Name · subtitle`.
pub fn entry_label(e: &Entry) -> String {
    [e.name.as_str(), e.subtitle.as_str()]
        .into_iter()
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join(" · ")
}

/// The master's lines no change touches, in page order: the summary, then each entry's note lines and bullets, for
/// the entries on the page. A line a change targets (suggested or yours, in or off) is edited through its row.
fn lines(r: &Review, changes: &[Change], t: &Tailored) -> Vec<LineView> {
    let touched: HashSet<&str> = changes.iter().filter_map(|c| target_line(&c.op)).collect();
    let summary_touched = changes.iter().any(|c| matches!(c.op, Op::Summary { .. }));
    let shown: HashSet<&str> = t
        .resume
        .sections
        .iter()
        .flat_map(|s| &s.entries)
        .map(|e| e.id.as_str())
        .collect();
    let mut out = Vec::new();
    for s in &r.base.sections {
        // The summary is one line (`Op::Summary`), whatever entry holds it.
        if s.kind == Some(SectionKind::Summary) {
            let text = s.entries.first().and_then(|e| {
                e.notes
                    .first()
                    .map(|n| n.text.clone())
                    .or_else(|| e.bullets.first().map(|b| b.text.clone()))
            });
            if let Some(text) = text.filter(|t| !t.trim().is_empty())
                && !summary_touched
            {
                out.push(LineView {
                    line: None,
                    place: "Summary".to_owned(),
                    text,
                    rewritable: true,
                });
            }
            continue;
        }
        for e in s.entries.iter().filter(|e| shown.contains(e.id.as_str())) {
            let place = entry_label(e);
            for (i, n) in e.notes.iter().enumerate() {
                let id = format!("{}#{}", e.id, i + 1);
                if touched.contains(id.as_str()) || n.text.trim().is_empty() {
                    continue;
                }
                out.push(LineView {
                    line: Some(id),
                    place: if n.label.is_empty() {
                        place.clone()
                    } else {
                        format!("{place} · {}", n.label)
                    },
                    text: n.text.clone(),
                    rewritable: false,
                });
            }
            for b in e.bullets.iter().filter(|b| !touched.contains(b.id.as_str())) {
                out.push(LineView {
                    line: Some(b.id.clone()),
                    place: place.clone(),
                    text: b.text.clone(),
                    rewritable: true,
                });
            }
        }
    }
    out
}

fn change_view(r: &Review, c: &Change, draft: &Draft, t: &Tailored) -> ChangeView {
    let text = text_of(r, c, t);
    let page = Page::of_text(&text);
    let mut marks: Vec<String> = Vec::new();
    for h in &c.hits {
        let aliases = r
            .req
            .skills
            .iter()
            .find(|k| same_term(&k.term, h))
            .map_or(&[][..], |k| &k.aliases[..]);
        if let Some((_, hit)) = find_literal(&page, h, aliases)
            && !marks.contains(&hit.text)
        {
            marks.push(hit.text);
        }
    }
    let (state, mut reason, mut warning) = match t.states.get(&c.key) {
        Some(State::In) | None => (ChangeState::In, None, None),
        Some(State::Off) => (ChangeState::Off, None, None),
        Some(State::LeftOut { reason }) => (ChangeState::LeftOut, Some(reason.clone()), None),
        Some(State::Forced { warning }) => (ChangeState::Forced, None, warning.clone()),
    };
    if warning.is_none() {
        warning = t.warnings.get(&c.key).cloned();
    }
    if reason.as_deref() == Some("") {
        reason = None;
    }
    let is_in = matches!(state, ChangeState::In | ChangeState::Forced);
    let line_edit = target_line(&c.op).is_some() && !matches!(c.op, Op::Cut { .. });
    let was = (line_edit || matches!(c.op, Op::Summary { .. }))
        .then(|| c.before.clone())
        .filter(|b| !b.trim().is_empty());
    ChangeView {
        key: c.key.clone(),
        op: c.op.target().0.to_owned(),
        meta: meta(r, c, t, is_in),
        text,
        marks,
        was,
        state,
        reason,
        warning,
        points: t.points.get(&c.key).copied(),
        yours: c.origin == Origin::Yours,
        custom: draft.custom.iter().any(|x| x.key == c.key),
        editable: line_edit || matches!(c.op, Op::Add { .. } | Op::Summary { .. }),
        rewritable: rewritable(&c.op),
        savable: savable(r, c, t).is_some(),
        why: c.why.clone(),
    }
}

/// The line a line edit makes, a new bullet's or summary's text, else one sentence saying what the change does.
fn text_of(r: &Review, c: &Change, t: &Tailored) -> String {
    let group = |id: &str| group_label(r, id);
    match &c.op {
        Op::Rewrite { .. }
        | Op::UseVariant { .. }
        | Op::Set { .. }
        | Op::Swap { .. }
        | Op::NoteAdd { .. }
        | Op::Add { .. }
        | Op::Summary { .. } => line_text(c, t),
        Op::Cut { .. } => format!("Cut “{}”", c.before),
        Op::SkillAdd { group: g, item } => format!("Add {item} to {}", group(g)),
        Op::SkillRemove { group: g, item } => format!("Remove {item} from {}", group(g)),
        Op::SkillRename { group: g, from, to } => {
            format!("Rename {from} to {to} in {}", group(g))
        }
        Op::SkillMove { from, to, item } => {
            format!("Move {item} from {} to {}", group(from), group(to))
        }
        Op::Include { entry, .. } => format!("Include {}", entry_description(r, entry)),
        Op::Exclude { entry } => format!("Leave out {}", entry_description(r, entry)),
        Op::SwapEntry { out, entry, .. } => format!(
            "Show {} in place of {}",
            entry_description(r, entry),
            entry_name(r, out)
        ),
        Op::Reorder { section, entries } => format!(
            "Reorder {}: {}",
            section_label(r, t, section),
            entries
                .iter()
                .map(|e| entry_name(r, e))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

/// The text a line edit, new bullet or summary puts on the page (what Rewrite starts from).
pub fn line_text(c: &Change, t: &Tailored) -> String {
    match &c.op {
        Op::Add { text, .. } | Op::Summary { text } => text.clone(),
        _ => t
            .after
            .get(&c.key)
            .cloned()
            .unwrap_or_else(|| c.before.clone()),
    }
}

/// The bullet or note line a line edit or cut targets.
pub fn target_line(op: &Op) -> Option<&str> {
    match op {
        Op::Rewrite { bullet, .. } | Op::UseVariant { bullet, .. } | Op::Cut { bullet } => {
            Some(bullet)
        }
        Op::Set { line, .. } | Op::Swap { line, .. } => Some(line),
        Op::NoteAdd { note, .. } => Some(note),
        _ => None,
    }
}

/// Rewrite with a note applies: bullet edits (not note lines) and the summary.
pub fn rewritable(op: &Op) -> bool {
    match op {
        Op::Rewrite { .. } | Op::UseVariant { .. } | Op::Add { .. } | Op::Summary { .. } => true,
        Op::Set { line, .. } | Op::Swap { line, .. } => !line.contains('#'),
        _ => false,
    }
}

/// The bullet whose variants change `c`'s line can join: a bullet edit (a rewrite, your wording, a swap) that isn't
/// left out and reads unlike every wording the bullet has. Left-out lines aren't offered: a variant is a phrasing you
/// approved, and later rewrites may draw on its facts.
pub fn savable<'a>(r: &Review, c: &'a Change, t: &Tailored) -> Option<&'a str> {
    let bullet = match &c.op {
        Op::Rewrite { bullet, .. } => bullet,
        Op::Set { line, .. } | Op::Swap { line, .. } if !line.contains('#') => line,
        _ => return None,
    };
    if matches!(t.states.get(&c.key), Some(State::LeftOut { .. })) {
        return None;
    }
    let b = r
        .bank
        .sections
        .iter()
        .flat_map(|s| &s.entries)
        .flat_map(|e| &e.bullets)
        .find(|b| &b.id == bullet)?;
    let text = line_text(c, t);
    let known = std::iter::once(&b.text)
        .chain(&b.variants)
        .any(|w| same_text(w, &text));
    (!known && !text.trim().is_empty()).then_some(bullet.as_str())
}

/// `Rewrite · adds Python and SQL · same 2 lines`: what the op is, the terms it adds, and its lines when it's in.
fn meta(r: &Review, c: &Change, t: &Tailored, is_in: bool) -> String {
    let worded = r.changes.iter().any(|s| s.key == c.key);
    let label = match &c.op {
        Op::Rewrite { .. } => "Rewrite".to_owned(),
        Op::UseVariant { .. } => "Your variant".to_owned(),
        Op::Cut { .. } => "Cut bullet".to_owned(),
        Op::Add { entry, .. } => format!("New bullet · {}", entry_name(r, entry)),
        Op::SkillAdd { .. } => "Add skill".to_owned(),
        Op::SkillRemove { .. } => "Remove skill".to_owned(),
        Op::SkillRename { .. } => "Rename skill".to_owned(),
        Op::SkillMove { .. } => "Move skill".to_owned(),
        Op::NoteAdd { .. } => "Add to line".to_owned(),
        Op::Swap { to, .. } if to.trim().is_empty() => "Cut words".to_owned(),
        Op::Swap { .. } => "Reword".to_owned(),
        Op::Set { .. } if worded => "Your wording".to_owned(),
        Op::Set { .. } => "Your edit".to_owned(),
        Op::Include { bullets, .. } if !bullets.is_empty() => {
            format!("From your bank · {} bullets", bullets.len())
        }
        Op::Include { .. } => "From your bank".to_owned(),
        Op::Exclude { .. } => "Leave out".to_owned(),
        Op::SwapEntry { .. } => "Swap entry".to_owned(),
        Op::Reorder { .. } => "Reorder".to_owned(),
        Op::Summary { .. } => "Summary".to_owned(),
    };
    let mut parts = vec![label];
    if !c.hits.is_empty() {
        parts.push(format!("adds {}", and_list(&c.hits)));
    }
    if is_in && let Some(lc) = line_count(c, t) {
        let lines = |n: u32| if n == 1 { "line" } else { "lines" };
        parts.push(if lc.lines == lc.orig {
            format!("same {} {}", lc.lines, lines(lc.lines))
        } else {
            format!("takes {} {}, not {}", lc.lines, lines(lc.lines), lc.orig)
        });
    }
    parts.join(" · ")
}

/// The wrapped-line count of the bullet or skills line `c` edits, as laid out.
fn line_count<'a>(c: &Change, t: &'a Tailored) -> Option<&'a LineCount> {
    let m = &t.rendered.metrics;
    match &c.op {
        Op::SkillAdd { group, .. }
        | Op::SkillRemove { group, .. }
        | Op::SkillRename { group, .. } => m.groups.iter().find(|l| &l.id == group),
        Op::SkillMove { to, .. } => m.groups.iter().find(|l| &l.id == to),
        Op::Cut { .. } => None,
        op => {
            let id = target_line(op)?;
            m.bullets.iter().find(|l| l.id == id)
        }
    }
}

/// `A`, `A and B`, `A, B and C`.
fn and_list(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

fn group_label(r: &Review, id: &str) -> String {
    r.base
        .sections
        .iter()
        .flat_map(|s| &s.groups)
        .find(|g| g.id == id)
        .map(|g| g.label.clone())
        .or_else(|| {
            r.bank
                .sections
                .iter()
                .flat_map(|s| &s.groups)
                .find(|g| g.id == id)
                .map(|g| g.label.clone())
        })
        .unwrap_or_else(|| id.to_owned())
}

fn entry_name(r: &Review, id: &str) -> String {
    r.bank
        .sections
        .iter()
        .flat_map(|s| &s.entries)
        .find(|e| e.id == id)
        .map_or_else(|| id.to_owned(), |e| e.name.clone())
}

/// `Name, subtitle, dates`, leaving out what's empty.
fn entry_description(r: &Review, id: &str) -> String {
    let Some(e) = r
        .bank
        .sections
        .iter()
        .flat_map(|s| &s.entries)
        .find(|e| e.id == id)
    else {
        return id.to_owned();
    };
    [e.name.as_str(), e.subtitle.as_str(), e.dates.as_str()]
        .into_iter()
        .filter(|p| !p.trim().is_empty())
        .collect::<Vec<_>>()
        .join(", ")
}

/// `WORK EXPERIENCE` → `Work experience`; two-letter capitals other than small words (`IT`, `UX`) stay.
pub fn sentence_case(title: &str) -> String {
    const SMALL: [&str; 9] = ["OF", "IN", "TO", "AT", "ON", "OR", "BY", "AN", "AS"];
    let words: Vec<String> = title
        .split_whitespace()
        .enumerate()
        .map(|(i, w)| {
            let acronym = w.chars().filter(|c| c.is_alphabetic()).count() == 2
                && w.chars().all(|c| !c.is_lowercase())
                && !SMALL.contains(&w);
            let lower = if acronym {
                w.to_owned()
            } else {
                w.to_lowercase()
            };
            if i == 0 {
                let mut cs = lower.chars();
                cs.next()
                    .map(|c| c.to_uppercase().chain(cs).collect())
                    .unwrap_or_default()
            } else {
                lower
            }
        })
        .collect();
    words.join(" ")
}

#[cfg(test)]
mod tests {
    use super::{and_list, sentence_case};

    #[test]
    fn sentence_case_keeps_acronyms() {
        assert_eq!(sentence_case("WORK EXPERIENCE"), "Work experience");
        assert_eq!(sentence_case("IT EXPERIENCE"), "IT experience");
        assert_eq!(sentence_case("SKILLS IN TECH"), "Skills in tech");
        assert_eq!(sentence_case("Technical Skills"), "Technical skills");
    }

    #[test]
    fn lists_read_as_prose() {
        let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        assert_eq!(and_list(&s(&["A"])), "A");
        assert_eq!(and_list(&s(&["A", "B"])), "A and B");
        assert_eq!(and_list(&s(&["A", "B", "C"])), "A, B and C");
    }
}

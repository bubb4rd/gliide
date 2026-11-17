//! Suggest's bullet rewrites that wrap to more lines than the bullet they replace, asked once more to fit. The fit check
//! leaves a rewrite like that off the page, and telling the model the bullet's length doesn't stop it running over
//! (`ez eval`, 2026-11-17: Flash Low and gpt-6-luna dropped more rewrites with it, not fewer), so the line is measured
//! as the template wraps it and, when it runs long, rewritten "shorter" with Rewrite's prompt.
//!
//! The steps are apart so the app can measure on its warm renderer (sync) and call the model between (async):
//! [`long_rewrites`], then a [`rewrite::rewrite`] per [`Shorten::input`], then [`apply`].

use resume_core::Requirements;
use resume_core::bank::Bank;
use resume_core::matcher::Page;
use resume_core::resume::Resume;
use resume_core::suggestions::{BulletOp, Suggestions};
use resume_llm::rewrite::{self, RewriteInput};
use resume_render::{Candidate, LineKind, RenderError, Renderer};

/// A rewrite that runs long, and what asking it to fit takes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shorten {
    /// Its index in `Suggestions::bullets`.
    pub index: usize,
    /// The lines the bullet it replaces takes, which it must fit in.
    pub lines: u32,
    /// The bullet's indent steps, as the template lays its entry out.
    depth: u32,
    line: String,
    facts: String,
    terms: Vec<String>,
    note: String,
    budget: usize,
}

impl Shorten {
    /// The Rewrite call's input.
    pub fn input(&self) -> RewriteInput<'_> {
        RewriteInput {
            line: &self.line,
            facts: &self.facts,
            terms: &self.terms,
            note: &self.note,
            budget: self.budget,
        }
    }
}

/// `s`'s bullet rewrites that wrap to more lines than the bullet they replace, as `renderer`'s template wraps them,
/// in one measure pass. A rewrite of a bullet `resume` doesn't show is left alone (the fit check says why).
pub fn long_rewrites(
    s: &Suggestions,
    resume: &Resume,
    bank: &Bank,
    req: &Requirements,
    renderer: &mut Renderer,
) -> Result<Vec<Shorten>, RenderError> {
    let rewrites: Vec<(usize, u32)> = s
        .bullets
        .iter()
        .enumerate()
        .filter(|(_, b)| b.op == BulletOp::Rewrite && !b.after.trim().is_empty())
        .filter_map(|(i, b)| depth_of(resume, &b.id).map(|d| (i, d)))
        .collect();
    let candidates: Vec<Candidate> = rewrites
        .iter()
        .flat_map(|&(i, depth)| {
            let b = &s.bullets[i];
            [bullet(depth, &b.before), bullet(depth, &b.after)]
        })
        .collect();
    let counts = renderer.measure(&candidates)?;
    let terms: Vec<String> = req.skills.iter().map(|k| k.term.clone()).collect();
    Ok(rewrites
        .iter()
        .zip(counts.chunks(2))
        .filter(|(_, c)| c[1].1 > c[0].1)
        .map(|(&(index, depth), c)| {
            let (lines, now) = (c[0].1, c[1].1);
            let b = &s.bullets[index];
            Shorten {
                index,
                lines,
                depth,
                line: b.after.clone(),
                facts: entry_holding(bank, &b.id)
                    .map(rewrite::entry_facts)
                    .unwrap_or_default(),
                terms: terms.clone(),
                note: format!(
                    "Shorter: it wraps to {now} lines and must fit in {lines}, as the line it replaces does. Keep its \
                     requirement terms and facts; cut words that carry neither."
                ),
                budget: rewrite::budget(&b.before),
            }
        })
        .collect())
}

/// Puts `text` in rewrite `job.index` when it fits `job.lines` as `renderer` wraps it: `after` becomes it, `hits` keep
/// the terms it still states, and it's no longer one of the bullet's own variants. False, and `s` unchanged, when it
/// still runs long (the fit check leaves it off the page as before).
pub fn apply(
    s: &mut Suggestions,
    job: &Shorten,
    text: &str,
    renderer: &mut Renderer,
) -> Result<bool, RenderError> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(false);
    }
    let lines = renderer.measure(&[bullet(job.depth, text)])?[0].1;
    if lines > job.lines {
        return Ok(false);
    }
    let b = &mut s.bullets[job.index];
    let page = Page::of_text(text);
    b.hits.retain(|h| page.find(h).is_some());
    b.after = text.to_owned();
    b.variant = None;
    Ok(true)
}

fn bullet(depth: u32, text: &str) -> Candidate {
    Candidate {
        id: String::new(),
        kind: LineKind::Bullet,
        depth,
        text: text.to_owned(),
        label: String::new(),
    }
}

/// The indent steps of bullet `id` on `resume` (1 under an entry with notes, as the template lays it out); `None` when
/// `resume` doesn't show it.
fn depth_of(resume: &Resume, id: &str) -> Option<u32> {
    resume
        .sections
        .iter()
        .flat_map(|s| &s.entries)
        .find(|e| e.bullets.iter().any(|b| b.id == id))
        .map(|e| u32::from(!e.notes.is_empty()))
}

/// The bank entry holding bullet `id`.
fn entry_holding<'a>(bank: &'a Bank, id: &str) -> Option<&'a resume_core::bank::BankEntry> {
    bank.sections
        .iter()
        .flat_map(|s| &s.entries)
        .find(|e| e.bullets.iter().any(|b| b.id == id))
}

#[cfg(test)]
mod tests {
    use resume_core::{Resume, migrate_v1, resolve};
    use resume_render::{FontOptions, Template};
    use serde_json::json;

    use super::*;

    fn fixtures() -> (Bank, Resume, Requirements) {
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures");
        let read = |p: &str| std::fs::read_to_string(format!("{root}/{p}")).unwrap();
        let v1: Resume = serde_json::from_str(&read("banks/jane-doe.v1.json")).unwrap();
        let m = migrate_v1(&v1);
        let resume = resolve(&m.bank, &m.master).unwrap();
        let req = serde_json::from_str(&read("requirements/gold/northwind-jr-swe.json")).unwrap();
        (m.bank, resume, req)
    }

    fn rewrite(id: &str, before: &str, after: &str, hits: &[&str]) -> serde_json::Value {
        json!({ "op": "rewrite", "id": id, "before": before, "after": after, "hits": hits,
                "sources": [id], "variant": 0, "priority": 1, "why": "terms" })
    }

    #[test]
    fn only_rewrites_that_wrap_longer_are_asked_and_a_fitting_reply_replaces_one() {
        let (bank, resume, req) = fixtures();
        let text = |id: &str| {
            let b = resume.sections.iter().flat_map(|s| &s.entries).flat_map(|e| &e.bullets);
            b.clone().find(|b| b.id == id).unwrap().text.clone()
        };
        let (a2, a3) = (text("acme.2"), text("acme.3"));
        let long = format!("{a2}, and wired every Docker build into the same pipeline so releases shipped weekly");
        let mut s: Suggestions = serde_json::from_value(json!({
            "verdict": "tailor", "headline": "", "evidence": [], "keywords": [], "skills": [], "bank": [],
            "order": [], "summary": null, "gaps": [],
            "bullets": [
                rewrite("acme.2", &a2, &long, &["GitHub Actions", "Docker"]),
                rewrite("acme.3", &a3, &a3.replace("React", "React.js"), &["React"]),
            ],
        }))
        .unwrap();
        let mut renderer = Renderer::new(&Template::classic_serif(), FontOptions::default());

        let jobs = long_rewrites(&s, &resume, &bank, &req, &mut renderer).unwrap();
        assert_eq!(jobs.len(), 1);
        let job = &jobs[0];
        assert_eq!((job.index, job.lines), (0, 1));
        assert_eq!(job.input().line, long);

        assert!(!apply(&mut s, job, &long, &mut renderer).unwrap());
        assert_eq!(s.bullets[0].after, long);

        let short = "Wrote GitHub Actions integration tests for the order API, catching 14 regressions";
        assert!(apply(&mut s, job, short, &mut renderer).unwrap());
        let b = &s.bullets[0];
        assert_eq!(b.after, short);
        assert_eq!(b.hits, ["GitHub Actions"]);
        assert_eq!(b.variant, None);
    }
}

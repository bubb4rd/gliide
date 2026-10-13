//! Best master for a posting (ARCHITECTURE.md §4): every candidate's literal score, best first.

use crate::requirements::Requirements;
use crate::resume::Resume;
use crate::score::{PageFacts, score};

/// Each candidate's [`score`] total against `req`, best first; equal totals keep the candidates' order.
pub fn rank<I: Clone>(
    candidates: &[(I, Resume, PageFacts)],
    req: &Requirements,
    today: (u32, u32),
) -> Vec<(I, f64)> {
    let mut out: Vec<(I, f64)> = candidates
        .iter()
        .map(|(id, r, facts)| (id.clone(), score(r, req, facts, today).total))
        .collect();
    out.sort_by(|a, b| b.1.total_cmp(&a.1));
    out
}

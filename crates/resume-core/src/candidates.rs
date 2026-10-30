//! Bank candidates (ARCHITECTURE.md §2, "Master only or the whole bank"): the bank entries a master doesn't show,
//! ranked by how much of the posting they state, computed in code before the Suggest call.

use std::collections::BTreeSet;

use crate::bank::{Bank, BankEntry};
use crate::master::Master;
use crate::matcher::{self, Page, Status};
use crate::requirements::Requirements;

/// Up to `n` ids of bank entries outside `master`, best first: each scored by the requirement skills its text states
/// (each skill's [`matcher::weight`], half for a near form only) plus 1 per job-title term. Entries that state none are
/// left out; equal scores keep the bank's order.
pub fn bank_candidates(bank: &Bank, master: &Master, req: &Requirements, n: usize) -> Vec<String> {
    let shown: BTreeSet<&str> = master
        .sections
        .iter()
        .flat_map(|s| &s.entries)
        .map(|e| e.id.as_str())
        .collect();
    let mut scored: Vec<(&str, f64)> = bank
        .sections
        .iter()
        .flat_map(|s| &s.entries)
        .filter(|e| !shown.contains(e.id.as_str()))
        .map(|e| (e.id.as_str(), overlap(e, req)))
        .filter(|&(_, w)| w > 0.0)
        .collect();
    scored.sort_by(|a, b| b.1.total_cmp(&a.1));
    scored
        .into_iter()
        .take(n)
        .map(|(id, _)| id.to_owned())
        .collect()
}

/// Weighted requirement-term overlap of entry `e`'s text with `req`.
fn overlap(e: &BankEntry, req: &Requirements) -> f64 {
    let page = Page::of_text(&entry_text(e));
    let skills: f64 = req
        .skills
        .iter()
        .map(|s| {
            let w = matcher::weight(s.priority, s.required);
            match matcher::find_requirement(&page, &s.term, &s.aliases).0 {
                Status::Exact | Status::Alias => w,
                Status::Near => w / 2.0,
                Status::Missing => 0.0,
            }
        })
        .sum();
    let titles = req
        .title_terms
        .iter()
        .filter(|t| matcher::find_literal(&page, t, &[]).is_some())
        .count() as f64;
    skills + titles
}

/// What an entry says: its name, subtitle, notes, bullets with their variants, the skills you tagged them with, and its
/// facts.
fn entry_text(e: &BankEntry) -> String {
    let mut lines = vec![e.name.clone(), e.subtitle.clone()];
    lines.extend(e.notes.iter().map(|n| format!("{}: {}", n.label, n.text)));
    for b in &e.bullets {
        lines.push(b.text.clone());
        lines.extend(b.variants.iter().cloned());
        lines.extend(b.skills.iter().cloned());
    }
    lines.extend(e.fact_lines().map(str::to_owned));
    lines.join("\n")
}

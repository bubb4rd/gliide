//! An entry's facts: what you've said about it that its lines don't print (ARCHITECTURE.md §2). A fact keeps its id
//! for good, so whatever cites one finds it after other facts go. A new fact takes the number after the highest the
//! entry holds, so only the most recently removed fact's id can come back.

use crate::bank::{Bank, BankEntry, Fact, FactSource};

/// A fact as the entry's editor sends it: `id` `None` for a new one, which gets an id and `new_source`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FactDraft {
    pub id: Option<String>,
    pub text: String,
    pub skills: Vec<String>,
    /// Where a fact came from; `None` for a new one.
    pub source: Option<FactSource>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FactError {
    #[error("Your bank has no entry `{0}`")]
    Entry(String),
    #[error("A fact needs its text")]
    Text,
    #[error("Two facts can't share the id `{0}`")]
    Duplicate(String),
}

/// Adds a fact to entry `entry` (`text` and each skill trimmed; empty skills dropped) and returns its id. A fact
/// already there with the same text (ignoring case and spacing) gains the skills instead, and its id is returned.
pub fn add_fact(
    bank: &mut Bank,
    entry: &str,
    text: &str,
    skills: &[String],
    source: FactSource,
) -> Result<String, FactError> {
    let text = clean(text).ok_or(FactError::Text)?;
    let e = entry_mut(bank, entry)?;
    let skills = clean_skills(skills);
    if let Some(f) = e.facts.iter_mut().find(|f| same(&f.text, &text)) {
        for s in skills {
            if !f.skills.iter().any(|x| same(x, &s)) {
                f.skills.push(s);
            }
        }
        return Ok(f.id.clone());
    }
    let id = next_id(e);
    e.facts.push(Fact {
        id: id.clone(),
        text,
        skills,
        source,
    });
    Ok(id)
}

/// Sets entry `entry`'s facts to `drafts`, in order: one with an id keeps it and its source (a fact taken back by
/// Undo comes back as it was); one without gets the next id and `new_source`. Refuses an empty text or a repeated id,
/// changing nothing.
pub fn set_facts(
    bank: &mut Bank,
    entry: &str,
    drafts: Vec<FactDraft>,
    new_source: FactSource,
) -> Result<(), FactError> {
    let e = entry_mut(bank, entry)?;
    let mut facts: Vec<Fact> = Vec::with_capacity(drafts.len());
    let mut fresh = Vec::new();
    for d in drafts {
        let text = clean(&d.text).ok_or(FactError::Text)?;
        let skills = clean_skills(&d.skills);
        match d.id {
            Some(id) => {
                if facts.iter().any(|f| f.id == id) {
                    return Err(FactError::Duplicate(id));
                }
                let source = d.source.unwrap_or_else(|| new_source.clone());
                facts.push(Fact {
                    id,
                    text,
                    skills,
                    source,
                });
            }
            None => {
                fresh.push(facts.len());
                facts.push(Fact {
                    id: String::new(),
                    text,
                    skills,
                    source: new_source.clone(),
                });
            }
        }
    }
    // New ids count on from every id the entry had or keeps, so none of the facts it holds is ever renumbered.
    let mut n = numbers(e)
        .chain(facts.iter().filter_map(|f| number(&e.id, &f.id)))
        .max()
        .unwrap_or(0);
    for i in fresh {
        n += 1;
        facts[i].id = format!("{}~{n}", e.id);
    }
    e.facts = facts;
    Ok(())
}

fn entry_mut<'a>(bank: &'a mut Bank, entry: &str) -> Result<&'a mut BankEntry, FactError> {
    bank.sections
        .iter_mut()
        .flat_map(|s| &mut s.entries)
        .find(|e| e.id == entry)
        .ok_or_else(|| FactError::Entry(entry.to_owned()))
}

/// `<entry>~<n>`, one past the highest number the entry's facts have.
fn next_id(e: &BankEntry) -> String {
    format!("{}~{}", e.id, numbers(e).max().unwrap_or(0) + 1)
}

fn numbers(e: &BankEntry) -> impl Iterator<Item = u32> + '_ {
    e.facts.iter().filter_map(|f| number(&e.id, &f.id))
}

fn number(entry: &str, id: &str) -> Option<u32> {
    id.strip_prefix(entry)?.strip_prefix('~')?.parse().ok()
}

fn clean(text: &str) -> Option<String> {
    let t = text.split_whitespace().collect::<Vec<_>>().join(" ");
    (!t.is_empty()).then_some(t)
}

fn clean_skills(skills: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for s in skills.iter().filter_map(|s| clean(s)) {
        if !out.iter().any(|x| same(x, &s)) {
            out.push(s);
        }
    }
    out
}

fn same(a: &str, b: &str) -> bool {
    clean(a).map(|a| a.to_lowercase()) == clean(b).map(|b| b.to_lowercase())
}

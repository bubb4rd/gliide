//! An entry's facts: its name, subtitle, place, dates and note lines, what heads it on the page (ARCHITECTURE.md §2).
//! Masters hold none of them, so every master that shows the entry prints the new ones.

use crate::bank::{Bank, Note};
use crate::dates::parse_range;

/// Everything that heads an entry, as printed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Facts {
    pub name: String,
    pub subtitle: String,
    pub place: String,
    pub dates: String,
    pub notes: Vec<Note>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FactsError {
    #[error("Your bank has no entry `{0}`")]
    Entry(String),
    #[error("An entry needs a name")]
    Name,
    #[error("A line under the entry needs its text")]
    Note,
}

/// Sets entry `entry`'s facts, each trimmed. New dates are read again into `start` and `end` (both null when they
/// can't be read); the same dates keep the months the entry has.
pub fn set_facts(bank: &mut Bank, entry: &str, facts: Facts) -> Result<(), FactsError> {
    let name = facts.name.trim();
    if name.is_empty() {
        return Err(FactsError::Name);
    }
    let notes = facts
        .notes
        .iter()
        .map(|n| match n.text.trim() {
            "" => Err(FactsError::Note),
            text => Ok(Note {
                label: n.label.trim().to_owned(),
                text: text.to_owned(),
            }),
        })
        .collect::<Result<Vec<_>, _>>()?;
    let e = bank
        .sections
        .iter_mut()
        .flat_map(|s| &mut s.entries)
        .find(|e| e.id == entry)
        .ok_or_else(|| FactsError::Entry(entry.to_owned()))?;
    let dates = facts.dates.trim();
    if dates != e.dates {
        (e.start, e.end) = parse_range(dates).unzip();
        e.dates = dates.to_owned();
    }
    e.name = name.to_owned();
    e.subtitle = facts.subtitle.trim().to_owned();
    e.place = facts.place.trim().to_owned();
    e.notes = notes;
    Ok(())
}

//! An entry's details: its name, subtitle, place, dates and note lines, what heads it on the page (ARCHITECTURE.md
//! §2). Masters hold none of them, so every master that shows the entry prints the new ones. A new entry starts as a
//! name alone, in no master.

use crate::bank::{Bank, BankEntry, Note, SectionKind};
use crate::dates::parse_range;
use crate::import::entry_id;

/// Everything that heads an entry, as printed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Details {
    pub name: String,
    pub subtitle: String,
    pub place: String,
    pub dates: String,
    pub notes: Vec<Note>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DetailsError {
    #[error("Your bank has no entry `{0}`")]
    Entry(String),
    #[error("An entry needs a name")]
    Name,
    #[error("A line under the entry needs its text")]
    Note,
    #[error("Your bank has no section `{0}` that holds entries")]
    Section(String),
}

/// Adds an entry named `name` (trimmed) at the end of bank section `section` (not a skills or summary section), with
/// nothing else yet: its id, a slug of the name no entry holds. No master shows it, so no resume does until a review
/// brings it in.
pub fn add_entry(bank: &mut Bank, section: &str, name: &str) -> Result<String, DetailsError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(DetailsError::Name);
    }
    let id = entry_id(bank, name);
    let s = bank
        .sections
        .iter_mut()
        .find(|s| s.id == section && !matches!(s.kind, SectionKind::Skills | SectionKind::Summary))
        .ok_or_else(|| DetailsError::Section(section.to_owned()))?;
    s.entries.push(BankEntry {
        id: id.clone(),
        name: name.to_owned(),
        url: String::new(),
        place: String::new(),
        dates: String::new(),
        start: None,
        end: None,
        subtitle: String::new(),
        tags: vec![],
        notes: vec![],
        bullets: vec![],
        facts: vec![],
    });
    Ok(id)
}

/// Sets entry `entry`'s details, each trimmed. New dates are read again into `start` and `end` (both null when they
/// can't be read); the same dates keep the months the entry has.
pub fn set_details(bank: &mut Bank, entry: &str, details: Details) -> Result<(), DetailsError> {
    let name = details.name.trim();
    if name.is_empty() {
        return Err(DetailsError::Name);
    }
    let notes = details
        .notes
        .iter()
        .map(|n| match n.text.trim() {
            "" => Err(DetailsError::Note),
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
        .ok_or_else(|| DetailsError::Entry(entry.to_owned()))?;
    let dates = details.dates.trim();
    if dates != e.dates {
        (e.start, e.end) = parse_range(dates).unzip();
        e.dates = dates.to_owned();
    }
    e.name = name.to_owned();
    e.subtitle = details.subtitle.trim().to_owned();
    e.place = details.place.trim().to_owned();
    e.notes = notes;
    Ok(())
}

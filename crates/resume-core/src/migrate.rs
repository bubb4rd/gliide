//! v1 → v2: a legacy v1 master JSON into a bank and an `swe` master that selects all of it.

use crate::bank::{
    BANK_SCHEMA, Bank, BankBullet, BankEntry, BankGroup, BankSection, Note, SectionKind,
};
use crate::dates::parse_range;
use crate::master::{Master, MasterBullet, MasterEntry, MasterGroup, MasterSection};
use crate::resume::Resume;

/// The result of [`migrate_v1`].
#[derive(Debug, Clone, PartialEq)]
pub struct Migrated {
    pub bank: Bank,
    pub master: Master,
    /// Entries whose printed dates couldn't be read; their `start`/`end` are null.
    pub warnings: Vec<String>,
}

/// The bank holding everything in `v1` (a legacy v1 master, whose shape [`Resume`] is), and the `swe` master selecting
/// every section, entry, bullet and skill in their original order, so `resolve(bank, master)` gives `v1` back. Build-only
/// fields (`orig`, `changed`, `added`, `new`) are dropped.
pub fn migrate_v1(v1: &Resume) -> Migrated {
    let mut warnings = Vec::new();
    let sections = v1
        .sections
        .iter()
        .map(|s| BankSection {
            id: s.id.clone(),
            kind: kind_of(&s.id, &s.title, !s.groups.is_empty()),
            title: s.title.clone(),
            entries: s
                .entries
                .iter()
                .map(|e| {
                    let (start, end) = match parse_range(&e.dates) {
                        Some((start, end)) => (Some(start), Some(end)),
                        None => {
                            if !e.dates.trim().is_empty() {
                                warnings.push(format!(
                                    "entry `{}`: can't read dates {:?}; start and end left null",
                                    e.id, e.dates
                                ));
                            }
                            (None, None)
                        }
                    };
                    BankEntry {
                        id: e.id.clone(),
                        name: e.name.clone(),
                        url: e.url.clone(),
                        place: e.place.clone(),
                        dates: e.dates.clone(),
                        start,
                        end,
                        subtitle: e.subtitle.clone(),
                        tags: vec![],
                        notes: e
                            .notes
                            .iter()
                            .map(|n| Note {
                                label: n.label.clone(),
                                text: n.text.clone(),
                            })
                            .collect(),
                        bullets: e
                            .bullets
                            .iter()
                            .map(|b| BankBullet {
                                id: b.id.clone(),
                                text: b.text.clone(),
                                variants: vec![],
                                skills: vec![],
                            })
                            .collect(),
                    }
                })
                .collect(),
            groups: s
                .groups
                .iter()
                .map(|g| BankGroup {
                    id: g.id.clone(),
                    label: g.label.clone(),
                    items: g.items.clone(),
                })
                .collect(),
        })
        .collect();
    let bank = Bank {
        schema: BANK_SCHEMA,
        name: v1.name.clone(),
        contact: v1.contact.clone(),
        sections,
    };
    let master = Master {
        id: "swe".into(),
        title: "Software Engineering".into(),
        template: "classic-serif".into(),
        page_limit: 1,
        sections: v1
            .sections
            .iter()
            .map(|s| MasterSection {
                id: s.id.clone(),
                title: None,
                entries: s
                    .entries
                    .iter()
                    .map(|e| MasterEntry {
                        id: e.id.clone(),
                        bullets: e
                            .bullets
                            .iter()
                            .map(|b| MasterBullet {
                                id: b.id.clone(),
                                variant: None,
                            })
                            .collect(),
                    })
                    .collect(),
                groups: s
                    .groups
                    .iter()
                    .map(|g| MasterGroup {
                        id: g.id.clone(),
                        items: g.items.clone(),
                    })
                    .collect(),
            })
            .collect(),
    };
    Migrated {
        bank,
        master,
        warnings,
    }
}

/// A v1 section's kind, from its id and heading; a section of skills lines is `skills`; otherwise `custom`.
fn kind_of(id: &str, title: &str, has_groups: bool) -> SectionKind {
    let words = format!("{id} {title}").to_lowercase();
    let has = |keys: &[&str]| keys.iter().any(|k| words.contains(k));
    if has(&["skill", "technolog", "tools"]) || has_groups {
        SectionKind::Skills
    } else if has(&["educat", "school", "academic"]) {
        SectionKind::Education
    } else if has(&["project"]) {
        SectionKind::Projects
    } else if has(&["certif", "licens"]) {
        SectionKind::Certifications
    } else if has(&["summary", "profile", "objective", "about"]) {
        SectionKind::Summary
    } else if has(&[
        "experience",
        "work",
        "employment",
        "career",
        "leadership",
        "intern",
    ]) {
        SectionKind::Experience
    } else {
        SectionKind::Custom
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds() {
        assert_eq!(
            kind_of("work", "WORK & LEADERSHIP EXPERIENCE", false),
            SectionKind::Experience
        );
        assert_eq!(
            kind_of("projects", "PROJECT EXPERIENCE", false),
            SectionKind::Projects
        );
        assert_eq!(
            kind_of("education", "EDUCATION", false),
            SectionKind::Education
        );
        assert_eq!(kind_of("skills", "SKILLS", true), SectionKind::Skills);
        assert_eq!(
            kind_of("certs", "CERTIFICATIONS", false),
            SectionKind::Certifications
        );
        assert_eq!(kind_of("misc", "VOLUNTEERING", false), SectionKind::Custom);
    }
}

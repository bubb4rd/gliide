//! Import: a resume read from a file, in the shape the import prompt writes (the v1 format, [`Resume`]), merged into
//! the bank, and the master that shows it as the file did (ARCHITECTURE.md §3, Import).
//!
//! [`plan`] reads the file's resume as a bank ([`migrate_v1`]: section kinds, months) and matches it against yours.
//! Each section goes into a bank section of its kind (among several, the one sharing the most entries, then the one
//! with the same heading; a `custom` section only into one with the same heading), else into a new section. An entry
//! with the same name (or one name starting the other: `State University` and `State University, College of
//! Engineering`) and the same first or last month as an entry of that section (an older resume says `Present`, a
//! school only its graduation) is offered as the same entry; its bullets match by text against that entry's bullets
//! and their variants. A skills line matches the line with the same label.
//!
//! [`merge`] adds what's new and changes nothing that exists: new sections and entries whole; a same entry's new
//! bullets (its name, dates, subtitle and notes stay yours), unless you keep it separate, which adds it as an entry of
//! its own; a same skills line's new items; the contact items you chose. Every master you have resolves as it did.
//! The new master selects what the file showed, in its order, under its headings.

use std::collections::{BTreeSet, HashSet};

use crate::apply::position_ci;
use crate::bank::{
    BANK_SCHEMA, Bank, BankBullet, BankEntry, BankGroup, BankSection, Contact, SectionKind,
};
use crate::master::{MasterBullet, MasterEntry, MasterGroup, MasterSection};
use crate::matcher::key;
use crate::migrate::migrate_v1;
use crate::resume::Resume;

/// How the file's resume goes into the bank, before you choose.
#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    /// The file's resume as a bank: kinds and months read, ids unique, bullets numbered `entry.1`, `entry.2`….
    pub incoming: Bank,
    /// One per section of `incoming`, in order.
    pub sections: Vec<SectionPlan>,
    /// The file's contact items your bank's contact line lacks. Offered, never added unasked: the contact line heads
    /// every resume.
    pub contact: Vec<Contact>,
    /// Entries whose printed dates couldn't be read; their months stay null.
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SectionPlan {
    /// The section's id in [`Plan::incoming`].
    pub id: String,
    /// The bank section it goes into; `None` for a new section.
    pub into: Option<String>,
    /// One per entry of the section, in order.
    pub entries: Vec<EntryPlan>,
    /// One per skills line of the section, in order.
    pub groups: Vec<GroupPlan>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EntryPlan {
    /// The entry's id in [`Plan::incoming`].
    pub id: String,
    /// The bank entry it's the same as, when there is one.
    pub same: Option<Same>,
}

/// A bank entry the file shows too.
#[derive(Debug, Clone, PartialEq)]
pub struct Same {
    /// The bank entry's id.
    pub entry: String,
    /// Per bullet of the file's entry, in order: the bank bullet with that text (with the variant, when the text is
    /// one of its variants), or `None` for a new bullet.
    pub bullets: Vec<Option<MasterBullet>>,
    /// Where the file prints something else than your bank; the bank's stay.
    pub differs: Vec<Field>,
}

/// An entry field the file can print differently from the bank.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Subtitle,
    Place,
    Dates,
    Link,
    Notes,
}

impl Field {
    /// `subtitle`, `place`, `dates`, `link` or `notes`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Subtitle => "subtitle",
            Self::Place => "place",
            Self::Dates => "dates",
            Self::Link => "link",
            Self::Notes => "notes",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct GroupPlan {
    /// The skills line's id in [`Plan::incoming`].
    pub id: String,
    /// The bank skills line with the same label, when there is one.
    pub same: Option<String>,
    /// Items that line lacks, in the file's order; every item for a new line.
    pub new_items: Vec<String>,
}

/// What you decided on the plan's offers.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Choices {
    /// Entries of [`Plan::incoming`] with a same entry that go in as entries of their own instead.
    pub separate: BTreeSet<String>,
    /// Indexes into [`Plan::contact`] to add to the bank's contact line.
    pub contact: BTreeSet<usize>,
}

/// The bank with the import in it, and the new master's sections over it.
#[derive(Debug, Clone, PartialEq)]
pub struct Merged {
    pub bank: Bank,
    pub sections: Vec<MasterSection>,
}

/// How `imported` goes into `bank` (`None` before the first import: everything is new).
pub fn plan(bank: Option<&Bank>, imported: &Resume) -> Plan {
    let migrated = migrate_v1(imported);
    let incoming = unique_ids(migrated.bank);
    let mut claimed: HashSet<&str> = HashSet::new();
    let mut sections = Vec::with_capacity(incoming.sections.len());
    for s in &incoming.sections {
        let into = bank.and_then(|b| target(b, s));
        let mut entries = Vec::with_capacity(s.entries.len());
        for e in &s.entries {
            let same = into
                .and_then(|t| {
                    t.entries
                        .iter()
                        .find(|b| !claimed.contains(b.id.as_str()) && same_entry(b, e))
                })
                .map(|b| {
                    claimed.insert(&b.id);
                    Same {
                        entry: b.id.clone(),
                        bullets: e.bullets.iter().map(|x| same_bullet(b, &x.text)).collect(),
                        differs: differs(b, e),
                    }
                });
            entries.push(EntryPlan {
                id: e.id.clone(),
                same,
            });
        }
        let groups = s
            .groups
            .iter()
            .map(|g| {
                let same = into.and_then(|t| t.groups.iter().find(|b| same_label(b, g)));
                let have: &[String] = same.map_or(&[], |b| &b.items);
                let mut new_items: Vec<String> = Vec::new();
                for item in &g.items {
                    if position_ci(have, item).is_none() && position_ci(&new_items, item).is_none()
                    {
                        new_items.push(item.clone());
                    }
                }
                GroupPlan {
                    id: g.id.clone(),
                    same: same.map(|b| b.id.clone()),
                    new_items,
                }
            })
            .collect();
        sections.push(SectionPlan {
            id: s.id.clone(),
            into: into.map(|t| t.id.clone()),
            entries,
            groups,
        });
    }
    let contact = match bank {
        Some(b) => incoming
            .contact
            .iter()
            .filter(|c| !b.contact.iter().any(|have| same_contact(have, c)))
            .cloned()
            .collect(),
        None => vec![],
    };
    Plan {
        incoming,
        sections,
        contact,
        warnings: migrated.warnings,
    }
}

/// `bank` with `plan`'s import in it as `choices` say, and the master sections showing the file. `bank` must be the
/// bank `plan` was made against; an entry or section it no longer has goes in as new.
pub fn merge(bank: Option<&Bank>, plan: &Plan, choices: &Choices) -> Merged {
    let mut out = bank.cloned().unwrap_or_else(|| Bank {
        schema: BANK_SCHEMA,
        name: plan.incoming.name.clone(),
        contact: plan.incoming.contact.clone(),
        sections: vec![],
    });
    out.contact.extend(
        choices
            .contact
            .iter()
            .filter_map(|&i| plan.contact.get(i))
            .cloned(),
    );
    let mut ids = Ids::of(&out);
    let mut sections = Vec::with_capacity(plan.sections.len());
    for (sp, s) in plan.sections.iter().zip(&plan.incoming.sections) {
        let at = match sp
            .into
            .as_ref()
            .and_then(|id| out.sections.iter().position(|b| &b.id == id))
        {
            Some(at) => at,
            None => {
                out.sections.push(BankSection {
                    id: ids.section(&s.id),
                    entries: vec![],
                    groups: vec![],
                    ..s.clone()
                });
                out.sections.len() - 1
            }
        };
        let section = &mut out.sections[at];
        let mut entries = Vec::with_capacity(s.entries.len());
        for (ep, e) in sp.entries.iter().zip(&s.entries) {
            let same = ep
                .same
                .as_ref()
                .filter(|_| !choices.separate.contains(&ep.id))
                .and_then(|same| {
                    let b = section.entries.iter_mut().find(|b| b.id == same.entry)?;
                    Some((same, b))
                });
            entries.push(match same {
                Some((same, b)) => {
                    let bullets = e
                        .bullets
                        .iter()
                        .zip(&same.bullets)
                        .map(|(x, m)| match m {
                            Some(m) => m.clone(),
                            None => {
                                let id = ids.bullet(&b.id);
                                b.bullets.push(BankBullet {
                                    id: id.clone(),
                                    text: x.text.clone(),
                                    variants: vec![],
                                    skills: vec![],
                                });
                                MasterBullet { id, variant: None }
                            }
                        })
                        .collect();
                    MasterEntry {
                        id: b.id.clone(),
                        bullets,
                    }
                }
                None => {
                    let id = ids.entry(&e.id);
                    let bullets: Vec<BankBullet> = e
                        .bullets
                        .iter()
                        .map(|x| BankBullet {
                            id: ids.bullet(&id),
                            ..x.clone()
                        })
                        .collect();
                    let shown = bullets
                        .iter()
                        .map(|b| MasterBullet {
                            id: b.id.clone(),
                            variant: None,
                        })
                        .collect();
                    section.entries.push(BankEntry {
                        id: id.clone(),
                        bullets,
                        ..e.clone()
                    });
                    MasterEntry { id, bullets: shown }
                }
            });
        }
        let mut groups = Vec::with_capacity(s.groups.len());
        for (gp, g) in sp.groups.iter().zip(&s.groups) {
            let line = match gp
                .same
                .as_ref()
                .and_then(|id| section.groups.iter().position(|b| &b.id == id))
            {
                Some(i) => {
                    let line = &mut section.groups[i];
                    for item in &gp.new_items {
                        if position_ci(&line.items, item).is_none() {
                            line.items.push(item.clone());
                        }
                    }
                    line
                }
                None => {
                    let mut items: Vec<String> = Vec::with_capacity(g.items.len());
                    for item in &g.items {
                        if position_ci(&items, item).is_none() {
                            items.push(item.clone());
                        }
                    }
                    section.groups.push(BankGroup {
                        id: ids.group(&g.id),
                        label: g.label.clone(),
                        items,
                    });
                    section.groups.last_mut().expect("just pushed")
                }
            };
            let mut items: Vec<String> = Vec::with_capacity(g.items.len());
            for item in &g.items {
                if let Some(i) = position_ci(&line.items, item)
                    && !items.contains(&line.items[i])
                {
                    items.push(line.items[i].clone());
                }
            }
            groups.push(MasterGroup {
                id: line.id.clone(),
                items,
            });
        }
        sections.push(MasterSection {
            id: section.id.clone(),
            title: (section.title != s.title).then(|| s.title.clone()),
            entries,
            groups,
        });
    }
    Merged {
        bank: out,
        sections,
    }
}

/// A master id for `title` (`Graphic Design` → `graphic-design`) that `taken` doesn't hold.
pub fn master_id(title: &str, taken: &[&str]) -> String {
    let base = slug(title, "master");
    unique(&base, |id| taken.contains(&id))
}

/// The bank section `s` goes into, if any.
fn target<'a>(bank: &'a Bank, s: &BankSection) -> Option<&'a BankSection> {
    let title = key(&s.title);
    // `max_by_key` keeps the last of equals; reversed, that's the first in the bank's order.
    bank.sections
        .iter()
        .rev()
        .filter(|b| b.kind == s.kind && (s.kind != SectionKind::Custom || key(&b.title) == title))
        .max_by_key(|b| {
            let shared = s
                .entries
                .iter()
                .filter(|e| b.entries.iter().any(|x| same_entry(x, e)))
                .count()
                + s.groups
                    .iter()
                    .filter(|g| b.groups.iter().any(|x| same_label(x, g)))
                    .count();
            (shared, key(&b.title) == title)
        })
}

/// The same name (or one starting the other) and the same first or last month (the same printed dates when either has
/// no months).
fn same_entry(bank: &BankEntry, file: &BankEntry) -> bool {
    let (a, b) = (key(&bank.name), key(&file.name));
    let names = !a.is_empty() && (a == b || starts_name(&a, &b) || starts_name(&b, &a));
    let dates = match (&bank.start, &file.start) {
        (Some(x), Some(y)) => x == y || bank.end == file.end,
        _ => key(&bank.dates) == key(&file.dates),
    };
    names && dates
}

/// `long` starts with `short`, ending at a word boundary.
fn starts_name(short: &str, long: &str) -> bool {
    long.strip_prefix(short)
        .is_some_and(|rest| rest.starts_with(|c: char| !c.is_alphanumeric()))
}

/// The bullet of `entry` whose text or a variant of it is `text`.
fn same_bullet(entry: &BankEntry, text: &str) -> Option<MasterBullet> {
    let k = key(text);
    entry.bullets.iter().find_map(|b| {
        if key(&b.text) == k {
            return Some(MasterBullet {
                id: b.id.clone(),
                variant: None,
            });
        }
        let i = b.variants.iter().position(|v| key(v) == k)?;
        Some(MasterBullet {
            id: b.id.clone(),
            variant: Some(i),
        })
    })
}

/// The fields the file prints, and prints otherwise than the bank.
fn differs(bank: &BankEntry, file: &BankEntry) -> Vec<Field> {
    let other = |a: &str, b: &str| !b.trim().is_empty() && key(a) != key(b);
    let line = |label: &str, text: &str| key(&format!("{label} {text}"));
    let notes: BTreeSet<String> = bank.notes.iter().map(|n| line(&n.label, &n.text)).collect();
    [
        (Field::Subtitle, other(&bank.subtitle, &file.subtitle)),
        (Field::Place, other(&bank.place, &file.place)),
        (Field::Dates, other(&bank.dates, &file.dates)),
        (
            Field::Link,
            !file.url.trim().is_empty() && url_key(&bank.url) != url_key(&file.url),
        ),
        (
            Field::Notes,
            file.notes
                .iter()
                .any(|n| !notes.contains(&line(&n.label, &n.text))),
        ),
    ]
    .into_iter()
    .filter_map(|(f, d)| d.then_some(f))
    .collect()
}

/// The same label, a trailing colon aside.
fn same_label(a: &BankGroup, b: &BankGroup) -> bool {
    let label = |g: &BankGroup| key(g.label.trim().trim_end_matches(':'));
    label(a) == label(b)
}

/// The same item of a contact line: the same text, link, or (for a phone number) digits.
fn same_contact(a: &Contact, b: &Contact) -> bool {
    let forms = |c: &Contact| -> Vec<String> {
        let mut out = vec![url_key(&c.text), url_key(&c.url)];
        let digits: String = c.text.chars().filter(char::is_ascii_digit).collect();
        if digits.len() >= 7 && !c.text.chars().any(char::is_alphabetic) {
            out.push(digits);
        }
        out.retain(|f| !f.is_empty());
        out
    };
    let a = forms(a);
    forms(b).iter().any(|f| a.contains(f))
}

/// A link or address as compared: lowercase, without its scheme, `www.` or trailing slash.
fn url_key(s: &str) -> String {
    let s = s.trim().to_lowercase();
    let s = ["https://", "http://", "mailto:"]
        .iter()
        .find_map(|p| s.strip_prefix(p))
        .unwrap_or(&s);
    let s = s.strip_prefix("www.").unwrap_or(s);
    s.trim_end_matches('/').to_owned()
}

/// `bank` with unique section, entry and skills line ids (an empty one from its title, name or label), and its
/// bullets numbered `entry.1`, `entry.2`… in order.
fn unique_ids(mut bank: Bank) -> Bank {
    let mut ids = Ids::default();
    for s in &mut bank.sections {
        s.id = ids.section(&slug_or(&s.id, &s.title, "section"));
        for e in &mut s.entries {
            e.id = ids.entry(&slug_or(&e.id, &e.name, "entry"));
            for (n, b) in e.bullets.iter_mut().enumerate() {
                b.id = format!("{}.{}", e.id, n + 1);
            }
        }
        for g in &mut s.groups {
            g.id = ids.group(&slug_or(&g.id, &g.label, "skills"));
        }
    }
    bank
}

/// `id` when it's a usable id, else a slug of `name`, else `fallback`.
fn slug_or(id: &str, name: &str, fallback: &str) -> String {
    let id = slug(id, "");
    if id.is_empty() {
        slug(name, fallback)
    } else {
        id
    }
}

/// Lowercase letters, digits and single dashes (`Tools & Technologies` → `tools-technologies`); `fallback` when
/// nothing is left.
fn slug(s: &str, fallback: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars().flat_map(char::to_lowercase) {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    let out = out.trim_end_matches('-');
    if out.is_empty() {
        fallback.to_owned()
    } else {
        out.to_owned()
    }
}

/// `base`, else `base-2`, `base-3`… the first that `taken` doesn't hold.
fn unique(base: &str, taken: impl Fn(&str) -> bool) -> String {
    if !taken(base) {
        return base.to_owned();
    }
    (2..)
        .map(|n| format!("{base}-{n}"))
        .find(|id| !taken(id))
        .expect("some suffix is free")
}

/// The ids a bank holds, so new ones don't repeat them. Entry, bullet and skills line ids are unique bank-wide.
#[derive(Default)]
struct Ids {
    sections: HashSet<String>,
    entries: HashSet<String>,
    bullets: HashSet<String>,
    groups: HashSet<String>,
}

impl Ids {
    fn of(bank: &Bank) -> Self {
        let mut ids = Self::default();
        for s in &bank.sections {
            ids.sections.insert(s.id.clone());
            for e in &s.entries {
                ids.entries.insert(e.id.clone());
                ids.bullets.extend(e.bullets.iter().map(|b| b.id.clone()));
            }
            ids.groups.extend(s.groups.iter().map(|g| g.id.clone()));
        }
        ids
    }

    fn section(&mut self, base: &str) -> String {
        Self::take(&mut self.sections, base)
    }

    fn entry(&mut self, base: &str) -> String {
        Self::take(&mut self.entries, base)
    }

    fn group(&mut self, base: &str) -> String {
        Self::take(&mut self.groups, base)
    }

    /// The next free `entry.n`, from 1.
    fn bullet(&mut self, entry: &str) -> String {
        let id = (1..)
            .map(|n| format!("{entry}.{n}"))
            .find(|id| !self.bullets.contains(id))
            .expect("some number is free");
        self.bullets.insert(id.clone());
        id
    }

    fn take(set: &mut HashSet<String>, base: &str) -> String {
        let id = unique(base, |id| set.contains(id));
        set.insert(id.clone());
        id
    }
}

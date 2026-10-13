//! `resolve(bank, master)`: the resume a master selects, in the shape templates read.

use crate::bank::{Bank, BankEntry, BankSection};
use crate::master::{Master, MasterEntry, MasterSection};
use crate::resume::{Bullet, Entry, Group, Note, Resume, Section};

/// A master selects something the bank doesn't have.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ResolveError {
    #[error("unknown section `{id}`")]
    Section { id: String },
    #[error("unknown entry `{id}` in section `{section}`")]
    Entry { section: String, id: String },
    #[error("unknown bullet `{id}` in entry `{entry}`")]
    Bullet { entry: String, id: String },
    #[error("bullet `{bullet}` has no variant {variant} (it has {count})")]
    Variant {
        bullet: String,
        variant: usize,
        count: usize,
    },
    #[error("unknown skills line `{id}` in section `{section}`")]
    Group { section: String, id: String },
    #[error("unknown skill `{item}` on skills line `{group}`")]
    Item { group: String, item: String },
}

/// The resume `master` selects from `bank`: its sections, entries, bullets (or the chosen variant) and skills, in the
/// master's order. Every id the master names must exist in the bank.
pub fn resolve(bank: &Bank, master: &Master) -> Result<Resume, ResolveError> {
    let sections = master
        .sections
        .iter()
        .map(|ms| {
            let bs = bank
                .sections
                .iter()
                .find(|s| s.id == ms.id)
                .ok_or_else(|| ResolveError::Section { id: ms.id.clone() })?;
            section(bs, ms)
        })
        .collect::<Result<_, _>>()?;
    Ok(Resume {
        name: bank.name.clone(),
        contact: bank.contact.clone(),
        sections,
    })
}

fn section(bs: &BankSection, ms: &MasterSection) -> Result<Section, ResolveError> {
    let entries =
        ms.entries
            .iter()
            .map(|me| {
                let be = bs.entries.iter().find(|e| e.id == me.id).ok_or_else(|| {
                    ResolveError::Entry {
                        section: bs.id.clone(),
                        id: me.id.clone(),
                    }
                })?;
                entry(be, me)
            })
            .collect::<Result<_, _>>()?;
    let groups =
        ms.groups
            .iter()
            .map(|mg| {
                let bg = bs.groups.iter().find(|g| g.id == mg.id).ok_or_else(|| {
                    ResolveError::Group {
                        section: bs.id.clone(),
                        id: mg.id.clone(),
                    }
                })?;
                if let Some(item) = mg.items.iter().find(|i| !bg.items.contains(i)) {
                    return Err(ResolveError::Item {
                        group: bg.id.clone(),
                        item: item.clone(),
                    });
                }
                Ok(Group {
                    id: bg.id.clone(),
                    label: bg.label.clone(),
                    items: mg.items.clone(),
                    orig: None,
                    added: vec![],
                })
            })
            .collect::<Result<_, _>>()?;
    Ok(Section {
        id: bs.id.clone(),
        title: ms.title.clone().unwrap_or_else(|| bs.title.clone()),
        entries,
        groups,
    })
}

fn entry(be: &BankEntry, me: &MasterEntry) -> Result<Entry, ResolveError> {
    let bullets =
        me.bullets
            .iter()
            .map(|mb| {
                let bb = be.bullets.iter().find(|b| b.id == mb.id).ok_or_else(|| {
                    ResolveError::Bullet {
                        entry: be.id.clone(),
                        id: mb.id.clone(),
                    }
                })?;
                let text = match mb.variant {
                    None => bb.text.clone(),
                    Some(v) => {
                        bb.variants
                            .get(v)
                            .cloned()
                            .ok_or_else(|| ResolveError::Variant {
                                bullet: bb.id.clone(),
                                variant: v,
                                count: bb.variants.len(),
                            })?
                    }
                };
                Ok(Bullet {
                    id: bb.id.clone(),
                    text,
                    orig: None,
                    changed: false,
                    new: false,
                })
            })
            .collect::<Result<_, _>>()?;
    Ok(Entry {
        id: be.id.clone(),
        name: be.name.clone(),
        url: be.url.clone(),
        place: be.place.clone(),
        dates: be.dates.clone(),
        subtitle: be.subtitle.clone(),
        notes: be
            .notes
            .iter()
            .map(|n| Note {
                label: n.label.clone(),
                text: n.text.clone(),
                changed: false,
            })
            .collect(),
        bullets,
    })
}

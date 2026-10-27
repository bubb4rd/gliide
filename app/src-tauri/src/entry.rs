//! A bank entry's editor (DESIGN.md, Experience bank): its details (`resume_core::details`), its facts
//! (`resume_core::facts`), its bullets with every wording and which master shows which, edits of the wordings
//! (`resume_core::variants`: every master keeps the words it showed unless the edit is about what it shows), and the
//! desk's preview: a master that shows the entry, or the entry alone in the default template.

use chrono::{SecondsFormat, Utc};
use resume_core::bank::{Fact, FactSource, Note};
use resume_core::details::{Details, set_details};
use resume_core::facts::{FactDraft, set_facts};
use resume_core::master::{MasterBullet, MasterEntry, MasterSection};
use resume_core::variants::{
    self, VariantError, add_variant, fold_into, make_main, remove_variant, set_wording,
};
use resume_core::{Bank, Master, Resume, resolve};

use crate::dto::{
    BulletView, EntryDetails, EntryEdit, EntryView, FactKind, FactLine, MasterRef, NoteLine,
    WordingView,
};
use crate::error::{AppError, Result};
use crate::state::{App, DEFAULT_TEMPLATE, lock};
use crate::view::sentence_case;

/// Entry `entry` with `preview` on the desk (a master that shows it; else the first that does).
pub fn entry_view(app: &App, entry: &str, preview: Option<&str>) -> Result<EntryView> {
    let (bank, masters) = bank_and_masters(app)?;
    view(app, &bank, &masters, entry, preview)
}

/// Applies `edit` to entry `entry`: a new bank revision, and one for each master whose wordings moved.
pub fn edit_entry(
    app: &App,
    entry: &str,
    edit: EntryEdit,
    preview: Option<&str>,
) -> Result<EntryView> {
    let (mut bank, mut masters) = bank_and_masters(app)?;
    let before = masters.clone();
    let owns = |bank: &Bank, bullet: &str| {
        find(bank, entry).is_some_and(|(_, e)| e.bullets.iter().any(|b| b.id == bullet))
    };
    let bullet = match &edit {
        EntryEdit::SetDetails { .. } | EntryEdit::SetFacts { .. } => None,
        EntryEdit::AddVariant { bullet, .. }
        | EntryEdit::SetWording { bullet, .. }
        | EntryEdit::RemoveVariant { bullet, .. }
        | EntryEdit::MakeMain { bullet, .. }
        | EntryEdit::Show { bullet, .. } => Some(bullet),
        EntryEdit::Fold { from, .. } => Some(from),
    };
    if bullet.is_some_and(|b| !owns(&bank, b)) {
        return Err(AppError::missing(
            "This bullet isn't in the entry any more; reopen it",
        ));
    }
    apply(&mut bank, &mut masters, entry, edit)?;
    {
        let store = lock(&app.store);
        store.put_bank(&bank)?;
        for (m, was) in masters.iter().zip(&before) {
            if m != was {
                store.put_master(m)?;
            }
        }
    }
    view(app, &bank, &masters, entry, preview)
}

fn apply(bank: &mut Bank, masters: &mut [Master], entry: &str, edit: EntryEdit) -> Result<()> {
    match edit {
        EntryEdit::SetDetails { details } => {
            return set_details(bank, entry, details.into())
                .map_err(|e| AppError::other(e.to_string()));
        }
        EntryEdit::SetFacts { facts } => {
            let drafts = facts.into_iter().map(draft).collect();
            let typed = FactSource {
                kind: resume_core::bank::FactKind::Edit,
                at: Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
                posting: None,
                company: None,
            };
            return set_facts(bank, entry, drafts, typed)
                .map_err(|e| AppError::other(e.to_string()));
        }
        EntryEdit::AddVariant { bullet, text } => add_variant(bank, &bullet, &text).map(drop),
        EntryEdit::SetWording {
            bullet,
            variant,
            text,
        } => set_wording(bank, &bullet, variant.map(|v| v as usize), &text),
        EntryEdit::RemoveVariant { bullet, variant } => {
            remove_variant(bank, masters, &bullet, variant as usize)
        }
        EntryEdit::MakeMain { bullet, variant } => {
            make_main(bank, masters, &bullet, variant as usize)
        }
        EntryEdit::Fold { from, into } => fold_into(bank, masters, &from, &into),
        EntryEdit::Show {
            master,
            bullet,
            variant,
        } => {
            let m = masters
                .iter_mut()
                .find(|m| m.id == master)
                .ok_or_else(|| AppError::missing("This master was deleted"))?;
            match variants::show(bank, m, &bullet, variant.map(|v| v as usize)) {
                Ok(true) => Ok(()),
                Ok(false) => {
                    return Err(AppError::other(format!(
                        "{} doesn't show this bullet",
                        m.title
                    )));
                }
                Err(e) => Err(e),
            }
        }
    }
    .map_err(variant_error)
}

pub fn variant_error(e: VariantError) -> AppError {
    AppError::other(e.to_string())
}

/// A fact as the editor shows it.
fn fact_line(f: &Fact) -> FactLine {
    FactLine {
        id: f.id.clone(),
        text: f.text.clone(),
        skills: f.skills.clone(),
        kind: match f.source.kind {
            resume_core::bank::FactKind::Review => FactKind::Review,
            resume_core::bank::FactKind::Edit => FactKind::Edit,
            resume_core::bank::FactKind::Practice => FactKind::Practice,
        },
        at: f.source.at.clone(),
        posting: f.source.posting.clone(),
        company: f.source.company.clone(),
    }
}

/// A fact as the editor sends it back: an empty `id` is a new fact.
fn draft(l: FactLine) -> FactDraft {
    let source = FactSource {
        kind: match l.kind {
            FactKind::Review => resume_core::bank::FactKind::Review,
            FactKind::Edit => resume_core::bank::FactKind::Edit,
            FactKind::Practice => resume_core::bank::FactKind::Practice,
        },
        at: l.at,
        posting: l.posting,
        company: l.company,
    };
    FactDraft {
        id: (!l.id.is_empty()).then_some(l.id),
        text: l.text,
        skills: l.skills,
        source: Some(source),
    }
}

impl From<EntryDetails> for Details {
    fn from(f: EntryDetails) -> Self {
        Self {
            name: f.name,
            subtitle: f.subtitle,
            place: f.place,
            dates: f.dates,
            notes: f
                .notes
                .into_iter()
                .map(|n| Note {
                    label: n.label,
                    text: n.text,
                })
                .collect(),
        }
    }
}

fn bank_and_masters(app: &App) -> Result<(Bank, Vec<Master>)> {
    let store = lock(&app.store);
    let (_, bank) = store.bank()?.ok_or_else(AppError::no_bank)?;
    let masters = store.masters()?.into_iter().map(|(_, m)| m).collect();
    Ok((bank, masters))
}

/// The bank entry `id` and its section's index.
fn find<'a>(bank: &'a Bank, id: &str) -> Option<(usize, &'a resume_core::bank::BankEntry)> {
    bank.sections
        .iter()
        .enumerate()
        .find_map(|(i, s)| s.entries.iter().find(|e| e.id == id).map(|e| (i, e)))
}

/// Every bullet `master` shows with the wording it shows.
fn shown(master: &Master) -> impl Iterator<Item = &MasterBullet> {
    master
        .sections
        .iter()
        .flat_map(|s| &s.entries)
        .flat_map(|e| &e.bullets)
}

fn view(
    app: &App,
    bank: &Bank,
    masters: &[Master],
    entry: &str,
    preview: Option<&str>,
) -> Result<EntryView> {
    let (section, e) = find(bank, entry)
        .ok_or_else(|| AppError::missing("This entry isn't in your bank any more"))?;
    let showing: Vec<&Master> = masters
        .iter()
        .filter(|m| {
            m.sections
                .iter()
                .any(|s| s.entries.iter().any(|x| x.id == entry))
        })
        .collect();
    let on_desk = preview
        .and_then(|id| showing.iter().find(|m| m.id == id))
        .or(showing.first())
        .copied();
    let bullets = e
        .bullets
        .iter()
        .map(|b| {
            let by = |w: Option<usize>| -> Vec<String> {
                masters
                    .iter()
                    .filter(|m| shown(m).any(|x| x.id == b.id && x.variant == w))
                    .map(|m| m.id.clone())
                    .collect()
            };
            let wordings = std::iter::once((None, &b.text))
                .chain(b.variants.iter().enumerate().map(|(i, v)| (Some(i), v)))
                .map(|(w, text)| WordingView {
                    variant: w.map(|i| i as u32),
                    text: text.clone(),
                    masters: by(w),
                })
                .collect();
            BulletView {
                id: b.id.clone(),
                wordings,
                skills: b.skills.clone(),
            }
        })
        .collect();
    let desk = match on_desk {
        Some(m) => m.clone(),
        None => Master {
            id: String::new(),
            title: String::new(),
            template: DEFAULT_TEMPLATE.to_owned(),
            page_limit: 1,
            sections: vec![MasterSection {
                id: bank.sections[section].id.clone(),
                title: None,
                entries: vec![MasterEntry {
                    id: e.id.clone(),
                    bullets: e
                        .bullets
                        .iter()
                        .map(|b| MasterBullet {
                            id: b.id.clone(),
                            variant: None,
                        })
                        .collect(),
                }],
                groups: vec![],
            }],
        },
    };
    let json = serde_json::to_string(&marked(resolve(bank, &desk)?, entry))?;
    let (pages, page_count) = app.with_renderer(&desk, |r| {
        let rendered = r.render(&json, true)?;
        Ok((rendered.svgs(), rendered.metrics.pages))
    })?;
    Ok(EntryView {
        id: e.id.clone(),
        section: sentence_case(&bank.sections[section].title),
        details: EntryDetails {
            name: e.name.clone(),
            subtitle: e.subtitle.clone(),
            place: e.place.clone(),
            dates: e.dates.clone(),
            notes: e
                .notes
                .iter()
                .map(|n| NoteLine {
                    label: n.label.clone(),
                    text: n.text.clone(),
                })
                .collect(),
        },
        facts: e.facts.iter().map(fact_line).collect(),
        bullets,
        masters: showing
            .iter()
            .map(|m| MasterRef {
                id: m.id.clone(),
                title: m.title.clone(),
            })
            .collect(),
        preview: on_desk.map(|m| m.id.clone()),
        pages,
        page_count,
        page_limit: desk.page_limit,
    })
}

/// `r` with entry `entry`'s bullets highlighted, so the desk shows where it is on the page.
fn marked(mut r: Resume, entry: &str) -> Resume {
    for e in r
        .sections
        .iter_mut()
        .flat_map(|s| &mut s.entries)
        .filter(|e| e.id == entry)
    {
        for b in &mut e.bullets {
            b.changed = true;
        }
    }
    r
}

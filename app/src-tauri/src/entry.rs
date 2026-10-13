//! A bank entry's editor (DESIGN.md, Experience bank): its bullets with every wording and which master shows which,
//! edits of the wordings (`resume_core::variants`: every master keeps the words it showed unless the edit is about
//! what it shows), and the desk's preview: a master that shows the entry, or the entry alone in the default template.

use resume_core::master::{MasterBullet, MasterEntry, MasterSection};
use resume_core::variants::{
    self, VariantError, add_variant, fold_into, make_main, remove_variant, set_wording,
};
use resume_core::{Bank, Master, Resume, resolve};

use crate::dto::{BulletView, EntryEdit, EntryView, MasterRef, WordingView};
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
        EntryEdit::AddVariant { bullet, .. }
        | EntryEdit::SetWording { bullet, .. }
        | EntryEdit::RemoveVariant { bullet, .. }
        | EntryEdit::MakeMain { bullet, .. }
        | EntryEdit::Show { bullet, .. } => bullet,
        EntryEdit::Fold { from, .. } => from,
    };
    if !owns(&bank, bullet) {
        return Err(AppError::missing(
            "This bullet isn't in the entry any more; reopen it",
        ));
    }
    match edit {
        EntryEdit::AddVariant { bullet, text } => add_variant(&mut bank, &bullet, &text).map(drop),
        EntryEdit::SetWording {
            bullet,
            variant,
            text,
        } => set_wording(&mut bank, &bullet, variant.map(|v| v as usize), &text),
        EntryEdit::RemoveVariant { bullet, variant } => {
            remove_variant(&mut bank, &mut masters, &bullet, variant as usize)
        }
        EntryEdit::MakeMain { bullet, variant } => {
            make_main(&mut bank, &mut masters, &bullet, variant as usize)
        }
        EntryEdit::Fold { from, into } => fold_into(&mut bank, &mut masters, &from, &into),
        EntryEdit::Show {
            master,
            bullet,
            variant,
        } => {
            let m = masters
                .iter_mut()
                .find(|m| m.id == master)
                .ok_or_else(|| AppError::missing("This master was deleted"))?;
            match variants::show(&bank, m, &bullet, variant.map(|v| v as usize)) {
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
    .map_err(variant_error)?;
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

pub fn variant_error(e: VariantError) -> AppError {
    AppError::other(e.to_string())
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
        name: e.name.clone(),
        subtitle: e.subtitle.clone(),
        place: e.place.clone(),
        dates: e.dates.clone(),
        notes: e
            .notes
            .iter()
            .map(|n| {
                if n.label.is_empty() {
                    n.text.clone()
                } else {
                    format!("{}: {}", n.label, n.text)
                }
            })
            .collect(),
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

//! The bank and masters: what the Library screens list, a master's template and page limit. Importing a resume into
//! the bank is `import`.

use resume_core::Master;
use resume_core::bank::SectionKind;
use resume_render::Template;

use crate::dto::{BankSummary, EntryRow, Library, MasterRow, SectionRow, TemplateRow};
use crate::error::{AppError, Result};
use crate::state::{App, DEFAULT_TEMPLATE, lock};
use crate::view::sentence_case;

/// The bank's summary and sections, every master with its page count, and the templates.
pub fn library(app: &App) -> Result<Library> {
    let templates = templates();
    let Some(now) = app.masters_now()? else {
        return Ok(Library {
            bank: None,
            masters: vec![],
            sections: vec![],
            templates,
        });
    };
    let (rev, bank) = &now.bank;
    let (bank_at, master_at) = {
        let store = lock(&app.store);
        let master_at = now
            .masters
            .iter()
            .map(|(_, m, _)| store.master_updated_at(&m.id))
            .collect::<std::result::Result<Vec<_>, _>>()?;
        (store.bank_updated_at()?, master_at)
    };
    let entries = bank.sections.iter().map(|s| s.entries.len()).sum::<usize>();
    let bullets = bank
        .sections
        .iter()
        .flat_map(|s| &s.entries)
        .map(|e| e.bullets.len())
        .sum::<usize>();
    let masters = now
        .masters
        .iter()
        .zip(master_at)
        .map(|((_, m, page), at)| MasterRow {
            id: m.id.clone(),
            title: m.title.clone(),
            template: m.template.clone(),
            page_limit: m.page_limit,
            entries: m.sections.iter().map(|s| s.entries.len()).sum::<usize>() as u32,
            pages: page.facts.pages,
            updated_at: at.unwrap_or_default(),
        })
        .collect();
    let shows = |id: &str| -> Vec<String> {
        now.masters
            .iter()
            .filter(|(_, m, _)| {
                m.sections.iter().any(|s| {
                    s.entries.iter().any(|e| e.id == id) || s.groups.iter().any(|g| g.id == id)
                })
            })
            .map(|(_, m, _)| m.id.clone())
            .collect()
    };
    let sections = bank
        .sections
        .iter()
        .map(|s| SectionRow {
            id: s.id.clone(),
            title: sentence_case(&s.title),
            kind: kind_name(s.kind).to_owned(),
            entries: s
                .entries
                .iter()
                .map(|e| EntryRow {
                    id: e.id.clone(),
                    name: e.name.clone(),
                    subtitle: e.subtitle.clone(),
                    dates: e.dates.clone(),
                    items: e.bullets.len() as u32,
                    tags: e.tags.clone(),
                    masters: shows(&e.id),
                })
                .chain(s.groups.iter().map(|g| EntryRow {
                    id: g.id.clone(),
                    name: g.label.clone(),
                    subtitle: String::new(),
                    dates: String::new(),
                    items: g.items.len() as u32,
                    tags: vec![],
                    masters: shows(&g.id),
                }))
                .collect(),
        })
        .collect();
    Ok(Library {
        bank: Some(BankSummary {
            name: bank.name.clone(),
            rev: rev.to_string(),
            entries: entries as u32,
            bullets: bullets as u32,
            updated_at: bank_at.unwrap_or_default(),
        }),
        masters,
        sections,
        templates,
    })
}

/// Sets `master`'s template and page limit (a new master revision).
pub fn set_master_options(
    app: &App,
    master: &str,
    template: &str,
    page_limit: u32,
) -> Result<Library> {
    if !templates().iter().any(|t| t.id == template) {
        return Err(AppError::other(format!("There's no template “{template}”")));
    }
    if !(1..=2).contains(&page_limit) {
        return Err(AppError::other("A resume takes 1 or 2 pages"));
    }
    {
        let store = lock(&app.store);
        let (_, old) = store
            .master(master)?
            .ok_or_else(|| AppError::missing("This master was deleted"))?;
        store.put_master(&Master {
            template: template.to_owned(),
            page_limit,
            ..old
        })?;
    }
    library(app)
}

/// The built-in templates, named for people: `classic-serif` → `Classic serif`.
fn templates() -> Vec<TemplateRow> {
    let t = Template::classic_serif();
    let words = t.manifest.name.replace('-', " ");
    let mut cs = words.chars();
    let name = cs
        .next()
        .map(|c| c.to_uppercase().chain(cs).collect())
        .unwrap_or_default();
    vec![TemplateRow {
        id: DEFAULT_TEMPLATE.to_owned(),
        name,
        ats_safe: t.manifest.ats_safe,
    }]
}

/// `experience`, `skills`…, as the bank's JSON names the kind.
fn kind_name(k: SectionKind) -> &'static str {
    match k {
        SectionKind::Summary => "summary",
        SectionKind::Experience => "experience",
        SectionKind::Projects => "projects",
        SectionKind::Education => "education",
        SectionKind::Skills => "skills",
        SectionKind::Certifications => "certifications",
        SectionKind::Custom => "custom",
    }
}

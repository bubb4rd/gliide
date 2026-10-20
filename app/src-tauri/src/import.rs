//! Importing a resume into the bank (ARCHITECTURE.md §3, Import): a PDF, DOCX, Markdown or text file copied by the
//! model (a v1 master JSON read as is), matched against the bank (`resume_core::import`), shown for review with
//! the master it makes on the desk, then merged. Nothing existing changes: new entries, bullets and skills are added,
//! and the master shows the file. The import waits in [`App::import`] until you add or discard it; each step plans it
//! against the bank as it is then.

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Arc;

use resume_core::import::{Choices, Merged, Plan, master_id, merge, plan};
use resume_core::{Bank, Master, Resume, resolve};
use resume_llm::import::{self, Link};
use resume_pipeline::{ResumeSource, read_resume};

use crate::dto::{
    ImportChoices, ImportContact, ImportEntry, ImportGroup, ImportSection, ImportView, Library,
    SameEntry, Stage,
};
use crate::error::{AppError, Result};
use crate::library::library;
use crate::state::{App, DEFAULT_TEMPLATE, blocking, lock, model_call};
use crate::view::sentence_case;

/// The import's job key: its `progress` events and `cancel` name it.
pub const JOB: &str = "import";

/// A file read and copied, waiting for review.
pub struct Pending {
    pub id: String,
    /// The file's name.
    pub file: String,
    pub resume: Resume,
    /// The PDF's page count, for the new master's page limit.
    pub pages: Option<usize>,
}

/// Reads the file at `path` (one model call unless it's a v1 master JSON) and holds it for review: the view
/// with nothing kept separate, no contact item added, and a new master.
pub async fn read_import(app: &Arc<App>, path: String) -> Result<ImportView> {
    let source = blocking(app, {
        let path = path.clone();
        move |_| read_resume(Path::new(&path)).map_err(|e| AppError::other(e.to_string()))
    })
    .await?;
    let (resume, pages) = match source {
        ResumeSource::AtsFit(r) => (r, None),
        ResumeSource::File(f) => {
            let llm = app.claude().await?;
            let model = app.settings().extract_model;
            let pages = f.pages;
            let m = model.clone();
            let (resume, out) = model_call(app, JOB, Stage::Importing, async move {
                let links: Vec<Link<'_>> = f
                    .links
                    .iter()
                    .map(|l| Link {
                        text: &l.text,
                        url: &l.url,
                    })
                    .collect();
                import::import(&llm, &import::schema(), &f.text, &links, &m).await
            })
            .await?;
            app.note_served(&model, &out);
            (resume, pages)
        }
    };
    let file = Path::new(&path)
        .file_name()
        .map_or_else(|| path.clone(), |n| n.to_string_lossy().into_owned());
    let id = uuid::Uuid::new_v4().to_string();
    *lock(&app.import) = Some(Pending {
        id: id.clone(),
        file,
        resume,
        pages,
    });
    blocking(app, move |app| {
        import_view(
            app,
            &ImportChoices {
                import: id,
                separate: vec![],
                contact: vec![],
                replace: None,
                title: String::new(),
            },
        )
    })
    .await
}

/// The waiting import as `choices` would add it: what goes where, and the master it makes, rendered with what's new
/// to the bank highlighted.
pub fn import_view(app: &App, choices: &ImportChoices) -> Result<ImportView> {
    let (file, resume, pages) = pending(app, &choices.import)?;
    let bank = lock(&app.store).bank()?.map(|(_, b)| b);
    let plan = plan(bank.as_ref(), &resume);
    let picked = picked(&plan, choices);
    let merged = merge(bank.as_ref(), &plan, &picked);
    let master = master(app, &merged, choices, pages)?;
    let json = serde_json::to_string(&marked(resolve(&merged.bank, &master)?, bank.as_ref()))?;
    let (svgs, page_count) = app.with_renderer(&master, |r| {
        let rendered = r.render(&json, true)?;
        Ok((rendered.svgs(), rendered.metrics.pages))
    })?;
    let mut v = view(&plan, &picked, bank.as_ref());
    v.id = choices.import.clone();
    v.file = file;
    v.pages = svgs;
    v.page_count = page_count;
    v.page_limit = master.page_limit;
    Ok(v)
}

/// Adds the waiting import as `choices` say: the bank with what's new (a new revision when anything is), and the
/// master (new, or a new revision of the one it replaces).
pub fn finish_import(app: &App, choices: &ImportChoices) -> Result<Library> {
    let (_, resume, pages) = pending(app, &choices.import)?;
    if choices.replace.is_none() && choices.title.trim().is_empty() {
        return Err(AppError::other("Name the new master first"));
    }
    let bank = lock(&app.store).bank()?.map(|(_, b)| b);
    let plan = plan(bank.as_ref(), &resume);
    let merged = merge(bank.as_ref(), &plan, &picked(&plan, choices));
    let master = master(app, &merged, choices, pages)?;
    resolve(&merged.bank, &master)?;
    {
        let store = lock(&app.store);
        store.put_bank(&merged.bank)?;
        store.put_master(&master)?;
    }
    lock(&app.import).take();
    library(app)
}

/// Drops the waiting import.
pub fn discard_import(app: &App) {
    lock(&app.import).take();
}

/// The waiting import `id`: its file name, resume and page count.
fn pending(app: &App, id: &str) -> Result<(String, Resume, Option<usize>)> {
    match lock(&app.import).as_ref() {
        Some(p) if p.id == id => Ok((p.file.clone(), p.resume.clone(), p.pages)),
        _ => Err(AppError::missing(
            "This import was added or replaced already; import the file again",
        )),
    }
}

/// `choices` as the core takes them: only entries the bank has a same entry for, only contact items offered.
fn picked(plan: &Plan, choices: &ImportChoices) -> Choices {
    let offered: BTreeSet<&str> = plan
        .sections
        .iter()
        .flat_map(|s| &s.entries)
        .filter(|e| e.same.is_some())
        .map(|e| e.id.as_str())
        .collect();
    Choices {
        separate: choices
            .separate
            .iter()
            .filter(|id| offered.contains(id.as_str()))
            .cloned()
            .collect(),
        contact: choices
            .contact
            .iter()
            .map(|&i| i as usize)
            .filter(|&i| i < plan.contact.len())
            .collect(),
    }
}

/// The master the import makes: a new one under the title you gave (the default template, the file's page count as
/// its page limit), or the one it replaces, keeping its title, template and page limit.
fn master(
    app: &App,
    merged: &Merged,
    choices: &ImportChoices,
    pages: Option<usize>,
) -> Result<Master> {
    let store = lock(&app.store);
    if let Some(id) = &choices.replace {
        let (_, old) = store
            .master(id)?
            .ok_or_else(|| AppError::missing("This master was deleted"))?;
        return Ok(Master {
            sections: merged.sections.clone(),
            ..old
        });
    }
    let masters = store.masters()?;
    let taken: Vec<&str> = masters.iter().map(|(_, m)| m.id.as_str()).collect();
    let title = choices.title.trim();
    Ok(Master {
        id: master_id(if title.is_empty() { "imported" } else { title }, &taken),
        title: title.to_owned(),
        template: DEFAULT_TEMPLATE.to_owned(),
        page_limit: pages.map_or(1, |n| n.clamp(1, 2) as u32),
        sections: merged.sections.clone(),
    })
}

/// `r` with what's new to `bank` highlighted, as the review highlights changes: the bullets and note lines of new
/// entries, new bullets, new skills. Nothing on a first import, where all of it is new.
fn marked(mut r: Resume, bank: Option<&Bank>) -> Resume {
    let Some(bank) = bank else {
        return r;
    };
    let entries = || bank.sections.iter().flat_map(|s| &s.entries);
    let had_entry: BTreeSet<&str> = entries().map(|e| e.id.as_str()).collect();
    let had_bullet: BTreeSet<&str> = entries()
        .flat_map(|e| &e.bullets)
        .map(|b| b.id.as_str())
        .collect();
    for s in &mut r.sections {
        for e in &mut s.entries {
            let new = !had_entry.contains(e.id.as_str());
            for b in &mut e.bullets {
                b.changed = new || !had_bullet.contains(b.id.as_str());
            }
            for n in &mut e.notes {
                n.changed = new;
            }
        }
        for g in &mut s.groups {
            let had = bank
                .sections
                .iter()
                .flat_map(|s| &s.groups)
                .find(|x| x.id == g.id);
            g.added = g
                .items
                .iter()
                .filter(|i| had.is_none_or(|x| !x.items.contains(i)))
                .cloned()
                .collect();
        }
    }
    r
}

/// What the review lists: each section of the file, where it goes, its entries and skills lines; the contact items
/// offered; dates that can't be read; counts. The caller fills in the id, file and pages.
fn view(plan: &Plan, picked: &Choices, bank: Option<&Bank>) -> ImportView {
    let entries = || {
        bank.into_iter()
            .flat_map(|b| &b.sections)
            .flat_map(|s| &s.entries)
    };
    let groups = || {
        bank.into_iter()
            .flat_map(|b| &b.sections)
            .flat_map(|s| &s.groups)
    };
    let (mut new_entries, mut same_entries, mut new_bullets, mut new_items) = (0, 0, 0, 0);
    let mut warnings = Vec::new();
    let mut sections = Vec::with_capacity(plan.sections.len());
    for (sp, s) in plan.sections.iter().zip(&plan.incoming.sections) {
        let mut rows = Vec::with_capacity(s.entries.len());
        for (ep, e) in sp.entries.iter().zip(&s.entries) {
            if e.start.is_none() && !e.dates.trim().is_empty() {
                warnings.push(format!(
                    "Can’t read the dates “{}” of {}: your years of experience won’t count it",
                    e.dates.trim(),
                    e.name
                ));
            }
            let separate = picked.separate.contains(&ep.id);
            let same = ep.same.as_ref().and_then(|same| {
                let b = entries().find(|b| b.id == same.entry)?;
                let new = same.bullets.iter().filter(|x| x.is_none()).count() as u32;
                Some(SameEntry {
                    name: b.name.clone(),
                    subtitle: b.subtitle.clone(),
                    dates: b.dates.clone(),
                    matched: same.bullets.len() as u32 - new,
                    new,
                    differs: same.differs.iter().map(|f| f.as_str().to_owned()).collect(),
                })
            });
            match &same {
                Some(same) if !separate => {
                    same_entries += 1;
                    new_bullets += same.new;
                }
                _ => {
                    new_entries += 1;
                    new_bullets += e.bullets.len() as u32;
                }
            }
            rows.push(ImportEntry {
                id: ep.id.clone(),
                name: e.name.clone(),
                subtitle: e.subtitle.clone(),
                dates: e.dates.clone(),
                bullets: e.bullets.len() as u32,
                same,
                separate,
            });
        }
        let lines = sp
            .groups
            .iter()
            .zip(&s.groups)
            .map(|(gp, g)| {
                new_items += gp.new_items.len() as u32;
                ImportGroup {
                    label: g.label.clone(),
                    items: g.items.len() as u32,
                    into: gp
                        .same
                        .as_deref()
                        .and_then(|id| groups().find(|b| b.id == id))
                        .map(|b| b.label.clone()),
                    new_items: gp.new_items.clone(),
                }
            })
            .collect();
        sections.push(ImportSection {
            title: sentence_case(&s.title),
            into: sp.into.as_deref().and_then(|id| {
                bank.and_then(|b| b.sections.iter().find(|x| x.id == id))
                    .map(|x| sentence_case(&x.title))
            }),
            entries: rows,
            groups: lines,
        });
    }
    ImportView {
        id: String::new(),
        file: String::new(),
        name: plan.incoming.name.clone(),
        first: bank.is_none(),
        sections,
        contact: plan
            .contact
            .iter()
            .enumerate()
            .map(|(i, c)| ImportContact {
                text: c.text.clone(),
                url: c.url.clone(),
                chosen: picked.contact.contains(&i),
            })
            .collect(),
        warnings,
        new_entries,
        same_entries,
        new_bullets,
        new_items,
        pages: vec![],
        page_count: 0,
        page_limit: 0,
    }
}

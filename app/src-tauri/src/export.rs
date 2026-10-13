//! Export and the applications log: the tailored PDF (no highlight) into the export folder, its text read back for
//! the on-file score, the posting's application, and `applications.csv` mirrored from the store.

use std::path::{Path, PathBuf};

use resume_core::{PageFacts, score};
use resume_pipeline::today;
use resume_store::{AppStatus, Application, applications_csv, parse_ats_fit_csv};

use crate::dto::{ApplicationRow, Exported};
use crate::error::{AppError, Result};
use crate::state::{App, lock};

/// The log mirrored for spreadsheets, in the export folder.
const CSV: &str = "applications.csv";
/// The header the app writes (`applications_csv`): ATS Fit's eight columns, then its own. A file with any other
/// header (ATS Fit's, or one edited in a spreadsheet) has its new rows imported before the app writes over it.
const APP_COLUMNS: [&str; 10] = [
    "date",
    "company",
    "role",
    "fit_planned",
    "fit_on_page",
    "resume",
    "cover_letter",
    "posting",
    "status",
    "fit_now",
];

/// Saves the open review's tailored resume as a PDF in the export folder, reads its text back for the on-file score,
/// records the posting's application and rewrites `applications.csv`. Refuses a resume over its page limit.
pub fn export_pdf(app: &App, review: &str) -> Result<Exported> {
    let slot = lock(&app.review);
    let r = slot
        .as_ref()
        .filter(|r| r.id == review)
        .ok_or_else(AppError::replaced)?;
    let last = r.last.as_ref().ok_or_else(AppError::replaced)?;
    let t = &last.t;
    let (pages, limit) = (t.rendered.metrics.pages, r.master.page_limit);
    if pages > limit {
        return Err(AppError::other(format!(
            "Over {limit} page{}: turn something off first",
            if limit == 1 { "" } else { "s" }
        )));
    }
    let dir = PathBuf::from(app.settings().export_dir);
    std::fs::create_dir_all(&dir)?;
    // Before the PDF: an unreadable log stops the export rather than being overwritten after it.
    import_applications_csv(app)?;

    let company = non_empty(&r.req.company)
        .or(r.posting.posting.company.as_deref())
        .unwrap_or("company");
    let surname = r.bank.name.split_whitespace().last().unwrap_or("resume");
    let stem = format!(
        "{}_{}_{}_resume",
        slug(company),
        slug(non_empty(&r.req.short_role).unwrap_or("role")),
        slug(surname)
    );
    let path = free_path(&dir, &stem);
    std::fs::write(&path, t.rendered.pdf()?)?;
    let ats_safe = app.with_renderer(&r.master, |renderer| Ok(renderer.manifest().ats_safe))?;
    let on_file = pdf_text(&path).map(|text| {
        let facts = PageFacts {
            pages,
            page_limit: limit,
            ats_safe,
            pdf_text: Some(text),
        };
        score(&t.resume, &r.req, &facts, today()).total
    });

    let store = lock(&app.store);
    let previous = store.application_for_posting(&r.posting_id)?;
    let a = Application {
        id: previous
            .as_ref()
            .map_or_else(|| uuid::Uuid::new_v4().to_string(), |a| a.id.clone()),
        posting_id: Some(r.posting_id.clone()),
        analysis_id: Some(r.id.clone()),
        date: previous.as_ref().map_or_else(
            || chrono::Local::now().format("%Y-%m-%d").to_string(),
            |a| a.date.clone(),
        ),
        company: non_empty(&r.req.company).unwrap_or(company).to_owned(),
        role: non_empty(&r.req.role)
            .or(r.posting.posting.title.as_deref())
            .unwrap_or_default()
            .to_owned(),
        url: r
            .posting
            .posting
            .url
            .clone()
            .or_else(|| r.posting.posting.source.clone()),
        status: previous.as_ref().map_or(AppStatus::Saved, |a| a.status),
        now: Some(t.now.total),
        projected: Some(t.projected.total),
        on_file,
        resume_path: Some(path.display().to_string()),
        cover_path: previous.as_ref().and_then(|a| a.cover_path.clone()),
        notes: previous
            .as_ref()
            .map(|a| a.notes.clone())
            .unwrap_or_default(),
        updated_at: String::new(),
    };
    store.put_application(&a)?;
    let saved = store.application_for_posting(&r.posting_id)?.unwrap_or(a);
    drop(store);
    write_csv(app)?;
    Ok(Exported {
        file_name: path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
        path: path.display().to_string(),
        on_file,
        application: row(&saved),
    })
}

/// Every application, newest first.
pub fn applications(app: &App) -> Result<Vec<ApplicationRow>> {
    Ok(lock(&app.store).applications()?.iter().map(row).collect())
}

/// Sets an application's status and rewrites `applications.csv`.
pub fn set_status(
    app: &App,
    application: &str,
    status: crate::dto::AppStatus,
) -> Result<ApplicationRow> {
    let a = lock(&app.store).set_application_status(application, status.into())?;
    write_csv(app)?;
    Ok(row(&a))
}

/// `applications.csv` in the export folder, from the store, after importing what the file holds that the store
/// doesn't.
fn write_csv(app: &App) -> Result<()> {
    let dir = PathBuf::from(app.settings().export_dir);
    std::fs::create_dir_all(&dir)?;
    import_applications_csv(app)?;
    let apps = lock(&app.store).applications()?;
    std::fs::write(dir.join(CSV), applications_csv(&apps))?;
    Ok(())
}

/// Imports the rows of `<export_dir>/applications.csv` the store doesn't have yet (same date, company, role and
/// resume file), unless the file carries the app's own header: ATS Fit's log, or one you edited, is never lost to the
/// app's next write. Runs before every write, so rows added there since are read too. A file that isn't UTF-8 is an
/// error: the caller must not write over it.
pub fn import_applications_csv(app: &App) -> Result<()> {
    let path = Path::new(&app.settings().export_dir).join(CSV);
    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(_) => return Err(unreadable()),
    };
    let text = String::from_utf8(bytes).map_err(|_| unreadable())?;
    if is_app_header(&text) {
        return Ok(());
    }
    let store = lock(&app.store);
    let have = store.applications()?;
    let same = |a: &Application, b: &Application| {
        a.date == b.date
            && a.company == b.company
            && a.role == b.role
            && a.resume_path == b.resume_path
    };
    let new: Vec<Application> = parse_ats_fit_csv(&text)
        .into_iter()
        .filter(|a| !have.iter().any(|h| same(h, a)))
        .collect();
    if !new.is_empty() {
        store.import_applications(&new)?;
    }
    Ok(())
}

fn unreadable() -> AppError {
    AppError::new(
        crate::dto::ErrorKind::Storage,
        "applications.csv isn't readable; it was left as is",
    )
}

/// The file's header is the app's own, read as `parse_ats_fit_csv` reads one: BOM stripped, cells trimmed, case
/// aside.
fn is_app_header(text: &str) -> bool {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let first = text.lines().next().unwrap_or("");
    let cells: Vec<String> = first.split(',').map(|c| c.trim().to_lowercase()).collect();
    cells == APP_COLUMNS
}

pub fn row(a: &Application) -> ApplicationRow {
    ApplicationRow {
        id: a.id.clone(),
        job: a.posting_id.clone(),
        date: a.date.clone(),
        company: a.company.clone(),
        role: a.role.clone(),
        now: a.now,
        projected: a.projected,
        on_file: a.on_file,
        resume: a.resume_path.clone(),
        cover_letter: a.cover_path.clone(),
        url: a.url.clone(),
        status: a.status.into(),
    }
}

fn non_empty(s: &str) -> Option<&str> {
    Some(s.trim()).filter(|s| !s.is_empty())
}

/// ATS Fit's file-name part: lowercase, runs of anything but letters and digits as one `_`, trimmed of `_`.
pub fn slug(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.to_lowercase().chars() {
        if c.is_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('_') {
            out.push('_');
        }
    }
    out.trim_matches('_').to_owned()
}

/// `<dir>/<stem>.pdf`, else `<stem>-2.pdf`, `-3`… the first not taken.
fn free_path(dir: &Path, stem: &str) -> PathBuf {
    let mut path = dir.join(format!("{stem}.pdf"));
    let mut n = 1;
    while path.exists() {
        n += 1;
        path = dir.join(format!("{stem}-{n}.pdf"));
    }
    path
}

/// The PDF's text layer as PDFKit reads it (what Preview copies and many ATS parsers see).
#[cfg(target_os = "macos")]
pub fn pdf_text(path: &Path) -> Option<String> {
    use objc2::AllocAnyThread;
    use objc2_foundation::{NSString, NSURL};
    use objc2_pdf_kit::PDFDocument;

    objc2::rc::autoreleasepool(|_| {
        let url = NSURL::fileURLWithPath(&NSString::from_str(path.to_str()?));
        // SAFETY: `url` is a valid file URL; PDFKit reads the file and returns nil when it isn't a PDF.
        let doc = unsafe { PDFDocument::initWithURL(PDFDocument::alloc(), &url) }?;
        // SAFETY: `doc` is a live PDFDocument.
        let text = unsafe { doc.string() }?.to_string();
        Some(text).filter(|t| !t.trim().is_empty())
    })
}

/// No PDF reader off macOS yet: the on-file score is unknown.
#[cfg(not(target_os = "macos"))]
pub fn pdf_text(_path: &Path) -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::{is_app_header, slug};

    #[test]
    fn only_the_apps_own_header_skips_the_import() {
        let app = "date,company,role,fit_planned,fit_on_page,resume,cover_letter,posting,status,fit_now\n";
        assert!(is_app_header(app));
        assert!(is_app_header(&format!("\u{feff}{}", app.to_uppercase())));
        assert!(!is_app_header(
            "date,company,role,fit_planned,fit_on_page,resume,cover_letter,posting\n"
        ));
        assert!(!is_app_header(&format!("{},notes\n", app.trim_end())));
    }

    #[test]
    fn slugs_like_ats_fit() {
        assert_eq!(slug("Globex"), "globex");
        assert_eq!(slug("  Jr. SDE / Platform "), "jr_sde_platform");
        assert_eq!(slug("AT&T"), "at_t");
    }
}

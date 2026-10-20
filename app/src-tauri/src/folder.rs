//! The export folder's PDFs (Settings → Export folder): listed for Resumes, and each one's pages as SVG for its desk.
//! Only files directly in the folder are read, named as the listing names them.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use chrono::{DateTime, Local};

use crate::dto::{FileKind, FolderFile, FolderPdf, FolderView};
use crate::error::{AppError, Result};
use crate::state::App;

/// The PDFs directly in the export folder, newest first (ties by name); none when the folder doesn't exist yet.
pub fn folder(app: &App) -> Result<FolderView> {
    let dir = PathBuf::from(app.settings().export_dir);
    let entries = match std::fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(FolderView {
                dir: dir.display().to_string(),
                files: Vec::new(),
            });
        }
        Err(e) => return Err(e.into()),
    };
    let mut files: Vec<(SystemTime, FolderFile)> = Vec::new();
    for entry in entries {
        let entry = entry?;
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        let meta = entry.metadata()?;
        // Hidden files include macOS's `._` AppleDouble companions, which aren't PDFs.
        if !meta.is_file() || name.starts_with('.') || !is_pdf(&name) {
            continue;
        }
        let modified = meta.modified()?;
        files.push((
            modified,
            FolderFile {
                path: entry.path().display().to_string(),
                modified_at: DateTime::<Local>::from(modified).to_rfc3339(),
                kind: kind(&name),
                name,
            },
        ));
    }
    files.sort_by(|(a, fa), (b, fb)| b.cmp(a).then_with(|| fa.name.cmp(&fb.name)));
    Ok(FolderView {
        dir: dir.display().to_string(),
        files: files.into_iter().map(|(_, f)| f).collect(),
    })
}

/// The export folder's PDF `name`, its pages as SVG.
pub fn folder_pdf(app: &App, name: &str) -> Result<FolderPdf> {
    // A bare file name from the listing: never a path that leaves the folder.
    if !is_pdf(name) || Path::new(name).file_name().and_then(|n| n.to_str()) != Some(name) {
        return Err(AppError::other(format!(
            "“{name}” isn’t a PDF in the export folder"
        )));
    }
    let path = Path::new(&app.settings().export_dir).join(name);
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(AppError::missing(format!(
                "{name} isn’t in the export folder anymore"
            )));
        }
        Err(e) => return Err(e.into()),
    };
    let pages = resume_render::pdf_svgs(bytes)
        .filter(|pages| !pages.is_empty())
        .ok_or_else(|| AppError::other(format!("{name} couldn’t be read as a PDF")))?;
    Ok(FolderPdf {
        name: name.to_owned(),
        path: path.display().to_string(),
        pages,
    })
}

/// What `name` says the file is. A cover letter first: `…_resume_cover_letter.pdf` is a letter.
fn kind(name: &str) -> FileKind {
    let stem = Path::new(name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(name)
        .to_lowercase();
    let words: Vec<&str> = stem
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();
    let joined = words.concat();
    if joined.contains("coverletter") {
        FileKind::CoverLetter
    } else if joined.contains("resume") || joined.contains("résumé") || words.contains(&"cv") {
        FileKind::Resume
    } else {
        FileKind::Other
    }
}

fn is_pdf(name: &str) -> bool {
    Path::new(name)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("pdf"))
}

#[cfg(test)]
mod tests {
    use super::kind;
    use crate::dto::FileKind::{CoverLetter, Other, Resume};

    #[test]
    fn a_file_is_what_its_name_says() {
        for (name, want) in [
            ("acme_swe_doe_resume.pdf", Resume),
            ("acme_swe_doe_resume-2.pdf", Resume),
            ("globex_swe_doe_cover_letter.pdf", CoverLetter),
            ("northwind_swe_doe_cover_letter-2.pdf", CoverLetter),
            ("Doe Resume Cover Letter.PDF", CoverLetter),
            ("CoverLetter.pdf", CoverLetter),
            ("Jane Doe CV.pdf", Resume),
            ("Résumé 2026.pdf", Resume),
            ("cvs_receipt.pdf", Other),
            ("offer.pdf", Other),
        ] {
            assert_eq!(kind(name), want, "{name}");
        }
    }
}

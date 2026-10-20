//! `applications.csv`: the application log in the export folder, which the app keeps writing so an existing
//! spreadsheet keeps working. The legacy eight columns come first, then the app's.

use crate::{AppStatus, Application, new_id, now};

/// The legacy header.
const LEGACY_HEADER: &str = "date,company,role,fit_planned,fit_on_page,resume,cover_letter,posting";

/// `applications.csv` for `apps`, given newest first like [`crate::Store::applications`]; written oldest first:
/// `date,company,role,fit_planned,fit_on_page,resume,cover_letter,posting,status,fit_now`. `fit_planned` is the
/// projected score and `fit_on_page` the on-file score, rounded to whole numbers; RFC 4180 quoting.
pub fn applications_csv(apps: &[Application]) -> String {
    let mut rows: Vec<&Application> = apps.iter().rev().collect();
    rows.sort_by(|a, b| a.date.cmp(&b.date));
    let mut out = format!("{LEGACY_HEADER},status,fit_now\n");
    for a in rows {
        let cells = [
            field(&a.date),
            field(&a.company),
            field(&a.role),
            score(a.projected),
            score(a.on_file),
            field(a.resume_path.as_deref().unwrap_or("")),
            field(a.cover_path.as_deref().unwrap_or("")),
            field(a.url.as_deref().unwrap_or("")),
            a.status.as_str().to_owned(),
            score(a.now),
        ];
        out.push_str(&cells.join(","));
        out.push('\n');
    }
    out
}

/// The rows of an `applications.csv` (the legacy eight-column header; the app's extra columns are read when present)
/// as applications with no posting, status `applied`. Newest first like [`crate::Store::applications`], so
/// `applications_csv(&parse_ats_fit_csv(text))` writes the rows back in their order. Text without a `date`,
/// `company` and `role` header gives none.
pub fn parse_ats_fit_csv(text: &str) -> Vec<Application> {
    let mut records = records(text.strip_prefix('\u{feff}').unwrap_or(text)).into_iter();
    let Some(header) = records.next() else {
        return Vec::new();
    };
    let column = |name: &str| {
        header
            .iter()
            .position(|h| h.trim().eq_ignore_ascii_case(name))
    };
    let (Some(date), Some(company), Some(role)) =
        (column("date"), column("company"), column("role"))
    else {
        return Vec::new();
    };
    let planned = column("fit_planned");
    let on_page = column("fit_on_page");
    let resume = column("resume");
    let cover = column("cover_letter");
    let posting = column("posting");
    let status = column("status");
    let fit_now = column("fit_now");
    let updated_at = now();
    let mut apps: Vec<Application> = records
        .map(|row| {
            let cell = |i: Option<usize>| i.and_then(|i| row.get(i)).map_or("", |s| s.trim());
            let text =
                |i: Option<usize>| Some(cell(i)).filter(|s| !s.is_empty()).map(str::to_owned);
            let number = |i: Option<usize>| cell(i).parse::<f64>().ok().filter(|n| n.is_finite());
            Application {
                id: new_id(),
                posting_id: None,
                analysis_id: None,
                date: cell(Some(date)).to_owned(),
                company: cell(Some(company)).to_owned(),
                role: cell(Some(role)).to_owned(),
                url: text(posting),
                status: AppStatus::parse(cell(status)).unwrap_or(AppStatus::Applied),
                now: number(fit_now),
                projected: number(planned),
                on_file: number(on_page),
                resume_path: text(resume),
                cover_path: text(cover),
                notes: String::new(),
                updated_at: updated_at.clone(),
            }
        })
        .collect();
    apps.reverse();
    apps
}

/// A CSV field, quoted when it holds a comma, quote or line break.
fn field(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_owned()
    }
}

fn score(n: Option<f64>) -> String {
    n.map(|n| format!("{}", n.round() as i64))
        .unwrap_or_default()
}

/// RFC 4180 records: quoted fields may hold commas, `""` and line breaks; LF or CRLF ends a record. Blank lines are
/// skipped.
fn records(text: &str) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut cell = String::new();
    let mut quoted = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if quoted {
            match c {
                '"' if chars.peek() == Some(&'"') => {
                    cell.push('"');
                    chars.next();
                }
                '"' => quoted = false,
                _ => cell.push(c),
            }
            continue;
        }
        match c {
            '"' => quoted = true,
            ',' => row.push(std::mem::take(&mut cell)),
            '\r' if chars.peek() == Some(&'\n') => {}
            '\n' => {
                row.push(std::mem::take(&mut cell));
                rows.push(std::mem::take(&mut row));
            }
            _ => cell.push(c),
        }
    }
    if !cell.is_empty() || !row.is_empty() {
        row.push(cell);
        rows.push(row);
    }
    rows.retain(|r| r.iter().any(|c| !c.is_empty()));
    rows
}

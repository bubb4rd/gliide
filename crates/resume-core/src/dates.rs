//! Printed dates (`Aug 2022 – May 2026`, `July 2024 – Present`) to ISO months.

/// The value of `end` for an entry that is ongoing.
pub const PRESENT: &str = "present";

/// `start` and `end` of printed dates: ISO months `YYYY-MM`, `end` possibly [`PRESENT`]. A single date is both start
/// and end. `None` when the text isn't a month, a month range, or a month to present.
///
/// Months are names or abbreviations (`June`, `Sep`, `Sept.`), `MM/YYYY` or `YYYY-MM`; ranges are joined by an en or
/// em dash, a hyphen or `to`.
pub fn parse_range(printed: &str) -> Option<(String, String)> {
    let s = printed.trim();
    if let Some(m) = month(s) {
        return Some((m.clone(), m));
    }
    // Every place a separator occurs; the first split whose halves both read wins.
    for (at, sep) in separators(s) {
        let (a, b) = (&s[..at], &s[at + sep.len()..]);
        if let (Some(start), Some(end)) = (month(a), month(b).or_else(|| present(b))) {
            return Some((start, end));
        }
    }
    None
}

fn separators(s: &str) -> impl Iterator<Item = (usize, &'static str)> + '_ {
    const SEPS: [&str; 4] = ["–", "—", "-", " to "];
    let lower = s.to_ascii_lowercase();
    SEPS.into_iter().flat_map(move |sep| {
        lower
            .match_indices(sep)
            .map(move |(i, _)| (i, sep))
            .collect::<Vec<_>>()
    })
}

fn present(s: &str) -> Option<String> {
    let w = s.trim().trim_end_matches('.').to_ascii_lowercase();
    matches!(w.as_str(), "present" | "current" | "now" | "ongoing").then(|| PRESENT.to_owned())
}

/// One month as `YYYY-MM`.
fn month(s: &str) -> Option<String> {
    let s = s.trim();
    let (m, y) = if let Some((y, m)) = s.split_once('-').filter(|(y, _)| y.len() == 4) {
        (m.parse::<u32>().ok()?, y)
    } else if let Some((m, y)) = s.split_once('/') {
        (m.parse::<u32>().ok()?, y)
    } else {
        let mut words = s
            .split(|c: char| c.is_whitespace() || c == ',')
            .filter(|w| !w.is_empty());
        let (name, y) = (words.next()?, words.next()?);
        if words.next().is_some() {
            return None;
        }
        (month_number(name)?, y)
    };
    let year = y.trim();
    if year.len() != 4 || !year.bytes().all(|b| b.is_ascii_digit()) || !(1..=12).contains(&m) {
        return None;
    }
    Some(format!("{year}-{m:02}"))
}

fn month_number(name: &str) -> Option<u32> {
    const MONTHS: [&str; 12] = [
        "january",
        "february",
        "march",
        "april",
        "may",
        "june",
        "july",
        "august",
        "september",
        "october",
        "november",
        "december",
    ];
    let w = name.trim_end_matches('.').to_ascii_lowercase();
    if w.len() < 3 {
        return None;
    }
    // A full name or any prefix of at least three letters (`Sep`, `Sept`).
    MONTHS
        .iter()
        .position(|m| m.starts_with(&w))
        .map(|i| i as u32 + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn range(s: &str) -> Option<(&'static str, &'static str)> {
        parse_range(s).map(|(a, b)| (&*a.leak(), &*b.leak()))
    }

    #[test]
    fn printed_ranges() {
        assert_eq!(range("June 2026 – Sep 2026"), Some(("2026-06", "2026-09")));
        assert_eq!(range("Aug 2022 – May 2026"), Some(("2022-08", "2026-05")));
        assert_eq!(
            range("October 2024 – May 2025"),
            Some(("2024-10", "2025-05"))
        );
        assert_eq!(range("July 2024 – Present"), Some(("2024-07", "present")));
        assert_eq!(range("Aug 2026 — current"), Some(("2026-08", "present")));
        assert_eq!(
            range("Sept. 2021 - Dec. 2021"),
            Some(("2021-09", "2021-12"))
        );
        assert_eq!(range("Jan 2020-Mar 2021"), Some(("2020-01", "2021-03")));
        assert_eq!(
            range("March, 2019 to April, 2020"),
            Some(("2019-03", "2020-04"))
        );
        assert_eq!(range("08/2022 – 05/2026"), Some(("2022-08", "2026-05")));
        assert_eq!(range("2022-08 – 2026-05"), Some(("2022-08", "2026-05")));
        assert_eq!(range("2022-08 - present"), Some(("2022-08", "present")));
    }

    #[test]
    fn single_month() {
        assert_eq!(range("May 2026"), Some(("2026-05", "2026-05")));
        assert_eq!(range("2026-05"), Some(("2026-05", "2026-05")));
    }

    #[test]
    fn unreadable() {
        assert_eq!(range(""), None);
        assert_eq!(range("Present"), None);
        assert_eq!(range("2022 – 2026"), None);
        assert_eq!(range("Summer 2024"), None);
        assert_eq!(range("Ma 2024"), None);
        assert_eq!(range("13/2024"), None);
        assert_eq!(range("Present – Aug 2022"), None);
    }
}

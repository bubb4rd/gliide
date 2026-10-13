//! `apply(bank, master, changes)`: the tailored resume (ARCHITECTURE.md §5). The master is never touched; the result is
//! the resolved master with the changes applied and the build fields a template reads for the fit check and the
//! preview's highlight.

use std::collections::BTreeSet;

use crate::bank::{Bank, BankBullet, BankEntry, BankSection, SectionKind};
use crate::change::{Change, Op};
use crate::master::Master;
use crate::resolve::{ResolveError, resolve};
use crate::resume::{Bullet, Entry, Group, Note, Resume, Section};

/// A change that can't be applied to this master: it names a line that isn't there, or asks for something the line
/// doesn't allow. Every variant but [`ApplyError::Resolve`] names the change by key.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ApplyError {
    #[error("the master doesn't resolve: {0}")]
    Resolve(#[from] ResolveError),
    #[error("change {key}: no {what} `{id}`")]
    Unknown {
        key: String,
        what: &'static str,
        id: String,
    },
    #[error("change {key}: {reason}")]
    Impossible { key: String, reason: String },
}

impl ApplyError {
    /// The key of the change that failed; `None` when the master itself doesn't resolve.
    pub fn key(&self) -> Option<&str> {
        match self {
            Self::Resolve(_) => None,
            Self::Unknown { key, .. } | Self::Impossible { key, .. } => Some(key),
        }
    }
}

/// The resume `master` selects from `bank`, with `changes` applied in a fixed order whatever order they come in (equal
/// steps keep their order): excluded, swapped and included entries; entry order; line edits (rewrites, variants, swaps
/// and note additions, then your text for a line, then cuts, then new bullets); skills lines; the summary.
///
/// Build fields: a changed bullet keeps its master text in `orig` (the line budget) and is `changed`; a new bullet, and
/// every bullet of an included or swapped-in entry, is `new` and `changed` (its id is the change key for a new bullet);
/// an edited note line is `changed`; an edited skills line keeps the master's items in `orig`, with the items it gained
/// in `added`.
///
/// A reorder ignores ids its section doesn't show (an entry whose include is off), and line edits to an entry that an
/// exclude or entry swap took off are skipped, so toggles never conflict.
pub fn apply(bank: &Bank, master: &Master, changes: &[&Change]) -> Result<Resume, ApplyError> {
    let mut r = resolve(bank, master)?;
    let mut ordered: Vec<&Change> = changes.to_vec();
    ordered.sort_by_key(|c| phase(&c.op));
    let mut gone = BTreeSet::new();
    for c in ordered {
        let target = match &c.op {
            Op::Rewrite { bullet, .. } | Op::UseVariant { bullet, .. } | Op::Cut { bullet } => {
                Some(bullet)
            }
            Op::Set { line, .. } | Op::Swap { line, .. } => Some(line),
            Op::NoteAdd { note, .. } => Some(note),
            Op::Add { entry, .. } => Some(entry),
            _ => None,
        };
        if target.is_some_and(|t| gone.contains(t)) {
            continue;
        }
        if let Op::Exclude { entry: out } | Op::SwapEntry { out, .. } = &c.op
            && let Some((si, ei)) = entry_at(&r, out)
        {
            let e = &r.sections[si].entries[ei];
            gone.insert(e.id.clone());
            gone.extend(e.bullets.iter().map(|b| b.id.clone()));
            gone.extend((1..=e.notes.len()).map(|n| format!("{}#{n}", e.id)));
        }
        apply_one(bank, &mut r, c)?;
    }
    for g in r.sections.iter_mut().flat_map(|s| s.groups.iter_mut()) {
        if let Some(orig) = &g.orig {
            g.added = g
                .items
                .iter()
                .filter(|i| position_ci(orig, i).is_none())
                .cloned()
                .collect();
        }
    }
    Ok(r)
}

/// Apply order. Your text for a line (`set`) comes after the other edits to it, so it is what the line ends up saying.
fn phase(op: &Op) -> u8 {
    match op {
        Op::Exclude { .. } => 0,
        Op::SwapEntry { .. } => 1,
        Op::Include { .. } => 2,
        Op::Reorder { .. } => 3,
        Op::Rewrite { .. } | Op::UseVariant { .. } | Op::Swap { .. } | Op::NoteAdd { .. } => 4,
        Op::Set { .. } => 5,
        Op::Cut { .. } => 6,
        Op::Add { .. } => 7,
        Op::SkillAdd { .. }
        | Op::SkillRemove { .. }
        | Op::SkillRename { .. }
        | Op::SkillMove { .. } => 8,
        Op::Summary { .. } => 9,
    }
}

fn apply_one(bank: &Bank, r: &mut Resume, c: &Change) -> Result<(), ApplyError> {
    let key = &c.key;
    let unknown = |what: &'static str, id: &str| ApplyError::Unknown {
        key: key.clone(),
        what,
        id: id.to_owned(),
    };
    let impossible = |reason: String| ApplyError::Impossible {
        key: key.clone(),
        reason,
    };
    match &c.op {
        Op::Exclude { entry } => {
            let (si, ei) = entry_at(r, entry).ok_or_else(|| unknown("entry", entry))?;
            let s = &mut r.sections[si];
            s.entries.remove(ei);
            if s.entries.is_empty() && s.groups.is_empty() {
                r.sections.remove(si);
            }
        }
        Op::SwapEntry {
            out,
            entry,
            bullets,
        } => {
            let (si, ei) = entry_at(r, out).ok_or_else(|| unknown("entry", out))?;
            if entry_at(r, entry).is_some() {
                return Err(impossible(format!("`{entry}` is already on the resume")));
            }
            let (_, be) = bank_entry(bank, entry).ok_or_else(|| unknown("bank entry", entry))?;
            r.sections[si].entries[ei] = shown_entry(be, bullets, key)?;
        }
        Op::Include { entry, bullets } => {
            if entry_at(r, entry).is_some() {
                return Err(impossible(format!("`{entry}` is already on the resume")));
            }
            let (bs, be) = bank_entry(bank, entry).ok_or_else(|| unknown("bank entry", entry))?;
            let e = shown_entry(be, bullets, key)?;
            let si = match r.sections.iter().position(|s| s.id == bs.id) {
                Some(si) => si,
                None => insert_section(bank, r, bs),
            };
            r.sections[si].entries.push(e);
        }
        Op::Reorder { section, entries } => {
            let s = r
                .sections
                .iter_mut()
                .find(|s| s.id == *section)
                .ok_or_else(|| unknown("section", section))?;
            let mut rest = std::mem::take(&mut s.entries);
            for id in entries {
                if let Some(i) = rest.iter().position(|e| e.id == *id) {
                    s.entries.push(rest.remove(i));
                }
            }
            s.entries.append(&mut rest);
        }
        Op::Rewrite { bullet, text } => {
            let b = bullet_mut(r, bullet).ok_or_else(|| unknown("bullet", bullet))?;
            set_bullet(b, text.clone());
        }
        Op::UseVariant { bullet, variant } => {
            let bb = bank_bullet(bank, bullet).ok_or_else(|| unknown("bullet", bullet))?;
            let text = bb.variants.get(*variant).cloned().ok_or_else(|| {
                impossible(format!(
                    "bullet `{bullet}` has no variant {variant} (it has {})",
                    bb.variants.len()
                ))
            })?;
            let b = bullet_mut(r, bullet).ok_or_else(|| unknown("bullet", bullet))?;
            set_bullet(b, text);
        }
        Op::Set { line, text } => match line_mut(r, line).ok_or_else(|| unknown("line", line))? {
            Line::Bullet(b) => set_bullet(b, text.clone()),
            Line::Note(n) => set_note(n, text.clone()),
        },
        Op::Swap { line, from, to } => {
            let l = line_mut(r, line).ok_or_else(|| unknown("line", line))?;
            let new = swap_phrase(l.text(), from, to)
                .ok_or_else(|| impossible(format!("“{from}” isn't in `{line}`")))?;
            match l {
                Line::Bullet(b) => set_bullet(b, new),
                Line::Note(n) => set_note(n, new),
            }
        }
        Op::NoteAdd { note, term } => {
            let Some(Line::Note(n)) = line_mut(r, note) else {
                return Err(unknown("note line", note));
            };
            let text = appended(&n.text, term);
            set_note(n, text);
        }
        Op::Cut { bullet } => {
            let e = r
                .sections
                .iter_mut()
                .flat_map(|s| s.entries.iter_mut())
                .find(|e| e.bullets.iter().any(|b| b.id == *bullet))
                .ok_or_else(|| unknown("bullet", bullet))?;
            e.bullets.retain(|b| b.id != *bullet);
        }
        Op::Add { entry, text } => {
            let (si, ei) = entry_at(r, entry).ok_or_else(|| unknown("entry", entry))?;
            r.sections[si].entries[ei].bullets.push(Bullet {
                id: key.clone(),
                text: text.clone(),
                orig: None,
                changed: true,
                new: true,
            });
        }
        Op::SkillAdd { group, item } => {
            let g = group_mut(r, group).ok_or_else(|| unknown("skills line", group))?;
            touch(g);
            if position_ci(&g.items, item).is_none() {
                g.items.push(item.clone());
            }
        }
        Op::SkillRemove { group, item } => {
            let g = group_mut(r, group).ok_or_else(|| unknown("skills line", group))?;
            let i = position_ci(&g.items, item)
                .ok_or_else(|| impossible(format!("“{item}” isn't on skills line `{group}`")))?;
            touch(g);
            g.items.remove(i);
        }
        Op::SkillRename { group, from, to } => {
            let g = group_mut(r, group).ok_or_else(|| unknown("skills line", group))?;
            let i = position_ci(&g.items, from)
                .ok_or_else(|| impossible(format!("“{from}” isn't on skills line `{group}`")))?;
            touch(g);
            g.items[i] = to.clone();
        }
        Op::SkillMove { from, to, item } => {
            if group_mut(r, to).is_none() {
                return Err(unknown("skills line", to));
            }
            let g = group_mut(r, from).ok_or_else(|| unknown("skills line", from))?;
            let i = position_ci(&g.items, item)
                .ok_or_else(|| impossible(format!("“{item}” isn't on skills line `{from}`")))?;
            touch(g);
            let moved = g.items.remove(i);
            let g = group_mut(r, to).expect("checked above");
            touch(g);
            if position_ci(&g.items, &moved).is_none() {
                g.items.push(moved);
            }
        }
        Op::Summary { text } => set_summary(bank, r, text),
    }
    Ok(())
}

/// `text` with `term` appended as a list item (`, term`), or `term` alone when `text` is empty.
pub fn appended(text: &str, term: &str) -> String {
    if text.trim().is_empty() {
        term.to_owned()
    } else {
        format!("{text}, {term}")
    }
}

/// `text` with the first whole-word occurrence of `from` (case aside) replaced by `to`; `None` when `from` isn't in it.
/// A capitalized `from` gives a capitalized `to` (a line's first word stays capitalized). A cut (`to` empty) takes a list
/// comma or a joining "and" with it, so the sentence still reads: "A, B, C" less "B" is "A, C"; "A and B." less "B" is
/// "A."; "A and B" less "A" is "B"; a line cut at its start is capitalized again.
pub fn swap_phrase(text: &str, from: &str, to: &str) -> Option<String> {
    let from = from.trim();
    if from.is_empty() {
        return None;
    }
    let (s, e) = find_phrase(text, from)?;
    let (before, after) = (&text[..s], &text[e..]);
    let to = to.trim();
    if !to.is_empty() {
        let found = &text[s..e];
        let to = if found.starts_with(char::is_uppercase) {
            capitalized(to)
        } else {
            to.to_owned()
        };
        return Some(format!("{before}{to}{after}"));
    }
    let (mut before, mut after) = (before.to_owned(), after.to_owned());
    if before.trim_end().ends_with(',') && after.starts_with(',') {
        before = before.trim_end().trim_end_matches(',').to_owned();
    } else if let (Some(b), true) = (
        strip_trailing_and(&before),
        after.is_empty() || after.starts_with([',', '.', ';', ')']),
    ) {
        before = b;
    } else if let Some(a) = strip_leading_and(&after) {
        after = format!(" {a}");
    } else if let Some(a) = after.strip_prefix(',') {
        after = format!(" {}", a.trim_start());
    }
    let joined = format!("{before}{after}");
    let mut out = joined.split_whitespace().collect::<Vec<_>>().join(" ");
    for (bad, good) in [
        (" ,", ","),
        (" .", "."),
        (" ;", ";"),
        (" )", ")"),
        ("( ", "("),
        ("()", ""),
    ] {
        out = out.replace(bad, good);
    }
    let out = out.trim().to_owned();
    Some(if text.starts_with(char::is_uppercase) {
        capitalized(&out)
    } else {
        out
    })
}

/// `s` with its first letter uppercase, when it starts with a lowercase letter.
pub fn capitalized(s: &str) -> String {
    let mut cs = s.chars();
    match cs.next() {
        Some(c) if c.is_lowercase() => c.to_uppercase().chain(cs).collect(),
        _ => s.to_owned(),
    }
}

/// `before` less a trailing `[,] and ` (whitespace on both sides of `and`).
fn strip_trailing_and(before: &str) -> Option<String> {
    if !before.ends_with(char::is_whitespace) {
        return None;
    }
    let rest = before.trim_end().strip_suffix("and")?;
    if !rest.ends_with(char::is_whitespace) {
        return None;
    }
    let rest = rest.trim_end();
    Some(rest.strip_suffix(',').unwrap_or(rest).to_owned())
}

/// `after` less a leading ` and ` (whitespace on both sides of `and`).
fn strip_leading_and(after: &str) -> Option<&str> {
    if !after.starts_with(char::is_whitespace) {
        return None;
    }
    let rest = after.trim_start().strip_prefix("and")?;
    rest.starts_with(char::is_whitespace)
        .then(|| rest.trim_start())
}

/// The byte range of the first whole-word occurrence of `phrase` in `text`, case-insensitively (curly apostrophes
/// straightened). Where the phrase begins or ends with a letter or digit, the text must not continue the word there.
pub fn find_phrase(text: &str, phrase: &str) -> Option<(usize, usize)> {
    let hay: Vec<(usize, char)> = text.char_indices().map(|(i, c)| (i, fold(c))).collect();
    let needle: Vec<char> = phrase.chars().map(fold).collect();
    let n = needle.len();
    if n == 0 || n > hay.len() {
        return None;
    }
    let (open_start, open_end) = (needle[0].is_alphanumeric(), needle[n - 1].is_alphanumeric());
    (0..=hay.len() - n).find_map(|s| {
        let matches = hay[s..s + n]
            .iter()
            .map(|&(_, c)| c)
            .eq(needle.iter().copied());
        let starts = !open_start || s == 0 || !hay[s - 1].1.is_alphanumeric();
        let ends = !open_end || hay.get(s + n).is_none_or(|&(_, c)| !c.is_alphanumeric());
        (matches && starts && ends).then(|| {
            let end = hay.get(s + n).map_or(text.len(), |&(i, _)| i);
            (hay[s].0, end)
        })
    })
}

fn fold(c: char) -> char {
    if matches!(c, '\u{2018}' | '\u{2019}') {
        return '\'';
    }
    let mut lower = c.to_lowercase();
    match (lower.next(), lower.next()) {
        (Some(l), None) => l,
        _ => c,
    }
}

/// The position of `item` in `items`, case and surrounding space aside.
pub(crate) fn position_ci(items: &[String], item: &str) -> Option<usize> {
    let want = item.trim().to_lowercase();
    items.iter().position(|x| x.trim().to_lowercase() == want)
}

/// A note line id, `entry#n` (1-based): the entry id and the note's index.
pub(crate) fn note_ref(id: &str) -> Option<(&str, usize)> {
    let (entry, n) = id.rsplit_once('#')?;
    let n: usize = n.parse().ok()?;
    n.checked_sub(1).map(|i| (entry, i))
}

/// The bank entry `id` and its section.
pub(crate) fn bank_entry<'a>(bank: &'a Bank, id: &str) -> Option<(&'a BankSection, &'a BankEntry)> {
    bank.sections
        .iter()
        .find_map(|s| s.entries.iter().find(|e| e.id == id).map(|e| (s, e)))
}

/// The bank bullet `id`.
pub(crate) fn bank_bullet<'a>(bank: &'a Bank, id: &str) -> Option<&'a BankBullet> {
    bank.sections
        .iter()
        .flat_map(|s| &s.entries)
        .flat_map(|e| &e.bullets)
        .find(|b| b.id == id)
}

fn entry_at(r: &Resume, id: &str) -> Option<(usize, usize)> {
    r.sections
        .iter()
        .enumerate()
        .find_map(|(si, s)| s.entries.iter().position(|e| e.id == id).map(|ei| (si, ei)))
}

fn bullet_mut<'a>(r: &'a mut Resume, id: &str) -> Option<&'a mut Bullet> {
    r.sections
        .iter_mut()
        .flat_map(|s| s.entries.iter_mut())
        .flat_map(|e| e.bullets.iter_mut())
        .find(|b| b.id == id)
}

fn group_mut<'a>(r: &'a mut Resume, id: &str) -> Option<&'a mut Group> {
    r.sections
        .iter_mut()
        .flat_map(|s| s.groups.iter_mut())
        .find(|g| g.id == id)
}

enum Line<'a> {
    Bullet(&'a mut Bullet),
    Note(&'a mut Note),
}

impl Line<'_> {
    fn text(&self) -> &str {
        match self {
            Line::Bullet(b) => &b.text,
            Line::Note(n) => &n.text,
        }
    }
}

/// The bullet `id`, else the note line `id` (`entry#n`).
fn line_mut<'a>(r: &'a mut Resume, id: &str) -> Option<Line<'a>> {
    if let Some((entry, i)) = note_ref(id) {
        let e = r
            .sections
            .iter_mut()
            .flat_map(|s| s.entries.iter_mut())
            .find(|e| e.id == entry)?;
        return e.notes.get_mut(i).map(Line::Note);
    }
    bullet_mut(r, id).map(Line::Bullet)
}

fn set_bullet(b: &mut Bullet, text: String) {
    if !b.new && b.orig.is_none() {
        b.orig = Some(b.text.clone());
    }
    b.text = text;
    b.changed = true;
}

fn set_note(n: &mut Note, text: String) {
    n.text = text;
    n.changed = true;
}

fn touch(g: &mut Group) {
    if g.orig.is_none() {
        g.orig = Some(g.items.clone());
    }
}

/// Bank entry `be` as shown for this posting: the bullets named (all when none are), each `new` and `changed`.
fn shown_entry(be: &BankEntry, bullets: &[String], key: &str) -> Result<Entry, ApplyError> {
    let picked: Vec<&BankBullet> = if bullets.is_empty() {
        be.bullets.iter().collect()
    } else {
        bullets
            .iter()
            .map(|id| {
                be.bullets
                    .iter()
                    .find(|b| b.id == *id)
                    .ok_or_else(|| ApplyError::Unknown {
                        key: key.to_owned(),
                        what: "bullet",
                        id: id.clone(),
                    })
            })
            .collect::<Result<_, _>>()?
    };
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
        bullets: picked
            .into_iter()
            .map(|b| Bullet {
                id: b.id.clone(),
                text: b.text.clone(),
                orig: None,
                changed: true,
                new: true,
            })
            .collect(),
        start: be.start.clone(),
        end: be.end.clone(),
    })
}

/// Bank section `bs`, empty, inserted where the bank's section order puts it among the resume's sections; its index.
fn insert_section(bank: &Bank, r: &mut Resume, bs: &BankSection) -> usize {
    let rank = |id: &str| bank.sections.iter().position(|s| s.id == id);
    let mine = rank(&bs.id);
    let at = r
        .sections
        .iter()
        .position(|s| rank(&s.id) > mine)
        .unwrap_or(r.sections.len());
    r.sections.insert(
        at,
        Section {
            id: bs.id.clone(),
            title: bs.title.clone(),
            entries: vec![],
            groups: vec![],
            kind: Some(bs.kind),
        },
    );
    at
}

/// The summary line's text on `r`: the first note (else bullet) of the summary section's first entry.
pub(crate) fn summary_text(r: &Resume) -> Option<&str> {
    let s = r
        .sections
        .iter()
        .find(|s| s.kind == Some(SectionKind::Summary))?;
    let e = s.entries.first()?;
    e.notes
        .first()
        .map(|n| n.text.as_str())
        .or_else(|| e.bullets.first().map(|b| b.text.as_str()))
}

/// The summary line becomes `text`. Without a summary section, one is added first: the bank's summary section (else
/// `summary`, `SUMMARY`) holding one entry with no header and a single unlabeled note line.
fn set_summary(bank: &Bank, r: &mut Resume, text: &str) {
    let si = match r
        .sections
        .iter()
        .position(|s| s.kind == Some(SectionKind::Summary))
    {
        Some(si) => si,
        None => {
            let (id, title) = bank
                .sections
                .iter()
                .find(|s| s.kind == SectionKind::Summary)
                .map_or(("summary".to_owned(), "SUMMARY".to_owned()), |s| {
                    (s.id.clone(), s.title.clone())
                });
            r.sections.insert(
                0,
                Section {
                    id,
                    title,
                    entries: vec![],
                    groups: vec![],
                    kind: Some(SectionKind::Summary),
                },
            );
            0
        }
    };
    let s = &mut r.sections[si];
    if s.entries.is_empty() {
        s.entries.push(Entry {
            id: s.id.clone(),
            name: String::new(),
            url: String::new(),
            place: String::new(),
            dates: String::new(),
            subtitle: String::new(),
            notes: vec![],
            bullets: vec![],
            start: None,
            end: None,
        });
    }
    let e = &mut s.entries[0];
    if let Some(n) = e.notes.first_mut() {
        set_note(n, text.to_owned());
    } else if let Some(b) = e.bullets.first_mut() {
        set_bullet(b, text.to_owned());
    } else {
        e.notes.push(Note {
            label: String::new(),
            text: text.to_owned(),
            changed: true,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn swaps_keep_the_sentence() {
        let s = |t: &str, f: &str, to: &str| swap_phrase(t, f, to);
        assert_eq!(
            s("Collaborated with the team", "collaborated", "partnered").as_deref(),
            Some("Partnered with the team")
        );
        assert_eq!(
            s(
                "Data Structures, Operating Systems, Databases",
                "Operating Systems",
                ""
            )
            .as_deref(),
            Some("Data Structures, Databases")
        );
        assert_eq!(
            s("Rust and Python.", "Python", "").as_deref(),
            Some("Rust.")
        );
        assert_eq!(s("Rust and Python", "Rust", "").as_deref(), Some("Python"));
        assert_eq!(
            s("Built tools, then shipped", "Built tools", "").as_deref(),
            Some("Then shipped")
        );
        assert_eq!(s("Wrote tests", "test", "spec"), None);
        assert_eq!(s("Wrote tests", "", "spec"), None);
        assert_eq!(
            s("Café menus in Postgres", "postgres", "PostgreSQL").as_deref(),
            Some("Café menus in PostgreSQL")
        );
    }

    #[test]
    fn note_refs() {
        assert_eq!(note_ref("state-u#2"), Some(("state-u", 1)));
        assert_eq!(note_ref("state-u#0"), None);
        assert_eq!(note_ref("acme.1"), None);
    }
}

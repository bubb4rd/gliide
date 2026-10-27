//! The grounding validator (ARCHITECTURE.md §3): checks each model-written line against your bank before it becomes a
//! change. Findings are user-facing sentences in [`Change::flags`](crate::change::Change); a flagged change starts left
//! out, and you can force it in.
//!
//! - Numbers, percentages, money and years in new text must appear in the lines it draws on (its `sources`, or the line
//!   it replaces).
//! - A bullet draws only on its own entry: a source line under another entry is flagged (a summary draws on the whole
//!   resume). A skills line backs a tool, not where it was used, so it is a source only for a requirement skill whose
//!   evidence is `yes` or `add`.
//! - Proper nouns and tool names (forms in the alias dictionary, and capitalized words that don't start a sentence) must
//!   appear somewhere in the bank, or be (part of) a requirement skill whose evidence is `yes` or `add`, or something
//!   you vouched for in a chat ask ([`Grounding::confirmed`]: a skill you confirmed, your own words). A degree
//!   abbreviation (`B.S.`, `MBA`, `Ph.D.`) counts as in the bank when an education entry states that level.
//! - `before` must be the line's current text; every id must exist; a swap's `from` must be in its line.
//! - A swap that trades a past-tense verb for another form ("Collaborated" → "collaboration") may not read right.

use std::collections::BTreeSet;

use crate::apply::{bank_entry, bank_fact, note_ref};
use crate::bank::{Bank, BankEntry, SectionKind};
use crate::matcher::{self, Page};
use crate::requirements::{DegreeLevel, Requirements};
use crate::score::degree_level;
use crate::suggestions::{Have, Suggestions};

/// What a line is checked against: the bank, its text, and the requirement skills your evidence backs.
pub struct Grounding<'a> {
    bank_doc: &'a Bank,
    /// Everything in the bank, as one block.
    bank: Page,
    bank_text: String,
    /// Forms (term and aliases) of the requirement skills whose evidence is `yes` or `add`.
    backed: Vec<String>,
    /// Degree levels the bank's education entries state.
    degrees: Vec<DegreeLevel>,
    /// What you vouched for though the bank doesn't show it (skills you confirmed, your own words in a chat ask), one
    /// block each: its names count as in the bank.
    confirmed: Page,
}

impl<'a> Grounding<'a> {
    pub fn new(bank: &'a Bank, s: &Suggestions, req: &Requirements) -> Self {
        let backed = s
            .evidence
            .iter()
            .filter(|e| matches!(e.have, Have::Yes | Have::Add))
            .filter_map(|e| {
                req.skills
                    .iter()
                    .find(|r| matcher::key(&r.term) == matcher::key(&e.skill))
            })
            .flat_map(|r| {
                std::iter::once(r.term.clone()).chain(matcher::all_aliases(&r.term, &r.aliases))
            })
            .collect();
        Self {
            backed,
            ..Self::of_bank(bank)
        }
    }

    /// The bank alone, no requirement skill backed: what a line written outside a review (Dig) is checked against.
    pub fn of_bank(bank: &'a Bank) -> Self {
        let bank_text = bank_text(bank);
        let degrees = bank
            .sections
            .iter()
            .filter(|s| s.kind == SectionKind::Education)
            .flat_map(|s| &s.entries)
            .flat_map(|e| {
                [e.name.as_str(), e.subtitle.as_str()]
                    .into_iter()
                    .chain(e.notes.iter().map(|n| n.text.as_str()))
                    .chain(e.bullets.iter().map(|b| b.text.as_str()))
            })
            .filter_map(degree_level)
            .collect();
        Self {
            bank_doc: bank,
            bank: Page::of_text(&bank_text),
            bank_text,
            backed: Vec::new(),
            degrees,
            confirmed: Page::new(std::iter::empty()),
        }
    }

    /// Treats the names in `vouched` (skills you confirmed, your own words) as in the bank for the name checks; numbers
    /// still need sources.
    pub fn confirmed(mut self, vouched: &[String]) -> Self {
        self.confirmed = Page::new(vouched.iter().map(|n| (None, n.as_str())));
        self
    }

    /// Why a line written for `line` (a bullet, or the entry a new bullet goes under) may not draw on bank line
    /// `source`, which `text` cites: it belongs to another entry, or it is a skills line and `text` states no
    /// requirement skill your evidence backs. `None` when it may, or when `source` is no bank line.
    pub fn cross_entry(&self, line: &str, source: &str, text: &str) -> Option<String> {
        let home = self.entry_of(line).map_or(line, |e| e.id.as_str());
        let elsewhere = |e: &BankEntry, what: Option<String>| {
            (e.id != home).then(|| match what {
                Some(w) => format!("draws on “{w}” from {}, not this entry", e.name),
                None => format!("draws on {}, not this entry", e.name),
            })
        };
        let skills_line = |label: &str| {
            let page = Page::of_text(text);
            (!self.backed.iter().any(|f| page.find(f).is_some())).then(|| {
                format!(
                    "draws on your {label} line, which backs only a skill this posting asks for and you have"
                )
            })
        };
        if let Some(e) = self.entry_of(source)
            && let Some(b) = e.bullets.iter().find(|b| b.id == source)
        {
            return elsewhere(e, Some(short(&b.text)));
        }
        if let Some((e, f)) = bank_fact(self.bank_doc, source) {
            return elsewhere(e, Some(short(&f.text)));
        }
        if let Some((entry, i)) = note_ref(source) {
            let (_, e) = bank_entry(self.bank_doc, entry)?;
            let n = e.notes.get(i)?;
            let shown = if n.label.is_empty() {
                n.text.clone()
            } else {
                format!("{}: {}", n.label, n.text)
            };
            return elsewhere(e, Some(short(&shown)));
        }
        if let Some((_, e)) = bank_entry(self.bank_doc, source) {
            return elsewhere(e, None);
        }
        for s in &self.bank_doc.sections {
            if let Some(g) = s.groups.iter().find(|g| g.id == source) {
                return skills_line(&g.label);
            }
            if s.id == source {
                if s.kind == SectionKind::Skills {
                    return skills_line(&s.title);
                }
                return s
                    .entries
                    .iter()
                    .any(|e| e.id != home)
                    .then(|| format!("draws on the whole {} section, not this entry", s.title));
            }
        }
        None
    }

    /// The bank entry holding bullet or fact `id`, else the entry `id`.
    fn entry_of(&self, id: &str) -> Option<&'a BankEntry> {
        let entries = self.bank_doc.sections.iter().flat_map(|s| &s.entries);
        entries
            .clone()
            .find(|e| e.bullets.iter().any(|b| b.id == id) || e.facts.iter().any(|f| f.id == id))
            .or_else(|| entries.clone().find(|e| e.id == id))
    }

    /// Every line of the bank, one per line: what a summary may draw its numbers from.
    pub fn bank_text(&self) -> &str {
        &self.bank_text
    }

    /// Findings for `text`, a model-written line: numbers not in `sources`, then names neither in the bank nor backed.
    pub fn check(&self, text: &str, sources: &[&str]) -> Vec<String> {
        let mut out = unsourced_numbers(text, sources);
        out.extend(self.unknown_names(text, false));
        out
    }

    /// Findings for a Skills item (or a term added to a list): every capitalized word counts as a name.
    pub fn check_item(&self, item: &str) -> Vec<String> {
        let mut out = unsourced_numbers(item, &[]);
        out.extend(self.unknown_names(item, true));
        out
    }

    /// Names in `text` neither in the bank nor backed by a requirement skill, each as a finding. With `every_word`, a
    /// capitalized first word counts as a name too. A word inside a dictionary form whose group the bank or a backed
    /// skill grounds ("Amazon Web Services" when the bank says "AWS") passes.
    fn unknown_names(&self, text: &str, every_word: bool) -> Vec<String> {
        let page = Page::of_text(text);
        let (mut grounded, mut tools) = (Vec::new(), Vec::new());
        for group in matcher::dictionary() {
            let Some(hit) = group.iter().find_map(|(_, form)| page.find(form)) else {
                continue;
            };
            if group
                .iter()
                .any(|(_, form)| self.bank.find(form).is_some() || self.confirmed.find(form).is_some())
                || self.backed_in(&page, &hit.text)
            {
                grounded.push(hit.text);
            } else {
                tools.push(hit.text);
            }
        }
        let known = |word: &str| {
            self.bank.find(word).is_some()
                || self.confirmed.find(word).is_some()
                || self.backed_in(&page, word)
                || grounded
                    .iter()
                    .any(|g| Page::of_text(g).find(word).is_some())
                || degree_level(word).is_some_and(|l| self.degrees.contains(&l))
        };
        let mut seen = BTreeSet::new();
        let mut out = Vec::new();
        let mut flag = |name: &str| {
            if seen.insert(name.to_lowercase()) {
                out.push(format!("“{name}” is not in your bank"));
            }
        };
        for (word, initial) in words(text) {
            let rest_upper = word.chars().skip(1).any(char::is_uppercase);
            let first_upper = word.starts_with(char::is_uppercase);
            let named = rest_upper || (first_upper && (every_word || !initial));
            if !named || matches!(word, "I" | "A") || !word.chars().any(char::is_alphabetic) {
                continue;
            }
            let ok = known(word)
                || (word.contains('/') && word.split('/').all(|p| p.is_empty() || known(p)));
            if !ok {
                flag(word);
            }
        }
        for t in &tools {
            flag(t);
        }
        out
    }

    /// `word` is part of a backed requirement form that `page` states.
    fn backed_in(&self, page: &Page, word: &str) -> bool {
        self.backed
            .iter()
            .any(|f| page.find(f).is_some() && Page::of_text(f).find(word).is_some())
    }
}

/// The first six words of `text`, with `…` when there are more.
fn short(text: &str) -> String {
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.len() <= 6 {
        return words.join(" ");
    }
    format!(
        "{}…",
        words[..6].join(" ").trim_end_matches([',', ';', ':'])
    )
}

/// The words of `text` with possessives and surrounding punctuation trimmed, each with whether it starts a sentence.
fn words(text: &str) -> Vec<(&str, bool)> {
    let mut out = Vec::new();
    let mut initial = true;
    for raw in text.split_whitespace() {
        if !raw.chars().any(char::is_alphanumeric) {
            continue;
        }
        let w = raw
            .trim_start_matches(|c: char| !c.is_alphanumeric() && c != '.')
            .trim_end_matches(|c: char| !c.is_alphanumeric() && c != '+' && c != '#');
        let w = w
            .strip_suffix("'s")
            .or_else(|| w.strip_suffix("\u{2019}s"))
            .unwrap_or(w);
        out.push((w, initial));
        initial = raw
            .trim_end_matches(['"', '\'', ')', ']', '\u{201d}', '\u{2019}'])
            .ends_with(['.', '!', '?', ':']);
    }
    out
}

/// A number as written and as compared: `$1,200` → `$1200`, `40 percent` → `40%`, `10x`, `5k`, `100+`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Number {
    pub shown: String,
    pub norm: String,
}

/// The numbers in `text`: digits not joined to letters before them (`k8s`, `ES6` and `EC2` are names), with a leading
/// `$`, decimal and thousands separators, and a trailing `%` (or ` percent`), `k`/`m`/`b`/`x` multiplier or `+`.
pub fn numbers(text: &str) -> Vec<Number> {
    let cs: Vec<(usize, char)> = text.char_indices().collect();
    let at = |i: usize| cs.get(i).map(|&(_, c)| c);
    let byte = |i: usize| cs.get(i).map_or(text.len(), |&(b, _)| b);
    let mut out = Vec::new();
    let mut i = 0;
    while i < cs.len() {
        let c = cs[i].1;
        if !c.is_ascii_digit() {
            i += 1;
            continue;
        }
        let prev = i.checked_sub(1).and_then(at);
        if prev.is_some_and(char::is_alphanumeric) {
            while at(i).is_some_and(char::is_alphanumeric) {
                i += 1;
            }
            continue;
        }
        let money = prev == Some('$');
        let start = if money { i - 1 } else { i };
        let mut digits = String::new();
        while let Some(d) = at(i) {
            if d.is_ascii_digit() {
                digits.push(d);
            } else if d == '.' && at(i + 1).is_some_and(|n| n.is_ascii_digit()) {
                digits.push('.');
            } else if d != ',' || !at(i + 1).is_some_and(|n| n.is_ascii_digit()) {
                break;
            }
            i += 1;
        }
        let mut suffix = String::new();
        let run_end = (i..)
            .find(|&j| !at(j).is_some_and(char::is_alphanumeric))
            .unwrap_or(i);
        if run_end == i + 1 && matches!(at(i), Some('k' | 'K' | 'm' | 'M' | 'b' | 'B' | 'x' | 'X'))
        {
            suffix.push(at(i).unwrap_or_default().to_ascii_lowercase());
            i += 1;
        }
        let mut end = i;
        if at(i) == Some('+') {
            suffix.push('+');
            i += 1;
            end = i;
        }
        if at(i) == Some('%') {
            suffix.push('%');
            i += 1;
            end = i;
        } else if text[byte(i)..].to_lowercase().starts_with(" percent") {
            suffix.push('%');
            end = i + " percent".len();
            i = end;
        }
        out.push(Number {
            shown: text[byte(start)..byte(end)].to_owned(),
            norm: format!("{}{digits}{suffix}", if money { "$" } else { "" }),
        });
    }
    out
}

/// A finding per number in `text` that none of `sources` states.
pub fn unsourced_numbers(text: &str, sources: &[&str]) -> Vec<String> {
    let known: BTreeSet<String> = sources
        .iter()
        .flat_map(|s| numbers(s))
        .map(|n| n.norm)
        .collect();
    let mut seen = BTreeSet::new();
    numbers(text)
        .into_iter()
        .filter(|n| !known.contains(&n.norm) && seen.insert(n.norm.clone()))
        .map(|n| format!("“{}” is not in the lines it draws on", n.shown))
        .collect()
}

/// `a` and `b` are the same text, whitespace and apostrophe style aside.
pub fn same_text(a: &str, b: &str) -> bool {
    let norm = |s: &str| {
        s.split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .replace(['\u{2018}', '\u{2019}'], "'")
            .replace(['\u{201c}', '\u{201d}'], "\"")
    };
    norm(a) == norm(b)
}

/// The finding when a suggestion's `before` isn't the line's current text.
pub fn stale(before: &str, current: &str) -> Option<String> {
    (!same_text(before, current))
        .then(|| "written for different text than this line has now".to_owned())
}

/// The form a phrase's last word gives it: `ed` (a past verb), `ing`, `noun` (-ion, -ment, -ance, -ence, -ity, -ness,
/// -ship), else `None`.
fn word_form(phrase: &str) -> Option<&'static str> {
    let word = phrase
        .trim_end_matches(|c: char| !c.is_alphabetic())
        .rsplit(|c: char| !c.is_alphabetic())
        .next()
        .unwrap_or("")
        .to_lowercase();
    if word.ends_with("ed") {
        Some("ed")
    } else if word.ends_with("ing") {
        Some("ing")
    } else if ["ion", "ment", "ance", "ence", "ity", "ness", "ship"]
        .iter()
        .any(|e| word.ends_with(e))
    {
        Some("noun")
    } else {
        None
    }
}

/// Why swapping `from` for `to` may not read right: a past-tense verb traded for another form or the reverse
/// ("Collaborated" → "collaboration"; "automated tests" → "automated testing" passes).
pub fn swap_warning(from: &str, to: &str) -> Option<String> {
    if to.trim().is_empty() || (word_form(from) == Some("ed")) == (word_form(to) == Some("ed")) {
        return None;
    }
    Some(format!(
        "check it reads right (“{}” → “{}” changes the word's form)",
        from.trim(),
        to.trim()
    ))
}

/// Every line of the bank, one per line: name, contact, section titles, entry headers, notes, bullets with their
/// variants and skills, facts with theirs, skills lines with every item.
pub fn bank_text(bank: &Bank) -> String {
    let mut lines = vec![bank.name.clone()];
    lines.extend(bank.contact.iter().map(|c| c.text.clone()));
    for s in &bank.sections {
        lines.push(s.title.clone());
        for e in &s.entries {
            lines.extend([
                e.name.clone(),
                e.place.clone(),
                e.dates.clone(),
                e.subtitle.clone(),
            ]);
            lines.extend(e.notes.iter().map(|n| format!("{}: {}", n.label, n.text)));
            for b in &e.bullets {
                lines.push(b.text.clone());
                lines.extend(b.variants.iter().cloned());
                lines.extend(b.skills.iter().cloned());
            }
            lines.extend(e.fact_lines().map(str::to_owned));
        }
        for g in &s.groups {
            lines.push(format!("{}: {}", g.label, g.items.join(", ")));
        }
    }
    lines.retain(|l| !l.trim().is_empty());
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn norms(text: &str) -> Vec<String> {
        numbers(text).into_iter().map(|n| n.norm).collect()
    }

    #[test]
    fn reads_numbers() {
        assert_eq!(
            norms(
                "cut 900 ms to 120ms, $1,200 saved, 40% faster, 3 percent, 10x, 5k users, 100+ tickets in 2025."
            ),
            [
                "900", "120", "$1200", "40%", "3%", "10x", "5k", "100+", "2025"
            ]
        );
        assert!(norms("k8s, ES6, EC2 and HTML5").is_empty());
        assert_eq!(numbers("about 40 percent")[0].shown, "40 percent");
    }

    #[test]
    fn sentence_starts() {
        let w = words("Built Rust tools. Shipped (AWS) Lambda's: Data");
        assert_eq!(
            w,
            [
                ("Built", true),
                ("Rust", false),
                ("tools", false),
                ("Shipped", true),
                ("AWS", false),
                ("Lambda", false),
                ("Data", true)
            ]
        );
    }

    #[test]
    fn word_forms() {
        assert!(swap_warning("Collaborated", "collaboration").is_some());
        assert!(swap_warning("collaboration", "Partnered").is_some());
        assert!(swap_warning("Collaborated", "Partnered").is_none());
        assert!(swap_warning("automated tests", "automated testing").is_none());
        assert!(swap_warning("Collaborated", "").is_none());
    }

    #[test]
    fn confirmed_names_pass_the_name_checks_only() {
        use crate::requirements::{Requirements, Seniority};
        use crate::suggestions::{Suggestions, Verdict};
        let bank: Bank = serde_json::from_value(serde_json::json!({
            "schema": 2, "name": "Jane Doe", "contact": [], "sections": []
        }))
        .unwrap();
        let s = Suggestions {
            verdict: Verdict::Tailor,
            headline: String::new(),
            evidence: vec![],
            keywords: vec![],
            bullets: vec![],
            skills: vec![],
            bank: vec![],
            order: vec![],
            summary: None,
            gaps: vec![],
        };
        let req = Requirements {
            role: String::new(),
            company: String::new(),
            short_role: String::new(),
            location: String::new(),
            seniority: Seniority::Entry,
            skills: vec![],
            title_terms: vec![],
            years_min: None,
            degree: None,
            certifications: vec![],
            knockouts: vec![],
        };
        let plain = Grounding::new(&bank, &s, &req);
        assert!(!plain.check_item("Bash").is_empty());
        let g = Grounding::new(&bank, &s, &req).confirmed(&["bash".to_owned()]);
        assert_eq!(g.check_item("Bash"), Vec::<String>::new());
        assert_eq!(g.check("Wrote Bash scripts", &[]), Vec::<String>::new());
        assert_eq!(g.check_item("Kafka"), ["“Kafka” is not in your bank"]);
        assert_eq!(
            g.check("Wrote 40 Bash scripts", &[]).len(),
            1,
            "the number still needs a source"
        );
    }
}

//! Literal matching, as ATS Fit's `literal_fit` (ARCHITECTURE.md §4): a requirement is on the page when its term, or
//! one of its aliases, appears there as a whole term, case-insensitively. Near forms (singular/plural, hyphen, space or
//! joined, `&`/`and`) are found too, and reported separately: an ATS that matches literally may miss them.
//!
//! Whole term: where a term begins or ends with a letter or digit, the page must not continue the word there. Letters
//! and digits continue a word; so do `+` and `#` after it (`C` is not in `C++` or `C#`) and a `.` joined to a letter
//! or digit on its far side (`JS` is not in `Node.js`, `Node` is not in `Node.js`). A term that begins or ends with
//! punctuation carries its own boundary there, so `.NET`, `C++`, `C#`, `Node.js` and `CI/CD` match as written.

use std::sync::LazyLock;

use serde::{Deserialize, Serialize};

use crate::requirements::{Skill, SkillKind};
use crate::resume::Resume;
use crate::text;

/// Text to match against, in blocks that each belong to a section (`None` for the name and contact line).
#[derive(Debug, Clone)]
pub struct Page {
    blocks: Vec<Block>,
}

#[derive(Debug, Clone)]
struct Block {
    section: Option<String>,
    /// The text, lowercased and with whitespace runs collapsed to one space.
    norm: Vec<char>,
    /// The same characters as printed, index for index with `norm`.
    orig: Vec<char>,
}

/// Where a term was found: the text as printed and the section it is in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Hit {
    pub text: String,
    pub section: Option<String>,
}

impl Page {
    pub fn new<'a>(blocks: impl IntoIterator<Item = (Option<&'a str>, &'a str)>) -> Self {
        let blocks = blocks
            .into_iter()
            .map(|(section, text)| {
                let (mut norm, mut orig) = (Vec::new(), Vec::new());
                for c in text.chars() {
                    let c = if c.is_whitespace() { ' ' } else { c };
                    if c == ' ' && norm.last().is_none_or(|&l| l == ' ') {
                        continue;
                    }
                    norm.push(fold(c));
                    orig.push(c);
                }
                if norm.last() == Some(&' ') {
                    norm.pop();
                    orig.pop();
                }
                Block {
                    section: section.map(str::to_owned),
                    norm,
                    orig,
                }
            })
            .collect();
        Self { blocks }
    }

    /// The resume's [`text::resume_text`], by section.
    pub fn of_resume(r: &Resume) -> Self {
        let blocks = text::blocks(r);
        Self::new(blocks.iter().map(|(s, t)| (*s, t.as_str())))
    }

    /// Plain text with no sections (a PDF's text layer).
    pub fn of_text(text: &str) -> Self {
        Self::new([(None, text)])
    }

    /// The first place `term` appears as a whole term, case-insensitively.
    pub fn find(&self, term: &str) -> Option<Hit> {
        let needle = key(term).chars().collect::<Vec<_>>();
        self.blocks.iter().find_map(|b| {
            let at = find_in(&b.norm, &needle)?;
            Some(Hit {
                text: b.orig[at..at + needle.len()].iter().collect(),
                section: b.section.clone(),
            })
        })
    }
}

/// How a requirement was found on the page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    /// The term itself.
    Exact,
    /// One of its aliases, from the requirement or `data/aliases.json`.
    Alias,
    /// Only a near form of the term or an alias.
    Near,
    Missing,
}

impl Status {
    /// Found as an ATS that matches literally would find it.
    pub fn literal(self) -> bool {
        matches!(self, Self::Exact | Self::Alias)
    }

    /// Found, counting near forms.
    pub fn with_near(self) -> bool {
        self != Self::Missing
    }
}

/// One requirement skill against the page.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SkillMatch {
    pub term: String,
    pub kind: SkillKind,
    pub priority: u8,
    pub required: bool,
    pub group: Option<String>,
    /// 3/2/1 by priority, ×1.5 when required.
    pub weight: f64,
    pub status: Status,
    /// The matched text as printed; `None` when missing.
    pub text: Option<String>,
    /// The section it was found in; `None` when missing or found in the name or contact line.
    pub section: Option<String>,
}

/// A skill's weight: 3, 2, 1 for priority 1, 2, 3, ×1.5 when required.
pub fn weight(priority: u8, required: bool) -> f64 {
    let base = match priority {
        0 | 1 => 3.0,
        2 => 2.0,
        _ => 1.0,
    };
    if required { base * 1.5 } else { base }
}

/// `skill` on `page`.
pub fn match_skill(page: &Page, skill: &Skill) -> SkillMatch {
    let (status, hit) = find_requirement(page, &skill.term, &skill.aliases);
    let (text, section) = hit.map_or((None, None), |h| (Some(h.text), h.section));
    SkillMatch {
        term: skill.term.clone(),
        kind: skill.kind,
        priority: skill.priority,
        required: skill.required,
        group: skill.group.clone(),
        weight: weight(skill.priority, skill.required),
        status,
        text,
        section,
    }
}

/// A requirement (`term`, with its own `aliases`) on `page`: the term, else an alias (its own, then the dictionary's),
/// else a near form of the term or an alias.
pub fn find_requirement(page: &Page, term: &str, aliases: &[String]) -> (Status, Option<Hit>) {
    if let Some((status, hit)) = find_literal(page, term, aliases) {
        return (status, Some(hit));
    }
    let near = std::iter::once(term)
        .chain(all_aliases(term, aliases).iter().map(String::as_str))
        .flat_map(near_forms)
        .find_map(|f| page.find(&f));
    match near {
        Some(h) => (Status::Near, Some(h)),
        None => (Status::Missing, None),
    }
}

/// [`find_requirement`] without near forms: [`Status::Exact`] or [`Status::Alias`] and where, else `None`.
pub fn find_literal(page: &Page, term: &str, aliases: &[String]) -> Option<(Status, Hit)> {
    if key(term).is_empty() {
        return None;
    }
    if let Some(h) = page.find(term) {
        return Some((Status::Exact, h));
    }
    all_aliases(term, aliases)
        .iter()
        .find_map(|a| page.find(a))
        .map(|h| (Status::Alias, h))
}

/// `own` aliases of `term`, then every other form in a `data/aliases.json` group that holds the term or one of them;
/// without duplicates or the term itself.
pub fn all_aliases(term: &str, own: &[String]) -> Vec<String> {
    let mut keys = vec![key(term)];
    let mut out = Vec::new();
    let mut push = |form: &str, keys: &mut Vec<String>| {
        let k = key(form);
        if !k.is_empty() && !keys.contains(&k) {
            keys.push(k);
            out.push(form.to_owned());
        }
    };
    for a in own {
        push(a, &mut keys);
    }
    let named = keys.clone();
    for group in dictionary() {
        if group.iter().any(|(k, _)| named.contains(k)) {
            for (_, form) in group {
                push(form, &mut keys);
            }
        }
    }
    out
}

/// Near forms of `term`, lowercased: its singular or plural, each spaced, hyphenated or joined, with `&` or `and`.
pub fn near_forms(term: &str) -> Vec<String> {
    let t = key(term);
    if t.is_empty() {
        return vec![];
    }
    let mut numbers = vec![t.clone()];
    // Short words (`Go`, `JS`, `C`) have no plural an ATS would mean.
    let last = t.rsplit(' ').next().unwrap_or(&t);
    if last.chars().count() >= 3 {
        let strip = |n: usize| t[..t.len() - n].to_owned();
        if t.ends_with("ies") {
            numbers.push(format!("{}y", strip(3)));
        } else if ["ses", "xes", "zes", "ches", "shes"]
            .iter()
            .any(|e| t.ends_with(e))
        {
            // `boxes` → `box`, `databases` → `database`.
            numbers.extend([strip(2), strip(1)]);
        } else if t.ends_with('s') && !t.ends_with("ss") {
            numbers.push(strip(1));
        } else if t.ends_with('y') && !t[..t.len() - 1].ends_with(['a', 'e', 'i', 'o', 'u']) {
            numbers.push(format!("{}ies", strip(1)));
        } else {
            numbers.extend([format!("{t}s"), format!("{t}es")]);
        }
    }
    let mut out: Vec<String> = Vec::new();
    for n in &numbers {
        for v in [
            n.clone(),
            n.replace('-', " "),
            n.replace(' ', "-"),
            n.replace([' ', '-'], ""),
            n.replace(" & ", " and "),
            n.replace(" and ", " & "),
        ] {
            if v != t && !v.is_empty() && !out.contains(&v) {
                out.push(v);
            }
        }
    }
    out
}

/// `s` as matching compares it: lowercased, whitespace runs collapsed, trimmed.
pub(crate) fn key(s: &str) -> String {
    s.split_whitespace()
        .map(|w| w.chars().map(fold).collect::<String>())
        .collect::<Vec<_>>()
        .join(" ")
}

/// One character, lowercased (when that keeps it one character) and with curly apostrophes straightened.
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

fn is_word(c: char) -> bool {
    c.is_alphanumeric()
}

/// The first whole-term occurrence of `needle` in `hay`.
fn find_in(hay: &[char], needle: &[char]) -> Option<usize> {
    let n = needle.len();
    if n == 0 || n > hay.len() {
        return None;
    }
    let (open_start, open_end) = (is_word(needle[0]), is_word(needle[n - 1]));
    (0..=hay.len() - n).find(|&s| {
        hay[s..s + n] == *needle
            && (!open_start || starts_word(hay, s))
            && (!open_end || ends_word(hay, s + n))
    })
}

/// Nothing before `at` continues the word.
fn starts_word(hay: &[char], at: usize) -> bool {
    match at.checked_sub(1).map(|i| hay[i]) {
        None => true,
        Some('.') => !(at >= 2 && is_word(hay[at - 2])),
        Some(c) => !is_word(c),
    }
}

/// Nothing from `at` on continues the word.
fn ends_word(hay: &[char], at: usize) -> bool {
    match hay.get(at) {
        None => true,
        Some('+' | '#') => false,
        Some('.') => !hay.get(at + 1).is_some_and(|&c| is_word(c)),
        Some(&c) => !is_word(c),
    }
}

#[derive(Deserialize)]
struct AliasFile {
    groups: Vec<Vec<String>>,
}

/// `data/aliases.json`: each group's forms, with their [`key`].
static DICTIONARY: LazyLock<Vec<Vec<(String, String)>>> = LazyLock::new(|| {
    let file: AliasFile = serde_json::from_str(include_str!("../../../data/aliases.json"))
        .expect("data/aliases.json is a valid alias dictionary");
    file.groups
        .into_iter()
        .map(|g| g.into_iter().map(|f| (key(&f), f)).collect())
        .collect()
});

fn dictionary() -> &'static [Vec<(String, String)>] {
    &DICTIONARY
}

#[cfg(test)]
mod tests {
    use super::*;

    fn on(text: &str, term: &str) -> Option<String> {
        Page::of_text(text).find(term).map(|h| h.text)
    }

    fn skill(term: &str, aliases: &[&str]) -> Skill {
        Skill {
            term: term.into(),
            aliases: aliases.iter().map(|&a| a.into()).collect(),
            kind: SkillKind::Hard,
            priority: 1,
            required: true,
            group: None,
        }
    }

    #[test]
    fn whole_terms_only() {
        assert_eq!(on("JavaScript, TypeScript", "Java"), None);
        assert_eq!(on("Java and JavaScript", "java").as_deref(), Some("Java"));
        assert_eq!(on("C++, C#, Objective-C++", "C"), None);
        assert_eq!(on("C/C++", "C").as_deref(), Some("C"));
        assert_eq!(on("Languages: C, Rust", "C").as_deref(), Some("C"));
        assert_eq!(on("Built with Node.js", "Node"), None);
        assert_eq!(on("Built with Node.js", "JS"), None);
        assert_eq!(on("Go, Golang", "Go").as_deref(), Some("Go"));
        assert_eq!(on("gopher", "go"), None);
        assert_eq!(on("Rust.", "Rust").as_deref(), Some("Rust"));
        assert_eq!(on("Python's typing", "Python").as_deref(), Some("Python"));
    }

    #[test]
    fn punctuated_terms_match_as_written() {
        assert_eq!(
            on("Built with Node.js.", "node.js").as_deref(),
            Some("Node.js")
        );
        assert_eq!(on("ASP.NET Core", ".NET").as_deref(), Some(".NET"));
        assert_eq!(on("C++17 and C#", "C++").as_deref(), Some("C++"));
        assert_eq!(on("C++17 and C#", "c#").as_deref(), Some("C#"));
        assert_eq!(
            on("GitHub CI/CD pipelines", "CI/CD").as_deref(),
            Some("CI/CD")
        );
        assert_eq!(on("CI/CD", "CI").as_deref(), Some("CI"));
        assert_eq!(on("C# only", "C++"), None);
    }

    #[test]
    fn case_and_whitespace_fold() {
        assert_eq!(
            on("MACHINE\n  learning", "Machine Learning").as_deref(),
            Some("MACHINE learning")
        );
        assert_eq!(
            on("Dean\u{2019}s List", "dean's list").as_deref(),
            Some("Dean\u{2019}s List")
        );
    }

    #[test]
    fn aliases_from_skill_and_dictionary() {
        let page = Page::of_text("Skills: JS, k8s, Postgres, Golang, RESTful services");
        for (term, alias, found) in [
            ("JavaScript", None, "JS"),
            ("Kubernetes", None, "k8s"),
            ("PostgreSQL", None, "Postgres"),
            ("Go", None, "Golang"),
            ("REST", None, "RESTful"),
            ("Containers", Some("k8s"), "k8s"),
        ] {
            let s = skill(term, alias.as_slice());
            let m = match_skill(&page, &s);
            assert_eq!(m.status, Status::Alias, "{term}");
            assert_eq!(m.text.as_deref(), Some(found), "{term}");
        }
        let m = match_skill(&Page::of_text("Go Lang"), &skill("Golang", &[]));
        assert_eq!(m.status, Status::Alias);
        let m = match_skill(
            &Page::of_text("continuous integration and continuous delivery"),
            &skill("CI/CD", &[]),
        );
        assert_eq!(m.status, Status::Alias);
        // The term itself wins over an alias.
        let m = match_skill(
            &Page::of_text("JS and JavaScript"),
            &skill("JavaScript", &[]),
        );
        assert_eq!(
            (m.status, m.text.as_deref()),
            (Status::Exact, Some("JavaScript"))
        );
    }

    #[test]
    fn near_forms_are_separate() {
        let near = |text: &str, term: &str| {
            let m = match_skill(&Page::of_text(text), &skill(term, &[]));
            (m.status, m.text)
        };
        assert_eq!(
            near("led code review sessions", "Code reviews"),
            (Status::Near, Some("code review".into()))
        );
        assert_eq!(near("web APIs", "Web API").0, Status::Near);
        // A plural the dictionary lists is an alias, not a near form.
        assert_eq!(near("REST APIs", "REST API").0, Status::Alias);
        assert_eq!(near("relational databases", "database").0, Status::Near);
        assert_eq!(near("two technologies", "technology").0, Status::Near);
        assert_eq!(near("fullstack apps", "full-stack").0, Status::Near);
        assert_eq!(near("full stack apps", "Full-Stack").0, Status::Near);
        assert_eq!(near("full-stack apps", "full stack").0, Status::Near);
        assert_eq!(
            near("research and development", "Research & Development").0,
            Status::Near
        );
        assert_eq!(near("R & D", "R and D").0, Status::Near);
        // Short terms have no plural: `JS` is not near `J`, `Go` is not near `goes`.
        assert_eq!(near("J", "JS").0, Status::Missing);
        assert_eq!(near("it goes", "Go").0, Status::Missing);
        assert_eq!(near("nothing here", "Kafka"), (Status::Missing, None));
    }

    #[test]
    fn hit_names_the_section() {
        let page = Page::new([
            (None, "Jane Doe\njane@example.com"),
            (Some("work"), "WORK\nBuilt a Rust service"),
            (Some("skills"), "Languages: Rust, Python"),
        ]);
        assert_eq!(
            page.find("python"),
            Some(Hit {
                text: "Python".into(),
                section: Some("skills".into())
            })
        );
        assert_eq!(page.find("rust").unwrap().section.as_deref(), Some("work"));
        assert_eq!(page.find("jane").unwrap().section, None);
    }

    #[test]
    fn weights() {
        assert_eq!(weight(1, true), 4.5);
        assert_eq!(weight(1, false), 3.0);
        assert_eq!(weight(2, true), 3.0);
        assert_eq!(weight(3, false), 1.0);
    }

    #[test]
    fn dictionary_loads() {
        assert!(dictionary().len() > 20);
        assert!(all_aliases("TypeScript", &[]).contains(&"TS".to_owned()));
        // The skill's own aliases come first, then the dictionary's, without the term itself.
        assert_eq!(
            all_aliases("JS", &["JavaScript".into()])[..2],
            ["JavaScript".to_owned(), "ECMAScript".to_owned()]
        );
    }
}

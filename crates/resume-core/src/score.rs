//! The composite score (ARCHITECTURE.md §4): six components measured from the page's text, none from the model.
//!
//! | Component | Weight | Value |
//! |---|---|---|
//! | `hard_tool` | 45 | Share of requirement weight (3/2/1 by priority, ×1.5 required) of hard skills and tools on the page |
//! | `domain_soft` | 10 | The same for domain and soft skills |
//! | `title` | 10 | Best title term: 1 when a subtitle or summary states it, 0.5 when it states one of its words |
//! | `experience` | 15 | `min(1, years / years_min)`, years from the union of months of entries in experience sections |
//! | `education` | 10 | Mean of the degree part and the certifications part, whichever the posting asks for |
//! | `format` | 10 | Share of format checks passed |
//!
//! A one-of skill group counts once: met when any member is, weighing as its heaviest member, in the component of
//! that member's kind. Components the posting doesn't specify drop out and the rest renormalize.
//!
//! **Degree part:** the best education entry whose subtitle or name states a degree (B.S., BS, Bachelor of…, B.A.,
//! M.S., Master, MBA, PhD, Associate of…, …): 1 when its level is at least the required one and it names one of the
//! required fields (or none are required), 0.5 when only the level is met, 0 when the level is below. When the posting
//! accepts equivalent experience, experience stands in: 2, 4, 6 or 8 years (associate, bachelor, master, PhD) of
//! experience-section months count as the degree, pro rata, and the part is the larger of the two.

use serde::{Deserialize, Serialize};

use crate::dates::PRESENT;
use crate::matcher::{self, Page, SkillMatch, find_literal};
use crate::requirements::{DegreeLevel, Knockout, Requirements, SkillKind};
use crate::resume::{Entry, Resume, SectionKind};
use crate::text;

/// What the score needs to know about the rendered page beyond its text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageFacts {
    /// Pages the rendered resume takes.
    pub pages: u32,
    /// The master's page limit.
    pub page_limit: u32,
    /// The template is single-column real text (its manifest's `atsSafe`).
    pub ats_safe: bool,
    /// The exported PDF's text layer, when there is one to read back.
    pub pdf_text: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Score {
    /// 0–100, one decimal: the present components' weighted mean.
    pub total: f64,
    /// The same, counting near forms of skills as matches.
    pub total_near: f64,
    pub components: Vec<Component>,
    /// Every requirement skill, in the posting's order.
    pub skills: Vec<SkillMatch>,
    /// For you to check; not scored.
    pub knockouts: Vec<Knockout>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Component {
    /// `hard_tool`, `domain_soft`, `title`, `experience`, `education` or `format`.
    pub id: &'static str,
    /// Default weight; the total renormalizes over present components.
    pub weight: f64,
    /// The posting specifies it; an absent component doesn't count.
    pub present: bool,
    /// 0–1.
    pub value: f64,
    /// 0–1, counting near forms (differs from `value` only for the skill components).
    pub value_near: f64,
    pub detail: String,
}

/// `resume` against `req`, with `facts` about its rendered page, on `today` (`(year, month)`, the month `present`
/// stands for).
pub fn score(resume: &Resume, req: &Requirements, facts: &PageFacts, today: (u32, u32)) -> Score {
    let page = Page::of_resume(resume);
    let skills: Vec<SkillMatch> = req
        .skills
        .iter()
        .map(|s| matcher::match_skill(&page, s))
        .collect();
    let units = units(&skills);
    let exp = Experience::of(resume, today);
    let components = vec![
        skills_component(
            "hard_tool",
            45.0,
            &units,
            [SkillKind::Hard, SkillKind::Tool],
            "hard skills or tools",
        ),
        skills_component(
            "domain_soft",
            10.0,
            &units,
            [SkillKind::Domain, SkillKind::Soft],
            "domain or soft skills",
        ),
        title_component(resume, &req.title_terms),
        experience_component(&exp, req.years_min),
        education_component(resume, &page, req, &exp, today),
        format_component(resume, facts, req, &skills),
    ];
    Score {
        total: total(&components, |c| c.value),
        total_near: total(&components, |c| c.value_near),
        components,
        skills,
        knockouts: req.knockouts.clone(),
    }
}

fn total(components: &[Component], value: impl Fn(&Component) -> f64) -> f64 {
    let (sum, weight) = components
        .iter()
        .filter(|c| c.present)
        .fold((0.0, 0.0), |(s, w), c| {
            (s + c.weight * value(c), w + c.weight)
        });
    if weight == 0.0 {
        0.0
    } else {
        round1(100.0 * sum / weight)
    }
}

fn round1(x: f64) -> f64 {
    (x * 10.0).round() / 10.0
}

fn absent(id: &'static str, weight: f64, detail: impl Into<String>) -> Component {
    Component {
        id,
        weight,
        present: false,
        value: 0.0,
        value_near: 0.0,
        detail: detail.into(),
    }
}

/// A scored requirement: one skill, or a one-of group of them.
struct Unit<'a> {
    members: Vec<&'a SkillMatch>,
}

impl Unit<'_> {
    /// The heaviest member (the first of equals): its weight and kind are the unit's.
    fn lead(&self) -> &SkillMatch {
        self.members
            .iter()
            .copied()
            .reduce(|a, b| if b.weight > a.weight { b } else { a })
            .expect("a unit has a member")
    }

    fn met(&self) -> bool {
        self.members.iter().any(|m| m.status.literal())
    }

    fn met_near(&self) -> bool {
        self.members.iter().any(|m| m.status.with_near())
    }

    fn label(&self) -> String {
        self.members
            .iter()
            .map(|m| m.term.as_str())
            .collect::<Vec<_>>()
            .join(" or ")
    }
}

/// Skills grouped into scored requirements, in the posting's order (a group where its first member is).
fn units(skills: &[SkillMatch]) -> Vec<Unit<'_>> {
    let mut units: Vec<Unit> = Vec::new();
    for s in skills {
        let grouped = s.group.as_deref().and_then(|g| {
            units
                .iter_mut()
                .find(|u| u.members[0].group.as_deref() == Some(g))
        });
        match grouped {
            Some(u) => u.members.push(s),
            None => units.push(Unit { members: vec![s] }),
        }
    }
    units
}

fn skills_component(
    id: &'static str,
    weight: f64,
    units: &[Unit],
    kinds: [SkillKind; 2],
    what: &str,
) -> Component {
    let mine: Vec<&Unit> = units
        .iter()
        .filter(|u| kinds.contains(&u.lead().kind))
        .collect();
    if mine.is_empty() {
        return absent(id, weight, format!("the posting lists no {what}"));
    }
    let (mut all, mut met, mut met_near) = (0.0, 0.0, 0.0);
    for u in &mine {
        let w = u.lead().weight;
        all += w;
        if u.met() {
            met += w;
        }
        if u.met_near() {
            met_near += w;
        }
    }
    let count = mine.iter().filter(|u| u.met()).count();
    let mut detail = format!(
        "{count} of {} on the page ({} of {} weight)",
        mine.len(),
        num(met),
        num(all)
    );
    let missing: Vec<String> = mine
        .iter()
        .filter(|u| !u.met())
        .map(|u| u.label())
        .collect();
    if !missing.is_empty() {
        detail += &format!("; missing: {}", missing.join(", "));
    }
    let near: Vec<String> = mine
        .iter()
        .filter(|u| !u.met())
        .flat_map(|u| &u.members)
        .filter_map(|m| {
            m.text
                .as_ref()
                .map(|t| format!("{} \u{2248} \"{t}\"", m.term))
        })
        .collect();
    if !near.is_empty() {
        detail += &format!("; near forms: {}", near.join(", "));
    }
    Component {
        id,
        weight,
        present: true,
        value: met / all,
        value_near: met_near / all,
        detail,
    }
}

/// A number as short as it reads: `4.5`, `12`.
fn num(x: f64) -> String {
    format!("{}", round1(x))
}

fn title_component(r: &Resume, terms: &[String]) -> Component {
    const ID: &str = "title";
    const WEIGHT: f64 = 10.0;
    let terms: Vec<&String> = terms
        .iter()
        .filter(|t| !matcher::key(t).is_empty())
        .collect();
    if terms.is_empty() {
        return absent(ID, WEIGHT, "the posting gives no title terms");
    }
    // Subtitles of every entry, and summary sections whole.
    let mut blocks: Vec<(Option<&str>, String)> = Vec::new();
    for s in &r.sections {
        if s.kind == Some(SectionKind::Summary) {
            blocks.push((Some(&s.id), text::section_text(s)));
        }
        for e in s.entries.iter().filter(|e| !e.subtitle.is_empty()) {
            blocks.push((Some(&s.id), e.subtitle.clone()));
        }
    }
    let page = Page::new(blocks.iter().map(|(s, t)| (*s, t.as_str())));
    let mut best = 0.0;
    let mut notes = Vec::new();
    for term in terms {
        let (value, note) = if let Some((_, hit)) = find_literal(&page, term, &[]) {
            (1.0, format!("\"{term}\" stated: \"{}\"", hit.text))
        } else {
            let words: Vec<String> = title_words(term)
                .into_iter()
                .filter_map(|w| page.find(w).map(|h| h.text))
                .collect();
            if words.is_empty() {
                (0.0, format!("\"{term}\" not stated"))
            } else {
                (
                    0.5,
                    format!("\"{term}\" partly stated: \"{}\"", words.join("\", \"")),
                )
            }
        };
        if value > best {
            best = value;
        }
        notes.push(note);
    }
    Component {
        id: ID,
        weight: WEIGHT,
        present: true,
        value: best,
        value_near: best,
        detail: format!("in a subtitle or summary: {}", notes.join("; ")),
    }
}

/// The words of a title term that make a partial match: not joiners or seniority words, at least two letters.
fn title_words(term: &str) -> Vec<&str> {
    const SKIP: [&str; 21] = [
        "and",
        "or",
        "of",
        "the",
        "a",
        "an",
        "in",
        "for",
        "to",
        "with",
        "senior",
        "sr",
        "junior",
        "jr",
        "lead",
        "staff",
        "principal",
        "intern",
        "entry",
        "level",
        "ii",
    ];
    term.split(|c: char| c.is_whitespace() || "/,-&()".contains(c))
        .filter(|w| w.chars().count() >= 2 && !SKIP.contains(&matcher::key(w).as_str()))
        .collect()
}

/// Months of experience: the union of entries' months in experience sections.
struct Experience {
    months: u32,
    /// Entries counted.
    dated: usize,
    /// Experience entries without readable months, or starting after today.
    undated: usize,
}

impl Experience {
    fn of(r: &Resume, today: (u32, u32)) -> Self {
        let now = today.0 * 12 + today.1.saturating_sub(1);
        let mut spans = Vec::new();
        let mut undated = 0;
        let entries = r
            .sections
            .iter()
            .filter(|s| s.kind == Some(SectionKind::Experience))
            .flat_map(|s| &s.entries);
        for e in entries {
            let start = e.start.as_deref().and_then(month_index);
            let end = e.end.as_deref().and_then(|m| {
                if m == PRESENT {
                    Some(now)
                } else {
                    month_index(m)
                }
            });
            match (start, end) {
                (Some(s), Some(end)) if s <= end.min(now) => spans.push((s, end.min(now))),
                _ => undated += 1,
            }
        }
        let dated = spans.len();
        spans.sort_unstable();
        let mut months = 0;
        let mut open: Option<(u32, u32)> = None;
        for (s, e) in spans {
            open = match open {
                Some((os, oe)) if s <= oe + 1 => Some((os, oe.max(e))),
                Some((os, oe)) => {
                    months += oe - os + 1;
                    Some((s, e))
                }
                None => Some((s, e)),
            };
        }
        if let Some((os, oe)) = open {
            months += oe - os + 1;
        }
        Self {
            months,
            dated,
            undated,
        }
    }

    fn years(&self) -> f64 {
        f64::from(self.months) / 12.0
    }
}

/// Months of experience in `r`'s experience sections: the union of its entries' `start`–`end` months, inclusive, with
/// `present` (and any later month) as `today`. Overlapping entries count once; entries without months don't count.
pub fn experience_months(r: &Resume, today: (u32, u32)) -> u32 {
    Experience::of(r, today).months
}

/// `YYYY-MM` as a month count.
fn month_index(s: &str) -> Option<u32> {
    let (y, m) = s.split_once('-')?;
    let (y, m) = (y.parse::<u32>().ok()?, m.parse::<u32>().ok()?);
    (y >= 1000 && (1..=12).contains(&m)).then(|| y * 12 + m - 1)
}

fn experience_component(exp: &Experience, years_min: Option<u32>) -> Component {
    const ID: &str = "experience";
    const WEIGHT: f64 = 15.0;
    let Some(need) = years_min.filter(|&y| y > 0) else {
        return absent(ID, WEIGHT, "the posting asks for no minimum years");
    };
    let value = (exp.years() / f64::from(need)).min(1.0);
    let mut detail = format!(
        "{:.1} years ({} months, overlaps once) from {} dated {} in experience sections vs {need} required",
        exp.years(),
        exp.months,
        exp.dated,
        plural(exp.dated, "entry", "entries"),
    );
    if exp.undated > 0 {
        detail += &format!(
            "; {} {} without readable dates not counted",
            exp.undated,
            plural(exp.undated, "entry", "entries")
        );
    }
    Component {
        id: ID,
        weight: WEIGHT,
        present: true,
        value,
        value_near: value,
        detail,
    }
}

fn plural<'a>(n: usize, one: &'a str, many: &'a str) -> &'a str {
    if n == 1 { one } else { many }
}

/// The highest degree level `text` states: `PhD`, `Ph.D.`, `Doctorate`; `Master's`, `M.S.`, `MS`, `MBA`, `M.A.`;
/// `Bachelor of…`, `B.S.`, `BS`, `B.A.`, `BFA`; `Associate of…`, `A.S.`, `A.A.S.`. `None` when it states none.
pub fn degree_level(text: &str) -> Option<DegreeLevel> {
    const FORMS: [(DegreeLevel, &[&str]); 4] = [
        (
            DegreeLevel::Phd,
            &[
                "phd",
                "ph.d",
                "ph. d",
                "doctorate",
                "doctoral",
                "doctor of philosophy",
                "d.phil",
                "dphil",
            ],
        ),
        (
            DegreeLevel::Master,
            &[
                "master", "masters", "master's", "m.s", "ms", "m.sc", "msc", "m.a", "m.eng",
                "meng", "mba", "m.b.a", "mfa", "m.f.a", "m.ed", "mcs", "m.c.s", "mse", "m.s.e",
            ],
        ),
        (
            DegreeLevel::Bachelor,
            &[
                "bachelor",
                "bachelors",
                "bachelor's",
                "b.s",
                "bs",
                "b.sc",
                "bsc",
                "b.a",
                "ba",
                "b.eng",
                "beng",
                "b.e",
                "bfa",
                "b.f.a",
                "bba",
                "b.b.a",
                "bse",
                "b.s.e",
                "bscs",
                "b.tech",
                "btech",
            ],
        ),
        (
            DegreeLevel::Associate,
            &[
                "associate of",
                "associates",
                "associate's",
                "associate degree",
                "a.a.s",
                "aas",
                "a.s",
                "a.a",
            ],
        ),
    ];
    let page = Page::of_text(text);
    FORMS
        .iter()
        .find(|(_, forms)| forms.iter().any(|f| page.find(f).is_some()))
        .map(|(level, _)| *level)
}

fn level_name(l: DegreeLevel) -> &'static str {
    match l {
        DegreeLevel::Associate => "associate",
        DegreeLevel::Bachelor => "bachelor",
        DegreeLevel::Master => "master",
        DegreeLevel::Phd => "PhD",
    }
}

/// Years of experience that stand in for a degree when the posting accepts equivalent experience.
fn equivalent_years(l: DegreeLevel) -> f64 {
    match l {
        DegreeLevel::Associate => 2.0,
        DegreeLevel::Bachelor => 4.0,
        DegreeLevel::Master => 6.0,
        DegreeLevel::Phd => 8.0,
    }
}

fn education_component(
    r: &Resume,
    page: &Page,
    req: &Requirements,
    exp: &Experience,
    today: (u32, u32),
) -> Component {
    const ID: &str = "education";
    const WEIGHT: f64 = 10.0;
    let certs: Vec<&String> = req
        .certifications
        .iter()
        .filter(|c| !matcher::key(c).is_empty())
        .collect();
    if req.degree.is_none() && certs.is_empty() {
        return absent(
            ID,
            WEIGHT,
            "the posting asks for no degree or certification",
        );
    }
    let mut parts = Vec::new();
    let mut notes = Vec::new();
    if let Some(d) = &req.degree {
        let (mut value, mut note) = (0.0, "no degree stated in education sections".to_owned());
        let entries = r
            .sections
            .iter()
            .filter(|s| s.kind == Some(SectionKind::Education))
            .flat_map(|s| &s.entries);
        let mut found = false;
        for e in entries {
            let line = [e.subtitle.as_str(), e.name.as_str()]
                .into_iter()
                .filter(|t| !t.is_empty())
                .collect::<Vec<_>>()
                .join("\n");
            let Some(level) = degree_level(&line) else {
                continue;
            };
            // The field may sit on its own line ("Major: Computer Science"); coursework never counts as a field.
            let field_text = [line.clone()]
                .into_iter()
                .chain(e.notes.iter().filter_map(field_note))
                .collect::<Vec<_>>()
                .join("\n");
            let field = d.fields.iter().find_map(|f| {
                find_literal(&Page::of_text(&field_text), f, &[]).map(|(_, h)| (f, h.text))
            });
            let level_ok = level >= d.level;
            let v = match (level_ok, d.fields.is_empty() || field.is_some()) {
                (false, _) => 0.0,
                (true, true) => 1.0,
                (true, false) => 0.5,
            };
            let expected = if ends_after(e, today) {
                ", expected"
            } else {
                ""
            };
            let field_note = match (&field, d.fields.is_empty()) {
                (_, true) => "no field required".to_owned(),
                (Some((f, text)), _) => format!("field {f} stated: \"{text}\""),
                (None, _) => format!("none of the fields {} stated", d.fields.join(", ")),
            };
            let n = format!(
                "\"{}\" ({}{expected}) {} {}; {field_note}",
                e.subtitle.trim(),
                level_name(level),
                if level_ok { "meets" } else { "is below" },
                level_name(d.level),
            );
            if !found || v > value {
                (value, note, found) = (v, n, true);
            }
        }
        if d.equivalent {
            let need = equivalent_years(d.level);
            let stand_in = (exp.years() / need).min(1.0);
            let eq = format!(
                "equivalent experience accepted: {:.1} of {need} years",
                exp.years()
            );
            if stand_in > value {
                value = stand_in;
                note = format!("{note}; {eq} stands in");
            } else {
                note = format!("{note}; {eq}");
            }
        }
        parts.push(value);
        notes.push(format!("degree {}: {note}", num(value)));
    }
    if !certs.is_empty() {
        let (met, missing): (Vec<&String>, Vec<&String>) = certs
            .iter()
            .partition(|c| find_literal(page, c, &[]).is_some());
        parts.push(met.len() as f64 / certs.len() as f64);
        let mut note = format!("certifications {} of {}", met.len(), certs.len());
        if !missing.is_empty() {
            note += &format!(
                "; missing: {}",
                missing
                    .iter()
                    .map(|c| c.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
        notes.push(note);
    }
    let value = parts.iter().sum::<f64>() / parts.len() as f64;
    Component {
        id: ID,
        weight: WEIGHT,
        present: true,
        value,
        value_near: value,
        detail: notes.join("; "),
    }
}

/// The text of a note line that names a degree's field: labeled `Major`, `Minor`, `Concentration`, `Specialization`,
/// `Emphasis`, `Focus` or `Track`, or an unlabeled line starting with one of those and a colon.
fn field_note(n: &crate::resume::Note) -> Option<String> {
    const LABELS: [&str; 7] = [
        "major",
        "minor",
        "concentration",
        "specialization",
        "emphasis",
        "focus",
        "track",
    ];
    let label = n.label.trim().to_lowercase();
    if LABELS.contains(&label.as_str()) {
        return Some(n.text.clone());
    }
    let (head, rest) = n.text.split_once(':')?;
    (label.is_empty() && LABELS.contains(&head.trim().to_lowercase().as_str()))
        .then(|| rest.trim().to_owned())
}

/// The entry ends after `today` (a degree in progress).
fn ends_after(e: &Entry, today: (u32, u32)) -> bool {
    let now = today.0 * 12 + today.1.saturating_sub(1);
    e.end
        .as_deref()
        .is_some_and(|m| m == PRESENT || month_index(m).is_some_and(|m| m > now))
}

fn format_component(
    r: &Resume,
    facts: &PageFacts,
    req: &Requirements,
    skills: &[SkillMatch],
) -> Component {
    let mut checks: Vec<(bool, String)> = vec![
        (
            facts.ats_safe,
            if facts.ats_safe {
                "ATS-safe template"
            } else {
                "template is not ATS-safe"
            }
            .to_owned(),
        ),
        {
            let odd: Vec<&str> = r
                .sections
                .iter()
                .filter(|s| !standard_heading(s.kind, &s.title))
                .map(|s| s.title.as_str())
                .collect();
            if odd.is_empty() {
                (true, "standard headings".to_owned())
            } else {
                (false, format!("non-standard headings: {}", odd.join(", ")))
            }
        },
        {
            let ok = r
                .contact
                .iter()
                .any(|c| c.url.starts_with("mailto:") || is_email(&c.text));
            (
                ok,
                if ok {
                    "email in contact"
                } else {
                    "no email in contact"
                }
                .to_owned(),
            )
        },
        {
            let ok = r
                .contact
                .iter()
                .any(|c| c.url.starts_with("tel:") || is_phone(&c.text));
            (
                ok,
                if ok {
                    "phone in contact"
                } else {
                    "no phone in contact"
                }
                .to_owned(),
            )
        },
        (
            facts.pages <= facts.page_limit,
            format!("{} of {} pages", facts.pages, facts.page_limit),
        ),
    ];
    if let Some(pdf) = &facts.pdf_text {
        let pdf = Page::of_text(pdf);
        let lost: Vec<&str> = req
            .skills
            .iter()
            .zip(skills)
            .filter(|(s, m)| {
                m.status.literal() && find_literal(&pdf, &s.term, &s.aliases).is_none()
            })
            .map(|(s, _)| s.term.as_str())
            .collect();
        checks.push(if lost.is_empty() {
            (true, "PDF text carries every matched skill".to_owned())
        } else {
            (false, format!("PDF text lacks: {}", lost.join(", ")))
        });
    }
    let passed = checks.iter().filter(|(ok, _)| *ok).count();
    let value = passed as f64 / checks.len() as f64;
    Component {
        id: "format",
        weight: 10.0,
        present: true,
        value,
        value_near: value,
        detail: format!(
            "{passed} of {} checks: {}",
            checks.len(),
            checks
                .iter()
                .map(|(_, n)| n.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

/// A heading an ATS files under its kind: `WORK & LEADERSHIP EXPERIENCE` for experience, `PROJECTS`, `EDUCATION`,
/// `SKILLS`… Custom sections (and sections of unknown kind) have no standard heading to miss.
fn standard_heading(kind: Option<SectionKind>, title: &str) -> bool {
    let words: &[&str] = match kind {
        Some(SectionKind::Experience) => {
            &["experience", "employment", "work history", "career history"]
        }
        Some(SectionKind::Projects) => &["project"],
        Some(SectionKind::Education) => &["education", "academic"],
        Some(SectionKind::Skills) => &[
            "skill",
            "technolog",
            "technical",
            "competenc",
            "expertise",
            "tools",
        ],
        Some(SectionKind::Summary) => &["summary", "profile", "objective", "about"],
        Some(SectionKind::Certifications) => &["certific", "licens", "credential"],
        Some(SectionKind::Custom) | None => return true,
    };
    let t = title.to_lowercase();
    words.iter().any(|w| t.contains(w))
}

fn is_email(s: &str) -> bool {
    let s = s.trim();
    !s.contains(char::is_whitespace)
        && s.split_once('@').is_some_and(|(user, host)| {
            !user.is_empty() && host.contains('.') && !host.ends_with('.')
        })
}

/// A phone number, optionally after a label (`Phone: …`): 10–15 digits and only phone punctuation.
fn is_phone(s: &str) -> bool {
    let number = s.rsplit_once(':').map_or(s, |(_, n)| n);
    let digits = number.chars().filter(char::is_ascii_digit).count();
    (10..=15).contains(&digits)
        && number
            .chars()
            .all(|c| c.is_ascii_digit() || " ()+-.".contains(c))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::requirements::{Degree, Seniority, Skill};
    use crate::resume::{Contact, Section};

    fn entry(id: &str, subtitle: &str, start: Option<&str>, end: Option<&str>) -> Entry {
        Entry {
            id: id.into(),
            name: format!("{id} Inc."),
            url: String::new(),
            place: String::new(),
            dates: String::new(),
            subtitle: subtitle.into(),
            notes: vec![],
            bullets: vec![],
            start: start.map(Into::into),
            end: end.map(Into::into),
        }
    }

    fn section(id: &str, kind: SectionKind, title: &str, entries: Vec<Entry>) -> Section {
        Section {
            id: id.into(),
            title: title.into(),
            entries,
            groups: vec![],
            kind: Some(kind),
        }
    }

    fn resume(sections: Vec<Section>) -> Resume {
        Resume {
            name: "Pat Example".into(),
            contact: vec![
                Contact {
                    text: "pat@example.com".into(),
                    url: "mailto:pat@example.com".into(),
                },
                Contact {
                    text: "+1 (555) 010-0100".into(),
                    url: String::new(),
                },
            ],
            sections,
        }
    }

    fn work(entries: Vec<Entry>) -> Section {
        section("work", SectionKind::Experience, "EXPERIENCE", entries)
    }

    fn requirements(skills: Vec<Skill>) -> Requirements {
        Requirements {
            role: "Software Engineer".into(),
            company: "Example".into(),
            short_role: "swe".into(),
            location: "Remote".into(),
            seniority: Seniority::Entry,
            skills,
            title_terms: vec![],
            years_min: None,
            degree: None,
            certifications: vec![],
            knockouts: vec![],
        }
    }

    fn skill(
        term: &str,
        kind: SkillKind,
        priority: u8,
        required: bool,
        group: Option<&str>,
    ) -> Skill {
        Skill {
            term: term.into(),
            aliases: vec![],
            kind,
            priority,
            required,
            group: group.map(Into::into),
        }
    }

    const FACTS: PageFacts = PageFacts {
        pages: 1,
        page_limit: 1,
        ats_safe: true,
        pdf_text: None,
    };

    fn component<'a>(s: &'a Score, id: &str) -> &'a Component {
        s.components.iter().find(|c| c.id == id).unwrap()
    }

    #[test]
    fn experience_months_union_overlaps_and_present() {
        let r = resume(vec![
            work(vec![
                // Jan–Dec 2020: 12 months.
                entry("a", "", Some("2020-01"), Some("2020-12")),
                // Jun 2020–Mar 2021 overlaps a: adds Jan–Mar 2021, 3 months.
                entry("b", "", Some("2020-06"), Some("2021-03")),
                // Inside a: adds nothing.
                entry("c", "", Some("2020-03"), Some("2020-04")),
                // Adjacent to b (Apr 2021): joins the span, 1 month.
                entry("d", "", Some("2021-04"), Some("2021-04")),
                // Jan 2024 to present (Jun 2024): 6 months.
                entry("e", "", Some("2024-01"), Some("present")),
                // No months: not counted.
                entry("f", "", None, None),
                // Ends after today: counted to today, inside e.
                entry("g", "", Some("2024-05"), Some("2025-01")),
            ]),
            // Not an experience section: not counted.
            section(
                "projects",
                SectionKind::Projects,
                "PROJECTS",
                vec![entry("p", "", Some("2010-01"), Some("2019-12"))],
            ),
        ]);
        assert_eq!(experience_months(&r, (2024, 6)), 12 + 3 + 1 + 6);
        let exp = Experience::of(&r, (2024, 6));
        assert_eq!((exp.dated, exp.undated), (6, 1));
        // Starting after today: not counted.
        let r = resume(vec![work(vec![entry(
            "z",
            "",
            Some("2025-01"),
            Some("present"),
        )])]);
        assert_eq!(experience_months(&r, (2024, 6)), 0);
    }

    #[test]
    fn degree_levels() {
        use DegreeLevel::*;
        for (text, level) in [
            ("B.S. Computer Science", Some(Bachelor)),
            ("BS in Computer Engineering", Some(Bachelor)),
            ("Bachelor of Science, Computer Science", Some(Bachelor)),
            ("Bachelor's in Design", Some(Bachelor)),
            ("B.A. Economics", Some(Bachelor)),
            ("BFA Graphic Design", Some(Bachelor)),
            ("M.S. Computer Science", Some(Master)),
            ("MS, Data Science", Some(Master)),
            ("Master of Engineering", Some(Master)),
            ("MBA", Some(Master)),
            ("Ph.D. Physics", Some(Phd)),
            ("PhD candidate, Robotics", Some(Phd)),
            ("Associate of Applied Science", Some(Associate)),
            ("A.A.S. Networking", Some(Associate)),
            ("BS/MS Computer Science", Some(Master)),
            ("Computer Science coursework", None),
            ("Boston Business School", None),
            ("Basics of design", None),
        ] {
            assert_eq!(degree_level(text), level, "{text}");
        }
    }

    #[test]
    fn group_counts_once_at_its_heaviest_member() {
        let mut req = requirements(vec![
            skill("Rust", SkillKind::Hard, 1, true, None),
            // One of React (p2, 2.0) or Vue (p1 required, 4.5): weighs 4.5, met by React.
            skill("React", SkillKind::Hard, 2, false, Some("ui")),
            skill("Vue", SkillKind::Hard, 1, true, Some("ui")),
            // Neither member on the page: 2.0 missing.
            skill("MongoDB", SkillKind::Tool, 2, false, Some("db")),
            skill("DynamoDB", SkillKind::Tool, 2, false, Some("db")),
        ]);
        let mut e = entry("a", "", None, None);
        e.bullets = vec![crate::resume::Bullet {
            id: "a.1".into(),
            text: "Built a React app in Rust".into(),
            orig: None,
            changed: false,
            new: false,
        }];
        let r = resume(vec![work(vec![e])]);
        let s = score(&r, &req, &FACTS, (2026, 10));
        let c = component(&s, "hard_tool");
        // (4.5 + 4.5) / (4.5 + 4.5 + 2.0)
        assert!((c.value - 9.0 / 11.0).abs() < 1e-9, "{c:?}");
        assert!(
            c.detail.starts_with("2 of 3 on the page (9 of 11 weight)"),
            "{}",
            c.detail
        );
        assert!(
            c.detail.contains("missing: MongoDB or DynamoDB"),
            "{}",
            c.detail
        );
        // Every member is still reported.
        assert_eq!(s.skills.len(), 5);
        assert_eq!(s.skills[2].status, matcher::Status::Missing);
        // Nothing domain/soft: that component drops out.
        assert!(!component(&s, "domain_soft").present);
        // A group goes to its heaviest member's component.
        req.skills[2].kind = SkillKind::Soft;
        let s = score(&r, &req, &FACTS, (2026, 10));
        assert!((component(&s, "domain_soft").value - 1.0).abs() < 1e-9);
        assert!((component(&s, "hard_tool").value - 4.5 / 6.5).abs() < 1e-9);
    }

    #[test]
    fn absent_components_renormalize() {
        let mut e = entry("a", "Software Engineer", Some("2024-01"), Some("2024-12"));
        e.bullets = vec![crate::resume::Bullet {
            id: "a.1".into(),
            text: "Wrote Python".into(),
            orig: None,
            changed: false,
            new: false,
        }];
        let r = resume(vec![
            work(vec![e]),
            section(
                "edu",
                SectionKind::Education,
                "EDUCATION",
                vec![entry("u", "B.S. Computer Science", None, None)],
            ),
        ]);
        let mut req = requirements(vec![
            skill("Python", SkillKind::Hard, 1, true, None),
            skill("Go", SkillKind::Hard, 1, true, None),
        ]);
        // Hard/tool 0.5 and format 1 only: (45 × 0.5 + 10 × 1) / 55.
        let s = score(&r, &req, &FACTS, (2026, 10));
        assert_eq!(s.total, round1(100.0 * 32.5 / 55.0));
        let present: Vec<&str> = s
            .components
            .iter()
            .filter(|c| c.present)
            .map(|c| c.id)
            .collect();
        assert_eq!(present, ["hard_tool", "format"]);
        // Years and degree asked: 1 of 2 years = 0.5; bachelor in CS = 1.
        req.years_min = Some(2);
        req.degree = Some(Degree {
            level: DegreeLevel::Bachelor,
            fields: vec!["Computer Science".into()],
            equivalent: false,
        });
        let s = score(&r, &req, &FACTS, (2026, 10));
        assert_eq!(component(&s, "experience").value, 0.5);
        assert_eq!(component(&s, "education").value, 1.0);
        assert_eq!(s.total, round1(100.0 * (22.5 + 10.0 + 7.5 + 10.0) / 80.0));
    }

    #[test]
    fn degree_part_and_equivalent_experience() {
        let r = resume(vec![
            work(vec![entry("a", "", Some("2020-01"), Some("2021-12"))]),
            section(
                "edu",
                SectionKind::Education,
                "EDUCATION",
                vec![entry("u", "A.S. Information Technology", None, None)],
            ),
        ]);
        let mut req = requirements(vec![]);
        req.degree = Some(Degree {
            level: DegreeLevel::Bachelor,
            fields: vec!["Computer Science".into()],
            equivalent: false,
        });
        let edu = |req: &Requirements| {
            let s = score(&r, req, &FACTS, (2026, 10));
            component(&s, "education").value
        };
        // An associate degree is below a bachelor.
        assert_eq!(edu(&req), 0.0);
        // Equivalent accepted: 2 of 4 years stand in.
        req.degree.as_mut().unwrap().equivalent = true;
        assert_eq!(edu(&req), 0.5);
        // Level met, field not: 0.5.
        req.degree = Some(Degree {
            level: DegreeLevel::Associate,
            fields: vec!["Computer Science".into()],
            equivalent: false,
        });
        assert_eq!(edu(&req), 0.5);
        // No field required: 1.
        req.degree.as_mut().unwrap().fields.clear();
        assert_eq!(edu(&req), 1.0);
        // Certifications average with the degree part.
        req.certifications = vec!["CompTIA A+".into()];
        assert_eq!(edu(&req), 0.5);
    }

    #[test]
    fn degree_field_from_major_line_not_coursework() {
        let mut e = entry("asu", "Bachelor of Science in Engineering", None, None);
        e.notes = vec![crate::resume::Note {
            label: "".into(),
            text: "Relevant Coursework: Computer Science Fundamentals".into(),
            changed: false,
        }];
        let mut r = resume(vec![section(
            "edu",
            SectionKind::Education,
            "EDUCATION",
            vec![e],
        )]);
        let mut req = requirements(vec![]);
        req.degree = Some(Degree {
            level: DegreeLevel::Bachelor,
            fields: vec!["Computer Science".into()],
            equivalent: false,
        });
        let edu = |r: &Resume| component(&score(r, &req, &FACTS, (2026, 10)), "education").value;
        // Coursework mentioning the field doesn't state the degree's field.
        assert_eq!(edu(&r), 0.5);
        // A labeled Major line does; so does an unlabeled "Major:" line.
        let notes = &mut r.sections[0].entries[0].notes;
        notes.push(crate::resume::Note {
            label: "Major".into(),
            text: "Computer Science".into(),
            changed: false,
        });
        assert_eq!(edu(&r), 1.0);
        let notes = &mut r.sections[0].entries[0].notes;
        notes[1] = crate::resume::Note {
            label: "".into(),
            text: "Major: Computer Science".into(),
            changed: false,
        };
        assert_eq!(edu(&r), 1.0);
    }

    #[test]
    fn format_checks() {
        let mut r = resume(vec![section("misc", SectionKind::Skills, "STUFF", vec![])]);
        r.contact.pop();
        let facts = PageFacts {
            pages: 2,
            page_limit: 1,
            ats_safe: true,
            pdf_text: None,
        };
        let s = score(&r, &requirements(vec![]), &facts, (2026, 10));
        let c = component(&s, "format");
        // ATS-safe and email pass; heading, phone and pages fail.
        assert_eq!(c.value, 2.0 / 5.0);
        assert!(
            c.detail.contains("non-standard headings: STUFF"),
            "{}",
            c.detail
        );
        assert!(c.detail.contains("no phone"), "{}", c.detail);
        assert!(c.detail.contains("2 of 1 pages"), "{}", c.detail);
        assert!(is_phone("Phone: 555.010.0100"));
        assert!(!is_phone("github.com/pat-2015"));
        assert!(!is_email("pat at example.com"));
    }

    #[test]
    fn title_exact_and_partial() {
        let r = resume(vec![work(vec![entry(
            "a",
            "Software Engineering Intern",
            None,
            None,
        )])]);
        let mut req = requirements(vec![]);
        req.title_terms = vec!["Software Engineer".into()];
        let s = score(&r, &req, &FACTS, (2026, 10));
        // "Software Engineer" is not in "Software Engineering"; "Software" is.
        assert_eq!(component(&s, "title").value, 0.5);
        req.title_terms.push("Software Engineering Intern".into());
        let s = score(&r, &req, &FACTS, (2026, 10));
        assert_eq!(component(&s, "title").value, 1.0);
        req.title_terms = vec!["Senior Data Scientist".into()];
        let s = score(&r, &req, &FACTS, (2026, 10));
        assert_eq!(component(&s, "title").value, 0.0);
    }
}

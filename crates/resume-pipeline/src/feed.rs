//! The job feed's model-free steps (ARCHITECTURE.md §7, Job feed): the settings, what Auto works out from your bank and
//! masters, the pull's query, and the local check that drops jobs before any model sees them.

use std::collections::{BTreeSet, HashMap, HashSet};

use resume_core::bank::{BankBullet, SectionKind};
use resume_core::dates::PRESENT;
use resume_core::feed::{FeedJob, Origin};
use resume_core::matcher::all_aliases;
use resume_core::resume::{Entry, Section};
use resume_core::score::experience_months;
use resume_core::{Bank, Master, Resume, resolve};
use resume_ingest::fantastic::{self, TimeRange};
use resume_ingest::freehire::{LINK_SITES, Query};
use resume_ingest::linkedin;
use serde::{Deserialize, Serialize};

/// The first pull looks this many days back.
pub const FIRST_PULL_DAYS: u32 = 3;
/// Jobs a pull asks for per page (freehire's most).
pub const PAGE: u32 = 100;
/// LinkedIn searches a pull runs at most (one per master title and country).
pub const LINKEDIN_SEARCHES: usize = 6;
/// Hours between Fantastic.jobs runs: each run is paid per job, so it asks for a whole day's new jobs once a day.
pub const FANTASTIC_HOURS: i64 = 24;
/// Jobs one Fantastic.jobs run returns at most.
pub const FANTASTIC_LIMIT: u32 = 300;

/// Settings → Feed, stored under the `feed` setting key. `None` on a filter = Auto.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FeedSettings {
    pub enabled: bool,
    /// The master jobs are checked against; `None`: all masters.
    pub master: Option<String>,
    pub seniority: Option<Vec<String>>,
    pub employment: Option<Vec<String>>,
    pub category: Option<Vec<String>>,
    pub countries: Option<Vec<String>>,
    /// Only jobs that sponsor visas.
    pub needs_visa: bool,
    pub include_clearance: bool,
    /// Jobs freehire couldn't place, beside the countries'.
    pub include_unknown_location: bool,
    /// Checks a day (each an Extract call).
    pub daily_checks: u32,
    /// The best master's score (percent) that makes a fit.
    pub threshold: f64,
    pub every_hours: u32,
    /// Search LinkedIn's guest job search on each pull.
    pub linkedin: bool,
    /// Company boards polled on each pull, as their careers links (`resume_ingest::watch::Board::url`).
    pub watchlist: Vec<String>,
    /// Apify API token: Fantastic.jobs' career-site index runs once a day while one is set.
    pub apify_token: Option<String>,
}

impl Default for FeedSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            master: None,
            seniority: None,
            employment: None,
            category: None,
            countries: None,
            needs_visa: false,
            include_clearance: true,
            include_unknown_location: false,
            daily_checks: 20,
            threshold: 40.0,
            every_hours: 3,
            linkedin: true,
            watchlist: vec![],
            apify_token: None,
        }
    }
}

/// The pull's progress, stored under `feed_state`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FeedCursor {
    /// The newest `found_at` pulled from freehire: the next pull stops at it.
    pub found_at: Option<String>,
    /// The last LinkedIn search that answered: the next one asks for jobs posted since.
    pub linkedin_at: Option<String>,
    /// LinkedIn limited or refused requests: it isn't asked again before this time.
    pub linkedin_retry_at: Option<String>,
    /// The last Fantastic.jobs run that answered.
    pub fantastic_at: Option<String>,
    pub last_pull: Option<String>,
    /// Why the last pull failed, a sentence per source that did.
    pub error: Option<String>,
}

/// What Auto works out, and what the local check compares jobs with.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Profile {
    /// Experience entries' start/end, overlaps merged.
    pub years: f64,
    /// An education entry ending `present`, in the future, or within 12 months of today.
    pub student_or_recent: bool,
    pub seniority: Vec<String>,
    pub employment: Vec<String>,
    pub category: Vec<String>,
    /// `us` when a bank place is in the US; empty: no filter.
    pub countries: Vec<String>,
    /// The masters' skills as [`skill_key`]s, with their `data/aliases.json` forms.
    pub skills: Vec<String>,
    /// Lowercase words of the masters' titles and their experience entries' subtitles, stopwords out.
    pub title_words: Vec<String>,
    /// The masters' titles as written, each once: LinkedIn's and Fantastic.jobs' searches.
    pub titles: Vec<String>,
}

/// Auto's values from `bank` and `masters` on `today` (`(year, month)`).
pub fn profile(bank: &Bank, masters: &[&Master], today: (u32, u32)) -> Profile {
    let years = f64::from(experience_months(&experience_only(bank), today)) / 12.0;
    let student_or_recent = student_or_recent(bank, today);
    let seniority: &[&str] = match years {
        y if y < 1.0 => &["intern", "junior"],
        y if y < 3.0 && student_or_recent => &["intern", "junior"],
        y if y < 3.0 => &["junior"],
        y if y < 6.0 => &["middle"],
        _ => &["senior"],
    };
    let employment: &[&str] = if student_or_recent {
        &["full_time", "internship"]
    } else {
        &["full_time"]
    };

    let mut category = Vec::new();
    let mut skills = BTreeSet::new();
    let mut title_words = BTreeSet::new();
    let bullets: HashMap<&str, &BankBullet> = bank
        .sections
        .iter()
        .flat_map(|s| &s.entries)
        .flat_map(|e| &e.bullets)
        .map(|b| (b.id.as_str(), b))
        .collect();
    let mut titles: Vec<String> = Vec::new();
    for master in masters {
        for c in categories(&master.title) {
            if !category.contains(&c) {
                category.push(c);
            }
        }
        let title = master.title.trim();
        if !title.is_empty() && !titles.iter().any(|t| t.eq_ignore_ascii_case(title)) {
            titles.push(title.to_owned());
        }
        title_words.extend(words(&master.title));
        let mut names: Vec<&str> = Vec::new();
        let resolved = resolve(bank, master).ok();
        if let Some(r) = &resolved {
            for s in &r.sections {
                names.extend(s.groups.iter().flat_map(|g| &g.items).map(String::as_str));
                if s.kind == Some(SectionKind::Experience) {
                    for e in &s.entries {
                        title_words.extend(words(&e.subtitle));
                    }
                }
            }
        }
        let shown = master
            .sections
            .iter()
            .flat_map(|s| &s.entries)
            .flat_map(|e| &e.bullets)
            .filter_map(|b| bullets.get(b.id.as_str()));
        names.extend(shown.flat_map(|b| &b.skills).map(String::as_str));
        for name in names {
            skills.insert(skill_key(name));
            skills.extend(all_aliases(name, &[]).iter().map(|a| skill_key(a)));
        }
    }
    skills.remove("");

    Profile {
        years,
        student_or_recent,
        seniority: strings(seniority),
        employment: strings(employment),
        category,
        countries: if in_us(bank) {
            vec!["us".to_owned()]
        } else {
            vec![]
        },
        skills: skills.into_iter().collect(),
        title_words: title_words.into_iter().collect(),
        titles,
    }
}

/// The pull's query: pinned values else auto; fresh_only, LINK_SITES excluded, newest first; needs_visa →
/// visa_sponsorship=true; !include_clearance → requires_clearance=false.
pub fn query(settings: &FeedSettings, profile: &Profile) -> Query {
    let pick =
        |pinned: &Option<Vec<String>>, auto: &Vec<String>| pinned.as_ref().unwrap_or(auto).clone();
    Query {
        q: None,
        seniority: pick(&settings.seniority, &profile.seniority),
        employment_type: pick(&settings.employment, &profile.employment),
        category: pick(&settings.category, &profile.category),
        countries: pick(&settings.countries, &profile.countries),
        include_unknown_location: settings.include_unknown_location,
        visa_sponsorship: settings.needs_visa.then_some(true),
        requires_clearance: (!settings.include_clearance).then_some(false),
        fresh_only: true,
        open_within_days: None,
        source_exclude: strings(LINK_SITES),
        newest_first: true,
    }
}

/// The places the pull's countries name, as LinkedIn and Fantastic.jobs spell them: empty for no countries (anywhere),
/// `None` when countries are set but gliide can't name any of them (the search would look everywhere).
fn places(countries: &[String]) -> Option<Vec<String>> {
    let named: Vec<String> = countries
        .iter()
        .filter_map(|c| country_name(c))
        .map(str::to_owned)
        .collect();
    (countries.is_empty() || !named.is_empty()).then_some(named)
}

/// LinkedIn searches for a pull: one per master title and place, at most [`LINKEDIN_SEARCHES`], for jobs posted in
/// the last `within_secs`. Seniority and employment as the pull's query has them (pinned else auto) become LinkedIn's
/// experience levels and job types. None when the countries can't be named.
pub fn linkedin_searches(
    settings: &FeedSettings,
    profile: &Profile,
    within_secs: u32,
) -> Vec<linkedin::Search> {
    profile
        .titles
        .iter()
        .flat_map(|title| linkedin_search(settings, profile, title, Some(within_secs)))
        .take(LINKEDIN_SEARCHES)
        .collect()
}

/// LinkedIn searches for `keywords` under the feed's filters, one per place (none when the countries can't be named),
/// posted in the last `within_secs` when given.
pub fn linkedin_search(
    settings: &FeedSettings,
    profile: &Profile,
    keywords: &str,
    within_secs: Option<u32>,
) -> Vec<linkedin::Search> {
    let q = query(settings, profile);
    let Some(places) = places(&q.countries) else {
        return vec![];
    };
    let mut experience: Vec<u8> = q
        .seniority
        .iter()
        .flat_map(|s| linkedin_levels(s).iter().copied())
        .collect();
    experience.sort_unstable();
    experience.dedup();
    let mut job_types: Vec<char> = q
        .employment_type
        .iter()
        .filter_map(|e| linkedin_type(e))
        .collect();
    job_types.dedup();
    let places: Vec<Option<String>> = if places.is_empty() {
        vec![None]
    } else {
        places.into_iter().map(Some).collect()
    };
    places
        .into_iter()
        .map(|location| linkedin::Search {
            keywords: keywords.to_owned(),
            location,
            posted_within_secs: within_secs,
            experience: experience.clone(),
            job_types: job_types.clone(),
        })
        .collect()
}

/// LinkedIn's `f_E` levels for a feed seniority: 1 internship, 2 entry, 3 associate, 4 mid-senior, 5 director,
/// 6 executive.
fn linkedin_levels(seniority: &str) -> &'static [u8] {
    match seniority {
        "intern" => &[1],
        "junior" => &[2],
        "middle" => &[3, 4],
        "senior" => &[4],
        "lead" => &[4, 5],
        "head" | "executive" => &[5, 6],
        _ => &[],
    }
}

/// LinkedIn's `f_JT` code for a feed employment type.
fn linkedin_type(employment: &str) -> Option<char> {
    Some(match employment {
        "full_time" => 'F',
        "part_time" => 'P',
        "contract" => 'C',
        "temporary" => 'T',
        "internship" => 'I',
        _ => return None,
    })
}

/// Fantastic.jobs' run for a pull: the masters' titles, the places, years of experience and employment from the
/// pull's query, visa sponsorship when you need it. None when the countries can't be named or there's no master.
pub fn fantastic_query(
    settings: &FeedSettings,
    profile: &Profile,
    time_range: TimeRange,
) -> Option<fantastic::Query> {
    let q = query(settings, profile);
    let locations = places(&q.countries)?;
    if profile.titles.is_empty() {
        return None;
    }
    let mut experience: Vec<String> = Vec::new();
    for level in q.seniority.iter().filter_map(|s| fantastic_years(s)) {
        if !experience.iter().any(|e| e == level) {
            experience.push(level.to_owned());
        }
    }
    let employment = q
        .employment_type
        .iter()
        .filter_map(|e| fantastic_employment(e))
        .map(str::to_owned)
        .collect();
    Some(fantastic::Query {
        titles: profile.titles.clone(),
        locations,
        time_range,
        experience,
        employment,
        visa_sponsorship: settings.needs_visa,
        limit: FANTASTIC_LIMIT,
    })
}

/// Fantastic.jobs' years-of-experience band for a feed seniority.
fn fantastic_years(seniority: &str) -> Option<&'static str> {
    Some(match seniority {
        "intern" | "junior" => "0-2",
        "middle" => "2-5",
        "senior" => "5-10",
        "lead" | "head" | "executive" => "10+",
        _ => return None,
    })
}

/// Fantastic.jobs' employment type for a feed employment type.
fn fantastic_employment(employment: &str) -> Option<&'static str> {
    Some(match employment {
        "full_time" => "FULL_TIME",
        "part_time" => "PART_TIME",
        "contract" => "CONTRACTOR",
        "temporary" => "TEMPORARY",
        "internship" => "INTERN",
        _ => return None,
    })
}

/// Country codes and their names as LinkedIn and Fantastic.jobs write them (English, no abbreviations).
const COUNTRIES: &[(&str, &str)] = &[
    ("us", "United States"),
    ("ca", "Canada"),
    ("gb", "United Kingdom"),
    ("ie", "Ireland"),
    ("de", "Germany"),
    ("fr", "France"),
    ("nl", "Netherlands"),
    ("be", "Belgium"),
    ("lu", "Luxembourg"),
    ("es", "Spain"),
    ("pt", "Portugal"),
    ("it", "Italy"),
    ("ch", "Switzerland"),
    ("at", "Austria"),
    ("pl", "Poland"),
    ("cz", "Czechia"),
    ("se", "Sweden"),
    ("no", "Norway"),
    ("dk", "Denmark"),
    ("fi", "Finland"),
    ("ee", "Estonia"),
    ("ro", "Romania"),
    ("gr", "Greece"),
    ("il", "Israel"),
    ("ae", "United Arab Emirates"),
    ("in", "India"),
    ("cn", "China"),
    ("tw", "Taiwan"),
    ("hk", "Hong Kong"),
    ("sg", "Singapore"),
    ("jp", "Japan"),
    ("kr", "South Korea"),
    ("au", "Australia"),
    ("nz", "New Zealand"),
    ("br", "Brazil"),
    ("mx", "Mexico"),
    ("ar", "Argentina"),
    ("co", "Colombia"),
    ("za", "South Africa"),
];

/// A country code (`us`, `uk` as `gb`) as LinkedIn and Fantastic.jobs name it.
fn country_name(code: &str) -> Option<&'static str> {
    let code = code.trim().to_ascii_lowercase();
    let code = if code == "uk" { "gb" } else { code.as_str() };
    COUNTRIES.iter().find(|(c, _)| *c == code).map(|(_, n)| *n)
}

/// The countries a job's place names (`China, Shanghai`, `San Francisco, CA`, `London, UK`), as codes: country names,
/// common short forms, and US states.
fn place_countries(location: &str) -> BTreeSet<&'static str> {
    location
        .split([',', ';', '(', ')', '|'])
        .map(|p| p.trim().trim_end_matches('.'))
        .filter(|p| !p.is_empty())
        .filter_map(|part| {
            let lower = part.to_lowercase();
            match lower.as_str() {
                "us" | "usa" | "u.s" | "u.s.a" | "united states of america" => return Some("us"),
                "uk" | "england" | "scotland" | "wales" => return Some("gb"),
                _ => {}
            }
            if US_STATES
                .iter()
                .any(|(code, name)| part == *code || lower == *name)
            {
                return Some("us");
            }
            COUNTRIES
                .iter()
                .find(|(_, name)| name.eq_ignore_ascii_case(part))
                .map(|(code, _)| *code)
        })
        .collect()
}

/// The local check's verdict on one job.
#[derive(Debug, Clone, PartialEq)]
pub struct LocalCheck {
    pub keep: bool,
    /// Shared skills + 0.5 × shared title words: the check queue's order.
    pub overlap: f64,
    /// Why it was dropped, as a sentence.
    pub reason: Option<String>,
}

/// No model. Drops: years_min > ceil(profile.years) + 1; job.url in `known_urls` (utm-free employer links of postings
/// you have); a watchlist or LinkedIn job whose title names a level outside the pull's seniority (boards don't filter by
/// level, and LinkedIn's mid-senior level takes in staff and principal roles); a watchlist job whose place names only
/// countries outside the pull's; zero skill overlap and zero title-word overlap. Overlap = shared skills + 0.5 ×
/// shared title words (level words like `intern` or `senior` don't count).
pub fn local_check(
    job: &FeedJob,
    profile: &Profile,
    pull: &Query,
    known_urls: &HashSet<String>,
) -> LocalCheck {
    let seniority = &pull.seniority;
    let job_skills: BTreeSet<String> = job.skills.iter().map(|s| skill_key(s)).collect();
    let shared_skills = job_skills
        .iter()
        .filter(|s| profile.skills.binary_search(s).is_ok())
        .count();
    let shared_words = words(&job.title)
        .filter(|w| profile.title_words.binary_search(w).is_ok())
        .collect::<BTreeSet<_>>()
        .len();
    let overlap = shared_skills as f64 + 0.5 * shared_words as f64;
    let most = profile.years.ceil() as u32 + 1;
    let reason = if known_urls.contains(&job.url) {
        Some("You already have this posting.".to_owned())
    } else if let Some(need) = job.years_min.filter(|&y| y > most) {
        Some(format!(
            "Asks for {need} years of experience; you have {:.1}.",
            profile.years
        ))
    } else if let Some(level) = (matches!(job.origin, Origin::Watchlist | Origin::Linkedin)
        && !seniority.is_empty())
    .then(|| title_level(&job.title))
    .flatten()
    .filter(|level| !seniority.iter().any(|s| s == level))
    {
        Some(format!(
            "The title says {level}; the feed looks for {}.",
            seniority.join(", ")
        ))
    } else if let Some(place) = job
        .location
        .as_deref()
        .filter(|_| job.origin == Origin::Watchlist && !pull.countries.is_empty())
        .filter(|place| {
            let named = place_countries(place);
            !named.is_empty()
                && !pull.countries.iter().any(|c| {
                    let c = c.trim().to_ascii_lowercase();
                    named.contains(if c == "uk" { "gb" } else { c.as_str() })
                })
        })
    {
        Some(format!("{place} is outside your countries."))
    } else if shared_skills == 0 && shared_words == 0 {
        Some("Shares no skill or title word with your masters.".to_owned())
    } else {
        None
    };
    LocalCheck {
        keep: reason.is_none(),
        overlap,
        reason,
    }
}

/// The level a title names, in the feed's seniority words: `intern`, `junior`, `senior` or `lead`; None when it names
/// none (most mid-level titles).
pub fn title_level(title: &str) -> Option<&'static str> {
    let lower = title.to_lowercase();
    let ws: Vec<&str> = lower
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();
    let has = |list: &[&str]| ws.iter().any(|w| list.contains(w));
    if has(&["intern", "internship", "interns", "coop", "apprentice"]) || lower.contains("co-op") {
        Some("intern")
    } else if has(&[
        "principal",
        "lead",
        "head",
        "director",
        "vp",
        "chief",
        "distinguished",
    ]) {
        Some("lead")
    } else if has(&["senior", "sr", "staff"]) {
        Some("senior")
    } else if has(&["junior", "jr", "entry", "graduate", "grad"]) {
        Some("junior")
    } else {
        None
    }
}

/// Skill names in freehire's vocabulary: lowercase, `c++`→`cpp`, `c#`→`csharp`, `.js`→`js`, other punctuation and
/// spaces removed (`Node.js`→`nodejs`, `CI/CD`→`cicd`); used on both sides.
pub fn skill_key(s: &str) -> String {
    s.to_lowercase()
        .replace("c++", "cpp")
        .replace("c#", "csharp")
        .replace(".js", "js")
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect()
}

/// The bank's experience sections as a resume, for [`experience_months`].
fn experience_only(bank: &Bank) -> Resume {
    let sections = bank
        .sections
        .iter()
        .filter(|s| s.kind == SectionKind::Experience)
        .map(|s| Section {
            id: s.id.clone(),
            title: s.title.clone(),
            entries: s
                .entries
                .iter()
                .map(|e| Entry {
                    id: e.id.clone(),
                    name: e.name.clone(),
                    url: String::new(),
                    place: String::new(),
                    dates: e.dates.clone(),
                    subtitle: String::new(),
                    notes: vec![],
                    bullets: vec![],
                    start: e.start.clone(),
                    end: e.end.clone(),
                })
                .collect(),
            groups: vec![],
            kind: Some(SectionKind::Experience),
        })
        .collect();
    Resume {
        name: bank.name.clone(),
        contact: vec![],
        sections,
    }
}

/// An education entry ending `present`, after today, or within 12 months before it.
fn student_or_recent(bank: &Bank, today: (u32, u32)) -> bool {
    let now = today.0 * 12 + today.1.saturating_sub(1);
    bank.sections
        .iter()
        .filter(|s| s.kind == SectionKind::Education)
        .flat_map(|s| &s.entries)
        .filter_map(|e| e.end.as_deref())
        .any(|end| end == PRESENT || month_index(end).is_some_and(|m| m + 12 >= now))
}

/// `YYYY-MM` as a month count.
fn month_index(s: &str) -> Option<u32> {
    let (y, m) = s.split_once('-')?;
    let (y, m) = (y.parse::<u32>().ok()?, m.parse::<u32>().ok()?);
    (1..=12).contains(&m).then(|| y * 12 + m - 1)
}

/// freehire categories a master's title names.
fn categories(title: &str) -> Vec<String> {
    let lower = title.to_lowercase();
    let ws: Vec<&str> = lower
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();
    let has = |prefixes: &[&str]| ws.iter().any(|w| prefixes.iter().any(|p| w.starts_with(p)));
    let mut out = Vec::new();
    if has(&["software", "engineer", "develop", "programm"]) {
        out.extend(["software_engineering", "backend", "frontend", "fullstack"]);
    }
    if has(&["design"]) {
        out.extend(["design", "creative"]);
    }
    if has(&["data"]) {
        out.extend(["data_engineering", "data_science", "data_analytics"]);
    }
    if ws.contains(&"it") || has(&["support", "helpdesk"]) || lower.contains("help desk") {
        out.extend(["support", "devops", "network_engineering", "sre"]);
    }
    strings(&out)
}

/// Words that say nothing about the role, levels among them (the level is checked on its own).
const STOPWORDS: &[&str] = &[
    "a",
    "an",
    "and",
    "at",
    "for",
    "in",
    "of",
    "on",
    "or",
    "the",
    "to",
    "with",
    "i",
    "ii",
    "iii",
    "iv",
    "jr",
    "sr",
    "remote",
    "hybrid",
    "onsite",
    "summer",
    "fall",
    "spring",
    "winter",
    "intern",
    "interns",
    "internship",
    "junior",
    "senior",
    "staff",
    "principal",
    "lead",
    "associate",
    "graduate",
    "grad",
    "new",
    "entry",
    "level",
    "co",
    "op",
];

/// Lowercase words of a title, stopwords and numbers out.
fn words(s: &str) -> impl Iterator<Item = String> + '_ {
    s.split(|c: char| !c.is_alphanumeric())
        .map(str::to_lowercase)
        .filter(|w| {
            !w.is_empty()
                && !w.chars().all(|c| c.is_ascii_digit())
                && !STOPWORDS.contains(&w.as_str())
        })
}

/// US state names and postal codes, for places like `Tempe, AZ`.
const US_STATES: &[(&str, &str)] = &[
    ("AL", "alabama"),
    ("AK", "alaska"),
    ("AZ", "arizona"),
    ("AR", "arkansas"),
    ("CA", "california"),
    ("CO", "colorado"),
    ("CT", "connecticut"),
    ("DE", "delaware"),
    ("FL", "florida"),
    ("GA", "georgia"),
    ("HI", "hawaii"),
    ("ID", "idaho"),
    ("IL", "illinois"),
    ("IN", "indiana"),
    ("IA", "iowa"),
    ("KS", "kansas"),
    ("KY", "kentucky"),
    ("LA", "louisiana"),
    ("ME", "maine"),
    ("MD", "maryland"),
    ("MA", "massachusetts"),
    ("MI", "michigan"),
    ("MN", "minnesota"),
    ("MS", "mississippi"),
    ("MO", "missouri"),
    ("MT", "montana"),
    ("NE", "nebraska"),
    ("NV", "nevada"),
    ("NH", "new hampshire"),
    ("NJ", "new jersey"),
    ("NM", "new mexico"),
    ("NY", "new york"),
    ("NC", "north carolina"),
    ("ND", "north dakota"),
    ("OH", "ohio"),
    ("OK", "oklahoma"),
    ("OR", "oregon"),
    ("PA", "pennsylvania"),
    ("RI", "rhode island"),
    ("SC", "south carolina"),
    ("SD", "south dakota"),
    ("TN", "tennessee"),
    ("TX", "texas"),
    ("UT", "utah"),
    ("VT", "vermont"),
    ("VA", "virginia"),
    ("WA", "washington"),
    ("WV", "west virginia"),
    ("WI", "wisconsin"),
    ("WY", "wyoming"),
    ("DC", "district of columbia"),
];

/// A bank place (an entry's, or the contact line's) in the US: a state, `USA`, `United States`.
fn in_us(bank: &Bank) -> bool {
    let places = bank
        .sections
        .iter()
        .flat_map(|s| &s.entries)
        .map(|e| e.place.as_str())
        .chain(bank.contact.iter().map(|c| c.text.as_str()));
    places.flat_map(|p| p.split(',')).any(|part| {
        let part = part.trim().trim_end_matches('.');
        let lower = part.to_lowercase();
        matches!(
            lower.as_str(),
            "us" | "usa" | "u.s" | "u.s.a" | "united states" | "united states of america"
        ) || US_STATES
            .iter()
            .any(|(code, name)| part == *code || lower == *name)
    })
}

fn strings(s: &[&str]) -> Vec<String> {
    s.iter().map(|s| (*s).to_owned()).collect()
}

#[cfg(test)]
mod tests;

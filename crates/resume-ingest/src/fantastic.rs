//! Fantastic.jobs' career-site index through its Apify actor (ARCHITECTURE.md §7, Job feed): new jobs from 58 ATS.

use std::time::Duration;

use resume_core::feed::{FeedJob, Origin};
use resume_core::salary;
use serde::Deserialize;
use serde_json::{Map, Value, json};

use crate::freehire::without_utm;
use crate::text::html_to_text;
use crate::{Fetcher, IngestError, check_status};

const HOST: &str = "api.apify.com";
const RUN: &str = "https://api.apify.com/v2/acts/fantastic-jobs~career-site-job-listing-api/run-sync-get-dataset-items";
/// Apify ends a synchronous run at 300 s (HTTP 408); the run itself is given 240.
const RUN_TIMEOUT_SECS: u32 = 240;
const WAIT: Duration = Duration::from_secs(300);

/// How far back the actor looks: jobs it indexed in the last hour, day or week.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TimeRange {
    Hour,
    #[default]
    Day,
    Week,
}

impl TimeRange {
    fn code(self) -> &'static str {
        match self {
            Self::Hour => "1h",
            Self::Day => "24h",
            Self::Week => "7d",
        }
    }
}

/// One run of the actor.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Query {
    /// Terms any of which a title matches.
    pub titles: Vec<String>,
    /// Terms any of which a location matches (`United States`, `Berlin`).
    pub locations: Vec<String>,
    pub time_range: TimeRange,
    /// Years of experience: `0-2`, `2-5`, `5-10`, `10+`.
    pub experience: Vec<String>,
    /// `FULL_TIME`, `PART_TIME`, `CONTRACTOR`, `TEMPORARY`, `INTERN`, …
    pub employment: Vec<String>,
    /// Only jobs offering visa sponsorship.
    pub visa_sponsorship: bool,
    /// Jobs at most; each is billed.
    pub limit: u32,
}

impl Query {
    /// The actor's input. Empty lists are left out, and the visa filter unless set.
    pub fn input(&self) -> Value {
        let mut input = Map::new();
        input.insert("timeRange".into(), json!(self.time_range.code()));
        input.insert("limit".into(), json!(self.limit));
        for (key, list) in [
            ("titleSearch", &self.titles),
            ("locationSearch", &self.locations),
            ("aiExperienceLevelFilter", &self.experience),
            ("aiEmploymentTypeFilter", &self.employment),
        ] {
            if !list.is_empty() {
                input.insert(key.into(), json!(list));
            }
        }
        if self.visa_sponsorship {
            input.insert("aiVisaSponsorshipFilter".into(), json!(true));
        }
        input.insert("descriptionType".into(), json!("text"));
        input.insert("includeCompanyDetails".into(), json!(false));
        Value::Object(input)
    }
}

/// Runs the actor on `query` with the Apify `token`, waiting up to 300 s for its jobs. An item without its id,
/// title, organization or link is left out.
pub async fn pull(
    fetcher: &impl Fetcher,
    token: &str,
    query: &Query,
) -> Result<Vec<FeedJob>, IngestError> {
    let url = format!(
        "{RUN}?token={}&timeout={RUN_TIMEOUT_SECS}",
        url::form_urlencoded::byte_serialize(token.as_bytes()).collect::<String>()
    );
    let resp = fetcher
        .post_json(&url, &query.input().to_string(), WAIT)
        .await
        .map_err(|message| {
            if message.starts_with("no answer within") {
                too_long()
            } else {
                network(message)
            }
        })?;
    if resp.status >= 400 {
        return Err(run_error(&resp));
    }
    let items: Vec<Value> = serde_json::from_str(&resp.body)
        .map_err(|_| network("its answer wasn't the list of jobs the actor returns".to_owned()))?;
    Ok(items
        .into_iter()
        .filter_map(|v| serde_json::from_value::<Item>(v).ok()?.into_feed())
        .collect())
}

fn run_error(resp: &crate::Response) -> IngestError {
    #[derive(Deserialize)]
    struct Answer {
        error: Detail,
    }
    #[derive(Deserialize)]
    struct Detail {
        #[serde(rename = "type", default)]
        kind: String,
        #[serde(default)]
        message: String,
    }
    let status = resp.status;
    let detail = serde_json::from_str::<Answer>(&resp.body)
        .ok()
        .map(|a| a.error);
    let kind = detail.as_ref().map_or("", |d| d.kind.as_str());
    let out_of_credit = status == 402
        || kind.contains("payment")
        || matches!(
            kind,
            "not-enough-usage-to-run-paid-actor" | "monthly-usage-limit-too-low" | "limit-reached"
        );
    if out_of_credit {
        return refused(
            "The Apify account is out of credit; add some at console.apify.com, then pull again",
        );
    }
    if matches!(status, 401 | 403) {
        return refused("Apify refused the token; check it in Settings → Feed");
    }
    if status == 408 || kind == "run-timeout-exceeded" {
        return too_long();
    }
    if status == 429 || status >= 500 {
        return check_status(resp, HOST, RUN)
            .err()
            .unwrap_or_else(|| network(format!("HTTP {status}")));
    }
    match detail.filter(|d| !d.message.is_empty()) {
        Some(d) => refused(&format!("The run failed (HTTP {status}): {}", d.message)),
        None => refused(&format!("The run failed (HTTP {status})")),
    }
}

fn too_long() -> IngestError {
    refused("The run took too long (over 300 s); pull a shorter time range or fewer jobs")
}

fn refused(message: &str) -> IngestError {
    IngestError::Refused {
        host: HOST.to_owned(),
        message: message.to_owned(),
    }
}

fn network(message: String) -> IngestError {
    IngestError::Network {
        host: HOST.to_owned(),
        message,
    }
}

/// An actor item (the fields the feed uses).
#[derive(Deserialize)]
struct Item {
    id: Option<Value>,
    title: Option<String>,
    organization: Option<String>,
    url: Option<String>,
    #[serde(default)]
    source: Option<String>,
    date_posted: Option<String>,
    date_created: Option<String>,
    locations_derived: Option<Vec<String>>,
    description_text: Option<String>,
    description_html: Option<String>,
    ai_key_skills: Option<Vec<String>>,
    ai_employment_type: Option<Vec<String>>,
    ai_experience_level: Option<String>,
    ai_work_arrangement: Option<String>,
    ai_salary_currency: Option<String>,
    ai_salary_value: Option<f64>,
    ai_salary_min_value: Option<f64>,
    ai_salary_max_value: Option<f64>,
    ai_salary_unit_text: Option<String>,
}

impl Item {
    fn into_feed(self) -> Option<FeedJob> {
        let id = match self.id? {
            Value::Number(n) => n.to_string(),
            Value::String(s) if !s.is_empty() => s,
            _ => return None,
        };
        let nonempty = |s: Option<String>| s.map(|s| s.trim().to_owned()).filter(|s| !s.is_empty());
        let title = nonempty(self.title)?;
        let company = nonempty(self.organization)?;
        let url = without_utm(&nonempty(self.url)?);
        let posted_at = self.date_posted.as_deref().and_then(rfc3339);
        let found_at = self
            .date_created
            .as_deref()
            .and_then(rfc3339)
            .or_else(|| posted_at.clone())?;
        let salary = match (
            self.ai_salary_currency.as_deref(),
            self.ai_salary_unit_text.as_deref(),
        ) {
            (Some(currency), Some(unit @ ("HOUR" | "YEAR"))) => salary::from_source(
                self.ai_salary_min_value.or(self.ai_salary_value),
                self.ai_salary_max_value,
                currency,
                unit,
            ),
            _ => None,
        };
        let description = nonempty(self.description_text)
            .or_else(|| self.description_html.map(|h| html_to_text(&h)))
            .filter(|t| !t.is_empty());
        Some(FeedJob {
            origin: Origin::Fantastic,
            slug: format!("fantastic:{id}"),
            title,
            company,
            company_slug: None,
            location: self.locations_derived.and_then(|l| l.into_iter().next()),
            url,
            source: self.source.unwrap_or_default(),
            external_id: None,
            skills: self
                .ai_key_skills
                .unwrap_or_default()
                .iter()
                .map(|s| skill(s))
                .filter(|s| !s.is_empty())
                .collect(),
            posted_at,
            found_at,
            seniority: None,
            employment_type: self
                .ai_employment_type
                .unwrap_or_default()
                .iter()
                .find_map(|t| employment_type(t)),
            work_mode: self.ai_work_arrangement.as_deref().and_then(work_mode),
            years_min: self
                .ai_experience_level
                .as_deref()
                .and_then(|l| l.split(['-', '+']).next()?.trim().parse().ok()),
            salary,
            description,
        })
    }
}

/// A skill in freehire's vocabulary: lowercase, `c++` → `cpp`, `c#` → `csharp`, `.js` → `js`, then letters and
/// digits only (`Node.js` → `nodejs`).
fn skill(name: &str) -> String {
    name.to_lowercase()
        .replace("c++", "cpp")
        .replace("c#", "csharp")
        .replace(".js", "js")
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .collect()
}

fn employment_type(ai: &str) -> Option<String> {
    let t = match ai {
        "FULL_TIME" => "full_time",
        "PART_TIME" => "part_time",
        "INTERN" => "internship",
        "CONTRACTOR" => "contract",
        _ => return None,
    };
    Some(t.to_owned())
}

fn work_mode(ai: &str) -> Option<String> {
    let mode = if ai.starts_with("Remote") {
        "remote"
    } else if ai == "Hybrid" {
        "hybrid"
    } else if ai == "On-site" {
        "onsite"
    } else {
        return None;
    };
    Some(mode.to_owned())
}

/// The actor's times are UTC without a zone (`2026-10-13T01:26:21.522544`), sometimes a bare date.
fn rfc3339(at: &str) -> Option<String> {
    let at = at.trim();
    let b = at.as_bytes();
    let date = b.len() >= 10
        && b[..10].iter().enumerate().all(|(i, c)| {
            if i == 4 || i == 7 {
                *c == b'-'
            } else {
                c.is_ascii_digit()
            }
        });
    if !date {
        return None;
    }
    if b.len() == 10 {
        return Some(format!("{at}T00:00:00Z"));
    }
    if b[10] != b'T' {
        return None;
    }
    let time = &at[11..];
    let zoned = time.ends_with('Z') || time.contains('+') || time.contains('-');
    Some(if zoned {
        at.to_owned()
    } else {
        format!("{at}Z")
    })
}

#[cfg(test)]
mod tests;

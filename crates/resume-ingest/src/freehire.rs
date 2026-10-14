//! freehire's public job API (ARCHITECTURE.md §7, Job feed): a search over every board it collects, and reading one
//! of its jobs. Nothing is sent back; jobs are applied to on the employer's link.

use serde::Deserialize;
use serde_json::Value;

use resume_core::feed::{FeedJob, Origin};
use resume_core::salary::{self, Salary};
use resume_core::{Method, Posting};

use crate::link::Link;
use crate::text::html_to_text;
use crate::{Fetcher, IngestError, check_status, read_link};

pub(crate) const API: &str = "https://freehire.me/api/v1";
pub(crate) const HOST: &str = "freehire.me";

/// Sources whose links are another board's redirect, not the employer's.
pub const LINK_SITES: &[&str] = &["adzuna", "whatjobs", "himalayas"];

/// A search's filters. Comma lists OR together; so do `countries` and the unknown-location region.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Query {
    pub q: Option<String>,
    pub seniority: Vec<String>,
    pub employment_type: Vec<String>,
    pub category: Vec<String>,
    pub countries: Vec<String>,
    /// Adds `regions=none` (jobs freehire couldn't place). Only with `countries`: alone it would keep nothing else.
    pub include_unknown_location: bool,
    pub visa_sponsorship: Option<bool>,
    /// `Some(false)` leaves clearance jobs out.
    pub requires_clearance: Option<bool>,
    pub fresh_only: bool,
    pub open_within_days: Option<u32>,
    pub source_exclude: Vec<String>,
    /// `sort=created_at&order=desc`; else freehire's order (relevance with `q`).
    pub newest_first: bool,
}

impl Query {
    /// The search URL for `limit` jobs from `offset`.
    pub fn url(&self, limit: u32, offset: u32) -> String {
        let mut params: Vec<(&str, String)> = Vec::new();
        if let Some(q) = self.q.as_deref().map(str::trim).filter(|q| !q.is_empty()) {
            params.push(("q", encode(q)));
        }
        let mut list = |key: &'static str, values: &[String]| {
            if !values.is_empty() {
                let joined: Vec<String> = values.iter().map(|v| encode(v)).collect();
                params.push((key, joined.join(",")));
            }
        };
        list("seniority", &self.seniority);
        list("employment_type", &self.employment_type);
        list("category", &self.category);
        list("countries", &self.countries);
        list("source_exclude", &self.source_exclude);
        if self.include_unknown_location && !self.countries.is_empty() {
            params.push(("regions", "none".to_owned()));
        }
        if let Some(v) = self.visa_sponsorship {
            params.push(("visa_sponsorship", v.to_string()));
        }
        if let Some(v) = self.requires_clearance {
            params.push(("requires_clearance", v.to_string()));
        }
        if self.fresh_only {
            params.push(("reality", "fresh".to_owned()));
        }
        if let Some(days) = self.open_within_days {
            params.push(("open_within_days", days.to_string()));
        }
        if self.newest_first {
            params.push(("sort", "created_at".to_owned()));
            params.push(("order", "desc".to_owned()));
        }
        params.push(("limit", limit.to_string()));
        params.push(("offset", offset.to_string()));
        let query: Vec<String> = params
            .into_iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect();
        format!("{API}/jobs/search?{}", query.join("&"))
    }
}

fn encode(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}

/// One page of a search.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchPage {
    pub jobs: Vec<FeedJob>,
    /// Jobs matching the query, across every page.
    pub total: u64,
}

/// `limit` jobs of `query` from `offset`. A job freehire lists without its slug, title, company or link is left out.
pub async fn search(
    fetcher: &impl Fetcher,
    query: &Query,
    limit: u32,
    offset: u32,
) -> Result<SearchPage, IngestError> {
    #[derive(Deserialize)]
    struct Page {
        data: Vec<Value>,
        meta: Meta,
    }
    #[derive(Deserialize)]
    struct Meta {
        total: u64,
    }
    let url = query.url(limit, offset);
    let body = get(fetcher, &url, &url).await?;
    let page: Page = serde_json::from_str(&body).map_err(|_| unreadable())?;
    Ok(SearchPage {
        jobs: page
            .data
            .into_iter()
            .filter_map(|v| serde_json::from_value::<Job>(v).ok())
            .map(Job::into_feed)
            .collect(),
        total: page.meta.total,
    })
}

/// Greenhouse, Lever, Ashby, Workday, SmartRecruiters, Workable, Recruitee: the board's own read by `external_id` (same
/// text and hash as pasting the link; a closed job fails). Anything else: `GET /jobs/{slug}`, its HTML description
/// through the page text code, `Method::Freehire`. `source` = the freehire page, `url` = `job.url`;
/// title/company/location/salary fall back to freehire's.
pub async fn read_job(fetcher: &impl Fetcher, job: &FeedJob) -> Result<Posting, IngestError> {
    let page = job.page_url();
    let mut posting = match board_link(job) {
        Some(link) => read_link(fetcher, &page, &link).await?,
        None => {
            #[derive(Deserialize)]
            struct One {
                data: Detail,
            }
            #[derive(Deserialize)]
            struct Detail {
                description: Option<String>,
                enrichment: Option<Enrichment>,
            }
            let url = format!("{API}/jobs/{}", job.slug);
            let body = get(fetcher, &url, &page).await?;
            let one: One = serde_json::from_str(&body).map_err(|_| unreadable())?;
            // Some sources' copies escape character references once more (Workday's `&amp;#xa;`).
            let html = one
                .data
                .description
                .unwrap_or_default()
                .replace("&amp;#", "&#");
            let text = html_to_text(&html);
            if text.is_empty() {
                return Err(IngestError::NotAPosting {
                    url: page.clone(),
                    reason: "the posting has no text".to_owned(),
                });
            }
            let mut posting = Posting::new(Method::Freehire, None, None, None, None, None, text);
            posting.salary = one.data.enrichment.as_ref().and_then(Enrichment::salary);
            posting
        }
    };
    posting.source = Some(page);
    posting.url = Some(job.url.clone());
    posting.title = posting.title.or_else(|| Some(job.title.clone()));
    posting.company = posting
        .company
        .or_else(|| Some(job.company.clone()).filter(|c| is_name(c)));
    posting.location = posting.location.or_else(|| job.location.clone());
    posting.salary = posting.salary.or_else(|| job.salary.clone());
    Ok(posting)
}

/// The board read for a job on an ATS gliide reads, from its `external_id` (`board:id`).
fn board_link(job: &FeedJob) -> Option<Link> {
    let (board, id) = job.external_id.as_deref()?.split_once(':')?;
    let eu = url::Url::parse(&job.url).is_ok_and(|u| u.host_str() == Some("jobs.eu.lever.co"));
    Link::board(&job.source, board, id, eu)
}

/// The body of a 2xx answer from freehire; `page` names the job in errors.
async fn get(fetcher: &impl Fetcher, url: &str, page: &str) -> Result<String, IngestError> {
    let resp = fetcher
        .get(url)
        .await
        .map_err(|message| IngestError::Network {
            host: HOST.to_owned(),
            message,
        })?;
    check_status(&resp, HOST, page)?;
    Ok(resp.body)
}

fn unreadable() -> IngestError {
    IngestError::Network {
        host: HOST.to_owned(),
        message: "its answer wasn't the job data it documents".to_owned(),
    }
}

/// A job as the API lists it (the fields the feed uses).
#[derive(Deserialize)]
struct Job {
    public_slug: String,
    title: String,
    company: String,
    company_slug: Option<String>,
    location: Option<String>,
    url: String,
    source: String,
    external_id: Option<String>,
    skills: Option<Vec<String>>,
    posted_at: Option<String>,
    created_at: String,
    work_mode: Option<String>,
    enrichment: Option<Enrichment>,
}

#[derive(Deserialize)]
struct Enrichment {
    seniority: Option<String>,
    employment_type: Option<String>,
    experience_years_min: Option<f64>,
    salary_min: Option<f64>,
    salary_max: Option<f64>,
    salary_currency: Option<String>,
    salary_period: Option<String>,
}

impl Enrichment {
    /// Pay per year or per hour; other periods are left out.
    fn salary(&self) -> Option<Salary> {
        let period = self.salary_period.as_deref()?;
        if !matches!(period, "year" | "hour") {
            return None;
        }
        salary::from_source(
            self.salary_min,
            self.salary_max,
            self.salary_currency.as_deref()?,
            period,
        )
    }
}

impl Job {
    fn into_feed(self) -> FeedJob {
        let e = self.enrichment.as_ref();
        FeedJob {
            origin: Origin::Freehire,
            description: None,
            salary: e.and_then(Enrichment::salary),
            seniority: e.and_then(|e| e.seniority.clone()),
            employment_type: e.and_then(|e| e.employment_type.clone()),
            years_min: e
                .and_then(|e| e.experience_years_min)
                .filter(|y| y.is_finite() && *y >= 0.0)
                .map(|y| y.round() as u32),
            url: without_utm(&self.url),
            slug: self.public_slug,
            title: self.title,
            company: self.company,
            company_slug: self.company_slug.filter(|s| !s.is_empty()),
            location: self.location.filter(|l| !l.trim().is_empty()),
            source: self.source,
            external_id: self.external_id.filter(|id| !id.is_empty()),
            skills: self.skills.unwrap_or_default(),
            posted_at: self.posted_at,
            found_at: self.created_at,
            work_mode: self.work_mode,
        }
    }
}

/// freehire names some Oracle, Taleo and Workday jobs' company by the site hosting it
/// (`egug.fa.us2.oraclecloud.com/CX`, `textron.taleo.net/textron_ur`) or an id; those aren't passed on as the company,
/// so Extract reads it from the text. `Booking.com` stays a name.
pub fn is_name(company: &str) -> bool {
    let c = company.trim();
    let hosted =
        !c.contains(char::is_whitespace) && (c.contains('/') || c.matches('.').count() >= 2);
    let id = c.len() >= 32 && c.chars().all(|ch| ch.is_ascii_hexdigit() || ch == '-');
    !c.is_empty() && !hosted && !id
}

/// `url` without its `utm_*` query parameters (the rest kept as written).
pub fn without_utm(url: &str) -> String {
    let (base, fragment) = match url.split_once('#') {
        Some((b, f)) => (b, Some(f)),
        None => (url, None),
    };
    let mut out = match base.split_once('?') {
        Some((path, query)) => {
            let kept: Vec<&str> = query
                .split('&')
                .filter(|p| !p.is_empty() && !p.starts_with("utm_"))
                .collect();
            if kept.is_empty() {
                path.to_owned()
            } else {
                format!("{path}?{}", kept.join("&"))
            }
        }
        None => base.to_owned(),
    };
    if let Some(f) = fragment {
        out.push('#');
        out.push_str(f);
    }
    out
}

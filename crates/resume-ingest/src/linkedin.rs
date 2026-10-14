//! LinkedIn's guest job search (ARCHITECTURE.md §7, Job feed): the jobs a search lists, without logging in.

use scraper::{ElementRef, Html, Selector};

use resume_core::feed::{FeedJob, Origin};
use resume_core::salary::{self, Salary};

use crate::text::html_to_text;
use crate::{Fetcher, IngestError, check_status};

const API: &str = "https://www.linkedin.com/jobs-guest/jobs/api/seeMoreJobPostings/search";
const HOST: &str = "linkedin.com";

/// Cards per page.
pub const PAGE: u32 = 10;

/// A search's filters, as LinkedIn's job search takes them.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Search {
    pub keywords: String,
    pub location: Option<String>,
    /// `f_TPR=r{secs}`: posted within this many seconds.
    pub posted_within_secs: Option<u32>,
    /// `f_E` codes: 1 internship, 2 entry, 3 associate, 4 mid-senior, 5 director, 6 executive.
    pub experience: Vec<u8>,
    /// `f_JT` codes: F full-time, P part-time, C contract, T temporary, I internship.
    pub job_types: Vec<char>,
}

impl Search {
    /// The URL of the page of cards from `start`.
    pub fn url(&self, start: u32) -> String {
        let mut params: Vec<(&str, String)> = Vec::new();
        let keywords = self.keywords.trim();
        if !keywords.is_empty() {
            params.push(("keywords", encode(keywords)));
        }
        if let Some(location) = self
            .location
            .as_deref()
            .map(str::trim)
            .filter(|l| !l.is_empty())
        {
            params.push(("location", encode(location)));
        }
        if let Some(secs) = self.posted_within_secs {
            params.push(("f_TPR", format!("r{secs}")));
        }
        if !self.experience.is_empty() {
            let codes: Vec<String> = self.experience.iter().map(u8::to_string).collect();
            params.push(("f_E", encode(&codes.join(","))));
        }
        if !self.job_types.is_empty() {
            let codes: Vec<String> = self.job_types.iter().map(char::to_string).collect();
            params.push(("f_JT", encode(&codes.join(","))));
        }
        params.push(("start", start.to_string()));
        let query: Vec<String> = params
            .into_iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect();
        format!("{API}?{}", query.join("&"))
    }
}

fn encode(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}

/// One page from `start`; an empty Vec means no more (LinkedIn answers a `start` past what it serves, about 1000,
/// with HTTP 400). `now` (RFC 3339) is every job's `found_at`. A card without its job id, title or company is left
/// out.
pub async fn search(
    fetcher: &impl Fetcher,
    search: &Search,
    start: u32,
    now: &str,
) -> Result<Vec<FeedJob>, IngestError> {
    let url = search.url(start);
    let resp = fetcher
        .get(&url)
        .await
        .map_err(|message| IngestError::Network {
            host: HOST.to_owned(),
            message,
        })?;
    if resp.status == 400 {
        return Ok(Vec::new());
    }
    check_status(&resp, HOST, &url)?;
    Ok(cards(&resp.body, now))
}

/// The jobs of a page of search cards.
fn cards(html: &str, now: &str) -> Vec<FeedJob> {
    let sel = |s: &str| Selector::parse(s).expect("valid selector");
    let card = sel("li");
    let urn = sel("[data-entity-urn]");
    let link = sel("a.base-card__full-link");
    let title = sel(".base-search-card__title");
    let company = sel(".base-search-card__subtitle");
    let location = sel(".job-search-card__location");
    let time = sel("time[datetime]");
    let pay = sel(".job-search-card__salary-info");
    let doc = Html::parse_document(html);
    doc.select(&card)
        .filter_map(|li| {
            let text = |s: &Selector| {
                li.select(s)
                    .next()
                    .map(|e| html_to_text(&e.inner_html()))
                    .filter(|t| !t.is_empty())
            };
            let id = li
                .select(&urn)
                .find_map(|e| {
                    e.value()
                        .attr("data-entity-urn")?
                        .strip_prefix("urn:li:jobPosting:")
                })
                .map(str::to_owned)
                .or_else(|| li.select(&link).find_map(id_in_link))
                .filter(|id| !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit()))?;
            Some(FeedJob {
                origin: Origin::Linkedin,
                slug: format!("linkedin:{id}"),
                title: text(&title)?,
                company: text(&company)?,
                company_slug: None,
                location: text(&location),
                url: format!("https://www.linkedin.com/jobs/view/{id}/"),
                source: "linkedin".to_owned(),
                skills: Vec::new(),
                posted_at: li
                    .select(&time)
                    .find_map(|e| e.value().attr("datetime"))
                    .and_then(day),
                found_at: now.to_owned(),
                seniority: None,
                employment_type: None,
                work_mode: None,
                years_min: None,
                salary: text(&pay).and_then(|p| pay_line(&p)),
                description: None,
                external_id: Some(id),
            })
        })
        .collect()
}

/// The job id at the end of a card's link: `…/jobs/view/java-engineer-at-acme-4478772592?position=1`.
fn id_in_link(a: ElementRef<'_>) -> Option<String> {
    let url = url::Url::parse(a.value().attr("href")?).ok()?;
    let last = url.path_segments()?.rfind(|s| !s.is_empty())?;
    let id = last.rsplit('-').next()?;
    Some(id.to_owned())
}

/// `2026-10-13` as `2026-10-13T00:00:00Z`.
fn day(date: &str) -> Option<String> {
    let date = date.trim();
    let b = date.as_bytes();
    let digits = |r: std::ops::Range<usize>| b[r].iter().all(u8::is_ascii_digit);
    (b.len() == 10 && b[4] == b'-' && b[7] == b'-' && digits(0..4) && digits(5..7) && digits(8..10))
        .then(|| format!("{date}T00:00:00Z"))
}

/// A card's salary line (`$120,000.00 - $150,000.00`), read as the pay line it is.
fn pay_line(line: &str) -> Option<Salary> {
    salary::find(&format!("Salary: {line}"))
}

#[cfg(test)]
mod tests;

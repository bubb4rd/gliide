//! The watchlist (ARCHITECTURE.md §7, Job feed): companies' own boards, every open job listed straight from the ATS.

use std::time::Duration;

use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use url::Url;

use resume_core::feed::{FeedJob, Origin};
use resume_core::salary::{self, Salary};

use crate::{Fetcher, IngestError, check_status};

/// The most jobs a board lists.
pub const MAX_JOBS: usize = 500;
/// Workday's search answers at most 20 jobs a request.
const WORKDAY_PAGE: usize = 20;
const WORKDAY_TIMEOUT: Duration = Duration::from_secs(20);
const SMARTRECRUITERS_PAGE: usize = 100;

/// A company's board as the watchlist names it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Board {
    Greenhouse {
        board: String,
    },
    /// `eu` for `jobs.eu.lever.co`.
    Lever {
        company: String,
        eu: bool,
    },
    Ashby {
        org: String,
    },
    /// `host` is the board's (`nvidia.wd5.myworkdayjobs.com`), `tenant` its first label.
    Workday {
        host: String,
        tenant: String,
        site: String,
    },
    SmartRecruiters {
        company: String,
    },
    Workable {
        account: String,
    },
    Recruitee {
        company: String,
    },
}

impl Board {
    /// A careers link as you'd paste it (board home, any job page on it, `boards.greenhouse.io/x`,
    /// `x.wd5.myworkdayjobs.com/en-US/Site`, `jobs.smartrecruiters.com/Co`, `apply.workable.com/acct/`,
    /// `co.recruitee.com`) → its board; `None` for anything else.
    pub fn parse(input: &str) -> Option<Board> {
        let s = input.trim();
        if s.is_empty() || s.contains(char::is_whitespace) {
            return None;
        }
        let lower = s.to_ascii_lowercase();
        let url = if lower.starts_with("https://") || lower.starts_with("http://") {
            Url::parse(s).ok()?
        } else {
            Url::parse(&format!("https://{s}")).ok()?
        };
        let host = url.host_str()?.to_ascii_lowercase();
        let segments: Vec<&str> = url
            .path_segments()
            .map(|s| s.filter(|s| !s.is_empty()).collect())
            .unwrap_or_default();
        let first = segments.first().copied().filter(|s| is_slug(s));
        let labels: Vec<&str> = host.split('.').collect();
        let board = match (host.as_str(), labels.as_slice()) {
            ("job-boards.greenhouse.io" | "boards.greenhouse.io", _) => {
                let board = if first == Some("embed") {
                    url.query_pairs()
                        .find(|(k, _)| k == "for")
                        .map(|(_, v)| v.into_owned())
                        .filter(|b| is_slug(b))?
                } else {
                    first?.to_owned()
                };
                Board::Greenhouse { board }
            }
            ("jobs.lever.co" | "jobs.eu.lever.co", _) => Board::Lever {
                company: first?.to_owned(),
                eu: host == "jobs.eu.lever.co",
            },
            ("jobs.ashbyhq.com", _) => Board::Ashby {
                org: first?.to_owned(),
            },
            ("jobs.smartrecruiters.com" | "careers.smartrecruiters.com", _) => {
                Board::SmartRecruiters {
                    company: first?.to_owned(),
                }
            }
            ("apply.workable.com", _) => Board::Workable {
                account: first.filter(|a| !["j", "api"].contains(a))?.to_owned(),
            },
            (_, [account, "workable", "com"]) if !["apply", "www"].contains(account) => {
                Board::Workable {
                    account: (*account).to_owned(),
                }
            }
            (_, [company, "recruitee", "com"]) if !["www", "app", "api"].contains(company) => {
                Board::Recruitee {
                    company: (*company).to_owned(),
                }
            }
            (_, [tenant, _, "myworkdayjobs", "com"]) => {
                let rest = match segments.as_slice() {
                    ["wday", "cxs", _, rest @ ..] => rest,
                    [locale, rest @ ..] if is_locale(locale) => rest,
                    rest => rest,
                };
                let site = rest.first().filter(|s| is_slug(s) && **s != "job")?;
                Board::Workday {
                    host: host.clone(),
                    tenant: (*tenant).to_owned(),
                    site: (*site).to_owned(),
                }
            }
            _ => return None,
        };
        is_slug(board.name()).then_some(board)
    }

    /// `greenhouse`, `lever`, `ashby`, `workday`, `smartrecruiters`, `workable` or `recruitee`.
    pub fn source(&self) -> &'static str {
        match self {
            Self::Greenhouse { .. } => "greenhouse",
            Self::Lever { .. } => "lever",
            Self::Ashby { .. } => "ashby",
            Self::Workday { .. } => "workday",
            Self::SmartRecruiters { .. } => "smartrecruiters",
            Self::Workable { .. } => "workable",
            Self::Recruitee { .. } => "recruitee",
        }
    }

    /// The board's careers page.
    pub fn url(&self) -> String {
        match self {
            Self::Greenhouse { board } => format!("https://job-boards.greenhouse.io/{board}"),
            Self::Lever { company, eu } => format!("https://{}/{company}", lever_host(*eu, "jobs")),
            Self::Ashby { org } => format!("https://jobs.ashbyhq.com/{org}"),
            Self::Workday { host, site, .. } => format!("https://{host}/{site}"),
            Self::SmartRecruiters { company } => {
                format!("https://jobs.smartrecruiters.com/{company}")
            }
            Self::Workable { account } => format!("https://apply.workable.com/{account}/"),
            Self::Recruitee { company } => format!("https://{company}.recruitee.com"),
        }
    }

    /// The board's slug (Workday's tenant), shown until a list names the company.
    pub fn name(&self) -> &str {
        match self {
            Self::Greenhouse { board } => board,
            Self::Lever { company, .. }
            | Self::SmartRecruiters { company }
            | Self::Recruitee { company } => company,
            Self::Ashby { org } => org,
            Self::Workday { tenant, .. } => tenant,
            Self::Workable { account } => account,
        }
    }

    /// The board in a job's slug and `external_id`: its name, Workday's `{tenant}/{site}`.
    fn key(&self) -> String {
        match self {
            Self::Workday { tenant, site, .. } => format!("{tenant}/{site}"),
            _ => self.name().to_owned(),
        }
    }
}

fn lever_host(eu: bool, sub: &str) -> String {
    if eu {
        format!("{sub}.eu.lever.co")
    } else {
        format!("{sub}.lever.co")
    }
}

fn is_slug(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
}

/// `en-US`, `fr`.
fn is_locale(s: &str) -> bool {
    let b = s.as_bytes();
    match b.len() {
        2 => b.iter().all(u8::is_ascii_alphabetic),
        5 => b[2] == b'-' && b[..2].iter().chain(&b[3..]).all(u8::is_ascii_alphabetic),
        _ => false,
    }
}

/// Every open job on the board (at most [`MAX_JOBS`]), newest first by posting date; undated jobs last in the
/// board's order. `now` (RFC 3339) is each job's `found_at`, and dates Workday's "Posted 3 Days Ago".
pub async fn list(
    fetcher: &impl Fetcher,
    board: &Board,
    now: &str,
) -> Result<Vec<FeedJob>, IngestError> {
    let mut jobs = match board {
        Board::Greenhouse { board: b } => greenhouse(fetcher, board, b).await?,
        Board::Lever { company, eu } => lever(fetcher, board, company, *eu).await?,
        Board::Ashby { org } => ashby(fetcher, board, org).await?,
        Board::Workday { host, tenant, site } => {
            workday(fetcher, board, host, tenant, site, now).await?
        }
        Board::SmartRecruiters { company } => smartrecruiters(fetcher, board, company).await?,
        Board::Workable { account } => workable(fetcher, board, account).await?,
        Board::Recruitee { company } => recruitee(fetcher, board, company).await?,
    };
    jobs.sort_by(|a, b| match (&a.posted_at, &b.posted_at) {
        (Some(a), Some(b)) => b.cmp(a),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    });
    jobs.truncate(MAX_JOBS);
    for job in &mut jobs {
        job.found_at = now.to_owned();
    }
    Ok(jobs)
}

/// One job's fields as a board lists them.
struct Listed {
    id: String,
    title: String,
    company: Option<String>,
    location: Option<String>,
    url: String,
    posted_at: Option<String>,
    employment_type: Option<String>,
    work_mode: Option<String>,
    salary: Option<Salary>,
}

impl Listed {
    fn into_feed(self, board: &Board) -> FeedJob {
        let key = board.key();
        FeedJob {
            origin: Origin::Watchlist,
            slug: format!("{}:{key}:{}", board.source(), self.id),
            title: self.title.trim().to_owned(),
            company: self
                .company
                .map(|c| c.trim().to_owned())
                .filter(|c| !c.is_empty())
                .unwrap_or_else(|| board.name().to_owned()),
            company_slug: None,
            location: self
                .location
                .map(|l| l.trim().to_owned())
                .filter(|l| !l.is_empty()),
            url: self.url,
            source: board.source().to_owned(),
            external_id: Some(format!("{key}:{}", self.id)),
            skills: Vec::new(),
            posted_at: self.posted_at,
            found_at: String::new(),
            seniority: None,
            employment_type: self.employment_type,
            work_mode: self.work_mode,
            years_min: None,
            salary: self.salary,
            description: None,
        }
    }
}

async fn greenhouse(
    fetcher: &impl Fetcher,
    board: &Board,
    b: &str,
) -> Result<Vec<FeedJob>, IngestError> {
    #[derive(Deserialize)]
    struct List {
        jobs: Vec<Job>,
    }
    #[derive(Deserialize)]
    struct Job {
        id: u64,
        title: String,
        company_name: Option<String>,
        location: Option<Named>,
        first_published: Option<String>,
        updated_at: Option<String>,
    }
    #[derive(Deserialize)]
    struct Named {
        name: Option<String>,
    }
    let url = format!("https://boards-api.greenhouse.io/v1/boards/{b}/jobs");
    let list: List = get(fetcher, board, &url).await?;
    Ok(list
        .jobs
        .into_iter()
        .map(|j| {
            Listed {
                url: format!("https://job-boards.greenhouse.io/{b}/jobs/{}", j.id),
                id: j.id.to_string(),
                title: j.title,
                company: j.company_name,
                location: j.location.and_then(|l| l.name),
                posted_at: j
                    .first_published
                    .or(j.updated_at)
                    .as_deref()
                    .and_then(rfc3339),
                employment_type: None,
                work_mode: None,
                salary: None,
            }
            .into_feed(board)
        })
        .collect())
}

async fn lever(
    fetcher: &impl Fetcher,
    board: &Board,
    company: &str,
    eu: bool,
) -> Result<Vec<FeedJob>, IngestError> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Job {
        id: String,
        text: String,
        categories: Option<Categories>,
        created_at: Option<i64>,
        workplace_type: Option<String>,
        salary_range: Option<SalaryRange>,
    }
    #[derive(Deserialize)]
    struct Categories {
        commitment: Option<String>,
        location: Option<String>,
    }
    #[derive(Deserialize)]
    struct SalaryRange {
        min: Option<f64>,
        max: Option<f64>,
        currency: Option<String>,
        interval: Option<String>,
    }
    let url = format!(
        "https://{}/v0/postings/{company}?mode=json",
        lever_host(eu, "api")
    );
    let jobs: Vec<Job> = get(fetcher, board, &url).await?;
    Ok(jobs
        .into_iter()
        .map(|j| {
            let (commitment, location) = j
                .categories
                .map(|c| (c.commitment, c.location))
                .unwrap_or_default();
            Listed {
                url: format!("https://{}/{company}/{}", lever_host(eu, "jobs"), j.id),
                id: j.id,
                title: j.text,
                company: None,
                location,
                posted_at: j.created_at.map(|ms| timestamp(ms.div_euclid(1000))),
                employment_type: commitment.as_deref().and_then(employment),
                work_mode: j.workplace_type.as_deref().and_then(work_mode),
                salary: j.salary_range.and_then(|s| {
                    salary::from_source(
                        s.min,
                        s.max,
                        s.currency.as_deref()?,
                        s.interval.as_deref()?,
                    )
                }),
            }
            .into_feed(board)
        })
        .collect())
}

async fn ashby(
    fetcher: &impl Fetcher,
    board: &Board,
    org: &str,
) -> Result<Vec<FeedJob>, IngestError> {
    #[derive(Deserialize)]
    struct List {
        jobs: Vec<Job>,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Job {
        id: String,
        title: String,
        location: Option<String>,
        published_at: Option<String>,
        employment_type: Option<String>,
        is_remote: Option<bool>,
        workplace_type: Option<String>,
        is_listed: Option<bool>,
    }
    let url = format!("https://api.ashbyhq.com/posting-api/job-board/{org}");
    let list: List = get(fetcher, board, &url).await?;
    Ok(list
        .jobs
        .into_iter()
        .filter(|j| j.is_listed != Some(false))
        .map(|j| {
            Listed {
                url: format!("https://jobs.ashbyhq.com/{org}/{}", j.id),
                id: j.id,
                title: j.title,
                company: None,
                location: j.location,
                posted_at: j.published_at.as_deref().and_then(rfc3339),
                employment_type: j.employment_type.as_deref().and_then(employment),
                work_mode: j
                    .workplace_type
                    .as_deref()
                    .and_then(work_mode)
                    .or_else(|| j.is_remote.filter(|r| *r).map(|_| "remote".to_owned())),
                salary: None,
            }
            .into_feed(board)
        })
        .collect())
}

async fn workday(
    fetcher: &impl Fetcher,
    board: &Board,
    host: &str,
    tenant: &str,
    site: &str,
    now: &str,
) -> Result<Vec<FeedJob>, IngestError> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Page {
        /// Only the first page states it; later ones say 0.
        #[serde(default)]
        total: usize,
        #[serde(default)]
        job_postings: Vec<Job>,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Job {
        title: String,
        external_path: String,
        locations_text: Option<String>,
        posted_on: Option<String>,
    }
    let url = format!("https://{host}/wday/cxs/{tenant}/{site}/jobs");
    let today = days(now);
    let mut jobs = Vec::new();
    let mut total = None;
    let mut offset = 0;
    while offset < MAX_JOBS {
        let body = format!(
            r#"{{"limit":{WORKDAY_PAGE},"offset":{offset},"appliedFacets":{{}},"searchText":""}}"#
        );
        let resp = fetcher
            .post_json(&url, &body, WORKDAY_TIMEOUT)
            .await
            .map_err(|message| IngestError::Network {
                host: host.to_owned(),
                message,
            })?;
        let page: Page = parse(board, host, resp)?;
        let total = *total.get_or_insert(page.total);
        let n = page.job_postings.len();
        for j in page.job_postings {
            let id = j
                .external_path
                .rsplit('/')
                .next()
                .map(|last| last.rsplit_once('_').map_or(last, |(_, id)| id))
                .unwrap_or_default()
                .to_owned();
            if id.is_empty() {
                continue;
            }
            jobs.push(
                Listed {
                    url: format!("https://{host}/{site}{}", j.external_path),
                    id,
                    title: j.title,
                    company: None,
                    location: j.locations_text.filter(|l| !is_location_count(l)),
                    posted_at: j
                        .posted_on
                        .as_deref()
                        .and_then(days_ago)
                        .zip(today)
                        .map(|(ago, today)| timestamp((today - ago) * 86_400)),
                    employment_type: None,
                    work_mode: None,
                    salary: None,
                }
                .into_feed(board),
            );
        }
        offset += n;
        if n < WORKDAY_PAGE || offset >= total {
            break;
        }
    }
    Ok(jobs)
}

/// Workday's `locationsText` for a job in several places ("2 Locations").
fn is_location_count(s: &str) -> bool {
    s.split_once(' ').is_some_and(|(n, word)| {
        n.bytes().all(|b| b.is_ascii_digit()) && word.eq_ignore_ascii_case("locations")
    })
}

/// "Posted Today" → 0, "Posted Yesterday" → 1, "Posted 3 Days Ago" → 3; `None` for "Posted 30+ Days Ago".
fn days_ago(posted_on: &str) -> Option<i64> {
    let s = posted_on.trim().to_ascii_lowercase();
    let s = s.strip_prefix("posted ").unwrap_or(&s);
    match s {
        "today" => Some(0),
        "yesterday" => Some(1),
        _ => s.strip_suffix(" days ago")?.parse().ok(),
    }
}

async fn smartrecruiters(
    fetcher: &impl Fetcher,
    board: &Board,
    company: &str,
) -> Result<Vec<FeedJob>, IngestError> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Page {
        total_found: usize,
        content: Vec<Job>,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Job {
        id: String,
        name: String,
        company: Option<Company>,
        released_date: Option<String>,
        location: Option<Location>,
        type_of_employment: Option<Labeled>,
    }
    #[derive(Deserialize)]
    struct Company {
        name: Option<String>,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Location {
        full_location: Option<String>,
        remote: Option<bool>,
        hybrid: Option<bool>,
    }
    #[derive(Deserialize)]
    struct Labeled {
        label: Option<String>,
    }
    let mut jobs = Vec::new();
    loop {
        let offset = jobs.len();
        let url = format!(
            "https://api.smartrecruiters.com/v1/companies/{company}/postings?limit={SMARTRECRUITERS_PAGE}&offset={offset}"
        );
        let page: Page = get(fetcher, board, &url).await?;
        let n = page.content.len();
        jobs.extend(page.content.into_iter().map(|j| {
            let (location, remote, hybrid) = j
                .location
                .map(|l| (l.full_location, l.remote, l.hybrid))
                .unwrap_or_default();
            Listed {
                url: format!("https://jobs.smartrecruiters.com/{company}/{}", j.id),
                id: j.id,
                title: j.name,
                company: j.company.and_then(|c| c.name),
                location: location.map(|l| join_parts(l.split(','))),
                posted_at: j.released_date.as_deref().and_then(rfc3339),
                employment_type: j
                    .type_of_employment
                    .and_then(|t| t.label)
                    .as_deref()
                    .and_then(employment),
                work_mode: match (remote, hybrid) {
                    (Some(true), _) => Some("remote".to_owned()),
                    (_, Some(true)) => Some("hybrid".to_owned()),
                    _ => None,
                },
                salary: None,
            }
            .into_feed(board)
        }));
        if n == 0 || jobs.len() >= page.total_found || jobs.len() >= MAX_JOBS {
            break;
        }
    }
    Ok(jobs)
}

async fn workable(
    fetcher: &impl Fetcher,
    board: &Board,
    account: &str,
) -> Result<Vec<FeedJob>, IngestError> {
    #[derive(Deserialize)]
    struct List {
        name: Option<String>,
        jobs: Vec<Job>,
    }
    #[derive(Deserialize)]
    struct Job {
        title: String,
        shortcode: String,
        employment_type: Option<String>,
        telecommuting: Option<bool>,
        published_on: Option<String>,
        city: Option<String>,
        state: Option<String>,
        country: Option<String>,
    }
    let url = format!("https://apply.workable.com/api/v1/widget/accounts/{account}");
    let list: List = get(fetcher, board, &url).await?;
    Ok(list
        .jobs
        .into_iter()
        .map(|j| {
            Listed {
                url: format!("https://apply.workable.com/{account}/j/{}/", j.shortcode),
                id: j.shortcode,
                title: j.title,
                company: list.name.clone(),
                location: Some(join_parts(
                    [j.city, j.state, j.country]
                        .iter()
                        .flatten()
                        .map(String::as_str),
                )),
                posted_at: j.published_on.as_deref().and_then(rfc3339),
                employment_type: j.employment_type.as_deref().and_then(employment),
                work_mode: j.telecommuting.filter(|t| *t).map(|_| "remote".to_owned()),
                salary: None,
            }
            .into_feed(board)
        })
        .collect())
}

async fn recruitee(
    fetcher: &impl Fetcher,
    board: &Board,
    company: &str,
) -> Result<Vec<FeedJob>, IngestError> {
    #[derive(Deserialize)]
    struct List {
        offers: Vec<Offer>,
    }
    #[derive(Deserialize)]
    struct Offer {
        slug: String,
        title: String,
        company_name: Option<String>,
        location: Option<String>,
        published_at: Option<String>,
        employment_type_code: Option<String>,
        remote: Option<bool>,
        hybrid: Option<bool>,
        on_site: Option<bool>,
        salary: Option<Pay>,
    }
    #[derive(Deserialize)]
    struct Pay {
        min: Option<Value>,
        max: Option<Value>,
        currency: Option<String>,
        period: Option<String>,
    }
    let url = format!("https://{company}.recruitee.com/api/offers/");
    let list: List = get(fetcher, board, &url).await?;
    Ok(list
        .offers
        .into_iter()
        .map(|o| {
            let amount = |v: &Option<Value>| match v {
                Some(Value::Number(n)) => n.as_f64(),
                Some(Value::String(s)) => s.trim().parse().ok(),
                _ => None,
            };
            Listed {
                url: format!("https://{company}.recruitee.com/o/{}", o.slug),
                id: o.slug,
                title: o.title,
                company: o.company_name,
                location: o.location,
                posted_at: o.published_at.as_deref().and_then(rfc3339),
                employment_type: o.employment_type_code.as_deref().and_then(employment),
                work_mode: match (o.remote, o.hybrid, o.on_site) {
                    (Some(true), _, _) => Some("remote".to_owned()),
                    (_, Some(true), _) => Some("hybrid".to_owned()),
                    (_, _, Some(true)) => Some("onsite".to_owned()),
                    _ => None,
                },
                salary: o.salary.and_then(|p| {
                    salary::from_source(
                        amount(&p.min),
                        amount(&p.max),
                        p.currency.as_deref()?,
                        p.period.as_deref()?,
                    )
                }),
            }
            .into_feed(board)
        })
        .collect())
}

/// GETs `url` on the board's API and reads its JSON.
async fn get<T: DeserializeOwned>(
    fetcher: &impl Fetcher,
    board: &Board,
    url: &str,
) -> Result<T, IngestError> {
    let host = crate::site(url);
    let resp = fetcher
        .get(url)
        .await
        .map_err(|message| IngestError::Network {
            host: host.clone(),
            message,
        })?;
    parse(board, &host, resp)
}

/// A 2xx answer's JSON; 404 and 410 say the board doesn't exist.
fn parse<T: DeserializeOwned>(
    board: &Board,
    host: &str,
    resp: crate::Response,
) -> Result<T, IngestError> {
    if matches!(resp.status, 404 | 410) {
        return Err(IngestError::NotABoard {
            url: board.url(),
            reason: format!(
                "{host} has no {} board by that name (HTTP {})",
                board.source(),
                resp.status
            ),
        });
    }
    check_status(&resp, host, &board.url())?;
    serde_json::from_str(&resp.body).map_err(|_| IngestError::Network {
        host: host.to_owned(),
        message: "its answer wasn't the job list it documents".to_owned(),
    })
}

/// `full_time`, `part_time`, `contract` or `internship` from a board's wording (`FullTime`, `Permanent`,
/// `fulltime_permanent`, `Intern`).
fn employment(s: &str) -> Option<String> {
    let s = s.to_ascii_lowercase();
    let kind = if s.contains("intern") {
        "internship"
    } else if s.contains("part") {
        "part_time"
    } else if ["contract", "temporary", "freelance", "fixed"]
        .iter()
        .any(|w| s.contains(w))
    {
        "contract"
    } else if s.contains("full") || s.contains("permanent") {
        "full_time"
    } else {
        return None;
    };
    Some(kind.to_owned())
}

/// `remote`, `hybrid` or `onsite` from a board's wording (`OnSite`, `on-site`).
fn work_mode(s: &str) -> Option<String> {
    let s: String = s
        .chars()
        .filter(char::is_ascii_alphabetic)
        .collect::<String>()
        .to_ascii_lowercase();
    matches!(s.as_str(), "remote" | "hybrid" | "onsite").then_some(s)
}

/// Comma-separated parts, empty ones dropped (`Sydney, , Australia` → `Sydney, Australia`).
fn join_parts<'a>(parts: impl Iterator<Item = &'a str>) -> String {
    parts
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join(", ")
}

/// A board's date or time as RFC 3339 UTC: `2026-10-02T15:29:09-04:00`, `2026-10-13 10:58:56 UTC`,
/// `2026-09-02T19:46:28.102+00:00`, a bare `2026-05-29` (midnight UTC).
fn rfc3339(at: &str) -> Option<String> {
    let at = at.trim();
    let day = days(at)?;
    let rest = &at[10..];
    if rest.is_empty() {
        return Some(timestamp(day * 86_400));
    }
    let rest = rest.strip_prefix(['T', ' '])?;
    let num = |s: &str| {
        s.parse::<i64>()
            .ok()
            .filter(|_| s.bytes().all(|b| b.is_ascii_digit()))
    };
    let (h, m, s) = (
        num(rest.get(0..2)?)?,
        num(rest.get(3..5)?)?,
        num(rest.get(6..8)?)?,
    );
    let mut zone = rest[8..].trim_start_matches(|c: char| c == '.' || c.is_ascii_digit());
    zone = zone.trim();
    let offset = match zone {
        "" | "Z" | "z" | "UTC" => 0,
        _ => {
            let sign = match zone.as_bytes()[0] {
                b'+' => 1,
                b'-' => -1,
                _ => return None,
            };
            let digits = zone[1..].replace(':', "");
            if digits.len() != 4 || !digits.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            sign * (num(&digits[..2])? * 3600 + num(&digits[2..])? * 60)
        }
    };
    Some(timestamp(day * 86_400 + h * 3600 + m * 60 + s - offset))
}

/// Days since 1970-01-01 of the `YYYY-MM-DD` that starts `at`.
fn days(at: &str) -> Option<i64> {
    let b = at.as_bytes();
    if b.len() < 10 || !b[..10].is_ascii() || b[4] != b'-' || b[7] != b'-' {
        return None;
    }
    let y: i64 = at[..4].parse().ok()?;
    let m: i64 = at[5..7].parse().ok()?;
    let d: i64 = at[8..10].parse().ok()?;
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    // Howard Hinnant's days_from_civil.
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(era * 146_097 + doe - 719_468)
}

/// RFC 3339 UTC for seconds since the Unix epoch.
fn timestamp(secs: i64) -> String {
    let (z, rem) = (secs.div_euclid(86_400) + 719_468, secs.rem_euclid(86_400));
    // Howard Hinnant's civil_from_days.
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

#[cfg(test)]
mod tests;

//! Postings in (ARCHITECTURE.md §7): a pasted link is fetched and parsed, anything else is the posting's text.
//!
//! A link is read from the best source that has it: the Greenhouse or Lever job APIs, Ashby's job page data,
//! LinkedIn's guest job page (search and feed links rewritten to the job), else the page's schema.org `JobPosting`
//! JSON-LD, else the page's main text. Sites that refuse automated requests, or only show the posting in a browser,
//! end in an error that says to paste the text instead.

mod boards;
mod fetch;
mod link;
mod page;
pub mod record;
mod text;

use std::time::{Duration, SystemTime};

use resume_core::{Method, Posting};

pub use fetch::{Fetcher, HttpFetcher, Response};
use link::Link;

/// Less text than this from a page means it draws the posting with JavaScript.
pub const MIN_PAGE_TEXT: usize = 400;

/// Why a posting couldn't be read. Every message is a sentence the app can show as is.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum IngestError {
    #[error("Nothing to read; paste a job link or the posting's text")]
    Empty,
    /// HTTP 401/403 (Indeed answers 401), or LinkedIn's 999.
    #[error(
        "{host} refuses automated requests (HTTP {status}); copy the posting's text and paste it instead"
    )]
    Blocked { host: String, status: u16 },
    /// HTTP 429.
    #[error(
        "{host} is limiting requests (HTTP 429){}; copy the posting's text and paste it instead",
        retry_hint(*.retry_after)
    )]
    RateLimited {
        host: String,
        retry_after: Option<Duration>,
    },
    /// The page draws the posting with JavaScript.
    #[error(
        "{host} only shows the posting in a browser ({chars} characters of text found); copy the posting's text and \
         paste it instead"
    )]
    TooLittleText { host: String, chars: usize },
    /// No HTTP answer, or a server error.
    #[error("Couldn't reach {host}: {message}")]
    Network { host: String, message: String },
    /// The link answered, but not with a posting (gone, not a page, not on the board).
    #[error("{url} isn't a job posting: {reason}")]
    NotAPosting { url: String, reason: String },
}

fn retry_hint(after: Option<Duration>) -> String {
    match after.map(|d| d.as_secs()) {
        Some(0) | None => String::new(),
        Some(s) if s < 120 => format!(", retry in {s} s"),
        Some(s) => format!(", retry in {} min", s.div_ceil(60)),
    }
}

/// A posting read by a source, before it becomes a [`Posting`].
pub(crate) struct Found {
    pub(crate) method: Method,
    pub(crate) url: Option<String>,
    pub(crate) title: Option<String>,
    pub(crate) company: Option<String>,
    pub(crate) location: Option<String>,
    pub(crate) text: String,
}

/// Reads a posting from a link or pasted text, fetching over HTTP.
pub async fn read(input: &str) -> Result<Posting, IngestError> {
    read_with(&HttpFetcher::new(), input).await
}

/// [`read`] with any [`Fetcher`]: tests replay recorded responses through this.
pub async fn read_with(fetcher: &impl Fetcher, input: &str) -> Result<Posting, IngestError> {
    let input = input.trim();
    match Link::parse(input) {
        Some(link) => read_link(fetcher, input, &link).await,
        None => {
            let text = text::tidy_pasted(input);
            if text.is_empty() {
                return Err(IngestError::Empty);
            }
            Ok(Posting::pasted(text))
        }
    }
}

async fn read_link(
    fetcher: &impl Fetcher,
    source: &str,
    link: &Link,
) -> Result<Posting, IngestError> {
    let page_url = link.page_url();
    let host = site(&page_url);
    let resp = fetcher
        .get(&link.request_url())
        .await
        .map_err(|message| IngestError::Network {
            host: host.clone(),
            message,
        })?;
    check_status(&resp, &host, &page_url)?;
    let not_a_posting = |reason: &str| IngestError::NotAPosting {
        url: page_url.clone(),
        reason: reason.to_owned(),
    };
    let found = match link {
        Link::Greenhouse { .. } => boards::greenhouse(&resp.body)
            .ok_or_else(|| not_a_posting("Greenhouse returned no posting"))?,
        Link::Lever { .. } => {
            boards::lever(&resp.body).ok_or_else(|| not_a_posting("Lever returned no posting"))?
        }
        Link::Ashby { .. } => match boards::ashby(&resp.body) {
            Some(found) => found.map_err(|r| not_a_posting(&r))?,
            None => page::read(&resp.body, MIN_PAGE_TEXT),
        },
        Link::Linkedin { .. } => {
            let found = boards::linkedin(&resp.body).ok_or_else(|| IngestError::TooLittleText {
                host: host.clone(),
                chars: 0,
            })?;
            Found {
                url: Some(page_url.clone()),
                ..found
            }
        }
        Link::Page { .. } => {
            if let Some(kind) = resp.content_type.as_deref().filter(|t| !is_text(t)) {
                return Err(not_a_posting(&format!("it's a {kind} file, not a page")));
            }
            let found = page::read(&resp.body, MIN_PAGE_TEXT);
            Found {
                url: found.url.or(Some(resp.url.clone())),
                ..found
            }
        }
    };
    let chars = found.text.chars().count();
    if chars < MIN_PAGE_TEXT && found.method == Method::Html {
        return Err(IngestError::TooLittleText { host, chars });
    }
    if found.text.is_empty() {
        return Err(not_a_posting("the posting has no text"));
    }
    Ok(Posting::new(
        found.method,
        Some(source.to_owned()),
        found.url.or(Some(page_url)),
        found.title,
        found.company,
        found.location,
        found.text,
    ))
}

/// Statuses that end the read: refused, rate-limited, gone, or a server failure.
fn check_status(resp: &Response, host: &str, url: &str) -> Result<(), IngestError> {
    match resp.status {
        200..=399 => Ok(()),
        401 | 403 | 999 => Err(IngestError::Blocked {
            host: host.to_owned(),
            status: resp.status,
        }),
        429 => Err(IngestError::RateLimited {
            host: host.to_owned(),
            retry_after: resp
                .retry_after
                .as_deref()
                .and_then(|v| retry_after(v, SystemTime::now())),
        }),
        500.. => Err(IngestError::Network {
            host: host.to_owned(),
            message: format!("the server failed (HTTP {})", resp.status),
        }),
        404 | 410 => Err(IngestError::NotAPosting {
            url: url.to_owned(),
            reason: format!(
                "HTTP {}; the posting may have closed, or the link is wrong",
                resp.status
            ),
        }),
        s => Err(IngestError::NotAPosting {
            url: url.to_owned(),
            reason: format!("the site answered HTTP {s}"),
        }),
    }
}

/// `Retry-After` as seconds or an HTTP date.
fn retry_after(value: &str, now: SystemTime) -> Option<Duration> {
    let value = value.trim();
    if let Ok(secs) = value.parse::<u64>() {
        return Some(Duration::from_secs(secs));
    }
    let at = httpdate::parse_http_date(value).ok()?;
    Some(at.duration_since(now).unwrap_or_default())
}

fn is_text(content_type: &str) -> bool {
    let t = content_type.to_ascii_lowercase();
    t.starts_with("text/") || t.contains("html") || t.contains("xml") || t.contains("json")
}

/// `linkedin.com` from `https://www.linkedin.com/jobs/…`.
fn site(url: &str) -> String {
    let host = url::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(str::to_owned))
        .unwrap_or_else(|| url.to_owned());
    host.strip_prefix("www.").map(str::to_owned).unwrap_or(host)
}

#[cfg(test)]
mod tests;

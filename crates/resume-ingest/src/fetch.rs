//! Getting a page: the [`Fetcher`] seam, so parsers run offline on recorded responses, and the HTTP one.

use std::time::Duration;

/// Some boards (Indeed, LinkedIn) answer bots differently from browsers.
const BROWSER_UA: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) \
                          Version/18.0 Safari/605.1.15";
const ACCEPT: &str = "text/html,application/xhtml+xml,application/json;q=0.9,*/*;q=0.8";
const TIMEOUT: Duration = Duration::from_secs(20);

/// A response as the parsers need it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
    /// The URL that answered, after redirects.
    pub url: String,
    pub status: u16,
    pub content_type: Option<String>,
    /// The raw `Retry-After` header, when sent.
    pub retry_after: Option<String>,
    pub body: String,
}

/// Something that GETs a URL, following redirects. `Err` is a network failure (no HTTP status), as a message.
pub trait Fetcher {
    fn get(&self, url: &str) -> impl Future<Output = Result<Response, String>> + Send;
}

/// The real thing: one `reqwest` client with a browser's headers.
#[derive(Debug, Clone)]
pub struct HttpFetcher {
    client: reqwest::Client,
}

impl HttpFetcher {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .user_agent(BROWSER_UA)
            .timeout(TIMEOUT)
            .build()
            .expect("the HTTP client's static configuration is valid");
        Self { client }
    }
}

impl Default for HttpFetcher {
    fn default() -> Self {
        Self::new()
    }
}

impl Fetcher for HttpFetcher {
    async fn get(&self, url: &str) -> Result<Response, String> {
        let resp = self
            .client
            .get(url)
            .header(reqwest::header::ACCEPT, ACCEPT)
            .header(reqwest::header::ACCEPT_LANGUAGE, "en-US,en;q=0.9")
            .send()
            .await
            .map_err(|e| network_message(&e))?;
        let header = |name| {
            resp.headers()
                .get(name)
                .and_then(|v: &reqwest::header::HeaderValue| v.to_str().ok())
                .map(str::to_owned)
        };
        let content_type = header(reqwest::header::CONTENT_TYPE);
        let retry_after = header(reqwest::header::RETRY_AFTER);
        let status = resp.status().as_u16();
        let final_url = resp.url().to_string();
        let body = resp.text().await.map_err(|e| network_message(&e))?;
        Ok(Response {
            url: final_url,
            status,
            content_type,
            retry_after,
            body,
        })
    }
}

/// The innermost cause reads best ("dns error: …", "operation timed out").
fn network_message(e: &reqwest::Error) -> String {
    if e.is_timeout() {
        return format!("no answer within {} s", TIMEOUT.as_secs());
    }
    let mut source: &dyn std::error::Error = e;
    while let Some(next) = source.source() {
        source = next;
    }
    source.to_string()
}

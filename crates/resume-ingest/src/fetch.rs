//! Getting a page: the [`Fetcher`] seam, so parsers run offline on recorded responses, and the HTTP one.

use std::time::Duration;

/// Some boards (Indeed, LinkedIn) answer bots differently from browsers.
const BROWSER_UA: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) \
                          Version/18.0 Safari/605.1.15";
const ACCEPT: &str = "text/html,application/xhtml+xml,application/json;q=0.9,*/*;q=0.8";
/// AVIF left out: the app can't decode it.
const ACCEPT_IMAGE: &str = "image/webp,image/png,image/svg+xml,image/*;q=0.8,*/*;q=0.5";
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

    /// POSTs `body` as JSON, waiting up to `timeout` for the answer (Workday's job search, Apify's runs). Fetchers
    /// that only replay GETs answer every POST with a network failure.
    fn post_json(
        &self,
        url: &str,
        body: &str,
        timeout: Duration,
    ) -> impl Future<Output = Result<Response, String>> + Send {
        let _ = (url, body, timeout);
        async { Err("this fetcher can't POST".to_owned()) }
    }
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

    /// GETs an image: its bytes and media type (`image/png`, lowercase, parameters dropped). `None` for any status
    /// but 200, a type that isn't `image/*`, or a body over `max_bytes`; `Err` is a network failure, as a message.
    pub async fn get_image(
        &self,
        url: &str,
        max_bytes: usize,
    ) -> Result<Option<(Vec<u8>, String)>, String> {
        let mut resp = self
            .client
            .get(url)
            .header(reqwest::header::ACCEPT, ACCEPT_IMAGE)
            .send()
            .await
            .map_err(|e| network_message(&e, TIMEOUT))?;
        let media_type = resp
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .and_then(|t| t.split(';').next())
            .map(|t| t.trim().to_ascii_lowercase())
            .unwrap_or_default();
        let too_long = resp.content_length().is_some_and(|n| n > max_bytes as u64);
        if resp.status() != reqwest::StatusCode::OK || !media_type.starts_with("image/") || too_long
        {
            return Ok(None);
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = resp
            .chunk()
            .await
            .map_err(|e| network_message(&e, TIMEOUT))?
        {
            if bytes.len() + chunk.len() > max_bytes {
                return Ok(None);
            }
            bytes.extend_from_slice(&chunk);
        }
        Ok(Some((bytes, media_type)))
    }
}

impl Default for HttpFetcher {
    fn default() -> Self {
        Self::new()
    }
}

impl Fetcher for HttpFetcher {
    async fn get(&self, url: &str) -> Result<Response, String> {
        let req = self
            .client
            .get(url)
            .header(reqwest::header::ACCEPT, ACCEPT)
            .header(reqwest::header::ACCEPT_LANGUAGE, "en-US,en;q=0.9");
        send(req, TIMEOUT).await
    }

    async fn post_json(
        &self,
        url: &str,
        body: &str,
        timeout: Duration,
    ) -> Result<Response, String> {
        let req = self
            .client
            .post(url)
            .timeout(timeout)
            .header(reqwest::header::ACCEPT, "application/json")
            .header(reqwest::header::ACCEPT_LANGUAGE, "en-US,en;q=0.9")
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(body.to_owned());
        send(req, timeout).await
    }
}

/// Sends `req` and reads its answer as text; `timeout` names the wait in a timeout's message.
async fn send(req: reqwest::RequestBuilder, timeout: Duration) -> Result<Response, String> {
    let resp = req.send().await.map_err(|e| network_message(&e, timeout))?;
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
    let body = resp
        .text()
        .await
        .map_err(|e| network_message(&e, timeout))?;
    Ok(Response {
        url: final_url,
        status,
        content_type,
        retry_after,
        body,
    })
}

/// The innermost cause reads best ("dns error: …", "operation timed out").
fn network_message(e: &reqwest::Error, timeout: Duration) -> String {
    if e.is_timeout() {
        return format!("no answer within {} s", timeout.as_secs());
    }
    let mut source: &dyn std::error::Error = e;
    while let Some(next) = source.source() {
        source = next;
    }
    source.to_string()
}

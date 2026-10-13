//! Recorded responses, so parsers are tested offline on what the sites really sent.
//!
//! A case directory (`fixtures/ingest/<case>/`) holds `request.json` (the input, the URL requested and the
//! response's status and headers) and the body as `body.json` or `body.html`. A read makes one request, so a case
//! holds one.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{Fetcher, Response};

/// `request.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Recording {
    /// What was pasted.
    pub input: String,
    /// The URL requested.
    pub request: String,
    /// The URL that answered, after redirects.
    pub url: String,
    pub status: u16,
    pub content_type: Option<String>,
    pub retry_after: Option<String>,
    /// The body's file name, beside `request.json`.
    pub body: String,
    /// Date fetched, `YYYY-MM-DD`.
    pub fetched: String,
    /// How the body differs from what was received, if it was trimmed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// Fetches with `inner` and writes the response into `dir` as a case.
pub struct Recorder<F> {
    inner: F,
    dir: PathBuf,
    input: String,
    fetched: String,
}

impl<F> Recorder<F> {
    pub fn new(inner: F, dir: impl Into<PathBuf>, input: &str, fetched: &str) -> Self {
        Self {
            inner,
            dir: dir.into(),
            input: input.trim().to_owned(),
            fetched: fetched.to_owned(),
        }
    }

    fn write(&self, request: &str, resp: &Response) -> std::io::Result<()> {
        let json = resp
            .content_type
            .as_deref()
            .is_some_and(|t| t.contains("json"));
        let body = if json { "body.json" } else { "body.html" };
        let rec = Recording {
            input: self.input.clone(),
            request: request.to_owned(),
            url: resp.url.clone(),
            status: resp.status,
            content_type: resp.content_type.clone(),
            retry_after: resp.retry_after.clone(),
            body: body.to_owned(),
            fetched: self.fetched.clone(),
            note: None,
        };
        fs::create_dir_all(&self.dir)?;
        let meta = serde_json::to_string_pretty(&rec).map_err(std::io::Error::other)?;
        fs::write(self.dir.join("request.json"), meta + "\n")?;
        fs::write(self.dir.join(body), &resp.body)
    }
}

impl<F: Fetcher + Sync> Fetcher for Recorder<F> {
    async fn get(&self, url: &str) -> Result<Response, String> {
        let resp = self.inner.get(url).await?;
        self.write(url, &resp)
            .map_err(|e| format!("couldn't record into {}: {e}", self.dir.display()))?;
        Ok(resp)
    }
}

/// Answers from a recorded case; a request for any other URL fails, so a test also checks which URL a link maps
/// to.
pub struct Replay {
    pub recording: Recording,
    body: String,
}

impl Replay {
    pub fn load(dir: &Path) -> std::io::Result<Self> {
        let meta = fs::read_to_string(dir.join("request.json"))?;
        let recording: Recording = serde_json::from_str(&meta).map_err(std::io::Error::other)?;
        let body = fs::read_to_string(dir.join(&recording.body))?;
        Ok(Self { recording, body })
    }
}

impl Fetcher for Replay {
    async fn get(&self, url: &str) -> Result<Response, String> {
        let r = &self.recording;
        if url != r.request {
            return Err(format!("recorded {}, asked for {url}", r.request));
        }
        Ok(Response {
            url: r.url.clone(),
            status: r.status,
            content_type: r.content_type.clone(),
            retry_after: r.retry_after.clone(),
            body: self.body.clone(),
        })
    }
}

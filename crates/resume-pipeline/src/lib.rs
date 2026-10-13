//! The per-posting steps the `ez` CLI and the app share: reading a posting, the cache keys of Extract and Suggest
//! output, the bank candidates Suggest sees, a master's template and page facts, and today's month.
//!
//! Callers own the caching: the CLI keeps model output in files under `local/cache/`, the app in its store. Both key
//! it with [`extract_key`] and [`suggest_key`], so a change to a prompt, a model or the master invalidates it.

use std::time::{SystemTime, UNIX_EPOCH};

use resume_core::candidates::bank_candidates;
use resume_core::change::Mode;
use resume_core::posting::text_hash;
use resume_core::{
    Bank, Master, Method, PageFacts, Posting, Requirements, ResolveError, Resume, resolve,
};
use resume_llm::extract;
use resume_llm::suggest::{self, SuggestInput};
use resume_render::{FontOptions, RenderError, Renderer, Template};

/// Bank entries outside the master Suggest sees in bank-aware mode (ARCHITECTURE.md §2).
pub const CANDIDATES: usize = 5;

#[derive(Debug, thiserror::Error)]
pub enum PipelineError {
    #[error("no built-in template `{0}`")]
    UnknownTemplate(String),
    #[error(transparent)]
    Resolve(#[from] ResolveError),
    #[error(transparent)]
    Render(#[from] RenderError),
    #[error("resume JSON: {0}")]
    Json(#[from] serde_json::Error),
}

/// A posting file: optional `---` front matter (`title`, `company`, `location`, `source`, `method`), then the text.
/// Front matter of a pasted fixture (`method: pasted text`) is ignored, as the app would have none.
pub fn posting_file(file: &str) -> Posting {
    let Some((front, body)) = file
        .strip_prefix("---\n")
        .and_then(|rest| rest.split_once("\n---\n"))
    else {
        return Posting::pasted(file.trim().to_owned());
    };
    let field = |key: &str| {
        front.lines().find_map(|l| {
            l.strip_prefix(key)
                .and_then(|v| v.strip_prefix(':'))
                .map(|v| v.trim().to_owned())
                .filter(|v| !v.is_empty())
        })
    };
    let text = body.trim().to_owned();
    if field("method").is_none_or(|m| m.starts_with("pasted")) {
        return Posting::pasted(text);
    }
    let source = field("source").filter(|s| s.starts_with("http"));
    Posting::new(
        Method::Html,
        source.clone(),
        source,
        field("title"),
        field("company"),
        field("location"),
        text,
    )
}

/// What keys Extract's output for `posting`: its text and the prompt and schema Extract runs with
/// (`<posting hash 16>-<prompt hash 12>`). The model is the caller's to add.
pub fn extract_key(posting: &Posting) -> String {
    let prompt = &text_hash(&format!("{}\n{}", extract::PROMPT, extract::SCHEMA))[..12];
    format!("{}-{prompt}", &posting.hash[..16])
}

/// What keys Suggest's output for `input` in `mode`: the posting, the master as Suggest reads it (with its bank
/// candidates), the mode, and the prompt and schema (`<posting 16>-<master 12>-<mode>-<prompt 12>`). The model is
/// the caller's to add.
pub fn suggest_key(input: &SuggestInput<'_>, mode: Mode) -> String {
    let prompt = &text_hash(&format!("{}\n{}", suggest::PROMPT, suggest::SCHEMA))[..12];
    let master = &text_hash(&suggest::input(&SuggestInput {
        posting: &Posting::pasted(String::new()),
        ..*input
    }))[..12];
    format!(
        "{}-{master}-{}-{prompt}",
        &input.posting.hash[..16],
        mode_name(mode)
    )
}

/// `master-only` or `bank-aware`, as `Mode` serializes.
pub fn mode_name(mode: Mode) -> &'static str {
    match mode {
        Mode::MasterOnly => "master-only",
        Mode::BankAware => "bank-aware",
    }
}

/// Bank entry ids Suggest may propose in `mode`: none for master-only, the best [`CANDIDATES`] by requirement-term
/// overlap for bank-aware.
pub fn candidates(bank: &Bank, master: &Master, req: &Requirements, mode: Mode) -> Vec<String> {
    match mode {
        Mode::MasterOnly => Vec::new(),
        Mode::BankAware => bank_candidates(bank, master, req, CANDIDATES),
    }
}

/// The master's built-in template.
pub fn template(master: &Master) -> Result<Template, PipelineError> {
    match master.template.as_str() {
        "classic-serif" => Ok(Template::classic_serif()),
        other => Err(PipelineError::UnknownTemplate(other.to_owned())),
    }
}

/// The master resolved and rendered with the bundled fonts: the resume and its page facts for the score.
pub fn master_page(bank: &Bank, master: &Master) -> Result<(Resume, PageFacts), PipelineError> {
    let resume = resolve(bank, master)?;
    let template = template(master)?;
    let rendered = Renderer::new(&template, FontOptions::default())
        .render(&serde_json::to_string(&resume)?, false)?;
    let facts = PageFacts {
        pages: rendered.metrics.pages,
        page_limit: master.page_limit,
        ats_safe: template.manifest.ats_safe,
        pdf_text: None,
    };
    Ok((resume, facts))
}

/// `Role · Company · Location`, then the posting's link.
pub fn heading(posting: &Posting, req: &Requirements) -> String {
    let mut h = format!("{} · {}", req.role, req.company);
    if !req.location.is_empty() {
        h += &format!(" · {}", req.location);
    }
    if let Some(url) = &posting.url {
        h += &format!("\n{url}");
    }
    h
}

/// The current `(year, month)` in UTC, the month a `present` end date stands for.
pub fn today() -> (u32, u32) {
    let days = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() / 86_400) as i64;
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year as u32, month as u32)
}

//! Every failure as an [`AppError`]: a sentence the UI shows as is, and its kind.

use resume_ingest::IngestError;
use resume_llm::{AgentKind, LlmError};
use resume_pipeline::PipelineError;
use resume_render::RenderError;
use resume_store::StoreError;
use resume_tailor::TailorError;

pub use crate::dto::AppError;
use crate::dto::ErrorKind;
use crate::settings;

pub type Result<T, E = AppError> = std::result::Result<T, E>;

impl AppError {
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    pub fn missing(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Missing, message)
    }

    pub fn other(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Other, message)
    }

    pub fn cancelled() -> Self {
        Self::new(ErrorKind::Cancelled, "Cancelled")
    }

    /// A review command named a review that isn't the open one.
    pub fn replaced() -> Self {
        Self::missing("This review was replaced; reopen it")
    }

    pub fn no_bank() -> Self {
        Self::missing("Import your resume first: Experience bank, then Import resume")
    }

    pub fn no_posting() -> Self {
        Self::missing("This posting was deleted")
    }
}

/// What to tell you when the agent failed: signed out, missing, slow, or an answer of the wrong shape.
impl From<LlmError> for AppError {
    fn from(e: LlmError) -> Self {
        let name = e.agent().map_or("The agent", AgentKind::name);
        let message = match &e {
            LlmError::NotFound(_) => format!(
                "{name} isn't installed, or gliide can't find it. Install it, or set its path in Settings."
            ),
            LlmError::Spawn { source, .. } => format!("{name} couldn't start: {source}."),
            LlmError::Timeout { after, .. } => format!(
                "{name} didn't answer within {} seconds. Try again.",
                after.as_secs()
            ),
            LlmError::Exit { agent, message, .. } | LlmError::Agent { agent, message } => {
                if signed_out(message) {
                    format!("{name} isn't signed in. {}", settings::sign_in(*agent))
                } else if message.is_empty() {
                    format!("{name} stopped without an answer. Try again.")
                } else {
                    format!("{name} failed: {message}")
                }
            }
            LlmError::Schema(m) => format!("A built-in prompt's schema is invalid: {m}"),
            LlmError::Invalid { .. } | LlmError::Decode { .. } => {
                "The agent's answer didn't have the expected shape, even after one repair. Try again."
                    .to_owned()
            }
        };
        Self::new(ErrorKind::Agent, message)
    }
}

/// The agent's message says it has no usable login.
fn signed_out(message: &str) -> bool {
    let m = message.to_lowercase();
    [
        "not logged in",
        "/login",
        "log in",
        "login",
        "sign in",
        "authenticat",
        "api key",
        "oauth",
        "credential",
    ]
    .iter()
    .any(|k| m.contains(k))
}

impl From<IngestError> for AppError {
    fn from(e: IngestError) -> Self {
        Self::new(ErrorKind::Ingest, e.to_string())
    }
}

impl From<StoreError> for AppError {
    fn from(e: StoreError) -> Self {
        Self::new(
            ErrorKind::Storage,
            format!("The app's data couldn't be read or saved ({e})"),
        )
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        Self::new(
            ErrorKind::Storage,
            format!("A file couldn't be read or written ({e})"),
        )
    }
}

impl From<TailorError> for AppError {
    fn from(e: TailorError) -> Self {
        Self::other(format!("The resume couldn't be laid out ({e})"))
    }
}

impl From<PipelineError> for AppError {
    fn from(e: PipelineError) -> Self {
        Self::other(format!("The resume couldn't be laid out ({e})"))
    }
}

impl From<RenderError> for AppError {
    fn from(e: RenderError) -> Self {
        Self::other(format!("The resume couldn't be rendered ({e})"))
    }
}

impl From<resume_core::ResolveError> for AppError {
    fn from(e: resume_core::ResolveError) -> Self {
        Self::other(format!("The master doesn't match your bank ({e})"))
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        Self::other(format!("Unexpected data ({e})"))
    }
}

impl From<tokio::task::JoinError> for AppError {
    fn from(e: tokio::task::JoinError) -> Self {
        if e.is_cancelled() {
            Self::cancelled()
        } else {
            Self::other(format!("A background step failed ({e})"))
        }
    }
}

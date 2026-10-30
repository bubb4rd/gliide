//! The agents gliide runs, and one [`Llm`] over whichever the user chose.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::{AntigravityCli, Call, ClaudeCli, CodexCli, Llm, LlmError, Schema, Structured, cli};

/// An agent CLI gliide can run.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentKind {
    #[default]
    Claude,
    Codex,
    Antigravity,
}

impl AgentKind {
    pub const ALL: [Self; 3] = [Self::Claude, Self::Codex, Self::Antigravity];

    /// Its binary's name.
    pub const fn bin(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::Antigravity => "agy",
        }
    }

    /// Its product's name, for messages: "Claude Code", "Codex", "Antigravity".
    pub const fn name(self) -> &'static str {
        match self {
            Self::Claude => "Claude Code",
            Self::Codex => "Codex",
            Self::Antigravity => "Antigravity",
        }
    }

    /// The model calls run on unless the user picks another.
    pub const fn default_model(self) -> &'static str {
        match self {
            Self::Claude => ClaudeCli::DEFAULT_MODEL,
            Self::Codex => CodexCli::DEFAULT_MODEL,
            Self::Antigravity => AntigravityCli::DEFAULT_MODEL,
        }
    }

    /// Its binary: the usual install locations (`~/.local/bin`, `~/.claude/local`, Homebrew, `/usr/local/bin`, then
    /// nvm's Node versions, newest first), else whatever a login shell finds.
    pub async fn find(self) -> Option<PathBuf> {
        cli::find(self.bin()).await
    }
}

impl std::str::FromStr for AgentKind {
    type Err = String;

    /// `claude`, `codex` or `antigravity`.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|a| serde_json::to_value(a).is_ok_and(|v| v == s))
            .ok_or_else(|| format!("unknown agent {s:?}; one of claude, codex, antigravity"))
    }
}

/// The agent the user runs, as one [`Llm`].
#[derive(Clone, Debug)]
pub enum Agent {
    Claude(ClaudeCli),
    Codex(CodexCli),
    Antigravity(AntigravityCli),
}

impl Agent {
    /// `kind`'s provider for its binary at `bin`.
    pub fn new(kind: AgentKind, bin: impl Into<PathBuf>) -> Self {
        match kind {
            AgentKind::Claude => Self::Claude(ClaudeCli::new(bin)),
            AgentKind::Codex => Self::Codex(CodexCli::new(bin)),
            AgentKind::Antigravity => Self::Antigravity(AntigravityCli::new(bin)),
        }
    }

    /// `kind`'s provider for the binary [`AgentKind::find`] finds.
    pub async fn discover(kind: AgentKind) -> Result<Self, LlmError> {
        kind.find()
            .await
            .map(|bin| Self::new(kind, bin))
            .ok_or(LlmError::NotFound(kind))
    }

    pub fn kind(&self) -> AgentKind {
        match self {
            Self::Claude(_) => AgentKind::Claude,
            Self::Codex(_) => AgentKind::Codex,
            Self::Antigravity(_) => AgentKind::Antigravity,
        }
    }

    pub fn bin(&self) -> &Path {
        match self {
            Self::Claude(a) => a.bin(),
            Self::Codex(a) => a.bin(),
            Self::Antigravity(a) => a.bin(),
        }
    }

    /// Time allowed per turn; a repair is a second turn with its own allowance.
    pub fn with_timeout(self, timeout: Duration) -> Self {
        match self {
            Self::Claude(a) => Self::Claude(a.with_timeout(timeout)),
            Self::Codex(a) => Self::Codex(a.with_timeout(timeout)),
            Self::Antigravity(a) => Self::Antigravity(a.with_timeout(timeout)),
        }
    }
}

impl Llm for Agent {
    async fn structured(&self, call: Call<'_>, schema: &Schema) -> Result<Structured, LlmError> {
        match self {
            Self::Claude(a) => a.structured(call, schema).await,
            Self::Codex(a) => a.structured(call, schema).await,
            Self::Antigravity(a) => a.structured(call, schema).await,
        }
    }
}

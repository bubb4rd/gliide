//! What `score`, `rank` and `tailor` share: reading a posting and extracting its requirements, cached in files.

use std::path::{Path, PathBuf};

use anyhow::Context;
use resume_core::{Posting, Requirements};
use resume_llm::{Agent, AgentKind, extract};
use resume_pipeline::{extract_key, posting_file};

/// Where the posting comes from, the bank, the agent, and the model-output cache.
#[derive(clap::Args)]
pub struct Source {
    /// A job link, a posting file (`.md` fixtures keep their front matter as hints), `-` for stdin, or the text.
    pub posting: String,
    #[arg(long)]
    pub bank: PathBuf,
    /// The agent CLI the model calls run on.
    #[arg(long, value_enum, default_value = "claude")]
    pub agent: AgentArg,
    /// Cache folder: requirements keyed by posting text, prompt and model (and suggestions, for `tailor`).
    #[arg(long, default_value = "local/cache")]
    pub cache: PathBuf,
    /// Ignore cached model output and call the model again.
    #[arg(long)]
    pub fresh: bool,
}

impl Source {
    /// `model`, else the agent's default.
    pub fn model<'a>(&self, model: &'a Option<String>) -> &'a str {
        model
            .as_deref()
            .unwrap_or(AgentKind::from(self.agent).default_model())
    }
}

/// An agent CLI gliide runs.
#[derive(Clone, Copy, clap::ValueEnum)]
pub enum AgentArg {
    Claude,
    Codex,
    Antigravity,
}

impl From<AgentArg> for AgentKind {
    fn from(a: AgentArg) -> Self {
        match a {
            AgentArg::Claude => Self::Claude,
            AgentArg::Codex => Self::Codex,
            AgentArg::Antigravity => Self::Antigravity,
        }
    }
}

pub fn runtime() -> anyhow::Result<tokio::runtime::Runtime> {
    Ok(tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?)
}

/// The posting and its requirements, extracted with `model`: from the cache when this text, prompt and model were
/// extracted before.
pub async fn requirements(src: &Source, model: &str) -> anyhow::Result<(Posting, Requirements)> {
    let posting = read_posting(&src.posting).await?;
    let cached = src
        .cache
        .join("requirements")
        .join(model)
        .join(format!("{}.json", extract_key(&posting)));
    if !src.fresh && cached.exists() {
        eprintln!("requirements: cached ({})", cached.display());
        return Ok((posting, super::read_json(&cached)?));
    }
    let llm = Agent::discover(src.agent.into()).await?;
    eprintln!(
        "requirements: extracting with {} --model {model} …",
        llm.kind().bin()
    );
    let (req, out) = extract::extract(&llm, &extract::schema(), &posting, model).await?;
    eprintln!(
        "requirements: {:.1} s{}",
        out.latency().as_secs_f64(),
        if out.repaired() {
            " (after one repair)"
        } else {
            ""
        }
    );
    super::write_json(&cached, &req)?;
    Ok((posting, req))
}

async fn read_posting(input: &str) -> anyhow::Result<Posting> {
    if input == "-" {
        let text = std::io::read_to_string(std::io::stdin())?;
        return Ok(resume_ingest::read(&text).await?);
    }
    let path = Path::new(input);
    if path.is_file() {
        let text =
            std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        return Ok(posting_file(&text));
    }
    Ok(resume_ingest::read(input).await?)
}

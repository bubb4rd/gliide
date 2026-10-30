//! Settings and the agents' status.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use resume_llm::AgentKind;
use serde_json::Value;
use tokio::process::Command;

use crate::dto::{AgentStatus, ModelOption, Settings};
use crate::error::{AppError, Result};
use crate::export;
use crate::state::App;

/// How long each check (`--version`, a sign-in check, a model list) may take.
const CHECK_TIMEOUT: Duration = Duration::from_secs(15);

/// Claude Code's `--model` aliases, newest model of each family.
const CLAUDE_MODELS: [(&str, &str); 3] =
    [("sonnet", "Sonnet"), ("opus", "Opus"), ("haiku", "Haiku")];

pub fn get_settings(app: &App) -> Settings {
    app.settings()
}

/// Saves `s` (trimmed; an empty model falls back to its agent's default). A new export folder holding an existing
/// `applications.csv` has its rows imported (when it can be read: every write imports again first).
pub fn set_settings(app: &App, mut s: Settings) -> Result<Settings> {
    for agent in AgentKind::ALL {
        let a = s.agents.get_mut(agent.into());
        for model in [&mut a.extract_model, &mut a.suggest_model] {
            *model = model.trim().to_owned();
            if model.is_empty() {
                *model = agent.default_model().to_owned();
            }
        }
        a.path = a
            .path
            .take()
            .map(|p| p.trim().to_owned())
            .filter(|p| !p.is_empty());
    }
    s.export_dir = s.export_dir.trim().to_owned();
    if !Path::new(&s.export_dir).is_absolute() {
        return Err(AppError::other("Choose a full folder path for exports"));
    }
    let moved = app.settings().export_dir != s.export_dir;
    app.put_settings(&s)?;
    if moved {
        let _ = export::import_applications_csv(app);
    }
    Ok(s)
}

/// Every agent's status, checked at once, in [`AgentKind::ALL`]'s order.
pub async fn agent_statuses(app: &Arc<App>) -> Vec<AgentStatus> {
    let s = app.settings();
    let path = |a: AgentKind| s.agents.get(a.into()).path.clone();
    let [claude, codex, antigravity] = AgentKind::ALL;
    let (claude, codex, antigravity) = tokio::join!(
        status(claude, path(claude)),
        status(codex, path(codex)),
        status(antigravity, path(antigravity)),
    );
    vec![claude, codex, antigravity]
}

/// `agent` found (`--version`) and signed in, and the models it offers. Only what says whether it's signed in is
/// read: `claude auth status`'s `loggedIn` and `subscriptionType`, `codex login status`'s exit and first words,
/// whether `agy models` answers; never an email or a token.
async fn status(agent: AgentKind, configured: Option<String>) -> AgentStatus {
    let name = agent.name();
    let path = match configured.filter(|p| !p.trim().is_empty()) {
        Some(p) => Some(PathBuf::from(p)).filter(|p| p.is_file()),
        None => agent.find().await,
    };
    let Some(path) = path else {
        return AgentStatus {
            agent: agent.into(),
            found: false,
            path: None,
            version: None,
            signed_in: None,
            plan: None,
            problem: Some(format!(
                "{name} isn't installed, or gliide can't find it. Install it, or set its path in Settings."
            )),
            models: Vec::new(),
        };
    };
    let version = run(&path, &["--version"])
        .await
        .filter(|out| out.ok)
        .and_then(|out| {
            let line = out.stdout.lines().next()?.trim();
            let v = line
                .trim_end_matches("(Claude Code)")
                .trim_start_matches("codex-cli")
                .trim();
            (!v.is_empty()).then(|| v.to_owned())
        });
    let (signed_in, plan, models) = match agent {
        AgentKind::Claude => claude_sign_in(&path).await,
        AgentKind::Codex => codex_sign_in(&path).await,
        AgentKind::Antigravity => antigravity_sign_in(&path).await,
    };
    let problem = match (version.is_some(), signed_in) {
        (false, _) => Some(format!(
            "{} didn't run. Reinstall {name}, or set its path in Settings.",
            path.display()
        )),
        (true, Some(false)) => Some(format!("{name} isn't signed in. {}", sign_in(agent))),
        (true, None) => Some(format!(
            "Couldn't check the {name} login. Run `{}` in Terminal to check it.",
            agent.bin()
        )),
        (true, Some(true)) => None,
    };
    AgentStatus {
        agent: agent.into(),
        found: true,
        path: Some(path.display().to_string()),
        version,
        signed_in,
        plan,
        problem,
        models,
    }
}

/// What to do to sign `agent` in, as a sentence.
pub fn sign_in(agent: AgentKind) -> &'static str {
    match agent {
        AgentKind::Claude => "Run `claude` in Terminal and sign in.",
        AgentKind::Codex => "Run `codex login` in Terminal and sign in.",
        AgentKind::Antigravity => "Run `agy` in Terminal and sign in.",
    }
}

type SignIn = (Option<bool>, Option<String>, Vec<ModelOption>);

/// `claude auth status --json`'s `loggedIn` and plan; the aliases as its models.
async fn claude_sign_in(bin: &Path) -> SignIn {
    let auth = run(bin, &["auth", "status", "--json"])
        .await
        .filter(|out| out.ok)
        .and_then(|out| serde_json::from_str::<Value>(out.stdout.trim()).ok());
    let signed_in = auth
        .as_ref()
        .and_then(|a| a.get("loggedIn"))
        .and_then(Value::as_bool);
    let plan = auth
        .as_ref()
        .and_then(|a| a.get("subscriptionType"))
        .and_then(Value::as_str)
        .filter(|p| !p.is_empty())
        .map(capitalized);
    let models = CLAUDE_MODELS
        .iter()
        .map(|(id, name)| ModelOption {
            id: (*id).to_owned(),
            name: (*name).to_owned(),
        })
        .collect();
    (signed_in, plan, models)
}

/// `codex login status`: exits 0 when signed in, saying how ("Logged in using ChatGPT"); the models `codex debug
/// models` lists for pickers.
async fn codex_sign_in(bin: &Path) -> SignIn {
    let (login, catalog) = tokio::join!(
        run(bin, &["login", "status"]),
        run(bin, &["debug", "models"])
    );
    let (signed_in, plan) = match &login {
        Some(out) if out.ok => {
            let said = out.all().to_lowercase();
            let plan = if said.contains("chatgpt") {
                Some("ChatGPT".to_owned())
            } else if said.contains("api key") {
                Some("API key".to_owned())
            } else {
                None
            };
            (Some(true), plan)
        }
        Some(out) if out.all().to_lowercase().contains("not logged in") => (Some(false), None),
        _ => (None, None),
    };
    let models = catalog
        .filter(|out| out.ok)
        .and_then(|out| serde_json::from_str::<Value>(&out.stdout).ok())
        .map(|v| codex_models(&v))
        .unwrap_or_default();
    (signed_in, plan, models)
}

/// The models a `codex debug models` catalog lists for pickers (`visibility: list`), in its order.
fn codex_models(catalog: &Value) -> Vec<ModelOption> {
    catalog
        .get("models")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|m| m.get("visibility").and_then(Value::as_str) == Some("list"))
        .filter_map(|m| {
            let id = m.get("slug").and_then(Value::as_str)?;
            let name = m.get("display_name").and_then(Value::as_str).unwrap_or(id);
            Some(ModelOption {
                id: id.to_owned(),
                name: name.to_owned(),
            })
        })
        .collect()
}

/// `agy models`: answers with the models when signed in. Failing with a word about signing in reads as signed out;
/// any other failure, unknown.
async fn antigravity_sign_in(bin: &Path) -> SignIn {
    match run(bin, &["models"]).await {
        Some(out) if out.ok => {
            let models = antigravity_models(&out.stdout);
            let signed_in = (!models.is_empty()).then_some(true);
            (signed_in, None, models)
        }
        Some(out) => {
            let said = out.all().to_lowercase();
            let signed_out = ["auth", "sign in", "log in", "login"]
                .iter()
                .any(|k| said.contains(k));
            (signed_out.then_some(false), None, Vec::new())
        }
        None => (None, None, Vec::new()),
    }
}

/// `agy models`' lines: a slug, a tab, its name.
fn antigravity_models(stdout: &str) -> Vec<ModelOption> {
    stdout
        .lines()
        .filter_map(|l| {
            let (id, name) = l.split_once('\t')?;
            let id = id.trim();
            (!id.is_empty() && !id.contains(' ')).then(|| ModelOption {
                id: id.to_owned(),
                name: name.trim().to_owned(),
            })
        })
        .collect()
}

/// One check's output.
struct Ran {
    ok: bool,
    stdout: String,
    stderr: String,
}

impl Ran {
    fn all(&self) -> String {
        format!("{}\n{}", self.stdout, self.stderr)
    }
}

/// `bin args…` run in an empty folder within [`CHECK_TIMEOUT`]; `None` when it couldn't start or ran out of time.
async fn run(bin: &Path, args: &[&str]) -> Option<Ran> {
    let cwd = std::env::temp_dir().join("resume-llm");
    std::fs::create_dir_all(&cwd).ok()?;
    let mut cmd = Command::new(bin);
    cmd.args(args)
        .current_dir(cwd)
        .env("PATH", path_with(bin))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let out = tokio::time::timeout(CHECK_TIMEOUT, cmd.output())
        .await
        .ok()?
        .ok()?;
    Some(Ran {
        ok: out.status.success(),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    })
}

/// `PATH` with the binary's folder first, so an nvm install's `node` shebang resolves (as `resume_llm` runs it).
fn path_with(bin: &Path) -> OsString {
    let mut dirs: Vec<PathBuf> = bin.parent().map(Path::to_path_buf).into_iter().collect();
    match std::env::var_os("PATH") {
        Some(path) => dirs.extend(std::env::split_paths(&path)),
        None => dirs.extend(["/usr/bin", "/bin"].map(PathBuf::from)),
    }
    std::env::join_paths(dirs).unwrap_or_default()
}

/// `pro` → `Pro`.
fn capitalized(s: &str) -> String {
    let mut cs = s.chars();
    cs.next()
        .map(|c| c.to_uppercase().chain(cs).collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn antigravity_models_skip_the_progress_line() {
        let out = "Fetching available models...\ngemini-3.8-flash-high\tGemini 3.8 Flash (High)\n\
                   claude-sonnet-5-5-low\tClaude Sonnet 5.5 (Low)\n";
        let models = antigravity_models(out);
        assert_eq!(
            models.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
            ["gemini-3.8-flash-high", "claude-sonnet-5-5-low"]
        );
        assert_eq!(models[0].name, "Gemini 3.8 Flash (High)");
    }

    #[test]
    fn codex_models_are_the_listed_ones() {
        let catalog = serde_json::json!({ "models": [
            { "slug": "gpt-reserve", "display_name": "GPT-Reserve", "visibility": "hide" },
            { "slug": "gpt-6-luna", "display_name": "GPT-6-Luna", "visibility": "list" },
        ]});
        assert_eq!(
            codex_models(&catalog),
            [ModelOption {
                id: "gpt-6-luna".to_owned(),
                name: "GPT-6-Luna".to_owned()
            }]
        );
    }
}

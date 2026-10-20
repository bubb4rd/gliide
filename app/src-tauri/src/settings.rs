//! Settings and the agent's status.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;
use tokio::process::Command;

use crate::dto::{AgentStatus, Settings};
use crate::error::{AppError, Result};
use crate::export;
use crate::state::App;

/// How long `claude --version` and `claude auth status` may take.
const CHECK_TIMEOUT: Duration = Duration::from_secs(15);

pub fn get_settings(app: &App) -> Settings {
    app.settings()
}

/// Saves `s` (trimmed; empty models fall back to the defaults' `sonnet`). A new export folder holding an existing
/// `applications.csv` has its rows imported (when it can be read: every write imports again first).
pub fn set_settings(app: &App, mut s: Settings) -> Result<Settings> {
    let trim = |v: &mut String, default: &str| {
        *v = v.trim().to_owned();
        if v.is_empty() {
            *v = default.to_owned();
        }
    };
    trim(
        &mut s.extract_model,
        resume_llm::extract::DEFAULT_EXTRACT_MODEL,
    );
    trim(
        &mut s.suggest_model,
        resume_llm::suggest::DEFAULT_SUGGEST_MODEL,
    );
    s.export_dir = s.export_dir.trim().to_owned();
    if !Path::new(&s.export_dir).is_absolute() {
        return Err(AppError::other("Choose a full folder path for exports"));
    }
    s.claude_path = s
        .claude_path
        .map(|p| p.trim().to_owned())
        .filter(|p| !p.is_empty());
    let moved = app.settings().export_dir != s.export_dir;
    app.put_settings(&s)?;
    if moved {
        let _ = export::import_applications_csv(app);
    }
    Ok(s)
}

/// `claude` found (`claude --version`) and signed in (`claude auth status`: only `loggedIn` and `subscriptionType`
/// are read; the email and tokens never are).
pub async fn agent_status(app: &Arc<App>) -> AgentStatus {
    let configured = app
        .settings()
        .claude_path
        .filter(|p| !p.trim().is_empty())
        .map(PathBuf::from);
    let path = match configured {
        Some(p) => Some(p).filter(|p| p.is_file()),
        None => resume_llm::find_claude().await,
    };
    let Some(path) = path else {
        return AgentStatus {
            found: false,
            path: None,
            version: None,
            signed_in: None,
            plan: None,
            problem: Some(
                "Claude Code isn't installed, or the app can't find it. Install it, or set its path in Settings."
                    .to_owned(),
            ),
        };
    };
    let version = run(&path, &["--version"]).await.and_then(|out| {
        let line = out.lines().next()?.trim();
        let v = line.trim_end_matches("(Claude Code)").trim();
        (!v.is_empty()).then(|| v.to_owned())
    });
    let auth = run(&path, &["auth", "status", "--json"])
        .await
        .and_then(|out| serde_json::from_str::<Value>(out.trim()).ok());
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
    let problem = match (version.is_some(), signed_in) {
        (false, _) => Some(format!(
            "{} didn't run. Reinstall Claude Code, or set its path in Settings.",
            path.display()
        )),
        (true, Some(false)) => {
            Some("Claude Code isn't signed in. Run `claude` in Terminal and sign in.".to_owned())
        }
        (true, None) => Some(
            "Couldn't check the Claude Code login. Run `claude` in Terminal to check it."
                .to_owned(),
        ),
        (true, Some(true)) => None,
    };
    AgentStatus {
        found: true,
        path: Some(path.display().to_string()),
        version,
        signed_in,
        plan,
        problem,
    }
}

/// `bin args…`'s stdout when it exits 0 within [`CHECK_TIMEOUT`].
async fn run(bin: &Path, args: &[&str]) -> Option<String> {
    let mut cmd = Command::new(bin);
    cmd.args(args)
        .env("PATH", path_with(bin))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    let out = tokio::time::timeout(CHECK_TIMEOUT, cmd.output())
        .await
        .ok()?
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
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

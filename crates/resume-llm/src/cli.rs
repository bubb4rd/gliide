//! What the agent CLI providers share: finding a binary, and running it once with a deadline.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Output, Stdio};
use std::time::{Duration, Instant};

use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use crate::{AgentKind, LlmError};

/// Makes `cwd`, the empty folder a call runs in: nothing there for the agent to pick up as context.
pub(crate) fn working_dir(agent: AgentKind, cwd: &Path) -> Result<(), LlmError> {
    std::fs::create_dir_all(cwd).map_err(|source| LlmError::Spawn {
        agent,
        bin: cwd.to_owned(),
        source,
    })
}

/// Runs `cmd` (its arguments and folder set) once: `input` on stdin (none: stdin closed), stdout and stderr captured,
/// the process killed once `timeout` passes. The output, and the wall time from spawn to exit in milliseconds.
pub(crate) async fn run(
    agent: AgentKind,
    bin: &Path,
    mut cmd: Command,
    input: Option<&[u8]>,
    timeout: Duration,
) -> Result<(Output, u64), LlmError> {
    cmd.env("PATH", path_with(bin))
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let started = Instant::now();
    let mut child = cmd.spawn().map_err(|source| {
        if source.kind() == std::io::ErrorKind::NotFound {
            LlmError::NotFound(agent)
        } else {
            LlmError::Spawn {
                agent,
                bin: bin.to_owned(),
                source,
            }
        }
    })?;
    let stdin = child.stdin.take();
    let run = async {
        let write = async move {
            if let (Some(mut stdin), Some(input)) = (stdin, input) {
                // A child that exits without reading closes the pipe; its output says why, so the error is moot.
                let _ = stdin.write_all(input).await;
            }
        };
        tokio::join!(write, child.wait_with_output()).1
    };
    // On timeout, dropping `run` drops the child, which kills it.
    let out = tokio::time::timeout(timeout, run)
        .await
        .map_err(|_| LlmError::Timeout {
            agent,
            after: timeout,
        })?
        .map_err(|source| LlmError::Spawn {
            agent,
            bin: bin.to_owned(),
            source,
        })?;
    Ok((out, started.elapsed().as_millis() as u64))
}

/// `PATH` with the binary's own folder first, so an nvm install's `node` shebang resolves.
fn path_with(bin: &Path) -> OsString {
    let mut dirs: Vec<PathBuf> = bin.parent().map(Path::to_path_buf).into_iter().collect();
    match std::env::var_os("PATH") {
        Some(path) => dirs.extend(std::env::split_paths(&path)),
        None => dirs.extend(["/usr/bin", "/bin"].map(PathBuf::from)),
    }
    std::env::join_paths(dirs).unwrap_or_default()
}

/// The binary `name`: the usual install locations (`~/.local/bin`, `~/.claude/local`, Homebrew, `/usr/local/bin`,
/// then nvm's Node versions, newest first), else whatever a login shell finds.
pub(crate) async fn find(name: &str) -> Option<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let mut candidates = Vec::new();
    if let Some(home) = &home {
        candidates.push(home.join(".local/bin").join(name));
        candidates.push(home.join(".claude/local").join(name));
    }
    candidates.push(Path::new("/opt/homebrew/bin").join(name));
    candidates.push(Path::new("/usr/local/bin").join(name));
    if let Some(home) = &home {
        candidates.extend(nvm_bins(&home.join(".nvm/versions/node"), name));
    }
    if let Some(found) = candidates.into_iter().find(|c| c.is_file()) {
        return Some(found);
    }

    let mut shell = Command::new("/bin/zsh");
    shell
        .args(["-lic", &format!("command -v {name}")])
        .stdin(Stdio::null())
        .kill_on_drop(true);
    let out = tokio::time::timeout(Duration::from_secs(10), shell.output())
        .await
        .ok()?
        .ok()?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    let last = stdout.trim_end().lines().last()?.trim();
    (last.starts_with('/') && last.ends_with(&format!("/{name}"))).then(|| PathBuf::from(last))
}

/// `<nvm>/<version>/bin/<name>` for every installed Node version, newest first.
fn nvm_bins(nvm: &Path, name: &str) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(nvm) else {
        return Vec::new();
    };
    let mut versions: Vec<(Vec<u64>, PathBuf)> = entries
        .filter_map(Result::ok)
        .map(|e| {
            let key = e
                .file_name()
                .to_string_lossy()
                .trim_start_matches('v')
                .split('.')
                .map(|p| p.parse().unwrap_or(0))
                .collect();
            (key, e.path().join("bin").join(name))
        })
        .collect();
    versions.sort_by(|a, b| b.0.cmp(&a.0));
    versions.into_iter().map(|(_, p)| p).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nvm_versions_newest_first() {
        let dir = tempfile::tempdir().unwrap();
        for v in ["v9.11.2", "v20.19.0", "v18.2.0"] {
            std::fs::create_dir_all(dir.path().join(v).join("bin")).unwrap();
        }
        let found: Vec<String> = nvm_bins(dir.path(), "claude")
            .iter()
            .map(|p| {
                p.parent()
                    .unwrap()
                    .parent()
                    .unwrap()
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        assert_eq!(found, ["v20.19.0", "v18.2.0", "v9.11.2"]);
    }
}

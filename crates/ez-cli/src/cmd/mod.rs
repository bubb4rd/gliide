//! One module per subcommand.

pub mod migrate;
pub mod render;
pub mod schema;
pub mod score;

use std::path::Path;

use anyhow::Context;
use serde::Serialize;
use serde::de::DeserializeOwned;

/// Reads and parses a JSON file.
pub fn read_json<T: DeserializeOwned>(path: &Path) -> anyhow::Result<T> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    serde_json::from_str(&text).with_context(|| format!("parsing {}", path.display()))
}

/// Writes `value` as pretty JSON with a trailing newline, creating parent folders.
pub fn write_json(path: &Path, value: &impl Serialize) -> anyhow::Result<()> {
    write(
        path,
        (serde_json::to_string_pretty(value)? + "\n").as_bytes(),
    )
}

/// Writes `bytes` to `path`, creating parent folders.
pub fn write(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    std::fs::write(path, bytes).with_context(|| format!("writing {}", path.display()))
}

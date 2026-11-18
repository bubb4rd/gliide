//! What's new (DESIGN.md, `WhatsNew`): the first launch on a new version shows that release's notes and picture once,
//! read from its GitHub release: the body (`RELEASE_NOTES`, RELEASING.md) and the picture attached as
//! `whats-new.webp` (or `.png`, `.jpg`). Nothing is kept: offline, it's asked again at the next launch. Got it
//! records the version (`whats_new_seen`); a fresh install records its own without showing anything, and a release
//! with no notes is recorded as seen. Versions skipped between two launches aren't shown; GitHub has them.

use std::sync::Arc;

use resume_ingest::{Fetcher, HttpFetcher};
use serde::Deserialize;

use crate::dto::{Picture, WhatsNew};
use crate::error::Result;
use crate::state::{App, blocking, lock};

/// The release of a tag, as GitHub's API serves it.
const RELEASES: &str = "https://api.github.com/repos/bubb4rd/gliide/releases/tags/v";
/// The store setting holding the last version whose notes were shown (or skipped).
const SEEN: &str = "whats_new_seen";
/// The picture a release may carry, by asset name, and its media type.
const PICTURES: [(&str, &str); 3] = [
    ("whats-new.webp", "image/webp"),
    ("whats-new.png", "image/png"),
    ("whats-new.jpg", "image/jpeg"),
];
/// A picture past this is left out (the card shows 320×168; the spec asks for 640×336).
const MAX_PICTURE: usize = 4 << 20;

#[derive(Deserialize)]
struct Release {
    body: Option<String>,
    published_at: Option<String>,
    html_url: String,
    #[serde(default)]
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
}

#[derive(Debug, PartialEq, Eq)]
enum Due {
    /// Show this version's notes.
    Show,
    /// A fresh install: record this version without showing anything.
    Record,
    No,
}

/// Whether `current`'s notes are due, given the last version seen and whether the first-run setup is done (before it
/// is, this is a fresh install; settings from before the setup existed load as done).
fn due(seen: Option<&str>, current: &str, onboarded: bool) -> Due {
    match seen {
        None if !onboarded => Due::Record,
        None => Due::Show,
        Some(seen) if parts(current) > parts(seen) => Due::Show,
        Some(_) => Due::No,
    }
}

/// A version's numbers, for comparing ("0.10.1" after "0.9.0").
fn parts(version: &str) -> Vec<u64> {
    version.split('.').map(|p| p.parse().unwrap_or(0)).collect()
}

/// The running version's notes and picture, when they're due and GitHub answers; `None` otherwise.
pub async fn whats_new(app: &Arc<App>) -> Result<Option<WhatsNew>> {
    let current = env!("CARGO_PKG_VERSION");
    let onboarded = app.settings().onboarded;
    let seen: Option<String> = blocking(app, |app| Ok(lock(&app.store).setting(SEEN)?)).await?;
    match due(seen.as_deref(), current, onboarded) {
        Due::No => return Ok(None),
        Due::Record => {
            record(app, current).await?;
            return Ok(None);
        }
        Due::Show => {}
    }
    let fetcher = HttpFetcher::new();
    let release = match fetcher.get(&format!("{RELEASES}{current}")).await {
        Ok(r) if r.status == 200 => serde_json::from_str::<Release>(&r.body).ok(),
        // Offline, or GitHub busy: ask again at the next launch.
        _ => return Ok(None),
    };
    let Some(release) = release else {
        return Ok(None);
    };
    let notes = release.body.as_deref().unwrap_or("").trim().to_owned();
    if notes.is_empty() {
        record(app, current).await?;
        return Ok(None);
    }
    let mut picture = None;
    for (name, media_type) in PICTURES {
        let Some(asset) = release.assets.iter().find(|a| a.name == name) else {
            continue;
        };
        if let Ok(Some(bytes)) = fetcher
            .get_file(&asset.browser_download_url, MAX_PICTURE)
            .await
        {
            picture = Some(Picture {
                bytes,
                media_type: media_type.to_owned(),
            });
        }
        break;
    }
    Ok(Some(WhatsNew {
        version: current.to_owned(),
        date: release.published_at,
        notes,
        url: release.html_url,
        picture,
    }))
}

/// Got it: `version`'s notes don't show again.
pub async fn dismiss(app: &Arc<App>, version: String) -> Result<()> {
    record(app, &version).await
}

async fn record(app: &Arc<App>, version: &str) -> Result<()> {
    let version = version.to_owned();
    blocking(app, move |app| {
        Ok(lock(&app.store).put_setting(SEEN, &version)?)
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shows_once_per_newer_version_and_never_on_a_fresh_install() {
        assert_eq!(due(None, "0.11.0", false), Due::Record);
        // Someone already using gliide when What's new arrived.
        assert_eq!(due(None, "0.11.0", true), Due::Show);
        assert_eq!(due(Some("0.10.1"), "0.11.0", true), Due::Show);
        assert_eq!(due(Some("0.9.0"), "0.10.0", true), Due::Show);
        assert_eq!(due(Some("0.11.0"), "0.11.0", true), Due::No);
        // Opened an older copy again.
        assert_eq!(due(Some("0.12.0"), "0.11.0", true), Due::No);
    }
}

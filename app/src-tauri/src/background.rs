//! Pulling the feed while gliide is closed (ARCHITECTURE.md §7, Job feed). launchd runs gliide's own executable as
//! `gliide --pull-feed` every 3 hours ([`headless`]: no Tauri, no window; one pull and its checks, then a line on
//! stderr), from the LaunchAgent the bundle carries (`launchd/dev.bubb4rd.gliide.feed.plist` here, copied to
//! `Contents/Library/LaunchAgents/` by tauri.conf.json's `bundle.macOS.files`). It's registered with SMAppService
//! (macOS 13+) while Settings → Feed's "Pull while gliide is closed" is on ([`set`]); macOS lists it in System
//! Settings → General → Login Items, where it can be held back ([`status`]).

use std::sync::Arc;

use crate::state::{App, STORE_FILE, data_dir};

/// The argument launchd starts gliide with (the agent's `ProgramArguments`): `main` runs [`headless`] instead of the app.
pub const FLAG: &str = "--pull-feed";

/// `gliide --pull-feed`: opens the data folder ([`data_dir`], `$GLIIDE_DATA_DIR` to run against a copy) and runs the
/// app's pull ([`crate::feed::pull_closed`]), saying what it did on stderr. The exit code: 0 when it pulled or had
/// nothing to do (the feed off, not due, another gliide pulling, no data yet), 1 when it couldn't open the data or the
/// pull failed.
pub fn headless() -> i32 {
    let say = |line: &str| eprintln!("gliide {FLAG}: {line}");
    let Some(dir) = data_dir() else {
        say("no data folder: HOME isn't set");
        return 1;
    };
    if !dir.join(STORE_FILE).is_file() {
        say(&format!("no data in {} yet; nothing pulled", dir.display()));
        return 0;
    }
    let app = match App::open(&dir, Box::new(|_| {})) {
        Ok(app) => Arc::new(app),
        Err(problem) => {
            say(&format!("couldn't open {}: {}", problem.path, problem.detail));
            return 1;
        }
    };
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(e) => {
            say(&format!("couldn't start: {e}"));
            return 1;
        }
    };
    match runtime.block_on(crate::feed::pull_closed(&app)) {
        Ok(line) => {
            say(&line);
            0
        }
        Err(e) => {
            say(&e.message);
            1
        }
    }
}

#[cfg(target_os = "macos")]
pub use mac::{open_login_items, set, status};

#[cfg(target_os = "macos")]
mod mac {
    use objc2::available;
    use objc2::rc::Retained;
    use objc2_foundation::NSString;
    use objc2_service_management::{SMAppService, SMAppServiceStatus};

    use crate::dto::BackgroundStatus;
    use crate::error::{AppError, Result};

    /// The agent's plist in the bundle's `Contents/Library/LaunchAgents`; its `Label` is the name without `.plist`.
    const PLIST: &str = "dev.bubb4rd.gliide.feed.plist";

    /// The agent's service; else why there can't be one here.
    fn service() -> std::result::Result<Retained<SMAppService>, BackgroundStatus> {
        if !available!(macos = 13.0) {
            return Err(BackgroundStatus::Unsupported);
        }
        if !crate::notify::bundled() {
            return Err(BackgroundStatus::Unbundled);
        }
        // SAFETY: SMAppService exists from macOS 13 (checked above); the name is a valid string.
        Ok(unsafe { SMAppService::agentServiceWithPlistName(&NSString::from_str(PLIST)) })
    }

    fn read(service: &SMAppService) -> BackgroundStatus {
        // SAFETY: a valid service object; `status` has no preconditions.
        match unsafe { service.status() } {
            SMAppServiceStatus::Enabled => BackgroundStatus::Enabled,
            SMAppServiceStatus::RequiresApproval => BackgroundStatus::RequiresApproval,
            SMAppServiceStatus::NotRegistered => BackgroundStatus::NotRegistered,
            _ => BackgroundStatus::NotFound,
        }
    }

    /// Where the agent stands, as macOS says.
    pub fn status() -> BackgroundStatus {
        service().map_or_else(|why| why, |s| read(&s))
    }

    /// Registers the agent (`on`) or unregisters it; nothing when it already stands so, when it's held back in Login
    /// Items (registering again can't change that), or where there can't be one. Errs with macOS's reason.
    pub fn set(on: bool) -> Result<()> {
        let Ok(service) = service() else {
            return Ok(());
        };
        // SAFETY (both calls): a valid service object, on any thread.
        let done = match (on, read(&service)) {
            (true, BackgroundStatus::NotRegistered) => unsafe { service.registerAndReturnError() },
            (false, BackgroundStatus::Enabled | BackgroundStatus::RequiresApproval) => unsafe {
                service.unregisterAndReturnError()
            },
            _ => return Ok(()),
        };
        done.map_err(|e| {
            AppError::other(format!(
                "macOS didn’t {} pulling while gliide is closed: {}",
                if on { "allow" } else { "stop" },
                e.localizedDescription()
            ))
        })
    }

    /// System Settings → General → Login Items, where macOS holds the agent back or lets it run.
    pub fn open_login_items() -> Result<()> {
        if !available!(macos = 13.0) {
            return Err(AppError::other("Login Items needs macOS 13 or later"));
        }
        // SAFETY: SMAppService exists from macOS 13 (checked above).
        unsafe { SMAppService::openSystemSettingsLoginItems() };
        Ok(())
    }
}

#[cfg(not(target_os = "macos"))]
pub fn status() -> crate::dto::BackgroundStatus {
    crate::dto::BackgroundStatus::Unsupported
}

#[cfg(not(target_os = "macos"))]
pub fn set(_on: bool) -> crate::error::Result<()> {
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub fn open_login_items() -> crate::error::Result<()> {
    Err(crate::error::AppError::other("Login Items are macOS only"))
}

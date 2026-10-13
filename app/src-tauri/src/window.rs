//! The main window: built from `tauri.conf.json` (where `create` is false), transparent, with the macOS sidebar
//! material behind the webview (DESIGN.md Layout). The page paints `--paper` over everything but the sidebar, whose
//! translucent `--sidebar` lets the material show through.

use tauri::{App, WebviewWindowBuilder};

/// Set before any page script runs when the native material sits behind the webview; `app.html` turns it into
/// `data-vibrancy` on the root so CSS lets the sidebar go translucent. Without it the sidebar stays opaque.
#[cfg(target_os = "macos")]
const VIBRANCY_FLAG: &str = "window.__EZ_VIBRANCY__ = true;";

/// Creates the main window and puts the sidebar material behind it.
pub fn setup(app: &mut App) -> Result<(), Box<dyn std::error::Error>> {
    let config = app
        .config()
        .app
        .windows
        .iter()
        .find(|w| w.label == "main")
        .ok_or("tauri.conf.json has no window labelled \"main\"")?
        .clone();
    let builder = WebviewWindowBuilder::from_config(app.handle(), &config)?;

    #[cfg(target_os = "macos")]
    {
        use window_vibrancy::{NSVisualEffectMaterial, NSVisualEffectState, apply_vibrancy};

        let window = builder.initialization_script(VIBRANCY_FLAG).build()?;
        // Like Finder's sidebar: the material dims when the window is inactive.
        let applied = apply_vibrancy(
            &window,
            NSVisualEffectMaterial::Sidebar,
            Some(NSVisualEffectState::FollowsWindowActiveState),
            None,
        );
        if let Err(e) = applied {
            // Keep the opaque `--sidebar` fallback rather than showing the desktop through the window.
            eprintln!("gliide: sidebar vibrancy unavailable ({e}); using the opaque sidebar");
            window.eval("delete window.__EZ_VIBRANCY__; document.documentElement.removeAttribute('data-vibrancy');")?;
        }
    }
    #[cfg(not(target_os = "macos"))]
    builder.build()?;

    Ok(())
}

// Prevents an additional console window on Windows in release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // launchd's run while gliide is closed: the feed pulled with no Tauri and no window.
    if std::env::args().skip(1).any(|a| a == gliide_lib::background::FLAG) {
        std::process::exit(gliide_lib::background::headless());
    }
    gliide_lib::run()
}

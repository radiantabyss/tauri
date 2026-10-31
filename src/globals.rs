// App-wide constants. Mirrors @radiantabyss/neutralino Support/Globals.js. Most of its globals
// have no Tauri counterpart to keep: APP_PATH and STATIC_PATH are Tauri's path APIs,
// DISPLAY_PROFILE was for window state (tauri-plugin-window-state now), and WINDOW_TYPE is a
// window's label.

use tauri::AppHandle;

pub const MAIN_WINDOW: &str = "main";

/// Whether this is a `tauri build` binary rather than `tauri dev`.
pub fn is_packaged() -> bool {
    !cfg!(debug_assertions)
}

/// Whether this is a "dev" build, i.e. built from tauri.conf.json (devtools: true) rather than
/// tauri.release.conf.json (devtools: false). Both are `tauri build` release-profile binaries,
/// so debug_assertions can't tell them apart; the main window's devtools flag can.
pub fn is_dev_build(app: &AppHandle) -> bool {
    app.config()
        .app
        .windows
        .iter()
        .find(|w| w.label == MAIN_WINDOW)
        .and_then(|w| w.devtools)
        .unwrap_or(false)
}

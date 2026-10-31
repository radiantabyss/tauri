// App storage. Mirrors @radiantabyss/neutralino Support/Storage.js, backed by
// tauri-plugin-store: one JSON file, settings.json, in the app data dir. The app registers
// the store plugin (bootstrap.rs); these calls need it.

use serde_json::Value;
use tauri::AppHandle;
use tauri_plugin_store::StoreExt;

const FILE: &str = "settings.json";

pub fn get(app: &AppHandle, key: &str) -> Option<Value> {
    app.store(FILE).ok()?.get(key)
}

/// Writes the file right away, so the value survives a crash.
pub fn set(app: &AppHandle, key: &str, value: Value) -> Result<(), String> {
    let store = app.store(FILE).map_err(|e| e.to_string())?;
    store.set(key, value);
    store.save().map_err(|e| e.to_string())
}

pub fn remove(app: &AppHandle, key: &str) -> Result<(), String> {
    let store = app.store(FILE).map_err(|e| e.to_string())?;
    store.delete(key);
    store.save().map_err(|e| e.to_string())
}

pub fn clear(app: &AppHandle) -> Result<(), String> {
    let store = app.store(FILE).map_err(|e| e.to_string())?;
    store.clear();
    store.save().map_err(|e| e.to_string())
}

// The right-click menu. Mirrors @radiantabyss/electron ContextMenu.js: the plugin gets a list of
// action names ('' for a separator), and each app action (like a file in app/ContextMenu)
// builds its own item for the window that was right-clicked: the label, whether it shows, and
// what a click runs.
//
//     pub fn item(win: &WebviewWindow) -> ContextMenuItem {
//         ContextMenuItem::new("Snap To Left")
//             .visible(is_panel_window(win))
//             .run(|app, win| { ... })
//     }
//
// Copy, Cut, Paste and SelectAll are built in and act on the webview's focus and selection, so
// they need no clipboard code. Inspect is built in too and only shows in dev builds.
// The frontend suppresses the webview's own menu and calls invoke('plugin:ra|show_context_menu').

use std::collections::HashMap;
use std::sync::Mutex;

use tauri::menu::{ContextMenu, Menu, MenuItem, PredefinedMenuItem};
use tauri::{AppHandle, Manager, State, WebviewWindow};

use crate::globals;

/// A context menu action builds its item for the right-clicked window.
pub type Action = fn(&WebviewWindow) -> ContextMenuItem;

type Run = Box<dyn Fn(&AppHandle, &WebviewWindow) + Send + Sync>;

pub(crate) const ID_PREFIX: &str = "ra-context:";

const BUILT_IN: &[&str] = &["Copy", "Cut", "Paste", "SelectAll", "Inspect"];

pub struct ContextMenuItem {
    label: String,
    visible: bool,
    enabled: bool,
    run: Option<Run>,
}

impl ContextMenuItem {
    pub fn new(label: impl Into<String>) -> ContextMenuItem {
        ContextMenuItem { label: label.into(), visible: true, enabled: true, run: None }
    }

    pub fn visible(mut self, visible: bool) -> Self {
        self.visible = visible;
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn run<F: Fn(&AppHandle, &WebviewWindow) + Send + Sync + 'static>(mut self, run: F) -> Self {
        self.run = Some(Box::new(run));
        self
    }
}

pub(crate) struct Registry {
    list: Vec<&'static str>,
    actions: HashMap<&'static str, Action>,
    // the label of the window the menu was opened on, for the click that follows
    window: Mutex<Option<String>>,
}

impl Registry {
    pub(crate) fn new(list: Vec<&'static str>, actions: HashMap<&'static str, Action>) -> Registry {
        Registry { list, actions, window: Mutex::new(None) }
    }
}

/// Every name in the list must be a separator, built in, or a registered action.
pub(crate) fn check(list: &[&'static str], actions: &HashMap<&'static str, Action>) -> Result<(), String> {
    for name in list {
        if !name.is_empty() && !BUILT_IN.contains(name) && !actions.contains_key(name) {
            return Err(format!("Context Menu Action \"{name}\" not found."));
        }
    }

    Ok(())
}

enum Entry {
    Separator,
    BuiltIn(&'static str),
    Item(&'static str, ContextMenuItem),
}

#[tauri::command]
pub(crate) fn show_context_menu(app: AppHandle, window: WebviewWindow, registry: State<'_, Registry>) -> tauri::Result<()> {
    let mut entries = Vec::new();

    for name in &registry.list {
        if name.is_empty() {
            entries.push(Entry::Separator);
        }
        else if *name == "Inspect" {
            if globals::is_dev_build(&app) {
                entries.push(Entry::BuiltIn(name));
            }
        }
        else if BUILT_IN.contains(name) {
            entries.push(Entry::BuiltIn(name));
        }
        else {
            let item = (registry.actions[name])(&window);
            if item.visible {
                entries.push(Entry::Item(name, item));
            }
        }
    }

    // hidden items can leave separators at the ends or next to each other
    let mut menu_entries: Vec<Entry> = Vec::new();
    for entry in entries {
        let after_separator = matches!(menu_entries.last(), None | Some(Entry::Separator));
        if matches!(entry, Entry::Separator) && after_separator {
            continue;
        }
        menu_entries.push(entry);
    }
    if matches!(menu_entries.last(), Some(Entry::Separator)) {
        menu_entries.pop();
    }

    if menu_entries.is_empty() {
        return Ok(());
    }

    let menu = Menu::new(&app)?;

    for entry in &menu_entries {
        match entry {
            Entry::Separator => menu.append(&PredefinedMenuItem::separator(&app)?)?,
            Entry::BuiltIn("Copy") => menu.append(&PredefinedMenuItem::copy(&app, Some("Copy"))?)?,
            Entry::BuiltIn("Cut") => menu.append(&PredefinedMenuItem::cut(&app, Some("Cut"))?)?,
            Entry::BuiltIn("Paste") => menu.append(&PredefinedMenuItem::paste(&app, Some("Paste"))?)?,
            Entry::BuiltIn("SelectAll") => menu.append(&PredefinedMenuItem::select_all(&app, Some("Select All"))?)?,
            Entry::BuiltIn(name) => menu.append(&MenuItem::with_id(&app, format!("{ID_PREFIX}{name}"), *name, true, None::<&str>)?)?,
            Entry::Item(name, item) => {
                menu.append(&MenuItem::with_id(&app, format!("{ID_PREFIX}{name}"), &item.label, item.enabled, None::<&str>)?)?
            }
        }
    }

    *registry.window.lock().unwrap() = Some(window.label().to_string());
    menu.popup(window.as_ref().window())?;

    Ok(())
}

/// A click on one of the menu's own items (the predefined ones handle themselves). The action
/// builds its item again for the window and runs it, on a blocking thread like the router's
/// actions.
pub(crate) fn run(app: &AppHandle, name: &str) {
    let registry = app.state::<Registry>();

    let label = match registry.window.lock().unwrap().clone() {
        Some(label) => label,
        None => return,
    };

    if name == "Inspect" {
        if let Some(webview) = app.get_webview_window(&label) {
            webview.open_devtools();
        }
        return;
    }

    let (action, window) = match (registry.actions.get(name), app.get_webview_window(&label)) {
        (Some(action), Some(window)) => (*action, window),
        _ => return,
    };

    let run = match action(&window).run {
        Some(run) => run,
        None => return,
    };

    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || run(&app, &window));
}

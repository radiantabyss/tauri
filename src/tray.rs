// The system tray. Mirrors @radiantabyss/neutralino Tray.js: a menu item's id is an action name
// plus its params, like a route ("SwitchVersion/php/8.3"). The actions are registered on the
// plugin (like app/Tray/*), every id is checked against them when the menu is set, and a click
// runs the action with the params in order. Call set() again to rebuild the menu.
//
//     ra::tray::set(app, Tray::new("main").on_click("Show").items([
//         TrayItem::new("SwitchVersion/php/8.3", "PHP 8.3").checked(true),
//         TrayItem::separator(),
//         TrayItem::new("Show", "Show"),
//         TrayItem::new("Exit", "Exit"),
//     ]))?;

use std::collections::HashMap;
use std::sync::Mutex;

use tauri::image::Image;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};

/// A tray action gets the params that follow its name in the item's id.
pub type Action = fn(&AppHandle, &[String]);

pub(crate) const ID_PREFIX: &str = "ra-tray:";

pub(crate) struct Registry {
    actions: HashMap<&'static str, Action>,
    // each tray's on_click item id, kept here so set() can change it on an existing icon
    clicks: Mutex<HashMap<String, String>>,
}

impl Registry {
    pub(crate) fn new(actions: HashMap<&'static str, Action>) -> Registry {
        Registry { actions, clicks: Mutex::new(HashMap::new()) }
    }

    fn check(&self, id: &str) -> Result<(), String> {
        if self.actions.contains_key(action_name(id)) {
            return Ok(());
        }

        Err(format!("Tray Action \"{id}\" not found."))
    }
}

pub struct Tray {
    id: String,
    tooltip: Option<String>,
    icon: Option<Image<'static>>,
    on_click: Option<String>,
    items: Vec<TrayItem>,
}

impl Tray {
    /// Most apps have one tray, "main"; more ids give more icons (one per panel, say).
    pub fn new(id: impl Into<String>) -> Tray {
        Tray { id: id.into(), tooltip: None, icon: None, on_click: None, items: Vec::new() }
    }

    /// Defaults to the app's name.
    pub fn tooltip(mut self, tooltip: impl Into<String>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    /// Defaults to the app's window icon.
    pub fn icon(mut self, icon: Image<'static>) -> Self {
        self.icon = Some(icon);
        self
    }

    /// The item id a left click on the icon runs, e.g. "Show".
    pub fn on_click(mut self, id: impl Into<String>) -> Self {
        self.on_click = Some(id.into());
        self
    }

    pub fn items(mut self, items: impl IntoIterator<Item = TrayItem>) -> Self {
        self.items = items.into_iter().collect();
        self
    }
}

pub struct TrayItem {
    id: String,
    text: String,
    checked: Option<bool>,
    enabled: bool,
    separator: bool,
}

impl TrayItem {
    pub fn new(id: impl Into<String>, text: impl Into<String>) -> TrayItem {
        TrayItem { id: id.into(), text: text.into(), checked: None, enabled: true, separator: false }
    }

    pub fn separator() -> TrayItem {
        TrayItem { id: String::new(), text: String::new(), checked: None, enabled: true, separator: true }
    }

    /// Shows a check mark box, ticked or not.
    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = Some(checked);
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

/// Creates the tray, or rebuilds the menu of one that is already there.
pub fn set(app: &AppHandle, tray: Tray) -> Result<(), String> {
    let registry = app.state::<Registry>();
    let menu = menu(app, &registry, &tray.items).map_err(|e| e.to_string())?;

    match &tray.on_click {
        Some(id) => {
            registry.check(id)?;
            registry.clicks.lock().unwrap().insert(tray.id.clone(), id.clone());
        }
        None => {
            registry.clicks.lock().unwrap().remove(&tray.id);
        }
    }

    let tooltip = tray.tooltip.unwrap_or_else(|| app.package_info().name.clone());

    if let Some(icon) = app.tray_by_id(&tray.id) {
        icon.set_menu(Some(menu)).map_err(|e| e.to_string())?;
        icon.set_tooltip(Some(tooltip)).map_err(|e| e.to_string())?;
        if let Some(image) = tray.icon {
            icon.set_icon(Some(image)).map_err(|e| e.to_string())?;
        }
        return Ok(());
    }

    let mut builder = TrayIconBuilder::with_id(&tray.id)
        .tooltip(tooltip)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|icon, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                click(icon.app_handle(), icon.id().as_ref());
            }
        });

    let image = tray.icon.or_else(|| app.default_window_icon().cloned());
    if let Some(image) = image {
        builder = builder.icon(image);
    }

    builder.build(app).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn remove(app: &AppHandle, id: &str) {
    app.remove_tray_by_id(id);
    app.state::<Registry>().clicks.lock().unwrap().remove(id);
}

fn menu(app: &AppHandle, registry: &Registry, items: &[TrayItem]) -> Result<Menu<tauri::Wry>, Box<dyn std::error::Error>> {
    let menu = Menu::new(app)?;

    for item in items {
        if item.separator {
            menu.append(&PredefinedMenuItem::separator(app)?)?;
            continue;
        }

        registry.check(&item.id)?;
        let id = format!("{ID_PREFIX}{}", item.id);

        match item.checked {
            Some(checked) => menu.append(&CheckMenuItem::with_id(app, id, &item.text, item.enabled, checked, None::<&str>)?)?,
            None => menu.append(&MenuItem::with_id(app, id, &item.text, item.enabled, None::<&str>)?)?,
        }
    }

    Ok(menu)
}

fn click(app: &AppHandle, tray_id: &str) {
    let id = app.state::<Registry>().clicks.lock().unwrap().get(tray_id).cloned();

    if let Some(id) = id {
        run(app, &id);
    }
}

fn action_name(id: &str) -> &str {
    id.split('/').next().unwrap_or("")
}

/// Runs the action an item id names, with the params after its name. On a blocking thread,
/// like the router's actions, so a slow one doesn't freeze the menu.
pub(crate) fn run(app: &AppHandle, id: &str) {
    let action = match app.state::<Registry>().actions.get(action_name(id)) {
        Some(action) => *action,
        None => return,
    };

    let params: Vec<String> = id.split('/').skip(1).map(String::from).collect();
    let app = app.clone();

    tauri::async_runtime::spawn_blocking(move || action(&app, &params));
}

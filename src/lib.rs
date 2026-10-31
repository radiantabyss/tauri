// @radiantabyss/tauri: the RA desktop stack for Tauri, like @radiantabyss/electron and
// @radiantabyss/neutralino are for theirs. It is the "ra" plugin: the app registers it in
// main.rs with its routes, tray actions and context menu, the frontend calls
// invoke('plugin:ra|dispatch', { method, path, payload }), and the app's capability lists
// "ra:default".

use std::collections::HashMap;

use tauri::menu::MenuEvent;
use tauri::plugin::{Builder as PluginBuilder, TauriPlugin};
use tauri::{AppHandle, Manager, Wry};

pub mod context_menu;
pub mod globals;
pub mod invoked;
pub mod router;
pub mod storage;
pub mod tray;

pub use context_menu::ContextMenuItem;
pub use invoked::Invoked;
pub use router::{Response, Route};
pub use tray::{Tray, TrayItem};

pub struct Builder {
    routes: Vec<Route>,
    tray_actions: HashMap<&'static str, tray::Action>,
    context_menu_actions: HashMap<&'static str, context_menu::Action>,
    context_menu: Vec<&'static str>,
}

impl Default for Builder {
    fn default() -> Self {
        Builder {
            routes: Vec::new(),
            tray_actions: HashMap::new(),
            context_menu_actions: HashMap::new(),
            context_menu: vec!["Copy", "Paste", "", "Inspect"],
        }
    }
}

impl Builder {
    pub fn new() -> Self {
        Self::default()
    }

    /// The app's routes (routes.rs), like app/Routes/*.
    pub fn routes(mut self, routes: Vec<Route>) -> Self {
        self.routes.extend(routes);
        self
    }

    /// A tray action, like a file in app/Tray: the first segment of a tray item's id.
    pub fn tray_action(mut self, name: &'static str, action: tray::Action) -> Self {
        self.tray_actions.insert(name, action);
        self
    }

    /// A context menu action, like a file in app/ContextMenu.
    pub fn context_menu_action(mut self, name: &'static str, action: context_menu::Action) -> Self {
        self.context_menu_actions.insert(name, action);
        self
    }

    /// The context menu, by action name, '' for a separator. Defaults to Copy, Paste and
    /// Inspect (dev builds only).
    pub fn context_menu(mut self, list: impl IntoIterator<Item = &'static str>) -> Self {
        self.context_menu = list.into_iter().collect();
        self
    }

    pub fn build(self) -> TauriPlugin<Wry> {
        let Builder { routes, tray_actions, context_menu_actions, context_menu } = self;

        PluginBuilder::new("ra")
            .invoke_handler(tauri::generate_handler![
                router::dispatch,
                context_menu::show_context_menu,
            ])
            .setup(move |app, _api| {
                context_menu::check(&context_menu, &context_menu_actions)?;

                app.manage(router::Routes::new(routes));
                app.manage(tray::Registry::new(tray_actions));
                app.manage(context_menu::Registry::new(context_menu, context_menu_actions));
                app.on_menu_event(on_menu_event);

                Ok(())
            })
            .build()
    }
}

// Every menu click in the app lands here, the tray's and the context menu's alike; the id's
// prefix says which one it came from.
fn on_menu_event(app: &AppHandle, event: MenuEvent) {
    let id = event.id().as_ref();

    if let Some(id) = id.strip_prefix(tray::ID_PREFIX) {
        tray::run(app, id);
    }
    else if let Some(name) = id.strip_prefix(context_menu::ID_PREFIX) {
        context_menu::run(app, name);
    }
}

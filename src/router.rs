// The IPC router. Mirrors @radiantabyss/neutralino Routing/Router.js: the frontend calls
// invoke('plugin:ra|dispatch', { method, path, payload }), the route is matched exact-first,
// then by {param}, and its action runs with the path params in order and the invoked payload.

use serde_json::Value;
use tauri::{AppHandle, State};

use crate::invoked::Invoked;

/// What every action returns: the data for the frontend, or the error message it shows.
pub type Response = Result<Value, String>;

/// Every action has this shape. `params` holds the path params in declaration order (empty
/// for paramless routes), like the positional args of an action's run(...params).
pub type Action = fn(&AppHandle, &[String], &Invoked) -> Response;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Method {
    Get,
    Post,
}

pub struct Route {
    pub method: Method,
    pub path: String,
    pub action: Action,
}

impl Route {
    pub fn get(path: impl Into<String>, action: Action) -> Route {
        Route { method: Method::Get, path: path.into(), action }
    }

    pub fn post(path: impl Into<String>, action: Action) -> Route {
        Route { method: Method::Post, path: path.into(), action }
    }
}

/// The CRUD routes of a domain, like RouteCrud.run('Peer'), for the actions it has:
/// `route_crud!(peer, [list, single, create])` gives /peer, /peer/single/{id} and /peer/create,
/// pointing at crate::domains::peer::actions::{list, single, create}::run. A domain named
/// two_factor routes under /two-factor.
// `crate::domains` is meant to resolve in the app that calls the macro, not in this crate.
#[allow(clippy::crate_in_macro_def)]
#[macro_export]
macro_rules! route_crud {
    (@route $domain:ident, list) => {
        $crate::Route::get($crate::router::crud_path(stringify!($domain), ""), crate::domains::$domain::actions::list::run)
    };
    (@route $domain:ident, single) => {
        $crate::Route::get($crate::router::crud_path(stringify!($domain), "/single/{id}"), crate::domains::$domain::actions::single::run)
    };
    (@route $domain:ident, create) => {
        $crate::Route::post($crate::router::crud_path(stringify!($domain), "/create"), crate::domains::$domain::actions::create::run)
    };
    (@route $domain:ident, edit) => {
        $crate::Route::get($crate::router::crud_path(stringify!($domain), "/edit/{id}"), crate::domains::$domain::actions::edit::run)
    };
    (@route $domain:ident, update) => {
        $crate::Route::post($crate::router::crud_path(stringify!($domain), "/update/{id}"), crate::domains::$domain::actions::update::run)
    };
    (@route $domain:ident, patch) => {
        $crate::Route::post($crate::router::crud_path(stringify!($domain), "/patch/{id}"), crate::domains::$domain::actions::patch::run)
    };
    (@route $domain:ident, delete) => {
        $crate::Route::get($crate::router::crud_path(stringify!($domain), "/delete/{id}"), crate::domains::$domain::actions::delete::run)
    };
    (@route $domain:ident, search) => {
        $crate::Route::get($crate::router::crud_path(stringify!($domain), "/search"), crate::domains::$domain::actions::search::run)
    };
    (@route $domain:ident, $other:ident) => {
        compile_error!(concat!(
            "route_crud!: unknown action `", stringify!($other),
            "`, expected list, single, create, edit, update, patch, delete or search"
        ))
    };
    ($domain:ident, [$($action:ident),* $(,)?]) => {
        vec![$($crate::route_crud!(@route $domain, $action)),*]
    };
}

#[doc(hidden)]
pub fn crud_path(domain: &str, suffix: &str) -> String {
    format!("/{}{}", domain.replace('_', "-"), suffix)
}

pub(crate) struct Routes(Vec<Route>);

impl Routes {
    pub(crate) fn new(routes: Vec<Route>) -> Routes {
        Routes(routes)
    }

    /// Exact matches win; then the {param} routes are tried segment by segment.
    fn find(&self, method: Method, path: &str) -> Option<(Action, Vec<String>)> {
        for route in &self.0 {
            if route.method == method && route.path == path {
                return Some((route.action, Vec::new()));
            }
        }

        for route in &self.0 {
            if route.method != method || !route.path.contains('{') {
                continue;
            }

            if let Some(params) = match_params(&route.path, path) {
                return Some((route.action, params));
            }
        }

        None
    }
}

fn match_params(pattern: &str, path: &str) -> Option<Vec<String>> {
    let pattern_segments: Vec<&str> = pattern.trim_matches('/').split('/').collect();
    let path_segments: Vec<&str> = path.trim_matches('/').split('/').collect();

    if pattern_segments.len() != path_segments.len() {
        return None;
    }

    let mut params = Vec::new();

    for (pattern_segment, path_segment) in pattern_segments.iter().zip(path_segments.iter()) {
        if pattern_segment.starts_with('{') && pattern_segment.ends_with('}') {
            params.push((*path_segment).to_string());
        }
        else if pattern_segment != path_segment {
            return None;
        }
    }

    Some(params)
}

#[tauri::command]
pub(crate) async fn dispatch(
    app: AppHandle,
    routes: State<'_, Routes>,
    method: String,
    path: String,
    payload: String,
) -> Response {
    let method = match method.to_uppercase().as_str() {
        "POST" => Method::Post,
        _ => Method::Get,
    };

    let (action, params) = match routes.find(method, &path) {
        Some(matched) => matched,
        None => return Err(format!("Route \"{}\" not found.", path)),
    };

    let invoked = Invoked::new(serde_json::from_str(&payload).unwrap_or(Value::Null));

    // Actions are synchronous and may block (native dialogs, the network), so they run on a
    // blocking thread, not on the async worker.
    tauri::async_runtime::spawn_blocking(move || action(&app, &params, &invoked))
        .await
        .map_err(|e| e.to_string())?
}

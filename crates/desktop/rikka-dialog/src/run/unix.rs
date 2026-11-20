use std::collections::HashMap;
use std::path::{Path, PathBuf};

use rikka_portal_request::{Answer, Results};
use zbus::blocking::Connection;
use zbus::zvariant::Value;

use crate::chooser::{Chooser, ChooserResponse, Purpose};
use crate::error::{Error, Result};
use crate::filter::Filter;
use crate::portal::FileChooserProxyBlocking;

const GLOB: u32 = 0;
const MIME_TYPE: u32 = 1;

pub(crate) fn run(chooser: &Chooser) -> Result<ChooserResponse> {
    let connection = Connection::session().map_err(portal_unavailable)?;
    let portal = FileChooserProxyBlocking::new(&connection).map_err(portal_unavailable)?;
    let parent = chooser
        .parent
        .as_ref()
        .map(ToString::to_string)
        .unwrap_or_default();
    let answer = rikka_portal_request::ask(&connection, |token| {
        let options = options(chooser, token);
        match chooser.purpose {
            Purpose::Open => portal.open_file(&parent, &chooser.title, options),
            Purpose::Save => portal.save_file(&parent, &chooser.title, options),
        }
    })
    .map_err(|error| match error {
        rikka_portal_request::Error::Unreachable(reason) => Error::Unavailable(reason),
        rikka_portal_request::Error::Malformed(reason) => Error::Malformed(reason),
        rikka_portal_request::Error::Silent => Error::NoResponse,
    })?;
    match answer {
        Answer::Granted(results) => Ok(ChooserResponse::Files(selected_paths(&results))),
        Answer::Cancelled => Ok(ChooserResponse::Cancelled),
        Answer::Ended => Err(Error::NoResponse),
    }
}

fn options<'a>(chooser: &'a Chooser, token: &str) -> HashMap<&'a str, Value<'a>> {
    let mut options = HashMap::from([
        ("handle_token", Value::from(token.to_owned())),
        ("modal", Value::from(true)),
    ]);
    if chooser.purpose == Purpose::Open {
        options.insert("multiple", Value::from(chooser.multiple));
        options.insert("directory", Value::from(chooser.directory));
    }
    if let Some(name) = &chooser.name {
        options.insert("current_name", Value::from(name.clone()));
    }
    if !chooser.accept_label.is_empty() {
        options.insert("accept_label", Value::from(chooser.accept_label.clone()));
    }
    if let Some(folder) = &chooser.folder {
        options.insert("current_folder", Value::from(nul_terminated(folder)));
    }
    if !chooser.filters.is_empty() && !chooser.directory {
        let filters: Vec<(String, Vec<(u32, String)>)> = chooser.filters.iter().map(wire).collect();
        options.insert("filters", Value::from(filters));
        options.insert("current_filter", Value::from(wire(&chooser.filters[0])));
    }
    options
}

fn wire(filter: &Filter) -> (String, Vec<(u32, String)>) {
    let globs = filter
        .patterns
        .iter()
        .map(|pattern| (GLOB, pattern.clone()));
    let rules = filter
        .mime_types
        .iter()
        .map(|mime_type| (MIME_TYPE, mime_type.clone()))
        .chain(globs)
        .collect();
    (filter.name.clone(), rules)
}

fn nul_terminated(path: &Path) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;

    let mut bytes = path.as_os_str().as_bytes().to_vec();
    bytes.push(0);
    bytes
}

fn selected_paths(results: &Results) -> Vec<PathBuf> {
    let Some(uris) = results.get("uris") else {
        return Vec::new();
    };
    let Ok(uris) = <Vec<String>>::try_from(uris.clone()) else {
        tracing::warn!("file chooser response has no uris array");
        return Vec::new();
    };
    uris.iter()
        .map(String::as_str)
        .filter_map(rikka_open::uri_to_path)
        .collect()
}

fn portal_unavailable(error: zbus::Error) -> Error {
    Error::Unavailable(error.to_string())
}

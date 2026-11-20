use std::ffi::OsString;

use rikka_settings::Schema;
use rikka_settings::seam::Settings;
use rikka_settings::session::network::{
    self, FTP, HTTP, HTTPS, IGNORED, MANUAL, MODE, Proxy, SAME_FOR_ALL, SAME_FOR_ALL_DEFAULT, SOCKS,
};

pub(crate) use rikka_settings::session::network::DOMAIN;

struct Kind {
    proxy: Proxy,
    scheme: &'static str,
    variables: [&'static str; 2],
}

const KINDS: [Kind; 4] = [
    Kind {
        proxy: HTTP,
        scheme: "http",
        variables: ["http_proxy", "HTTP_PROXY"],
    },
    Kind {
        proxy: HTTPS,
        scheme: "http",
        variables: ["https_proxy", "HTTPS_PROXY"],
    },
    Kind {
        proxy: FTP,
        scheme: "http",
        variables: ["ftp_proxy", "FTP_PROXY"],
    },
    Kind {
        proxy: SOCKS,
        scheme: "socks5",
        variables: ["all_proxy", "ALL_PROXY"],
    },
];

const IGNORED_VARIABLES: [&str; 2] = ["no_proxy", "NO_PROXY"];

fn variables() -> impl Iterator<Item = &'static str> {
    KINDS
        .iter()
        .flat_map(|kind| kind.variables)
        .chain(IGNORED_VARIABLES)
}

pub(crate) fn schema() -> Schema {
    network::schema()
}

pub(crate) fn environment(settings: &Settings) -> Vec<(&'static str, Option<OsString>)> {
    let mut environment: Vec<(&'static str, Option<OsString>)> =
        variables().map(|variable| (variable, None)).collect();
    if settings.read::<String>(MODE).as_deref() != Some(MANUAL) {
        return environment;
    }

    let shared = settings.read_or(SAME_FOR_ALL, SAME_FOR_ALL_DEFAULT);
    let http = address(settings, &KINDS[0]);
    for kind in &KINDS {
        let address = match shared && kind.proxy != SOCKS {
            true => http.clone(),
            false => address(settings, kind),
        };
        let Some(address) = address else {
            continue;
        };
        for variable in kind.variables {
            set(&mut environment, variable, &address);
        }
    }

    let ignored = settings.read::<String>(IGNORED).unwrap_or_default();
    if !ignored.is_empty() {
        for variable in IGNORED_VARIABLES {
            set(&mut environment, variable, &ignored);
        }
    }
    environment
}

fn set(environment: &mut [(&'static str, Option<OsString>)], variable: &str, value: &str) {
    for entry in environment.iter_mut() {
        if entry.0 == variable {
            entry.1 = Some(value.into());
        }
    }
}

fn address(settings: &Settings, kind: &Kind) -> Option<String> {
    let host = settings.read::<String>(kind.proxy.host)?;
    let port = settings.read::<i64>(kind.proxy.port)?;
    if host.is_empty() || port == 0 {
        return None;
    }
    Some(format!("{}://{host}:{port}", kind.scheme))
}

use std::collections::HashMap;
use std::ffi::OsString;
use std::io::Write as _;
use std::os::unix::ffi::OsStrExt as _;
use std::path::{Path, PathBuf};

use rikka_session::environment::BUS_ADDRESS_ENV;

use super::bus::PrivateBus;
use super::kind::SessionKind;
use crate::context::SessionRef;
use crate::state::Core;

const ENVIRONMENT_FILE: &str = "session.env";

const LOGIN_ONLY_VARIABLE: &str = "PATH";

const OUTLIVING_UNITS: &[&str] = &[
    "xdg-desktop-portal.service",
    "rikka-portal-cast.service",
    "rikka-portal-files.service",
];

const SESSION_UNIT: &str = "rikka-session.target";

const SHUTDOWN_UNIT: &str = "rikka-session-shutdown.target";

fn start_graphical_session_target(manager: &zbus::blocking::Proxy<'_>) {
    match manager.call_method("StartUnit", &(SESSION_UNIT, "replace")) {
        Ok(_) => tracing::info!("started {SESSION_UNIT}"),
        Err(error) => {
            return tracing::warn!("cannot start {SESSION_UNIT}; portals will not run: {error}");
        }
    }
    for unit in OUTLIVING_UNITS {
        match manager.call_method("TryRestartUnit", &(*unit, "replace")) {
            Ok(_) => tracing::debug!("restarted {unit}"),
            Err(error) => tracing::debug!("cannot restart {unit}: {error}"),
        }
    }
}

fn stop_graphical_session_target(manager: &zbus::blocking::Proxy<'_>) {
    match manager.call_method("StartUnit", &(SHUTDOWN_UNIT, "replace-irreversibly")) {
        Ok(_) => tracing::info!("stopping graphical session target"),
        Err(error) => tracing::debug!("cannot start {SHUTDOWN_UNIT}: {error}"),
    }
}

fn manager(connection: &zbus::blocking::Connection) -> zbus::Result<zbus::blocking::Proxy<'_>> {
    const SERVICE: &str = "org.freedesktop.systemd1";

    zbus::blocking::proxy::Builder::new(connection)
        .destination(SERVICE)?
        .path("/org/freedesktop/systemd1")?
        .interface("org.freedesktop.systemd1.Manager")?
        .cache_properties(zbus::proxy::CacheProperties::No)
        .build()
}

fn update_activation_environment(
    connection: &zbus::blocking::Connection,
    environment: &HashMap<String, String>,
) {
    match connection.call_method(
        Some("org.freedesktop.DBus"),
        "/org/freedesktop/DBus",
        Some("org.freedesktop.DBus"),
        "UpdateActivationEnvironment",
        &(environment,),
    ) {
        Ok(_) => tracing::info!(
            "updated D-Bus activation environment ({} variables)",
            environment.len()
        ),
        Err(error) => tracing::warn!("cannot update D-Bus activation environment: {error}"),
    }
}

fn set_systemd_environment(
    manager: &zbus::blocking::Proxy<'_>,
    environment: &HashMap<String, String>,
) {
    let assignments: Vec<String> = environment
        .iter()
        .map(|(name, value)| format!("{name}={value}"))
        .collect();
    match manager.call_method("SetEnvironment", &(assignments,)) {
        Ok(_) => tracing::info!("updated systemd user environment"),
        Err(error) => tracing::debug!("no user systemd manager: {error}"),
    }
}

fn systemd_forgets(manager: &zbus::blocking::Proxy<'_>, names: &[&'static str]) {
    match manager.call_method("UnsetEnvironment", &(names,)) {
        Ok(_) => tracing::info!("unset session variables in systemd user environment"),
        Err(error) => tracing::debug!("no user systemd manager: {error}"),
    }
}

fn write_environment(
    path: &Path,
    environment: &[(&'static str, Option<OsString>)],
) -> std::io::Result<()> {
    let folder = path.parent().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "path has no parent directory",
        )
    })?;
    std::fs::create_dir_all(folder)?;
    set_private(folder)?;

    let mut file = rikka_fs::Replace::new(path).stage()?;
    for (name, value) in environment {
        let Some(value) = value else { continue };
        if value.as_bytes().contains(&b'\n') {
            tracing::warn!("skipping {name}: value contains a newline");
            continue;
        }
        file.write_all(name.as_bytes())?;
        file.write_all(b"=")?;
        file.write_all(value.as_bytes())?;
        file.write_all(b"\n")?;
    }
    file.commit()
}

fn set_private(folder: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt as _;

    std::fs::set_permissions(folder, std::fs::Permissions::from_mode(0o700))
}

impl SessionRef<'_> {
    fn write_session_environment(&self) {
        let Some(path) = self
            .core
            .session_directory()
            .map(|at| at.join(ENVIRONMENT_FILE))
        else {
            tracing::debug!("no runtime directory; session environment not written");
            return;
        };
        match write_environment(&path, &self.session_environment()) {
            Ok(()) => tracing::info!("session environment written to {}", path.display()),
            Err(error) => tracing::warn!(
                "cannot write session environment to {}: {error}",
                path.display()
            ),
        }
    }

    pub(crate) fn publish_session_environment(&self) {
        self.write_session_environment();

        match &self.session.standing {
            SessionKind::Login => self.publish_to_login_session(),
            SessionKind::Nested(bus) | SessionKind::Headless(Some(bus)) => {
                self.publish_to_private_bus(bus);
            }
            SessionKind::Headless(None) => {
                tracing::info!("headless session; environment not published");
            }
        }
    }

    fn publish_to_login_session(&self) {
        let environment = self.published_environment(LOGIN_ONLY_VARIABLE);

        let Ok(connection) = zbus::blocking::Connection::session() else {
            tracing::debug!("no session bus; environment not published");
            return;
        };
        update_activation_environment(&connection, &environment);

        let Ok(manager) = manager(&connection) else {
            return tracing::debug!(
                "no user systemd manager; graphical session target not started"
            );
        };
        set_systemd_environment(&manager, &environment);
        start_graphical_session_target(&manager);
    }

    fn publish_to_private_bus(&self, bus: &PrivateBus) {
        let connected = zbus::blocking::connection::Builder::address(bus.address())
            .and_then(zbus::blocking::connection::Builder::build);
        match connected {
            Ok(connection) => {
                update_activation_environment(
                    &connection,
                    &self.published_environment(BUS_ADDRESS_ENV),
                );
            }
            Err(error) => tracing::warn!("cannot connect to private session bus: {error}"),
        }
    }

    pub(crate) fn retire_session_environment(&self) {
        self.core.forget_session_environment();

        if !self.session.standing.is_login() {
            return;
        }
        let Ok(connection) = zbus::blocking::Connection::session() else {
            tracing::debug!("no session bus; nothing to retract");
            return;
        };
        let Ok(manager) = manager(&connection) else {
            return;
        };
        stop_graphical_session_target(&manager);
        systemd_forgets(&manager, &self.published_names());
    }

    fn published_environment(&self, excluded: &str) -> HashMap<String, String> {
        self.session_environment()
            .into_iter()
            .filter(|(name, _)| *name != excluded)
            .filter_map(|(name, value)| {
                let value = value?.into_string().ok()?;
                Some((name.to_owned(), value))
            })
            .collect()
    }

    fn published_names(&self) -> Vec<&'static str> {
        self.session_environment()
            .into_iter()
            .map(|(name, _)| name)
            .filter(|name| *name != LOGIN_ONLY_VARIABLE)
            .collect()
    }
}

impl Core {
    fn forget_session_environment(&self) {
        let Some(path) = self.session_directory().map(|at| at.join(ENVIRONMENT_FILE)) else {
            return;
        };
        if let Err(error) = std::fs::remove_file(&path)
            && error.kind() != std::io::ErrorKind::NotFound
        {
            tracing::warn!(
                "cannot remove stale session environment {}: {error}",
                path.display()
            );
        }
    }

    fn session_directory(&self) -> Option<PathBuf> {
        super::directory(self.socket_name())
    }
}

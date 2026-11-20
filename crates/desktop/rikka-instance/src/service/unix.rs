use std::collections::HashMap;
use std::ffi::OsString;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::PathBuf;

use zbus::blocking::Connection;
use zbus::fdo::{RequestNameFlags, RequestNameReply};
use zbus::zvariant::{OwnedValue, Value};

use super::Startup;
use crate::error::{Error, Result};
use crate::launch_request::{LaunchRequest, Sink};
use crate::name::Name;

const INTERFACE: &str = "org.freedesktop.Application";

const TOKEN: &str = "activation-token";
const STARTUP_ID: &str = "desktop-startup-id";

const CWD: &str = "cwd";

const COMMAND_LINE: &str = "command-line";

#[derive(Debug)]
pub struct Service {
    _connection: Connection,
    name: Name,
}

impl Service {
    #[must_use]
    pub fn name(&self) -> &Name {
        &self.name
    }
}

#[derive(Debug)]
pub struct Peer {
    connection: Connection,
    name: Name,
}

impl Peer {
    pub fn forward(&self, request: &LaunchRequest) -> Result<()> {
        let proxy = zbus::blocking::Proxy::new(
            &self.connection,
            self.name.bus(),
            self.name.path(),
            INTERFACE,
        )
        .map_err(call_error)?;
        let platform = platform_data(request);
        match (request.action(), request.arguments(), request.uris()) {
            (Some(action), _, _) => proxy.call::<_, _, ()>(
                "ActivateAction",
                &(action, Vec::<Value<'_>>::new(), platform),
            ),
            (None, [], []) => proxy.call::<_, _, ()>("Activate", &(platform,)),
            (None, [], uris) => proxy.call::<_, _, ()>("Open", &(uris, platform)),
            (None, arguments, _) => proxy.call::<_, _, ()>(
                "ActivateAction",
                &(COMMAND_LINE, vec![command_line(arguments)], platform),
            ),
        }
        .map_err(call_error)
    }
}

fn call_error(error: zbus::Error) -> Error {
    Error::Call(error.to_string())
}

#[must_use]
pub fn claim(id: &str, sink: Sink) -> Startup {
    let Some(name) = Name::from_app_id(id) else {
        tracing::debug!(id, "application id is not a valid bus name");
        return Startup::Standalone;
    };

    let connection = match Connection::session() {
        Ok(connection) => connection,
        Err(error) => {
            tracing::debug!("no session bus to claim `{}` on: {error}", name.bus());
            return Startup::Standalone;
        }
    };

    let claimed = connection
        .request_name_with_flags(name.bus(), RequestNameFlags::DoNotQueue.into())
        .map(|reply| reply == RequestNameReply::PrimaryOwner)
        .unwrap_or(false);
    if !claimed {
        return Startup::Running(Peer { connection, name });
    }

    let served = connection
        .object_server()
        .at(name.path(), Application { sink });
    match served {
        Ok(true) => Startup::Primary(Service {
            _connection: connection,
            name,
        }),
        outcome => {
            tracing::warn!("`{}` is owned but not served: {outcome:?}", name.bus());
            Startup::Standalone
        }
    }
}

struct Application {
    sink: Sink,
}

#[zbus::interface(name = "org.freedesktop.Application")]
impl Application {
    fn activate(&self, platform_data: HashMap<String, OwnedValue>) {
        (self.sink)(apply_platform_data(
            LaunchRequest::default(),
            &platform_data,
        ));
    }

    fn open(&self, uris: Vec<String>, platform_data: HashMap<String, OwnedValue>) {
        (self.sink)(apply_platform_data(
            LaunchRequest::from_uris(uris),
            &platform_data,
        ));
    }

    fn activate_action(
        &self,
        action_name: String,
        parameter: Vec<OwnedValue>,
        platform_data: HashMap<String, OwnedValue>,
    ) {
        let request = match action_name.as_str() {
            COMMAND_LINE => LaunchRequest::from_command_line(
                command_line_arguments(&parameter),
                cwd(&platform_data),
            ),
            _ => LaunchRequest::from_action(action_name),
        };
        (self.sink)(apply_platform_data(request, &platform_data));
    }
}

fn command_line(arguments: &[OsString]) -> Value<'static> {
    let bytes: Vec<Vec<u8>> = arguments
        .iter()
        .map(|argument| argument.as_bytes().to_vec())
        .collect();
    Value::from(bytes)
}

fn command_line_arguments(parameter: &[OwnedValue]) -> Vec<OsString> {
    let Some(first) = parameter.first().and_then(|value| value.try_clone().ok()) else {
        return Vec::new();
    };
    Vec::<Vec<u8>>::try_from(first)
        .map(|arguments| arguments.into_iter().map(OsString::from_vec).collect())
        .unwrap_or_default()
}

fn platform_data(request: &LaunchRequest) -> HashMap<&'static str, Value<'_>> {
    let mut platform = HashMap::new();
    if let Some(token) = request.token() {
        platform.insert(TOKEN, Value::from(token));
        platform.insert(STARTUP_ID, Value::from(token));
    }
    if let Some(cwd) = request.cwd() {
        platform.insert(CWD, Value::from(cwd.as_os_str().as_bytes().to_vec()));
    }
    platform
}

fn apply_platform_data(
    request: LaunchRequest,
    platform: &HashMap<String, OwnedValue>,
) -> LaunchRequest {
    let request = match token(platform) {
        Some(token) => request.with_token(token),
        None => request,
    };
    match cwd(platform) {
        Some(cwd) => request.with_cwd(cwd),
        None => request,
    }
}

fn token(platform: &HashMap<String, OwnedValue>) -> Option<String> {
    let value = platform.get(TOKEN).or_else(|| platform.get(STARTUP_ID))?;
    String::try_from(value.try_clone().ok()?)
        .ok()
        .filter(|token| !token.is_empty())
}

fn cwd(platform: &HashMap<String, OwnedValue>) -> Option<PathBuf> {
    let bytes = Vec::<u8>::try_from(platform.get(CWD)?.try_clone().ok()?).ok()?;
    let path = PathBuf::from(OsString::from_vec(bytes));
    path.is_absolute().then_some(path)
}

impl std::fmt::Debug for Application {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Application").finish_non_exhaustive()
    }
}

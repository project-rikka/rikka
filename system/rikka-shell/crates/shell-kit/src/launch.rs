use std::path::PathBuf;
use std::process::Command;

use rikka_seam::{Call, Manifest, Method, MethodInfo, Request, SeamError, Service, Version};
use rikka_session::settings_app::Link;

use crate::Host;

pub const CAPABILITY: &str = "rikka.launch";

pub const SPAWN: &str = "spawn";

pub const UPDATES: &str = "rikka-updates";

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Spawn {
    pub argv: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub env: Vec<(String, String)>,
}

impl Spawn {
    #[must_use]
    pub fn new<S: Into<String>>(argv: impl IntoIterator<Item = S>) -> Spawn {
        Spawn {
            argv: argv.into_iter().map(Into::into).collect(),
            ..Spawn::default()
        }
    }

    #[must_use]
    pub fn settings(link: impl Into<Link>) -> Spawn {
        Spawn::new(link.into().argv())
    }

    #[must_use]
    pub fn request(&self) -> Request {
        Request::new(CAPABILITY, SPAWN).encode(self)
    }
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
struct Spawned {
    pid: u32,
}

#[derive(Debug, Default)]
pub struct LaunchService;

impl LaunchService {
    #[must_use]
    pub fn new() -> LaunchService {
        LaunchService
    }

    fn spawn(call: Call) {
        let request = match call.decode::<Spawn>() {
            Ok(request) => request,
            Err(error) => return call.fail(error),
        };
        let Some((program, arguments)) = request.argv.split_first() else {
            return call.fail(SeamError::InvalidParams(
                "no program to start: `argv` is empty".to_owned(),
            ));
        };

        let mut command = Command::new(program);
        command.args(arguments);
        if let Some(cwd) = &request.cwd {
            command.current_dir(cwd);
        }
        for (name, value) in &request.env {
            command.env(name, value);
        }

        match rikka_open::spawn_detached(&mut command) {
            Ok(pid) => {
                tracing::info!("started {program} as pid {pid}");
                call.encode(&Spawned { pid });
            }
            Err(error) => call.fail(SeamError::Failed(error.to_string())),
        }
    }
}

impl Service<Host> for LaunchService {
    fn manifest(&self) -> Manifest {
        Manifest::new(CAPABILITY, Version::new(1, 0))
            .summary("Launches programs in the session")
            .method(
                MethodInfo::new(SPAWN)
                    .summary("Spawns `argv` detached from the shell and returns its pid"),
            )
    }

    fn call(&mut self, _: &mut Host, method: Method, call: Call) {
        match &*method {
            SPAWN => LaunchService::spawn(call),
            _ => call.unknown_method(),
        }
    }
}

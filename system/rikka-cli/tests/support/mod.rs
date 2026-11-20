use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::{Command, ExitStatus, Stdio};
use std::sync::{Arc, Mutex};

use rikka_seam::net::{SOCKET_ENV, SocketListener};
use rikka_seam::{Call, Manifest, Method, MethodInfo, Service, ServiceContext};
use rikka_session::protocol::Protocol;
use rikka_settings::seam::SettingsService;
use rikka_settings::{Key, Layers, Schema};
use rikka_test_seam::Host;
use serde::Serialize;
use serde_json::Value;
use tempfile::TempDir;

const DISPLAY: &str = "wayland-7";

pub(crate) fn input() -> Schema {
    Schema::new("rikka.input")
        .summary("What the session's own key combinations do")
        .key(Key::string("close-window", "Super+Q").summary("Asks the focused window to close"))
        .key(Key::int("repeat-delay", 250).range(100..=2000))
        .key(Key::string("cursor-theme", "default").choices(["default", "Adwaita"]))
}

pub(crate) struct Session {
    runtime: TempDir,
    _system: TempDir,
    user: TempDir,
    programs: Vec<(Host, SocketListener)>,
}

impl Session {
    pub(crate) fn start() -> Session {
        let runtime = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(runtime.path().join("rikka").join(DISPLAY)).unwrap();
        let system = tempfile::tempdir().unwrap();
        let user = tempfile::tempdir().unwrap();
        let layers = Layers::new(system.path(), user.path());
        let mut session = Session {
            runtime,
            _system: system,
            user,
            programs: Vec::new(),
        };
        session.serve(
            "seam",
            SettingsService::with_layers(layers)
                .with_schema(input())
                .expect("the schema holds together")
                .with_schema(rikka_settings::session::appearance::schema())
                .expect("the appearance schema holds together"),
        );
        session
    }

    pub(crate) fn serve(&mut self, program: &str, service: impl Service + Send + 'static) {
        let host = Host::start(move |seam| {
            seam.register(service).expect("the service registers");
        });
        let socket = self
            .runtime
            .path()
            .join("rikka")
            .join(DISPLAY)
            .join(format!("{program}.sock"));
        let listener =
            SocketListener::serve(socket, host.client().clone()).expect("the socket binds");
        self.programs.push((host, listener));
    }

    pub(crate) fn user_file(&self, domain: &str) -> PathBuf {
        self.user.path().join(format!("{domain}.toml"))
    }

    pub(crate) fn command(&self, arguments: &[&str]) -> Command {
        let mut command = rikka(arguments);
        command
            .env_remove(SOCKET_ENV)
            .env("XDG_RUNTIME_DIR", self.runtime.path())
            .env("WAYLAND_DISPLAY", DISPLAY);
        command
    }

    pub(crate) fn run(&self, arguments: &[&str]) -> Output {
        Output::run(&mut self.command(arguments))
    }

    pub(crate) fn expect_success(&self, arguments: &[&str]) -> String {
        self.run(arguments).expect_success(arguments)
    }

    pub(crate) fn expect_failure(&self, arguments: &[&str]) -> String {
        self.run(arguments).expect_failure(arguments)
    }
}

pub(crate) fn rikka(arguments: &[&str]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_rikka-cli"));
    command.args(arguments).stdin(Stdio::null());
    command
}

pub(crate) struct Output {
    pub(crate) status: ExitStatus,
    pub(crate) stdout: String,
    pub(crate) stderr: String,
}

impl Output {
    pub(crate) fn run(command: &mut Command) -> Output {
        let output = command.output().expect("rikka was built beside this test");
        Output {
            status: output.status,
            stdout: String::from_utf8(output.stdout).expect("rikka writes text"),
            stderr: String::from_utf8(output.stderr).expect("rikka writes text"),
        }
    }

    pub(crate) fn expect_success(self, arguments: &[&str]) -> String {
        assert!(
            self.status.success(),
            "rikka {arguments:?} failed: {}",
            self.stderr
        );
        self.stdout
    }

    pub(crate) fn expect_failure(self, arguments: &[&str]) -> String {
        assert_eq!(
            self.status.code(),
            Some(1),
            "rikka {arguments:?} was expected to fail: {}{}",
            self.stdout,
            self.stderr
        );
        self.stderr
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RecordedCall {
    pub(crate) method: String,
    pub(crate) params: Value,
}

#[derive(Clone)]
pub(crate) struct Recorder {
    protocol: Protocol,
    answers: BTreeMap<String, Value>,
    calls: Arc<Mutex<Vec<RecordedCall>>>,
}

impl Recorder {
    pub(crate) fn new(protocol: Protocol) -> Recorder {
        Recorder {
            protocol,
            answers: BTreeMap::new(),
            calls: Arc::default(),
        }
    }

    pub(crate) fn answer(mut self, method: &str, answer: impl Serialize) -> Recorder {
        self.answers.insert(
            method.to_owned(),
            serde_json::to_value(answer).expect("an answer is JSON"),
        );
        self
    }

    pub(crate) fn accept(self, method: &str) -> Recorder {
        self.answer(method, ())
    }

    pub(crate) fn calls(&self) -> Vec<RecordedCall> {
        self.calls.lock().unwrap().clone()
    }
}

impl Service for Recorder {
    fn manifest(&self) -> Manifest {
        self.answers.keys().fold(
            Manifest::new(self.protocol.capability(), self.protocol.version()),
            |manifest, method| manifest.method(MethodInfo::new(method)),
        )
    }

    fn attach(&mut self, _seam: ServiceContext) {}

    fn call(&mut self, _: &mut (), method: Method, call: Call) {
        self.calls.lock().unwrap().push(RecordedCall {
            method: String::from(&*method),
            params: call.payload().value().clone(),
        });
        match self.answers.get(&*method) {
            Some(answer) => call.encode(answer),
            None => call.unknown_method(),
        }
    }
}

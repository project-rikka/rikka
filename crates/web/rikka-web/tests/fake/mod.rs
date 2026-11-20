use rikka_sync::Mutex;
use std::fs;
use std::os::fd::{OwnedFd, RawFd};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use rikka_gecko_protocol::message::{Closed, Hello, Ready};
use rikka_gecko_protocol::{
    CallbackId, EmbedderMessage, EventMessage, GeckoMessage, PROTOCOL, Reply, Target, WindowId,
    read_frame, write_frame,
};
use rikka_picture::SharedGpu;
use rikka_web::{Runtime, RuntimeBuilder};
use rustix::event::{PollFd, PollFlags, Timespec, poll};
use rustix::fs::{CWD, Mode, OFlags, mkfifoat, open};
use rustix::io::{Errno, read};
use rustix::process::{
    Pid, PidfdFlags, PidfdGetfdFlags, Signal, pidfd_getfd, pidfd_open, pidfd_send_signal,
};
use serde_json::Value;

use crate::support::PATIENCE;

const HANDOFF: &str = "handoff";

static INSTALLED: OnceLock<PathBuf> = OnceLock::new();
static STARTING: Mutex<()> = Mutex::new(());

pub(crate) struct FakeGecko {
    process: OwnedFd,
    stream: UnixStream,
}

struct Handoff {
    pid: Pid,
    channel: RawFd,
}

impl FakeGecko {
    pub(crate) fn start() -> (Runtime, FakeGecko) {
        FakeGecko::start_with(Runtime::builder())
    }

    pub(crate) fn start_with(builder: RuntimeBuilder) -> (Runtime, FakeGecko) {
        let handoff = install();
        let headless = SharedGpu {
            primary_node: None,
            render_node: None,
            texture_formats: Vec::new(),
        };
        let _starting = STARTING.lock();
        let reports = open(
            handoff,
            OFlags::RDONLY | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .expect("the fake engine's handoff opens");
        let runtime = builder
            .gpu(headless)
            .start()
            .expect("the runtime starts the fake engine");
        (runtime, FakeGecko::adopt(&receive_handoff(&reports)))
    }

    pub(crate) fn started() -> (Runtime, FakeGecko) {
        let (runtime, mut gecko) = FakeGecko::start();
        gecko.hello();
        (runtime, gecko)
    }

    fn adopt(handoff: &Handoff) -> FakeGecko {
        let process =
            pidfd_open(handoff.pid, PidfdFlags::empty()).expect("the fake engine can be watched");
        let channel = pidfd_getfd(&process, handoff.channel, PidfdGetfdFlags::empty())
            .expect("the fake engine's channel can be borrowed");
        let stream = UnixStream::from(channel);
        stream
            .set_read_timeout(Some(PATIENCE))
            .expect("a read timeout");
        FakeGecko { process, stream }
    }

    pub(crate) fn send(&mut self, message: &GeckoMessage) {
        write_frame(&mut self.stream, message).expect("the runtime hears the fake engine");
    }

    pub(crate) fn hello(&mut self) {
        self.send(&GeckoMessage::Hello(Hello {
            protocol: PROTOCOL,
            version: "158.0".into(),
        }));
    }

    pub(crate) fn ready(&mut self, window: &WindowId) {
        self.send(&GeckoMessage::Ready(Ready {
            window: window.clone(),
        }));
    }

    pub(crate) fn closed(&mut self, window: &WindowId) {
        self.send(&GeckoMessage::Closed(Closed {
            window: window.clone(),
        }));
    }

    pub(crate) fn event(
        &mut self,
        target: Target,
        event: &str,
        data: Value,
        callback: Option<u64>,
    ) {
        self.send(&GeckoMessage::Event(EventMessage {
            target,
            event: event.to_owned(),
            data: Some(data),
            callback: callback.and_then(CallbackId::new),
        }));
    }

    pub(crate) fn reply(&mut self, callback: CallbackId, success: bool, data: Value) {
        self.send(&GeckoMessage::Reply(Reply {
            callback,
            success,
            data: Some(data),
        }));
    }

    pub(crate) fn receive(&mut self) -> EmbedderMessage {
        match read_frame(&mut self.stream) {
            Ok(Some(message)) => message,
            Ok(None) => panic!("the runtime closed the channel"),
            Err(error) => panic!("the runtime sent nothing in time: {error}"),
        }
    }

    pub(crate) fn receive_until<T>(
        &mut self,
        mut pick: impl FnMut(EmbedderMessage) -> Option<T>,
    ) -> T {
        loop {
            if let Some(picked) = pick(self.receive()) {
                return picked;
            }
        }
    }

    pub(crate) fn opened(&mut self) -> WindowId {
        self.receive_until(|message| match message {
            EmbedderMessage::Open(open) => Some(open.window),
            _ => None,
        })
    }

    pub(crate) fn reply_to(&mut self, callback: u64) -> Reply {
        self.receive_until(|message| match message {
            EmbedderMessage::Reply(reply) if reply.callback.get() == callback => Some(reply),
            _ => None,
        })
    }

    pub(crate) fn next_event(&mut self, name: &str) -> EventMessage {
        self.receive_until(|message| match message {
            EmbedderMessage::Event(event) if event.event == name => Some(event),
            _ => None,
        })
    }

    pub(crate) fn quiet_for(&mut self, quiet: Duration) -> Vec<EmbedderMessage> {
        self.stream
            .set_read_timeout(Some(quiet))
            .expect("a read timeout");
        let mut handle_message = Vec::new();
        while let Ok(Some(message)) = read_frame(&mut self.stream) {
            handle_message.push(message);
        }
        self.stream
            .set_read_timeout(Some(PATIENCE))
            .expect("a read timeout");
        handle_message
    }

    pub(crate) fn crash(self) {
        drop(self);
    }
}

impl Drop for FakeGecko {
    fn drop(&mut self) {
        let _ = pidfd_send_signal(&self.process, Signal::KILL);
    }
}

fn install() -> &'static Path {
    INSTALLED.get_or_init(|| {
        let folder = tempfile::tempdir()
            .expect("a folder for the fake engine")
            .keep();
        let handoff = folder.join(HANDOFF);
        mkfifoat(CWD, &handoff, Mode::RUSR | Mode::WUSR).expect("the fake engine's handoff");
        let program = folder.join("rikka-gecko");
        let script = format!(
            "#!/bin/sh\nprintf '%s %s\\n' \"$$\" \"$RIKKA_GECKO_CHANNEL\" > '{}'\nexec sleep 60\n",
            handoff.display()
        );
        fs::write(&program, script).expect("the fake engine is written");
        fs::set_permissions(&program, fs::Permissions::from_mode(0o755))
            .expect("the fake engine is executable");
        unsafe { std::env::set_var("RIKKA_GECKO_PATH", &folder) };
        handoff
    })
}

fn receive_handoff(reports: &OwnedFd) -> Handoff {
    let deadline = Instant::now() + PATIENCE;
    let mut report = Vec::new();
    while !report.ends_with(b"\n") {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let timeout = Timespec::try_from(remaining).expect("patience fits a timespec");
        let mut readable = [PollFd::new(reports, PollFlags::IN)];
        let ready = poll(&mut readable, Some(&timeout)).expect("the handoff can be watched");
        assert!(ready > 0, "the fake engine reported itself in time");
        let mut chunk = [0; 64];
        match read(reports, &mut chunk) {
            Ok(0) => panic!("the fake engine closed its handoff early"),
            Ok(length) => report.extend_from_slice(&chunk[..length]),
            Err(Errno::AGAIN | Errno::INTR) => {}
            Err(error) => panic!("the handoff cannot be read: {error}"),
        }
    }
    let report = std::str::from_utf8(&report).expect("the handoff is text");
    let (pid, channel) = report
        .trim_end()
        .split_once(' ')
        .expect("the handoff names a process and a channel");
    Handoff {
        pid: pid
            .parse()
            .ok()
            .and_then(Pid::from_raw)
            .expect("the handoff names a process"),
        channel: channel.parse().expect("the handoff names a channel"),
    }
}

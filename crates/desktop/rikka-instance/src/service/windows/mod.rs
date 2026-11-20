mod frame;
mod identity;
mod pipe;

use std::convert::Infallible;
use std::os::windows::io::AsRawHandle as _;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use rikka_worker::{EventFd, EventNotifier, Stop, Worker};
use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::Pipes::GetNamedPipeServerProcessId;
use windows::Win32::UI::WindowsAndMessaging::AllowSetForegroundWindow;
use windows::core::HSTRING;

use self::identity::{Descriptor, Identity};
use self::pipe::{Client, Creation, Server};
use super::Startup;
use crate::error::{Error, Result};
use crate::launch_request::{LaunchRequest, Sink};
use crate::name::Name;

const NAME: &str = "rikka-instance";

const READ_TIMEOUT: Duration = Duration::from_secs(5);

const ACCEPTED: u8 = 1;

pub struct Service {
    name: Name,
    _worker: Worker<Halt>,
}

impl Service {
    #[must_use]
    pub fn name(&self) -> &Name {
        &self.name
    }
}

impl std::fmt::Debug for Service {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Service")
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

#[derive(Debug)]
pub struct Peer {
    pipe: HSTRING,
    sid: String,
}

impl Peer {
    pub fn forward(&self, request: &LaunchRequest) -> Result<()> {
        let client = Client::connect(&self.pipe).map_err(call_error)?;
        let owner = identity::owner_of(client.handle()).map_err(call_error)?;
        if owner != self.sid {
            return Err(Error::Call(format!(
                "instance pipe is owned by {owner}, not the current user"
            )));
        }
        let mut server = 0u32;
        unsafe {
            GetNamedPipeServerProcessId(HANDLE(client.handle().as_raw_handle()), &mut server)
        }
        .map_err(|error| call_error(error.into()))?;
        if let Err(error) = unsafe { AllowSetForegroundWindow(server) } {
            tracing::debug!(%error, server, "failed to let the running instance take the foreground");
        }
        client
            .write_all(&frame::encode(request))
            .map_err(call_error)?;
        let mut answer = [0u8; 1];
        client.read_exact(&mut answer).map_err(call_error)?;
        match answer {
            [ACCEPTED] => Ok(()),
            [other] => Err(Error::Call(format!("unexpected answer {other}"))),
        }
    }
}

#[must_use]
pub fn claim(id: &str, sink: Sink) -> Startup {
    let Some(name) = Name::from_app_id(id) else {
        tracing::debug!(id, "application id is not a valid instance name");
        return Startup::Standalone;
    };
    let identity = match Identity::current() {
        Ok(identity) => identity,
        Err(error) => {
            tracing::warn!(%error, "failed to identify the current user");
            return Startup::Standalone;
        }
    };
    let pipe = HSTRING::from(format!(
        r"\\.\pipe\{}.{}.{}",
        name.bus(),
        identity.sid,
        identity.session
    ));
    let descriptor = match Descriptor::owner_only(&identity.sid) {
        Ok(descriptor) => descriptor,
        Err(error) => {
            tracing::warn!(%error, "failed to secure the instance pipe");
            return Startup::Standalone;
        }
    };
    let server = match Server::create(&pipe, &descriptor) {
        Ok(Creation::First(server)) => server,
        Ok(Creation::Taken) => {
            return Startup::Running(Peer {
                pipe,
                sid: identity.sid,
            });
        }
        Err(error) => {
            tracing::warn!(%error, "failed to create the instance pipe for `{}`", name.bus());
            return Startup::Standalone;
        }
    };
    match listen(server, sink) {
        Ok(worker) => Startup::Primary(Service {
            name,
            _worker: worker,
        }),
        Err(error) => {
            tracing::warn!(%error, "failed to serve `{}`", name.bus());
            Startup::Standalone
        }
    }
}

struct Halt {
    stopped: Arc<AtomicBool>,
    notifier: EventNotifier,
}

impl Stop for Halt {
    fn stop(&self) {
        self.stopped.store(true, Ordering::Release);
        if let Err(error) = self.notifier.notify() {
            tracing::debug!(%error, "failed to wake the instance listener");
        }
    }
}

fn listen(server: Server, sink: Sink) -> std::io::Result<Worker<Halt>> {
    let (notifier, wake) = rikka_worker::event_fd()?;
    let stopped = Arc::new(AtomicBool::new(false));
    let halt = Halt {
        stopped: Arc::clone(&stopped),
        notifier,
    };
    Worker::spawn(NAME, halt, move || {
        Ok::<_, Infallible>(move || serve(&server, &wake, &stopped, &sink))
    })
    .map_err(|error| match error {
        rikka_worker::Error::Spawn(error) => error,
        rikka_worker::Error::Startup(never) => match never {},
        rikka_worker::Error::Exited => std::io::Error::other(error),
    })
}

fn serve(server: &Server, wake: &EventFd, stopped: &AtomicBool, sink: &Sink) {
    loop {
        match server.accept(wake) {
            Ok(Some(())) => {}
            Ok(None) => return,
            Err(error) => return tracing::warn!(%error, "instance listener stopped"),
        }
        if stopped.load(Ordering::Acquire) {
            return;
        }
        let received = receive(server, wake);
        if stopped.load(Ordering::Acquire) {
            return;
        }
        match received {
            Ok(Some(request)) => {
                sink(request);
                if let Err(error) = server.write_all(wake, READ_TIMEOUT, &[ACCEPTED]) {
                    tracing::debug!(%error, "failed to answer a forwarded launch");
                }
            }
            Ok(None) => return,
            Err(error) => tracing::debug!(%error, "dropped a malformed launch request"),
        }
        server.disconnect();
    }
}

fn receive(server: &Server, wake: &EventFd) -> std::io::Result<Option<LaunchRequest>> {
    let mut header = [0u8; frame::HEADER];
    if server
        .read_exact(wake, READ_TIMEOUT, &mut header)?
        .is_none()
    {
        return Ok(None);
    }
    let length = frame::length(header);
    if length > frame::LIMIT {
        return Err(std::io::Error::other(format!(
            "launch request of {length} bytes exceeds the limit"
        )));
    }
    let mut body = vec![0u8; length];
    if server.read_exact(wake, READ_TIMEOUT, &mut body)?.is_none() {
        return Ok(None);
    }
    frame::decode(&body)
        .map(Some)
        .ok_or_else(|| std::io::Error::other("malformed launch request"))
}

fn call_error(error: std::io::Error) -> Error {
    Error::Call(error.to_string())
}

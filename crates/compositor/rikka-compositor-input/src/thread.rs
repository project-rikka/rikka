use std::sync::Arc;
use std::sync::mpsc;
use std::thread::JoinHandle;

use rikka_compositor_reactor::Sender;

use crate::access::DeviceAccess;
use crate::cursor::CursorSink;
use crate::error::{Error, Result};
use crate::leds::Leds;
use crate::notice::InputNotice;
use crate::queue::InputQueue;
use crate::settings::DeviceSettings;
use crate::shared::PointerShared;
use crate::worker::{self, Setup};

const DEFAULT_SEAT: &str = "seat0";
const THREAD_NAME: &str = "rikka-input";

#[derive(Debug)]
pub(crate) enum Control {
    Configure(DeviceSettings),
    Leds(Leds),
    Suspend,
    Resume,
    Stop,
}

#[derive(Debug)]
pub struct InputThread {
    control: Sender<Control>,
    thread: Option<JoinHandle<()>>,
}

impl InputThread {
    pub fn builder(
        queue: &InputQueue,
        notices: Sender<InputNotice>,
        pointer: Arc<PointerShared>,
        access: DeviceAccess,
    ) -> InputThreadBuilder {
        InputThreadBuilder {
            queue: queue.clone(),
            notices,
            pointer,
            access,
            seat: None,
            settings: None,
            cursor: None,
        }
    }

    pub fn configure(&self, settings: DeviceSettings) -> Result<()> {
        self.send(Control::Configure(settings))
    }

    pub fn set_leds(&self, leds: Leds) -> Result<()> {
        self.send(Control::Leds(leds))
    }

    pub fn suspend(&self) -> Result<()> {
        self.send(Control::Suspend)
    }

    pub fn resume(&self) -> Result<()> {
        self.send(Control::Resume)
    }

    fn send(&self, control: Control) -> Result<()> {
        self.control.send(control).map_err(|_| Error::Stopped)
    }
}

impl Drop for InputThread {
    fn drop(&mut self) {
        let _ = self.control.send(Control::Stop);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[must_use]
pub struct InputThreadBuilder {
    queue: InputQueue,
    notices: Sender<InputNotice>,
    pointer: Arc<PointerShared>,
    access: DeviceAccess,
    seat: Option<String>,
    settings: Option<DeviceSettings>,
    cursor: Option<Box<dyn CursorSink>>,
}

impl InputThreadBuilder {
    pub fn seat(mut self, seat: impl Into<String>) -> Self {
        self.seat = Some(seat.into());
        self
    }

    pub fn settings(mut self, settings: DeviceSettings) -> Self {
        self.settings = Some(settings);
        self
    }

    pub fn cursor(mut self, sink: impl CursorSink) -> Self {
        self.cursor = Some(Box::new(sink));
        self
    }

    pub fn spawn(self) -> Result<InputThread> {
        let setup = Setup {
            seat: self.seat.unwrap_or_else(session_seat),
            queue: self.queue,
            notices: self.notices,
            pointer: self.pointer,
            access: self.access,
            settings: self.settings,
            cursor: self.cursor,
        };
        let (ready, started) = mpsc::sync_channel(1);
        let thread = std::thread::Builder::new()
            .name(THREAD_NAME.to_owned())
            .spawn(move || worker::run(setup, ready))
            .map_err(Error::Spawn)?;
        let control = started
            .recv()
            .map_err(|_| Error::Stopped)?
            .map_err(Error::Spawn)?;
        Ok(InputThread {
            control,
            thread: Some(thread),
        })
    }
}

impl std::fmt::Debug for InputThreadBuilder {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("InputThreadBuilder")
            .field("seat", &self.seat)
            .field("settings", &self.settings)
            .field("cursor", &self.cursor.is_some())
            .finish_non_exhaustive()
    }
}

fn session_seat() -> String {
    std::env::var("XDG_SEAT")
        .ok()
        .filter(|seat| !seat.is_empty())
        .unwrap_or_else(|| DEFAULT_SEAT.to_owned())
}

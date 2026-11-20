use std::sync::Arc;
use std::time::Duration;

use rikka_types::{DragAction, DragActions};
use rikka_wayland::Host;
use rikka_wayland_client::wl_surface::WlSurface;
use rikka_wayland_client::xdg_toplevel::XdgToplevel;

use crate::wayland::data_device::error::{Error, Result};
use crate::wayland::data_device::worker::{self, Command, DragEvent, Worker};

const ANSWER: Duration = Duration::from_secs(2);

#[derive(Clone, Debug)]
pub(crate) struct Start {
    pub(crate) entries: Vec<(String, Arc<[u8]>)>,
    pub(crate) actions: DragActions,
    pub(crate) origin: WlSurface,
    pub(crate) icon: Option<Icon>,
    pub(crate) moves_windows: bool,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Icon {
    pub(crate) scale: i32,
    pub(crate) hotspot_x: i32,
    pub(crate) hotspot_y: i32,
}

#[derive(Clone, Debug)]
pub(crate) struct DragStarted {
    pub(crate) icon: Option<WlSurface>,
    pub(crate) moves_windows: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Answer {
    pub(crate) mime: String,
    pub(crate) actions: DragActions,
    pub(crate) preferred: DragAction,
}

#[derive(Debug)]
pub(crate) struct Drags {
    worker: Worker,
}

impl Drags {
    pub(crate) fn on(host: &Host, sink: impl Fn(DragEvent) + Send + 'static) -> Result<Self> {
        let worker = worker::start(host.clone(), Box::new(sink))?;
        Ok(Self { worker })
    }

    pub(crate) fn start(&self, start: Start) -> Result<DragStarted> {
        self.request(|reply| Command::Start {
            start: Box::new(start),
            reply,
        })
    }

    pub(crate) fn attach(
        &self,
        toplevel: &XdgToplevel,
        x_offset: i32,
        y_offset: i32,
    ) -> Result<()> {
        self.request(|reply| Command::Attach {
            toplevel: toplevel.clone(),
            x_offset,
            y_offset,
            reply,
        })
    }

    pub(crate) fn accept(&self, answer: Option<Answer>) -> Result<()> {
        self.send(Command::Accept(answer))
    }

    pub(crate) fn release_icon(&self) -> Result<()> {
        self.send(Command::ReleaseIcon)
    }

    pub(crate) fn cancel(&self) -> Result<()> {
        self.send(Command::Cancel)
    }

    fn request<R>(
        &self,
        command: impl FnOnce(rikka_worker::Reply<Result<R>>) -> Command,
    ) -> Result<R> {
        self.worker.handle().request(command, ANSWER)?
    }

    fn send(&self, command: Command) -> Result<()> {
        self.worker.handle().send(command).map_err(Error::from)
    }
}

mod incoming;
mod outgoing;
mod state;

use std::ops::ControlFlow;
use std::os::unix::net::UnixStream;
use std::sync::Arc;

use rikka_types::DragAction;
use rikka_wayland::Host;
use rikka_wayland_client::Connection;
use rikka_wayland_client::wl_compositor::WlCompositor;
use rikka_wayland_client::wl_data_device_manager::WlDataDeviceManager;
use rikka_wayland_client::wl_seat::WlSeat;
use rikka_wayland_client::wl_surface::WlSurface;
use rikka_wayland_client::xdg_toplevel::XdgToplevel;
use rikka_wayland_client::xdg_toplevel_drag_manager_v1::XdgToplevelDragManagerV1;
use rikka_worker::Reply;

use crate::wayland::data_device::delivery::Delivery;
use crate::wayland::data_device::drags::{Answer, DragStarted, Start};
use crate::wayland::data_device::error::{Error, Result, startup};

use state::State;

pub(crate) type CommandSender = rikka_worker::CommandSender<Command>;
type CommandReceiver = rikka_worker::CommandReceiver<Command>;

#[derive(Debug)]
pub(crate) enum DragEvent {
    Entered {
        surface: WlSurface,
        x: f64,
        y: f64,
        delivery: Delivery,
    },
    Moved {
        x: f64,
        y: f64,
    },
    Left,
    Dropped {
        action: Option<DragAction>,
    },
    Ended(End),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum End {
    Landed(DragAction),
    Nowhere,
    Aborted,
}

pub(crate) type EventSink = Box<dyn Fn(DragEvent) + Send + 'static>;

pub(crate) enum Command {
    Start {
        start: Box<Start>,
        reply: Reply<Result<DragStarted>>,
    },
    Attach {
        toplevel: XdgToplevel,
        x_offset: i32,
        y_offset: i32,
        reply: Reply<Result<()>>,
    },
    Accept(Option<Answer>),
    Receive {
        id: u64,
        mime: String,
        reply: Reply<Result<UnixStream>>,
    },
    Finish {
        id: u64,
    },
    ReleaseIcon,
    Cancel,
    Stop,
}

pub(crate) type Worker = rikka_worker::Worker<Arc<CommandSender>>;

pub(crate) fn start(host: Host, sink: EventSink) -> Result<Worker> {
    let (sender, commands) = rikka_worker::command_channel(|| Command::Stop).map_err(Error::Io)?;
    let sender = Arc::new(sender);
    let posted = Arc::clone(&sender);

    Worker::spawn("rikka-drag", sender, move || {
        run(host, commands, posted, sink)
    })
    .map_err(startup)
}

fn run(
    host: Host,
    commands: CommandReceiver,
    sender: Arc<CommandSender>,
    sink: EventSink,
) -> Result<impl FnOnce()> {
    let serials = host.serials()?;
    let display = host.display();
    let mut event_loop = display.event_loop::<State>();
    state::listen(&mut event_loop);

    let seat: WlSeat = display.bind(1..=9)?;
    let manager: WlDataDeviceManager = display.bind_on(&event_loop.route(), 3..=3)?;
    let compositor: WlCompositor = display.bind(1..=6)?;
    let windows: Option<XdgToplevelDragManagerV1> = display.bind(1..=1).ok();

    let device = manager.get_data_device(&seat);
    let mut state = State::new(sink, sender, manager, compositor, windows, device, serials);

    Ok(move || {
        let conn = host.connection();
        let served = event_loop.run(&mut state, &commands, |state, command| {
            handle_command(state, conn, command)
        });
        if let Err(error) = served {
            tracing::warn!("drag event loop stopped: {error}");
        }
    })
}

fn handle_command(state: &mut State, conn: &Connection, command: Command) -> ControlFlow<()> {
    match command {
        Command::Start { start, reply } => {
            let _ = reply.send(state.begin(conn, *start));
        }
        Command::Attach {
            toplevel,
            x_offset,
            y_offset,
            reply,
        } => {
            let _ = reply.send(state.attach(conn, &toplevel, x_offset, y_offset));
        }
        Command::Accept(answer) => state.accept(answer),
        Command::Receive { id, mime, reply } => {
            let _ = reply.send(state.receive(conn, id, &mime));
        }
        Command::Finish { id } => state.finish(id),
        Command::ReleaseIcon => state.release_icon(),
        Command::Cancel => state.cancel(),
        Command::Stop => return ControlFlow::Break(()),
    }
    if let Err(error) = conn.flush() {
        tracing::debug!("drag thread connection error: {error}");
    }
    ControlFlow::Continue(())
}

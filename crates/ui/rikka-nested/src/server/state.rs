use std::os::unix::net::UnixListener;
use std::sync::Arc;
use std::sync::mpsc;
use std::time::Duration;

use rikka_clipboard::Clipboard;
use rikka_compositor_output::{OutputProtocols, Outputs};
use rikka_compositor_reactor::Reactor;
use rikka_compositor_seat::{Capabilities, SeatId, SeatProtocols, Seats};
use rikka_compositor_selection::{SelectionProtocols, Selections};
use rikka_compositor_server::{ClientOptions, DisplayHandle};
use rikka_compositor_shell::{Shell, XdgShell};
use rikka_compositor_surface::{SurfaceProtocols, Surfaces};
use rikka_compositor_types::OutputId;
use rikka_picture::SharedGpu;

use super::bridge::Bridge;
use super::content::Contents;
use super::drag::Drags;
use super::keyboard::Keyboard;
use super::output::display_output;
use super::pacing::Pacing;
use super::pointer::Pointer;
use super::seat_events::CursorState;
use super::views::Views;
use super::{accept, gpu, keyboard};
use crate::command::{Command, Link};
use crate::event::{Event, EventSender};

pub(crate) struct Setup {
    pub(crate) listener: UnixListener,
    pub(crate) gpu: Option<SharedGpu>,
    pub(crate) gpu_patience: Duration,
    pub(crate) clipboard: Option<Clipboard>,
    pub(crate) event_sender: Arc<EventSender>,
    pub(crate) ready: mpsc::Sender<Result<Link, String>>,
}

pub(crate) struct State {
    pub(crate) display: DisplayHandle<State>,
    pub(crate) surfaces: Surfaces,
    pub(crate) seats: Seats,
    pub(crate) shell: Shell,
    pub(crate) selections: Selections,
    pub(crate) outputs: Outputs,
    pub(crate) seat: SeatId,
    pub(crate) output: OutputId,
    pub(crate) views: Views,
    pub(crate) contents: Contents,
    pub(crate) pacing: Pacing,
    pub(crate) keyboard: Keyboard,
    pub(crate) pointer: Pointer,
    pub(crate) bridge: Bridge,
    pub(crate) drags: Drags,
    pub(crate) cursor_state: CursorState,
    pub(crate) event_sender: Arc<EventSender>,
    pub(crate) link: Link,
    pub(crate) gpu_settled: bool,
    pub(crate) stopped: bool,
}

pub(crate) fn run(setup: Setup) {
    let Setup {
        listener,
        gpu,
        gpu_patience,
        clipboard,
        event_sender,
        ready,
    } = setup;
    let (reactor, mut state) = match start(event_sender) {
        Ok(started) => started,
        Err(reason) => {
            let _ = ready.send(Err(reason));
            return;
        }
    };
    match gpu {
        Some(gpu) => state.settle_gpu(Some(gpu)),
        None => {
            if let Err(error) = gpu::await_published(state.link.clone(), gpu_patience) {
                let _ = ready.send(Err(format!("failed to wait for GPU: {error}")));
                return;
            }
        }
    }
    if ready.send(Ok(state.link.clone())).is_err() {
        return;
    }
    if let Err(error) = reactor.run(&mut state, |state| state.stopped || state.gpu_settled) {
        tracing::error!("nested display stopped during startup: {error}");
        return;
    }
    if state.stopped {
        return;
    }
    if let Some(clipboard) = clipboard {
        state.connect_session_clipboard(&clipboard);
    }
    let _listening = match accept::listen(&reactor, listener) {
        Ok(token) => token,
        Err(error) => {
            tracing::error!("nested display cannot accept clients: {error}");
            return;
        }
    };
    if let Err(error) = reactor.run(&mut state, |state| state.stopped) {
        tracing::error!("nested display stopped: {error}");
    }
    state.bridge.disconnect();
}

fn start(event_sender: Arc<EventSender>) -> Result<(Reactor<State>, State), String> {
    let reactor = Reactor::<State>::new().map_err(|error| error.to_string())?;
    let display = DisplayHandle::new(&reactor);
    display.install(SurfaceProtocols);
    display.install(SeatProtocols);
    display.install(XdgShell::new());
    display.install(SelectionProtocols);
    display.install(OutputProtocols);
    display.on_disconnect(|state: &mut State, client, reason| {
        state.event_sender.send(Event::ClientGone {
            pid: client.credentials().pid,
            reason: reason.to_string(),
        });
    });
    let shell = Shell::new(&display);
    let mut seats = Seats::new();
    let seat = seats.add(&display, "seat0", keyboard::initial_keymap()?);
    let mut outputs = Outputs::new();
    let output = outputs.connect("rikka-nested");
    outputs.publish(&display, output, display_output(None, 1.0));
    let link = reactor
        .mailbox(|state: &mut State, command: Command| state.command(command))
        .map_err(|error| error.to_string())?;
    let mut state = State {
        display,
        surfaces: Surfaces::new(),
        seats,
        shell,
        selections: Selections::new(),
        outputs,
        seat,
        output,
        views: Views::default(),
        contents: Contents::default(),
        pacing: Pacing::default(),
        keyboard: Keyboard::default(),
        pointer: Pointer::default(),
        bridge: Bridge::default(),
        drags: Drags::default(),
        cursor_state: CursorState::default(),
        event_sender,
        link,
        gpu_settled: false,
        stopped: false,
    };
    let mut seat_handle = rikka_compositor_seat::seat(&mut state, seat);
    seat_handle
        .set_capabilities(Capabilities::POINTER | Capabilities::KEYBOARD | Capabilities::TOUCH);
    seat_handle.serve_text_input(true);
    keyboard::follow_session(&mut state);
    reactor.each_turn(State::settle);
    Ok((reactor, state))
}

impl State {
    pub(crate) fn client_options() -> ClientOptions {
        ClientOptions::new().unprivileged()
    }

    fn command(&mut self, command: Command) {
        match command {
            Command::Adopt {
                view,
                app_id,
                shared,
            } => self.adopt(view, app_id, shared),
            Command::Forget(view) => self.forget(view),
            Command::Resize { view, size, scale } => self.resize(view, size, scale),
            Command::Visible { view, visible } => self.set_visible(view, visible),
            Command::Fullscreen { view, fullscreen } => self.set_fullscreen(view, fullscreen),
            Command::Close(view) => self.close(view),
            Command::Input { view, input, at } => self.input(view, input, at),
            Command::Tick { view, tick } => self.on_tick(view, tick),
            Command::Released(hold) => self.contents.release(&mut self.surfaces, hold),
            Command::SessionSelection { kind, mime_types } => {
                self.session_selection(kind, mime_types)
            }
            Command::ClientSend {
                kind,
                generation,
                mime_type,
                fd,
            } => self.client_send(kind, generation, mime_type, fd),
            Command::DragRead { generation, offer } => self.drag_read(generation, offer),
            Command::Gpu(gpu) => self.settle_gpu(gpu),
            Command::Stop => self.stopped = true,
        }
    }
}

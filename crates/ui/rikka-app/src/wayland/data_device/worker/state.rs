use std::os::unix::net::UnixStream;
use std::sync::{Arc, Weak};
use std::task::{Wake, Waker};

use rikka_wayland_client::Connection;
use rikka_wayland_client::wl_compositor::WlCompositor;
use rikka_wayland_client::wl_data_device::{self, WlDataDevice};
use rikka_wayland_client::wl_data_device_manager::WlDataDeviceManager;
use rikka_wayland_client::wl_data_offer::{self, WlDataOffer};
use rikka_wayland_client::wl_data_source::{self, WlDataSource};
use rikka_wayland_client::wl_surface::WlSurface;
use rikka_wayland_client::xdg_toplevel::XdgToplevel;
use rikka_wayland_client::xdg_toplevel_drag_manager_v1::XdgToplevelDragManagerV1;

use rikka_types::{DragAction, DragActions};

use crate::wayland::data_device::actions::{action_of, actions_of, wire_all};
use crate::wayland::data_device::delivery::Delivery;
use crate::wayland::data_device::drags::{Answer, DragStarted, Start};
use crate::wayland::data_device::error::{Error, Result};
use crate::wayland::data_device::worker::incoming::{Incoming, unanswered};
use crate::wayland::data_device::worker::outgoing::OutgoingDrag;
use crate::wayland::data_device::worker::{Command, CommandSender, DragEvent, End, EventSink};
use rikka_wayland::{EventLoop, Serial, SerialKind, Serials};

const HOLDING: [SerialKind; 2] = [SerialKind::ButtonPressed, SerialKind::ButtonReleased];

#[derive(Default)]
struct IncomingOffer {
    mimes: Vec<String>,
    actions: DragActions,
}

pub(super) struct State {
    sink: EventSink,
    sender: Arc<CommandSender>,
    manager: WlDataDeviceManager,
    compositor: WlCompositor,
    windows: Option<XdgToplevelDragManagerV1>,
    device: WlDataDevice,
    serials: Serials,
    outgoing: Option<OutgoingDrag>,
    icon: Option<WlSurface>,
    hovering: Option<Incoming>,
    dropped: Vec<Incoming>,
    incoming: IncomingOffer,
    offers: u64,
}

impl State {
    pub(super) fn new(
        sink: EventSink,
        sender: Arc<CommandSender>,
        manager: WlDataDeviceManager,
        compositor: WlCompositor,
        windows: Option<XdgToplevelDragManagerV1>,
        device: WlDataDevice,
        serials: Serials,
    ) -> Self {
        Self {
            sink,
            sender,
            manager,
            compositor,
            windows,
            device,
            serials,
            outgoing: None,
            icon: None,
            hovering: None,
            dropped: Vec::new(),
            incoming: IncomingOffer::default(),
            offers: 0,
        }
    }

    pub(super) fn begin(&mut self, conn: &Connection, start: Start) -> Result<DragStarted> {
        if self.outgoing.is_some() {
            return Err(Error::Busy);
        }
        let released = SenderWaker(Arc::downgrade(&self.sender));
        self.serials
            .register_waker(SerialKind::ButtonReleased, Waker::from(Arc::new(released)));
        let Some(Serial {
            serial,
            kind: SerialKind::ButtonPressed,
        }) = self.serials.latest(&HOLDING)
        else {
            return Err(Error::Ungrabbed);
        };
        self.release_icon();

        let source = self.manager.create_data_source();
        for (mime, _) in &start.entries {
            source.offer(mime);
        }
        source.set_actions(wire_all(start.actions));
        let windows = start
            .moves_windows
            .then(|| {
                self.windows
                    .as_ref()
                    .map(|manager| manager.get_xdg_toplevel_drag(&source))
            })
            .flatten();

        let icon = start.icon.map(|icon| {
            let surface = self.compositor.create_surface();
            surface.set_buffer_scale(icon.scale.max(1));
            if surface.version() >= 5 {
                surface.offset(-icon.hotspot_x, -icon.hotspot_y);
            }
            surface
        });

        self.device
            .start_drag(Some(&source), &start.origin, icon.as_ref(), serial);
        conn.flush()
            .map_err(|_| Error::Unavailable("the session stopped listening"))?;

        let moves_windows = windows.is_some();
        self.outgoing = Some(OutgoingDrag::new(source, windows, start.entries));
        self.icon = icon;
        Ok(DragStarted {
            icon: self.icon.clone(),
            moves_windows,
        })
    }

    pub(super) fn attach(
        &mut self,
        conn: &Connection,
        toplevel: &XdgToplevel,
        x_offset: i32,
        y_offset: i32,
    ) -> Result<()> {
        let windows = self
            .outgoing
            .as_ref()
            .and_then(|outgoing| outgoing.windows.as_ref())
            .ok_or(Error::Unavailable("no drag here carries windows"))?;
        windows.attach(toplevel, x_offset, y_offset);
        conn.flush()
            .map_err(|_| Error::Unavailable("the session stopped listening"))
    }

    pub(super) fn accept(&mut self, answer: Option<Answer>) {
        if let Some(hovering) = self.hovering.as_mut() {
            hovering.answer(answer);
        }
    }

    pub(super) fn receive(&mut self, conn: &Connection, id: u64, mime: &str) -> Result<UnixStream> {
        let incoming = self
            .hovering
            .iter()
            .chain(&self.dropped)
            .find(|incoming| incoming.id == id)
            .ok_or_else(unanswered)?;
        let stream = incoming.receive(mime)?;
        conn.flush()
            .map_err(|_| Error::Unavailable("the session stopped listening"))?;
        Ok(stream)
    }

    pub(super) fn finish(&mut self, id: u64) {
        if let Some(index) = self.dropped.iter().position(|incoming| incoming.id == id) {
            self.dropped.swap_remove(index).finish();
        }
    }

    pub(super) fn release_icon(&mut self) {
        if let Some(icon) = self.icon.take() {
            icon.destroy();
        }
    }

    pub(super) fn cancel(&mut self) {
        if self.outgoing.is_some() {
            self.ended(End::Aborted);
        }
    }

    fn ended(&mut self, how: End) {
        let Some(outgoing) = self.outgoing.take() else {
            return;
        };
        outgoing.release();
        self.dispatch_event(DragEvent::Ended(how));
    }

    fn entered(&mut self, serial: u32, surface: WlSurface, x: f64, y: f64, offer: WlDataOffer) {
        let IncomingOffer { mimes, actions } = std::mem::take(&mut self.incoming);
        self.forget_hovering();
        self.offers += 1;
        let id = self.offers;
        self.hovering = Some(Incoming::new(id, offer, serial));
        let delivery = Delivery::new(id, mimes, actions, Arc::clone(&self.sender));
        self.dispatch_event(DragEvent::Entered {
            surface,
            x,
            y,
            delivery,
        });
    }

    fn dropped(&mut self) {
        let Some(incoming) = self.hovering.take() else {
            return;
        };
        let action = incoming.chosen;
        self.dropped.push(incoming);
        self.dispatch_event(DragEvent::Dropped { action });
    }

    fn forget_hovering(&mut self) {
        if let Some(incoming) = self.hovering.take() {
            incoming.forget();
        }
    }

    fn chosen(&mut self, offer: &WlDataOffer, action: Option<DragAction>) {
        if let Some(incoming) = self
            .hovering
            .iter_mut()
            .chain(self.dropped.iter_mut())
            .find(|incoming| &incoming.offer == offer)
        {
            incoming.chosen = action;
        }
    }

    fn dispatch_event(&self, event: DragEvent) {
        (self.sink)(event);
    }
}

pub(super) fn listen(event_loop: &mut EventLoop<State>) {
    event_loop.on::<WlDataDevice>(on_wl_data_device);
    event_loop.on::<WlDataOffer>(on_wl_data_offer);
    event_loop.on::<WlDataSource>(on_wl_data_source);
}

struct SenderWaker(Weak<CommandSender>);

impl Wake for SenderWaker {
    fn wake(self: Arc<Self>) {
        if let Some(sender) = self.0.upgrade() {
            drop(sender.send(Command::Cancel));
        }
    }
}

fn on_wl_data_device(state: &mut State, _device: WlDataDevice, event: wl_data_device::Event) {
    match event {
        wl_data_device::Event::DataOffer { id: _ } => {
            state.incoming = IncomingOffer::default();
        }
        wl_data_device::Event::Enter {
            serial,
            surface,
            x,
            y,
            id,
        } => match id {
            Some(offer) => state.entered(serial, surface, x, y, offer),
            None => state.incoming = IncomingOffer::default(),
        },
        wl_data_device::Event::Motion { x, y, .. } => {
            state.dispatch_event(DragEvent::Moved { x, y })
        }
        wl_data_device::Event::Leave => {
            state.forget_hovering();
            state.dispatch_event(DragEvent::Left);
        }
        wl_data_device::Event::Drop => state.dropped(),
        wl_data_device::Event::Selection { id } => {
            state.incoming = IncomingOffer::default();
            if let Some(offer) = id {
                offer.destroy();
            }
        }
        _ => {}
    }
}

fn on_wl_data_offer(state: &mut State, offer: WlDataOffer, event: wl_data_offer::Event) {
    match event {
        wl_data_offer::Event::Offer { mime_type } => state.incoming.mimes.push(mime_type),
        wl_data_offer::Event::SourceActions {
            source_actions: actions,
        } => state.incoming.actions = actions_of(actions),
        wl_data_offer::Event::Action { dnd_action: action } => {
            state.chosen(&offer, action_of(action))
        }
        _ => {}
    }
}

fn on_wl_data_source(state: &mut State, source: WlDataSource, event: wl_data_source::Event) {
    let Some(outgoing) = state
        .outgoing
        .as_mut()
        .filter(|outgoing| outgoing.source == source)
    else {
        return;
    };
    match event {
        wl_data_source::Event::Send { mime_type, fd } => outgoing.send(&mime_type, fd),
        wl_data_source::Event::Action { dnd_action: action } => outgoing.chosen(action_of(action)),
        wl_data_source::Event::DndDropPerformed => outgoing.performed(),
        wl_data_source::Event::DndFinished => {
            let end = outgoing.finished();
            state.ended(end);
        }
        wl_data_source::Event::Cancelled => {
            let end = outgoing.cancelled();
            state.ended(end);
        }
        _ => {}
    }
}

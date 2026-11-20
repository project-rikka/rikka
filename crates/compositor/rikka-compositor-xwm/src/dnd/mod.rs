mod action;
mod message;

use std::borrow::Cow;

use rikka_types::DragAction;
use x11rb_protocol::protocol::xproto::{
    Atom, AtomEnum, ClientMessageEvent, ConfigureWindowAux, ConfigureWindowRequest,
    CreateWindowAux, CreateWindowRequest, EventMask, GetPropertyReply, GetPropertyRequest,
    MapWindowRequest, StackMode, UnmapWindowRequest, Window, WindowClass,
};

use crate::error::Result;
use crate::event::XwmEvent;
use crate::fault::{Outcome, describe};
use crate::link::Link;
use crate::pending::{ExpectedReply, Resume};
use crate::properties::Field;
use crate::record::Xdnd;
use crate::selection::{MimeList, SelectionKind, offered_mime_types, targets_of};
use crate::window::XWindowId;
use crate::wire::{COPY_DEPTH_FROM_PARENT, COPY_FROM_PARENT, CURRENT_TIME, NONE, Reply, parse};
use crate::xwm::Xwm;

use message::{INLINE_TYPES, VERSION};

const TYPE_LIMIT: u32 = 1024;
const OLDEST: u32 = 3;
const COVERS: u16 = i16::MAX as u16;
const PROBED_FIELDS: [Field; 2] = [Field::XdndProxy, Field::XdndAware];

#[derive(Debug)]
pub(crate) struct Dnd {
    proxy: Window,
    shown: bool,
    attempts: u64,
    outgoing: Option<Outgoing>,
    incoming: Option<Incoming>,
}

#[derive(Debug)]
struct Outgoing {
    attempt: u64,
    target: Window,
    deliver_to: Window,
    version: u32,
    entered: bool,
    probes: Vec<(Window, Xdnd)>,
    probing: usize,
    awaiting: bool,
    pending: Option<(i32, i32, Atom)>,
    dropping: bool,
}

#[derive(Debug)]
struct Incoming {
    source: Window,
    version: u32,
    accepted: bool,
    action: Atom,
    announced: bool,
    dropped: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Route {
    Unknown(Window),
    Refused,
    To(Window, u32),
}

impl Dnd {
    pub(crate) fn new(link: &mut Link) -> Outcome<Self> {
        let proxy = link.wire.generate_id()?;
        link.wire.send(CreateWindowRequest {
            depth: COPY_DEPTH_FROM_PARENT,
            wid: proxy,
            parent: link.root,
            x: 0,
            y: 0,
            width: COVERS,
            height: COVERS,
            border_width: 0,
            class: WindowClass::INPUT_OUTPUT,
            visual: COPY_FROM_PARENT,
            value_list: Cow::Owned(
                CreateWindowAux::new()
                    .override_redirect(1)
                    .event_mask(EventMask::NO_EVENT),
            ),
        })?;
        let aware = link.atoms.XdndAware;
        link.set_atoms(proxy, aware, &[VERSION])?;
        Ok(Self {
            proxy,
            shown: false,
            attempts: 0,
            outgoing: None,
            incoming: None,
        })
    }

    pub(crate) fn shown_proxy(&self) -> Option<Window> {
        self.shown.then_some(self.proxy)
    }
}

impl Xwm {
    pub fn drag_enter(
        &mut self,
        window: XWindowId,
        mime_types: MimeList,
        at: (i32, i32),
        action: DragAction,
    ) -> Result<()> {
        self.drag_leave()?;
        let (selections, mut io) = self.split();
        selections.offer(&mut io, SelectionKind::Drag, mime_types)?;
        self.dnd.attempts += 1;
        let attempt = self.dnd.attempts;
        self.dnd.outgoing = Some(Outgoing {
            attempt,
            target: window.raw(),
            deliver_to: window.raw(),
            version: VERSION,
            entered: false,
            probes: Vec::new(),
            probing: 0,
            awaiting: false,
            pending: Some((at.0, at.1, action::atom(action, &self.link.atoms))),
            dropping: false,
        });
        self.continue_drag(attempt)?;
        Ok(())
    }

    pub fn drag_motion(&mut self, at: (i32, i32), action: DragAction) -> Result<()> {
        let action = action::atom(action, &self.link.atoms);
        let Some(outgoing) = self.dnd.outgoing.as_mut() else {
            return Ok(());
        };
        if outgoing.awaiting || !outgoing.entered {
            outgoing.pending = Some((at.0, at.1, action));
            return Ok(());
        }
        outgoing.awaiting = true;
        let (deliver_to, target) = (outgoing.deliver_to, outgoing.target);
        let source = self.selections.window(SelectionKind::Drag);
        let kind = self.link.atoms.XdndPosition;
        self.post(
            deliver_to,
            target,
            kind,
            message::position(source, at.0, at.1, CURRENT_TIME, action),
        )?;
        Ok(())
    }

    pub fn drag_leave(&mut self) -> Result<()> {
        let Some(outgoing) = self.dnd.outgoing.take() else {
            return Ok(());
        };
        if !outgoing.entered {
            return Ok(());
        }
        let source = self.selections.window(SelectionKind::Drag);
        let kind = self.link.atoms.XdndLeave;
        self.post(
            outgoing.deliver_to,
            outgoing.target,
            kind,
            message::leave(source),
        )?;
        Ok(())
    }

    pub fn drag_drop(&mut self) -> Result<()> {
        let Some(outgoing) = self.dnd.outgoing.as_mut() else {
            return Ok(());
        };
        if !outgoing.entered {
            outgoing.dropping = true;
            return Ok(());
        }
        let (deliver_to, target) = (outgoing.deliver_to, outgoing.target);
        self.post_drop(deliver_to, target)?;
        Ok(())
    }

    pub fn show_drop_proxy(&mut self, shown: bool) -> Result<()> {
        if self.dnd.shown == shown {
            return Ok(());
        }
        let proxy = self.dnd.proxy;
        if shown {
            self.link.wire.send(ConfigureWindowRequest {
                window: proxy,
                value_list: Cow::Owned(ConfigureWindowAux::new().stack_mode(StackMode::ABOVE)),
            })?;
            self.link.wire.send(MapWindowRequest { window: proxy })?;
        } else {
            self.link.wire.send(UnmapWindowRequest { window: proxy })?;
        }
        self.dnd.shown = shown;
        self.publish_clients()?;
        Ok(())
    }

    pub fn answer_drag(&mut self, accepted: bool, action: Option<DragAction>) -> Result<()> {
        let atom = action.map_or(NONE, |action| action::atom(action, &self.link.atoms));
        let Some(incoming) = self
            .dnd
            .incoming
            .as_mut()
            .filter(|incoming| !incoming.dropped)
        else {
            return Ok(());
        };
        incoming.accepted = accepted && atom != NONE;
        incoming.action = atom;
        self.send_status()?;
        Ok(())
    }

    pub fn finish_drag(&mut self, accepted: bool, action: Option<DragAction>) -> Result<()> {
        let Some(incoming) = self.dnd.incoming.take() else {
            return Ok(());
        };
        let atom = action.map_or(NONE, |action| action::atom(action, &self.link.atoms));
        let kind = self.link.atoms.XdndFinished;
        let proxy = self.dnd.proxy;
        self.post(
            incoming.source,
            incoming.source,
            kind,
            message::finished(proxy, incoming.version, accepted, atom),
        )?;
        Ok(())
    }

    pub(crate) fn dnd_message(&mut self, event: &ClientMessageEvent) -> Outcome<bool> {
        let atoms = self.link.atoms;
        let kind = event.type_;
        let known = [
            atoms.XdndEnter,
            atoms.XdndPosition,
            atoms.XdndLeave,
            atoms.XdndDrop,
            atoms.XdndStatus,
            atoms.XdndFinished,
        ];
        if !known.contains(&kind) || event.format != 32 {
            return Ok(false);
        }
        let data = event.data.as_data32();
        match kind {
            kind if kind == atoms.XdndStatus => self.handle_status(data)?,
            kind if kind == atoms.XdndFinished => self.handle_finished(data),
            _ if event.window != self.dnd.proxy => return Ok(false),
            kind if kind == atoms.XdndEnter => self.entered(data)?,
            kind if kind == atoms.XdndPosition => self.positioned(data)?,
            kind if kind == atoms.XdndLeave => self.left(data),
            _ => self.dropped(data),
        }
        Ok(true)
    }

    pub(crate) fn continue_drag(&mut self, attempt: u64) -> Outcome {
        let Some(outgoing) = self
            .dnd
            .outgoing
            .as_ref()
            .filter(|outgoing| outgoing.attempt == attempt && !outgoing.entered)
        else {
            return Ok(());
        };
        if outgoing.probing > 0 {
            return Ok(());
        }
        let (deliver_to, version) = match self.route(outgoing.target, &outgoing.probes) {
            Route::To(deliver_to, version) => (deliver_to, version.min(VERSION)),
            Route::Refused => {
                self.dnd.outgoing = None;
                return Ok(());
            }
            Route::Unknown(window) => return self.probe(attempt, window),
        };
        let (selections, mut io) = self.split();
        if let Some(sequence) = selections.advertise(&mut io, SelectionKind::Drag)? {
            io.pending.after(sequence, Resume::DragEnter { attempt });
            return Ok(());
        }
        let types = selections.advertised_types(SelectionKind::Drag);
        let source = selections.window(SelectionKind::Drag);
        let list = io.link.atoms.XdndTypeList;
        io.link.set_atoms(source, list, types)?;
        let entering = message::enter(source, version, types);
        let Some(outgoing) = self.dnd.outgoing.as_mut() else {
            return Ok(());
        };
        outgoing.entered = true;
        outgoing.deliver_to = deliver_to;
        outgoing.version = version;
        let target = outgoing.target;
        let pending = outgoing.pending.take();
        let dropping = outgoing.dropping;
        let enter = self.link.atoms.XdndEnter;
        self.post(deliver_to, target, enter, entering)?;
        if let Some((x, y, action)) = pending {
            if let Some(outgoing) = self.dnd.outgoing.as_mut() {
                outgoing.awaiting = true;
            }
            let position = self.link.atoms.XdndPosition;
            self.post(
                deliver_to,
                target,
                position,
                message::position(source, x, y, CURRENT_TIME, action),
            )?;
        }
        if dropping {
            self.post_drop(deliver_to, target)?;
        }
        Ok(())
    }

    pub(crate) fn probe_read(
        &mut self,
        attempt: u64,
        window: Window,
        field: Field,
        reply: Reply,
    ) -> Outcome {
        let Some(outgoing) = self
            .dnd
            .outgoing
            .as_mut()
            .filter(|outgoing| outgoing.attempt == attempt)
        else {
            return Ok(());
        };
        outgoing.probing -= 1;
        let word = match reply {
            Ok(packet) => {
                let reply: GetPropertyReply = parse(&packet)?;
                reply
                    .value32()
                    .and_then(|mut values| values.next())
                    .filter(|value| *value != NONE)
            }
            Err(error) => {
                tracing::debug!("failed to probe drag target: {}", describe(&error));
                None
            }
        };
        if let Some((_, facts)) = outgoing
            .probes
            .iter_mut()
            .find(|(probed, _)| *probed == window)
        {
            match field {
                Field::XdndProxy => facts.proxy = word,
                _ => facts.version = word,
            }
        }
        if outgoing.probing == 0 {
            return self.continue_drag(attempt);
        }
        Ok(())
    }

    pub(crate) fn drag_types_read(&mut self, source: Window, reply: Reply) -> Outcome {
        let atoms = match reply {
            Ok(packet) => {
                let reply: GetPropertyReply = parse(&packet)?;
                reply.value32().map(Iterator::collect).unwrap_or_default()
            }
            Err(error) => {
                tracing::debug!("failed to read XdndTypeList: {}", describe(&error));
                Vec::new()
            }
        };
        self.drag_types_known(source, atoms)
    }

    pub(crate) fn drag_types_named(&mut self, source: Window, atoms: &[Atom]) {
        let Some(incoming) = self
            .dnd
            .incoming
            .as_mut()
            .filter(|incoming| incoming.source == source && !incoming.announced)
        else {
            return;
        };
        incoming.announced = true;
        let dropped = incoming.dropped;
        let (selections, io) = self.split();
        let targets = targets_of(&io, atoms);
        let mime_types = offered_mime_types(&targets);
        selections.adopt_targets(SelectionKind::Drag, targets);
        self.events.push(XwmEvent::X11DragEntered { mime_types });
        if dropped {
            self.events.push(XwmEvent::X11DragDropped);
        }
    }

    fn drag_types_known(&mut self, source: Window, atoms: Vec<Atom>) -> Outcome {
        match self
            .names
            .learn_names(&mut self.link.wire, &mut self.pending, &atoms)?
        {
            Some(sequence) => self
                .pending
                .after(sequence, Resume::DragTypes { source, atoms }),
            None => self.drag_types_named(source, &atoms),
        }
        Ok(())
    }

    fn route(&self, target: Window, probes: &[(Window, Xdnd)]) -> Route {
        let facts = |window: Window| {
            self.windows
                .get(&XWindowId::new(window))
                .map(|record| record.xdnd)
                .or_else(|| {
                    probes
                        .iter()
                        .find(|(probed, _)| *probed == window)
                        .map(|(_, facts)| *facts)
                })
        };
        let Some(own) = facts(target) else {
            return Route::Unknown(target);
        };
        let (deliver_to, version) = match own.proxy {
            None => (target, own.version),
            Some(proxy) => match facts(proxy) {
                None => return Route::Unknown(proxy),
                Some(proxied) if proxied.proxy == Some(proxy) => (proxy, proxied.version),
                Some(_) => (target, own.version),
            },
        };
        version
            .filter(|version| *version >= OLDEST)
            .map_or(Route::Refused, |version| Route::To(deliver_to, version))
    }

    fn probe(&mut self, attempt: u64, window: Window) -> Outcome {
        for field in PROBED_FIELDS {
            let sequence = self
                .link
                .wire
                .ask(field.request(window, &self.link.atoms))?;
            self.pending.expect(
                sequence,
                ExpectedReply::Probe {
                    attempt,
                    window,
                    field,
                },
            );
        }
        if let Some(outgoing) = self.dnd.outgoing.as_mut() {
            outgoing.probes.push((window, Xdnd::default()));
            outgoing.probing = PROBED_FIELDS.len();
        }
        Ok(())
    }

    fn post_drop(&mut self, deliver_to: Window, target: Window) -> Outcome {
        let source = self.selections.window(SelectionKind::Drag);
        let kind = self.link.atoms.XdndDrop;
        self.post(
            deliver_to,
            target,
            kind,
            message::drop(source, CURRENT_TIME),
        )
    }

    fn post(&mut self, deliver_to: Window, window: Window, kind: Atom, data: [u32; 5]) -> Outcome {
        self.link.send_event(
            deliver_to,
            EventMask::NO_EVENT,
            ClientMessageEvent::new(32, window, kind, data),
        )
    }

    fn handle_status(&mut self, data: [u32; 5]) -> Outcome {
        let status = message::read_status(data);
        let Some(outgoing) = self
            .dnd
            .outgoing
            .as_mut()
            .filter(|outgoing| outgoing.entered && outgoing.target == status.target)
        else {
            return Ok(());
        };
        outgoing.awaiting = false;
        let pending = outgoing.pending.take();
        self.events.push(XwmEvent::DragStatus {
            accepted: status.accepted,
            action: action::from_atom(status.action, &self.link.atoms),
        });
        if let Some((x, y, action)) = pending
            && let Some(outgoing) = self.dnd.outgoing.as_mut()
        {
            outgoing.awaiting = true;
            let (deliver_to, target) = (outgoing.deliver_to, outgoing.target);
            let source = self.selections.window(SelectionKind::Drag);
            let kind = self.link.atoms.XdndPosition;
            self.post(
                deliver_to,
                target,
                kind,
                message::position(source, x, y, CURRENT_TIME, action),
            )?;
        }
        Ok(())
    }

    fn handle_finished(&mut self, data: [u32; 5]) {
        let Some(outgoing) = self.dnd.outgoing.take() else {
            return;
        };
        let finished = message::read_finished(data, outgoing.version);
        if !outgoing.entered || finished.target != outgoing.target {
            self.dnd.outgoing = Some(outgoing);
            return;
        }
        self.events.push(XwmEvent::DragFinished {
            accepted: finished.accepted,
            action: action::from_atom(finished.action, &self.link.atoms),
        });
    }

    fn entered(&mut self, data: [u32; 5]) -> Outcome {
        let enter = message::read_enter(data);
        self.dnd.incoming = Some(Incoming {
            source: enter.source,
            version: enter.version.min(VERSION),
            accepted: false,
            action: NONE,
            announced: false,
            dropped: false,
        });
        if !enter.listed && enter.types.len() <= INLINE_TYPES {
            return self.drag_types_known(enter.source, enter.types);
        }
        let sequence = self.link.wire.ask(GetPropertyRequest {
            delete: false,
            window: enter.source,
            property: self.link.atoms.XdndTypeList,
            type_: AtomEnum::ATOM.into(),
            long_offset: 0,
            long_length: TYPE_LIMIT,
        })?;
        self.pending.expect(
            sequence,
            ExpectedReply::DragTypes {
                source: enter.source,
            },
        );
        Ok(())
    }

    fn positioned(&mut self, data: [u32; 5]) -> Outcome {
        let hovering = self
            .dnd
            .incoming
            .as_ref()
            .is_some_and(|incoming| incoming.source == data[0] && !incoming.dropped);
        match hovering {
            true => self.send_status(),
            false => Ok(()),
        }
    }

    fn left(&mut self, data: [u32; 5]) {
        let Some(incoming) = self
            .dnd
            .incoming
            .take_if(|incoming| incoming.source == data[0] && !incoming.dropped)
        else {
            return;
        };
        if incoming.announced {
            self.events.push(XwmEvent::X11DragLeft);
        }
    }

    fn dropped(&mut self, data: [u32; 5]) {
        let Some(incoming) = self
            .dnd
            .incoming
            .as_mut()
            .filter(|incoming| incoming.source == data[0] && !incoming.dropped)
        else {
            return;
        };
        incoming.dropped = true;
        if incoming.announced {
            self.events.push(XwmEvent::X11DragDropped);
        }
    }

    fn send_status(&mut self) -> Outcome {
        let Some(incoming) = self.dnd.incoming.as_ref() else {
            return Ok(());
        };
        let (source, accepted, action) = (incoming.source, incoming.accepted, incoming.action);
        let kind = self.link.atoms.XdndStatus;
        let proxy = self.dnd.proxy;
        self.post(
            source,
            source,
            kind,
            message::status(proxy, accepted, action),
        )
    }
}

use std::borrow::Cow;
use std::os::fd::{AsFd, BorrowedFd};
use std::os::unix::net::UnixStream;
use std::time::Instant;

use rustc_hash::FxHashMap;
use x11rb_protocol::SequenceNumber;
use x11rb_protocol::protocol::xproto::{
    ChangeWindowAttributesAux, ChangeWindowAttributesRequest, ClientMessageEvent, CloseFontRequest,
    CreateGlyphCursorRequest, EventMask, FreeCursorRequest, InternAtomReply, InternAtomRequest,
    OpenFontRequest, QueryExtensionReply, QueryExtensionRequest, SetSelectionOwnerRequest, Window,
};
use x11rb_protocol::protocol::{ErrorKind, Event, bigreq, composite, res, xfixes};
use x11rb_protocol::x11_utils::ExtensionInformation;

use crate::atoms::Atoms;
use crate::configure::Configures;
use crate::dnd::Dnd;
use crate::error::{Error, Result};
use crate::event::XwmEvent;
use crate::ewmh::supported;
use crate::fault::{Fault, Outcome, describe};
use crate::link::Link;
use crate::names::{AtomNames, interned_up_front};
use crate::pending::{ExpectedReply, Pending, Resume};
use crate::poller::Poller;
use crate::record::Record;
use crate::selection::Selections;
use crate::stacking::Stacking;
use crate::window::{XWindow, XWindowId};
use crate::wire::{CURRENT_TIME, NONE, Reply, Wire, parse};
use crate::xsettings::XSettings;

pub(crate) fn client_events() -> EventMask {
    EventMask::PROPERTY_CHANGE | EventMask::FOCUS_CHANGE
}
const LEFT_POINTER_GLYPH: u16 = 68;
const MANAGER_NAME: &[u8] = b"Rikka";
const EXTENSIONS: [&str; 4] = [
    xfixes::X11_EXTENSION_NAME,
    composite::X11_EXTENSION_NAME,
    res::X11_EXTENSION_NAME,
    bigreq::X11_EXTENSION_NAME,
];

#[derive(Debug)]
pub struct Xwm {
    pub(crate) link: Link,
    pub(crate) poller: Poller,
    pub(crate) names: AtomNames,
    pub(crate) pending: Pending,
    pub(crate) windows: FxHashMap<XWindowId, Record>,
    pub(crate) dirty: Vec<XWindowId>,
    pub(crate) epochs: u64,
    pub(crate) deferred: Option<(Event, SequenceNumber)>,
    pub(crate) stacking: Stacking,
    pub(crate) configures: Configures,
    pub(crate) selections: Selections,
    pub(crate) dnd: Dnd,
    pub(crate) xsettings: XSettings,
    pub(crate) xsettings_window: Window,
    pub(crate) focused: Option<XWindowId>,
    pub(crate) resources: bool,
    pub(crate) events: Vec<XwmEvent>,
}

#[derive(Debug)]
pub(crate) struct Io<'a> {
    pub(crate) link: &'a mut Link,
    pub(crate) names: &'a mut AtomNames,
    pub(crate) pending: &'a mut Pending,
    pub(crate) poller: &'a mut Poller,
    pub(crate) events: &'a mut Vec<XwmEvent>,
}

impl Xwm {
    pub fn start(connection: UnixStream) -> Result<Self> {
        let (mut wire, setup) = Wire::connect(connection)?;
        let root = setup
            .roots
            .first()
            .ok_or_else(|| Error::Handshake("Xwayland offered no screen".into()))?
            .root;
        let interned = intern_atoms(&mut wire)?;
        let atoms = Atoms::from_interned(interned.iter().copied());
        let resources = negotiate_extensions(&mut wire, root)?;
        let mut link = Link::new(
            wire,
            atoms,
            root,
            setup.resource_id_base,
            setup.resource_id_mask,
        );
        become_manager(&mut link)?;
        let selections = Selections::new(&mut link)?;
        let dnd = Dnd::new(&mut link)?;
        let xsettings_window = announce_settings(&mut link)?;
        let poller = Poller::new(link.wire.as_fd()).map_err(Error::Poll)?;
        let mut xwm = Self {
            link,
            poller,
            names: AtomNames::new(&interned),
            pending: Pending::default(),
            windows: FxHashMap::default(),
            dirty: Vec::new(),
            epochs: 0,
            deferred: None,
            stacking: Stacking::default(),
            configures: Configures::default(),
            selections,
            dnd,
            xsettings: XSettings::default(),
            xsettings_window,
            focused: None,
            resources,
            events: Vec::new(),
        };
        xwm.publish_settings()?;
        xwm.flush()?;
        xwm.poller.wake();
        Ok(xwm)
    }

    #[must_use]
    pub fn window(&self, window: XWindowId) -> Option<&XWindow> {
        self.windows.get(&window).map(|record| &record.window)
    }

    pub fn windows(&self) -> impl Iterator<Item = &XWindow> {
        self.windows.values().map(|record| &record.window)
    }

    #[must_use]
    pub fn window_for_serial(&self, serial: u64) -> Option<XWindowId> {
        self.windows()
            .find(|window| window.surface_serial == Some(serial))
            .map(|window| window.id)
    }

    #[must_use]
    pub fn focused(&self) -> Option<XWindowId> {
        self.focused
    }

    #[must_use]
    pub fn stacking(&self) -> &[XWindowId] {
        self.stacking.order()
    }

    pub fn dispatch_into(&mut self, events: &mut Vec<XwmEvent>) -> Result<()> {
        let ready = self.poller.ready().map_err(Error::Poll)?;
        if ready.connection {
            self.link.wire.receive()?;
        }
        let mut sources = self.poller.take_sources();
        for source in sources.drain(..) {
            let (selections, mut io) = self.split();
            selections.ready(&mut io, source).or_else(tolerate)?;
        }
        self.poller.return_sources(sources);
        if ready.timer {
            let (selections, mut io) = self.split();
            selections
                .expire(&mut io, Instant::now())
                .or_else(tolerate)?;
        }
        self.process()?;
        self.poller
            .arm(self.selections.deadline())
            .map_err(Error::Poll)?;
        events.append(&mut self.events);
        Ok(())
    }

    pub fn flush(&mut self) -> Result<()> {
        let blocked = self.link.wire.flush()?;
        self.poller
            .want_writable(self.link.wire.as_fd(), blocked)
            .map_err(Error::Poll)?;
        if !self.events.is_empty() {
            self.poller.wake();
        }
        Ok(())
    }

    pub(crate) fn split(&mut self) -> (&mut Selections, Io<'_>) {
        (
            &mut self.selections,
            Io {
                link: &mut self.link,
                names: &mut self.names,
                pending: &mut self.pending,
                poller: &mut self.poller,
                events: &mut self.events,
            },
        )
    }

    pub(crate) fn tracked(&self, window: XWindowId) -> Result<&XWindow> {
        self.window(window).ok_or(Error::UnknownWindow(window))
    }

    pub(crate) fn managed(&self, window: XWindowId) -> Result<&XWindow> {
        let tracked = self.tracked(window)?;
        if tracked.override_redirect {
            return Err(Error::OverrideRedirect(window));
        }
        Ok(tracked)
    }

    pub(crate) fn publish_settings(&mut self) -> Result<()> {
        let encoded = self.xsettings.encode();
        let property = self.link.atoms._XSETTINGS_SETTINGS;
        self.link
            .set_bytes(self.xsettings_window, property, property, &encoded)?;
        Ok(())
    }

    fn process(&mut self) -> Result<()> {
        loop {
            if let Some((event, sequence)) = self.deferred.take() {
                self.answer_through(SequenceNumber::MAX)?;
                if self.stalled_by(&event) {
                    self.deferred = Some((event, sequence));
                    break;
                }
                self.handle(event, sequence).or_else(tolerate)?;
                continue;
            }
            let Some((raw, sequence)) = self.link.wire.event() else {
                self.answer_through(SequenceNumber::MAX)?;
                break;
            };
            self.answer_through(sequence)?;
            let event = match self.link.wire.parse_event(&raw) {
                Ok(event) => event,
                Err(error) => {
                    tracing::debug!("failed to parse Xwayland event: {error}");
                    continue;
                }
            };
            if self.stalled_by(&event) {
                self.send_reads()?;
                self.deferred = Some((event, sequence));
                continue;
            }
            self.handle(event, sequence).or_else(tolerate)?;
        }
        self.send_reads()?;
        Ok(())
    }

    fn stalled_by(&self, event: &Event) -> bool {
        let subjects = match event {
            Event::MapRequest(request) => [Some(request.window), None],
            Event::MapNotify(mapped) => [Some(mapped.window), None],
            Event::ClientMessage(message) => [Some(message.window), None],
            Event::ConfigureRequest(request) => [Some(request.window), None],
            Event::FocusIn(focus) => [Some(focus.event), self.focused.map(XWindowId::raw)],
            _ => return false,
        };
        subjects.into_iter().flatten().any(|raw| {
            self.windows
                .get(&XWindowId::new(raw))
                .is_some_and(|record| !record.settled())
        })
    }

    fn answer_through(&mut self, limit: SequenceNumber) -> Result<()> {
        while let Some(sequence) = self.pending.next().filter(|sequence| *sequence <= limit) {
            let Some(reply) = self.link.wire.reply(sequence)? else {
                break;
            };
            let Some((awaiting, then)) = self.pending.take() else {
                break;
            };
            self.handle_reply(awaiting, reply).or_else(tolerate)?;
            for resume in then {
                self.resume(resume).or_else(tolerate)?;
            }
        }
        Ok(())
    }

    fn handle_reply(&mut self, awaiting: ExpectedReply, reply: Reply) -> Outcome {
        match awaiting {
            ExpectedReply::Property {
                window,
                epoch,
                field,
                quiet,
            } => self.property_read(window, epoch, field, quiet, reply),
            ExpectedReply::Pid { window, epoch } => self.pid_read(window, epoch, reply),
            ExpectedReply::Geometry { window, epoch } => self.geometry_read(window, epoch, reply),
            ExpectedReply::AtomName { atom } => self.names.named(atom, reply),
            ExpectedReply::Atom { name } => self.names.interned(name, reply),
            ExpectedReply::Targets { kind, generation } => {
                let (selections, mut io) = self.split();
                selections.targets_read(&mut io, kind, generation, reply)
            }
            ExpectedReply::Chunk { source } => {
                let (selections, mut io) = self.split();
                selections.chunk_arrived(&mut io, source, reply)
            }
            ExpectedReply::DragTypes { source } => self.drag_types_read(source, reply),
            ExpectedReply::Probe {
                attempt,
                window,
                field,
            } => self.probe_read(attempt, window, field, reply),
        }
    }

    fn resume(&mut self, resume: Resume) -> Outcome {
        match resume {
            Resume::Targets {
                kind,
                generation,
                atoms,
            } => {
                let (selections, mut io) = self.split();
                selections.targets_named(&mut io, kind, generation, &atoms);
                Ok(())
            }
            Resume::DragTypes { source, atoms } => {
                self.drag_types_named(source, &atoms);
                Ok(())
            }
            Resume::Request { request, access } => {
                let (selections, mut io) = self.split();
                selections.requested(&mut io, &request, access)
            }
            Resume::DragEnter { attempt } => self.continue_drag(attempt),
        }
    }
}

impl AsFd for Xwm {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.poller.as_fd()
    }
}

pub(crate) fn tolerate(fault: Fault) -> Result<()> {
    match fault {
        Fault::Refused(error) => {
            tracing::debug!("Xwayland rejected a request: {}", describe(&error));
            Ok(())
        }
        fault => Err(fault.into()),
    }
}

fn intern_atoms(wire: &mut Wire) -> Result<Vec<u32>> {
    let requested = interned_up_front()
        .map(|name| {
            wire.ask(InternAtomRequest {
                only_if_exists: false,
                name: Cow::Borrowed(name.as_bytes()),
            })
        })
        .collect::<Outcome<Vec<_>>>()?;
    let queried = EXTENSIONS
        .iter()
        .map(|name| {
            wire.ask(QueryExtensionRequest {
                name: Cow::Borrowed(name.as_bytes()),
            })
        })
        .collect::<Outcome<Vec<_>>>()?;
    let mut interned = Vec::with_capacity(requested.len());
    for sequence in requested {
        let reply: InternAtomReply = parse(&wire.wait(sequence)?.map_err(Fault::Refused)?)?;
        interned.push(reply.atom);
    }
    for (name, sequence) in EXTENSIONS.into_iter().zip(queried) {
        let reply: QueryExtensionReply = parse(&wire.wait(sequence)?.map_err(Fault::Refused)?)?;
        if reply.present {
            wire.learn_extension(
                name,
                ExtensionInformation {
                    major_opcode: reply.major_opcode,
                    first_event: reply.first_event,
                    first_error: reply.first_error,
                },
            );
        }
    }
    Ok(interned)
}

fn negotiate_extensions(wire: &mut Wire, root: Window) -> Result<bool> {
    for required in [xfixes::X11_EXTENSION_NAME, composite::X11_EXTENSION_NAME] {
        if !wire.has_extension(required) {
            return Err(Error::MissingExtension(required));
        }
    }
    let big = wire
        .has_extension(bigreq::X11_EXTENSION_NAME)
        .then(|| wire.ask(bigreq::EnableRequest))
        .transpose()?;
    let xfixes = wire.ask(xfixes::QueryVersionRequest {
        client_major_version: 5,
        client_minor_version: 0,
    })?;
    let composite = wire.ask(composite::QueryVersionRequest {
        client_major_version: 0,
        client_minor_version: 4,
    })?;
    let resources = wire
        .has_extension(res::X11_EXTENSION_NAME)
        .then(|| {
            wire.ask(res::QueryVersionRequest {
                client_major: 1,
                client_minor: 2,
            })
        })
        .transpose()?;
    let refusal = wire.refusal(ChangeWindowAttributesRequest {
        window: root,
        value_list: Cow::Owned(ChangeWindowAttributesAux::new().event_mask(
            EventMask::SUBSTRUCTURE_REDIRECT
                | EventMask::SUBSTRUCTURE_NOTIFY
                | EventMask::PROPERTY_CHANGE,
        )),
    })?;
    if let Some(sequence) = big {
        let reply: bigreq::EnableReply = parse(&wire.wait(sequence)?.map_err(Fault::Refused)?)?;
        wire.allow_request_words(reply.maximum_request_length);
    }
    for sequence in [xfixes, composite] {
        wire.wait(sequence)?.map_err(Fault::Refused)?;
    }
    let resources = match resources {
        Some(sequence) => wire.wait(sequence)?.is_ok(),
        None => false,
    };
    match refusal {
        None => Ok(resources),
        Some(error) if error.error_kind == ErrorKind::Access => Err(Error::WindowManagerRunning),
        Some(error) => Err(Error::Refused(describe(&error))),
    }
}

fn become_manager(link: &mut Link) -> Result<()> {
    let root = link.root;
    let atoms = link.atoms;
    link.wire.send(composite::RedirectSubwindowsRequest {
        window: root,
        update: composite::Redirect::MANUAL,
    })?;
    set_root_cursor(link)?;

    let check = link.helper_window(EventMask::NO_EVENT)?;
    link.set_windows(root, atoms._NET_SUPPORTING_WM_CHECK, &[check])?;
    link.set_windows(check, atoms._NET_SUPPORTING_WM_CHECK, &[check])?;
    link.set_bytes(check, atoms._NET_WM_NAME, atoms.UTF8_STRING, MANAGER_NAME)?;
    link.set_atoms(root, atoms._NET_SUPPORTED, &supported(&atoms))?;
    link.set_windows(root, atoms._NET_CLIENT_LIST, &[])?;
    link.set_windows(root, atoms._NET_CLIENT_LIST_STACKING, &[])?;
    link.set_windows(root, atoms._NET_ACTIVE_WINDOW, &[NONE])?;
    for selection in [atoms.WM_S0, atoms._NET_WM_CM_S0] {
        link.wire.send(SetSelectionOwnerRequest {
            owner: check,
            selection,
            time: CURRENT_TIME,
        })?;
        announce(link, selection, check)?;
    }
    Ok(())
}

fn set_root_cursor(link: &mut Link) -> Outcome {
    let font = link.wire.generate_id()?;
    link.wire.send(OpenFontRequest {
        fid: font,
        name: Cow::Borrowed(b"cursor"),
    })?;
    let cursor = link.wire.generate_id()?;
    link.wire.send(CreateGlyphCursorRequest {
        cid: cursor,
        source_font: font,
        mask_font: font,
        source_char: LEFT_POINTER_GLYPH,
        mask_char: LEFT_POINTER_GLYPH + 1,
        fore_red: 0,
        fore_green: 0,
        fore_blue: 0,
        back_red: u16::MAX,
        back_green: u16::MAX,
        back_blue: u16::MAX,
    })?;
    link.wire.send(ChangeWindowAttributesRequest {
        window: link.root,
        value_list: Cow::Owned(ChangeWindowAttributesAux::new().cursor(cursor)),
    })?;
    link.wire.send(FreeCursorRequest { cursor })?;
    link.wire.send(CloseFontRequest { font })?;
    Ok(())
}

fn announce_settings(link: &mut Link) -> Result<Window> {
    let window = link.helper_window(EventMask::NO_EVENT)?;
    let selection = link.atoms._XSETTINGS_S0;
    link.wire.send(SetSelectionOwnerRequest {
        owner: window,
        selection,
        time: CURRENT_TIME,
    })?;
    announce(link, selection, window)?;
    Ok(window)
}

fn announce(link: &mut Link, selection: u32, owner: Window) -> Outcome {
    let message = ClientMessageEvent::new(
        32,
        link.root,
        link.atoms.MANAGER,
        [CURRENT_TIME, selection, owner, 0, 0],
    );
    let root = link.root;
    link.send_event(root, EventMask::STRUCTURE_NOTIFY, message)
}

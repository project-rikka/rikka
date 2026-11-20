use std::os::fd::OwnedFd;
use std::rc::Rc;
use std::time::Instant;

use rustix::fs::{OFlags, fcntl_getfl, fcntl_setfl};
use rustix::pipe::{PipeFlags, pipe_with};
use x11rb_protocol::SequenceNumber;
use x11rb_protocol::protocol::xfixes::{
    SelectSelectionInputRequest, SelectionEventMask, SelectionNotifyEvent as OwnerChange,
};
use x11rb_protocol::protocol::xproto::{
    Atom, AtomEnum, ConvertSelectionRequest, DeletePropertyRequest, DestroyWindowRequest,
    EventMask, GetPropertyReply, GetPropertyRequest, Property, PropertyNotifyEvent,
    SelectionNotifyEvent, SelectionRequestEvent, SetSelectionOwnerRequest, Timestamp, Window,
};

use crate::error::{Error, Result};
use crate::event::XwmEvent;
use crate::fault::{Fault, Outcome, describe};
use crate::link::{Link, SelectionReply};
use crate::pending::{ExpectedReply, Resume};
use crate::poller::{Poller, SourceId};
use crate::selection::incoming::IncomingTransfer;
use crate::selection::kind::SelectionKind;
use crate::selection::mime::{
    MimeList, advertised_targets, mime_type_for_target, offered_mime_types, target_for_mime_type,
};
use crate::selection::outgoing::OutgoingTransfer;
use crate::wire::{CURRENT_TIME, NONE, Reply, parse};
use crate::xwm::Io;

const TARGET_LIMIT: u32 = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Owner {
    Nobody,
    Bridge,
    Client(Window),
}

#[derive(Debug, Clone)]
pub(crate) struct Target {
    pub(crate) atom: Atom,
    pub(crate) name: Rc<str>,
}

impl AsRef<str> for Target {
    fn as_ref(&self) -> &str {
        &self.name
    }
}

#[derive(Debug)]
struct Slot {
    kind: SelectionKind,
    atom: Atom,
    window: Window,
    owner: Owner,
    acquired: Timestamp,
    generation: u64,
    offer: Option<Offer>,
    targets: Vec<Target>,
}

#[derive(Debug)]
struct Offer {
    mime_types: MimeList,
    advertised: Option<Box<[Atom]>>,
}

const BUILT_IN_TARGETS: usize = 2;

impl Slot {
    fn offered(&self) -> &[Box<str>] {
        self.offer
            .as_ref()
            .map_or(&[][..], |offer| &*offer.mime_types)
    }

    fn advertised(&self) -> &[Atom] {
        self.offer
            .as_ref()
            .and_then(|offer| offer.advertised.as_deref())
            .unwrap_or_default()
    }
}

#[derive(Debug)]
pub(crate) struct Selections {
    slots: [Slot; 3],
    outgoing: Vec<OutgoingTransfer>,
    incoming: Vec<IncomingTransfer>,
    spare: Vec<Window>,
}

impl Selections {
    pub(crate) fn new(link: &mut Link) -> Outcome<Self> {
        let atoms = link.atoms;
        let mut slot = |kind: SelectionKind, atom: Atom| -> Outcome<Slot> {
            let window = link.helper_window(EventMask::PROPERTY_CHANGE)?;
            link.wire.send(SelectSelectionInputRequest {
                window,
                selection: atom,
                event_mask: SelectionEventMask::SET_SELECTION_OWNER
                    | SelectionEventMask::SELECTION_WINDOW_DESTROY
                    | SelectionEventMask::SELECTION_CLIENT_CLOSE,
            })?;
            Ok(Slot {
                kind,
                atom,
                window,
                owner: Owner::Nobody,
                acquired: CURRENT_TIME,
                generation: 0,
                offer: None,
                targets: Vec::new(),
            })
        };
        let clipboard = slot(SelectionKind::Clipboard, atoms.CLIPBOARD)?;
        let primary = slot(SelectionKind::Primary, atoms.PRIMARY)?;
        let drag = slot(SelectionKind::Drag, atoms.XdndSelection)?;
        link.wire.send(SetSelectionOwnerRequest {
            owner: clipboard.window,
            selection: atoms.CLIPBOARD_MANAGER,
            time: CURRENT_TIME,
        })?;
        Ok(Self {
            slots: [clipboard, primary, drag],
            outgoing: Vec::new(),
            incoming: Vec::new(),
            spare: Vec::new(),
        })
    }

    pub(crate) fn offer(
        &mut self,
        io: &mut Io<'_>,
        kind: SelectionKind,
        mime_types: MimeList,
    ) -> Outcome {
        if mime_types.is_empty() {
            return self.withdraw(io.link, kind);
        }
        self.slot_mut(kind).offer = Some(Offer {
            mime_types,
            advertised: None,
        });
        self.advertise(io, kind)?;
        let slot = self.slot_mut(kind);
        slot.targets.clear();
        slot.owner = Owner::Bridge;
        io.link.wire.send(SetSelectionOwnerRequest {
            owner: slot.window,
            selection: slot.atom,
            time: CURRENT_TIME,
        })?;
        Ok(())
    }

    pub(crate) fn advertise(
        &mut self,
        io: &mut Io<'_>,
        kind: SelectionKind,
    ) -> Outcome<Option<SequenceNumber>> {
        let Some(offer) = self
            .slot_mut(kind)
            .offer
            .as_mut()
            .filter(|offer| offer.advertised.is_none())
        else {
            return Ok(None);
        };
        let names = advertised_targets(&offer.mime_types);
        if let Some(sequence) =
            io.names
                .learn_atoms(&mut io.link.wire, io.pending, names.iter().copied())?
        {
            return Ok(Some(sequence));
        }
        let atoms = io.link.atoms;
        let built_in: [Atom; BUILT_IN_TARGETS] = [atoms.TARGETS, atoms.TIMESTAMP];
        offer.advertised = Some(
            built_in
                .into_iter()
                .chain(names.iter().filter_map(|name| io.names.atom(name)))
                .collect(),
        );
        Ok(None)
    }

    pub(crate) fn advertised_types(&self, kind: SelectionKind) -> &[Atom] {
        self.slot(kind)
            .advertised()
            .get(BUILT_IN_TARGETS..)
            .unwrap_or_default()
    }

    pub(crate) fn withdraw(&mut self, link: &mut Link, kind: SelectionKind) -> Outcome {
        let slot = self.slot_mut(kind);
        slot.offer = None;
        if slot.owner == Owner::Bridge {
            slot.owner = Owner::Nobody;
            link.wire.send(SetSelectionOwnerRequest {
                owner: NONE,
                selection: slot.atom,
                time: slot.acquired,
            })?;
        }
        Ok(())
    }

    pub(crate) fn fetch(
        &mut self,
        io: &mut Io<'_>,
        kind: SelectionKind,
        mime_type: &str,
        fd: OwnedFd,
    ) -> Result<()> {
        let slot = self.slot(kind);
        let target = matches!(slot.owner, Owner::Client(_))
            .then(|| target_for_mime_type(mime_type, &slot.targets))
            .flatten()
            .ok_or_else(|| Error::NotOffered {
                kind,
                mime_type: mime_type.to_owned(),
            })?
            .atom;
        let selection = slot.atom;
        let flags = fcntl_getfl(&fd).map_err(|error| Error::Transfer(error.into()))?;
        fcntl_setfl(&fd, flags | OFlags::NONBLOCK)
            .map_err(|error| Error::Transfer(error.into()))?;
        let window = match self.spare.pop() {
            Some(window) => window,
            None => io.link.helper_window(EventMask::PROPERTY_CHANGE)?,
        };
        let transfer = match IncomingTransfer::new(kind, window, fd, io.poller) {
            Ok(transfer) => transfer,
            Err(error) => {
                self.spare.push(window);
                return Err(Error::Transfer(error));
            }
        };
        io.link.wire.send(ConvertSelectionRequest {
            requestor: window,
            selection,
            target,
            property: io.link.atoms._WL_SELECTION,
            time: CURRENT_TIME,
        })?;
        self.incoming.push(transfer);
        Ok(())
    }

    pub(crate) fn owner_changed(&mut self, io: &mut Io<'_>, change: &OwnerChange) -> Outcome {
        let Some(slot) = self.slot_for(change.selection) else {
            return Ok(());
        };
        slot.generation += 1;
        if change.owner == slot.window {
            slot.owner = Owner::Bridge;
            slot.acquired = change.selection_timestamp;
            return Ok(());
        }
        slot.targets.clear();
        if change.owner == NONE {
            let previous = std::mem::replace(&mut slot.owner, Owner::Nobody);
            if matches!(previous, Owner::Client(_)) {
                io.events.push(match slot.kind {
                    SelectionKind::Drag => XwmEvent::X11DragOwner { owned: false },
                    kind => XwmEvent::SelectionCleared { kind },
                });
            }
            return Ok(());
        }
        slot.owner = Owner::Client(change.owner);
        slot.offer = None;
        if slot.kind == SelectionKind::Drag {
            io.events.push(XwmEvent::X11DragOwner { owned: true });
            return Ok(());
        }
        io.link.wire.send(ConvertSelectionRequest {
            requestor: slot.window,
            selection: slot.atom,
            target: io.link.atoms.TARGETS,
            property: io.link.atoms._WL_SELECTION,
            time: change.timestamp,
        })?;
        Ok(())
    }

    pub(crate) fn window(&self, kind: SelectionKind) -> Window {
        self.slot(kind).window
    }

    pub(crate) fn adopt_targets(&mut self, kind: SelectionKind, targets: Vec<Target>) {
        self.slot_mut(kind).targets = targets;
    }

    pub(crate) fn lost(&mut self, owner: Window, selection: Atom) {
        if let Some(slot) = self.slot_for(selection)
            && slot.window == owner
            && slot.owner == Owner::Bridge
        {
            slot.owner = Owner::Nobody;
            slot.offer = None;
        }
    }

    pub(crate) fn converted(&mut self, io: &mut Io<'_>, notify: &SelectionNotifyEvent) -> Outcome {
        let delivered = notify.property != NONE;
        if let Some(index) = self
            .slots
            .iter()
            .position(|slot| slot.window == notify.requestor)
        {
            if notify.target == io.link.atoms.TARGETS {
                self.targets_arrived(io, index, delivered)?;
            }
            return Ok(());
        }
        let Some(index) = self
            .incoming
            .iter()
            .position(|transfer| transfer.window == notify.requestor)
        else {
            return Ok(());
        };
        let finished = self.incoming[index].converted(io, delivered);
        self.settle_incoming(io, index, finished)
    }

    pub(crate) fn requested(
        &mut self,
        io: &mut Io<'_>,
        request: &SelectionRequestEvent,
        access: Access,
    ) -> Outcome {
        let reply = SelectionReply {
            requestor: request.requestor,
            selection: request.selection,
            target: request.target,
            property: if request.property == NONE {
                request.target
            } else {
                request.property
            },
            time: request.time,
        };
        let Some(slot) = self
            .slots
            .iter()
            .find(|slot| slot.atom == request.selection)
        else {
            return io.link.reply_selection(reply, false);
        };
        let stale = request.time != CURRENT_TIME && request.time < slot.acquired;
        if slot.owner != Owner::Bridge
            || request.owner != slot.window
            || request.requestor == slot.window
            || stale
            || !(access.allowed || slot.kind == SelectionKind::Drag)
        {
            return io.link.reply_selection(reply, false);
        }
        let atoms = io.link.atoms;
        let resume = || Resume::Request {
            request: *request,
            access,
        };
        if request.target == atoms.TARGETS {
            let kind = slot.kind;
            if let Some(sequence) = self.advertise(io, kind)? {
                io.pending.after(sequence, resume());
                return Ok(());
            }
            io.link.set_atoms(
                reply.requestor,
                reply.property,
                self.slot(kind).advertised(),
            )?;
            return io.link.reply_selection(reply, true);
        }
        if request.target == atoms.TIMESTAMP {
            io.link.set_words(
                reply.requestor,
                reply.property,
                AtomEnum::INTEGER.into(),
                &[slot.acquired],
            )?;
            return io.link.reply_selection(reply, true);
        }
        if request.target == atoms.DELETE {
            return io.link.reply_selection(reply, true);
        }
        if let Some(sequence) =
            io.names
                .learn_names(&mut io.link.wire, io.pending, &[request.target])?
        {
            io.pending.after(sequence, resume());
            return Ok(());
        }
        let mime_type = io
            .names
            .name(request.target)
            .and_then(|target| mime_type_for_target(target, slot.offered()));
        let Some(mime_type) = mime_type else {
            return io.link.reply_selection(reply, false);
        };
        let kind = slot.kind;
        self.replace_stale(io, reply)?;
        let (read, write) = match open_pipe() {
            Ok(ends) => ends,
            Err(error) => {
                tracing::warn!("failed to serve X11 selection request: {error}");
                return io.link.reply_selection(reply, false);
            }
        };
        let chunk = io.link.transfer_chunk();
        match OutgoingTransfer::start(kind, reply, access.requestor_events, read, io.poller, chunk)
        {
            Ok(transfer) => self.outgoing.push(transfer),
            Err(error) => {
                tracing::warn!("failed to poll X11 selection request: {error}");
                return io.link.reply_selection(reply, false);
            }
        }
        io.events.push(XwmEvent::SendSelection {
            kind,
            mime_type,
            fd: write,
        });
        Ok(())
    }

    pub(crate) fn property_changed(
        &mut self,
        io: &mut Io<'_>,
        change: &PropertyNotifyEvent,
    ) -> Outcome {
        if change.state == Property::NEW_VALUE
            && change.atom == io.link.atoms._WL_SELECTION
            && let Some(index) = self
                .incoming
                .iter()
                .position(|transfer| transfer.window == change.window)
        {
            let finished = self.incoming[index].property_arrived(io);
            self.settle_incoming(io, index, finished)?;
        }
        if change.state == Property::DELETE
            && let Some(index) = self
                .outgoing
                .iter()
                .position(|transfer| transfer.serves(change.window, change.atom))
        {
            let finished = self.outgoing[index].deleted(io.link, io.poller);
            self.settle_outgoing(io, index, finished)?;
        }
        Ok(())
    }

    pub(crate) fn ready(&mut self, io: &mut Io<'_>, source: SourceId) -> Outcome {
        if let Some(index) = self
            .outgoing
            .iter()
            .position(|transfer| transfer.source == source)
        {
            let finished = self.outgoing[index].readable(io.link, io.poller);
            return self.settle_outgoing(io, index, finished);
        }
        if let Some(index) = self
            .incoming
            .iter()
            .position(|transfer| transfer.source == source)
        {
            let finished = self.incoming[index].writable(io);
            return self.settle_incoming(io, index, finished);
        }
        Ok(())
    }

    pub(crate) fn chunk_arrived(
        &mut self,
        io: &mut Io<'_>,
        source: SourceId,
        reply: Reply,
    ) -> Outcome {
        let Some(index) = self
            .incoming
            .iter()
            .position(|transfer| transfer.source == source)
        else {
            return Ok(());
        };
        let finished = self.incoming[index].arrived(io, reply);
        self.settle_incoming(io, index, finished)
    }

    pub(crate) fn requestor_gone(&mut self, poller: &Poller, window: Window) {
        self.outgoing.retain_mut(|transfer| {
            let gone = transfer.reply.requestor == window;
            if gone {
                transfer.release(poller);
            }
            !gone
        });
    }

    pub(crate) fn expire(&mut self, io: &mut Io<'_>, now: Instant) -> Outcome {
        while let Some(index) = self
            .outgoing
            .iter()
            .position(|transfer| transfer.deadline <= now)
        {
            let mut transfer = self.outgoing.swap_remove(index);
            tracing::debug!("{} selection transfer to X11 timed out", transfer.kind);
            transfer.abort(io.link, io.poller)?;
        }
        while let Some(index) = self
            .incoming
            .iter()
            .position(|transfer| transfer.deadline <= now)
        {
            tracing::debug!(
                "{} selection transfer from X11 timed out",
                self.incoming[index].kind
            );
            self.settle_incoming(io, index, Ok(true))?;
        }
        Ok(())
    }

    pub(crate) fn deadline(&self) -> Option<Instant> {
        self.outgoing
            .iter()
            .map(|transfer| transfer.deadline)
            .chain(self.incoming.iter().map(|transfer| transfer.deadline))
            .min()
    }

    pub(crate) fn targets_read(
        &mut self,
        io: &mut Io<'_>,
        kind: SelectionKind,
        generation: u64,
        reply: Reply,
    ) -> Outcome {
        if self.slot(kind).generation != generation {
            return Ok(());
        }
        let reply: GetPropertyReply = match reply {
            Ok(packet) => parse(&packet)?,
            Err(error) => {
                tracing::debug!("X11 {kind} TARGETS request failed: {}", describe(&error));
                return Ok(());
            }
        };
        let atoms: Vec<Atom> = reply.value32().map(Iterator::collect).unwrap_or_default();
        match io
            .names
            .learn_names(&mut io.link.wire, io.pending, &atoms)?
        {
            Some(sequence) => io.pending.after(
                sequence,
                Resume::Targets {
                    kind,
                    generation,
                    atoms,
                },
            ),
            None => self.targets_named(io, kind, generation, &atoms),
        }
        Ok(())
    }

    pub(crate) fn targets_named(
        &mut self,
        io: &mut Io<'_>,
        kind: SelectionKind,
        generation: u64,
        atoms: &[Atom],
    ) {
        let slot = self.slot_mut(kind);
        if slot.generation != generation || !matches!(slot.owner, Owner::Client(_)) {
            return;
        }
        slot.targets = targets_of(io, atoms);
        io.events.push(XwmEvent::SelectionOffered {
            kind,
            mime_types: offered_mime_types(&slot.targets),
        });
    }

    fn targets_arrived(&mut self, io: &mut Io<'_>, index: usize, delivered: bool) -> Outcome {
        let slot = &self.slots[index];
        if !matches!(slot.owner, Owner::Client(_)) {
            return Ok(());
        }
        if !delivered {
            io.events
                .push(XwmEvent::SelectionCleared { kind: slot.kind });
            return Ok(());
        }
        let sequence = io.link.wire.ask(GetPropertyRequest {
            delete: true,
            window: slot.window,
            property: io.link.atoms._WL_SELECTION,
            type_: AtomEnum::ATOM.into(),
            long_offset: 0,
            long_length: TARGET_LIMIT,
        })?;
        io.pending.expect(
            sequence,
            ExpectedReply::Targets {
                kind: slot.kind,
                generation: slot.generation,
            },
        );
        Ok(())
    }

    fn replace_stale(&mut self, io: &mut Io<'_>, reply: SelectionReply) -> Outcome {
        while let Some(index) = self
            .outgoing
            .iter()
            .position(|transfer| transfer.serves(reply.requestor, reply.property))
        {
            let mut transfer = self.outgoing.swap_remove(index);
            transfer.abort(io.link, io.poller)?;
        }
        Ok(())
    }

    fn settle_outgoing(
        &mut self,
        io: &mut Io<'_>,
        index: usize,
        finished: Outcome<bool>,
    ) -> Outcome {
        match finished {
            Ok(false) => Ok(()),
            Ok(true) => {
                self.outgoing.swap_remove(index).release(io.poller);
                Ok(())
            }
            Err(Fault::Refused(error)) => {
                tracing::debug!("selection transfer to X11 failed: {}", describe(&error));
                self.outgoing.swap_remove(index).abort(io.link, io.poller)
            }
            Err(fault) => Err(fault),
        }
    }

    fn settle_incoming(
        &mut self,
        io: &mut Io<'_>,
        index: usize,
        finished: Outcome<bool>,
    ) -> Outcome {
        let done = match finished {
            Ok(done) => done,
            Err(Fault::Refused(error)) => {
                tracing::debug!("selection transfer from X11 failed: {}", describe(&error));
                true
            }
            Err(fault) => return Err(fault),
        };
        if done {
            let mut transfer = self.incoming.swap_remove(index);
            transfer.release(io.poller);
            if transfer.reusable() {
                io.link.wire.send(DeletePropertyRequest {
                    window: transfer.window,
                    property: io.link.atoms._WL_SELECTION,
                })?;
                self.spare.push(transfer.window);
            } else {
                io.link.wire.send(DestroyWindowRequest {
                    window: transfer.window,
                })?;
            }
        }
        Ok(())
    }

    fn slot(&self, kind: SelectionKind) -> &Slot {
        &self.slots[kind as usize]
    }

    fn slot_mut(&mut self, kind: SelectionKind) -> &mut Slot {
        &mut self.slots[kind as usize]
    }

    fn slot_for(&mut self, selection: Atom) -> Option<&mut Slot> {
        self.slots.iter_mut().find(|slot| slot.atom == selection)
    }
}

pub(crate) fn targets_of(io: &Io<'_>, atoms: &[Atom]) -> Vec<Target> {
    atoms
        .iter()
        .filter_map(|atom| {
            io.names.name(*atom).map(|name| Target {
                atom: *atom,
                name: Rc::clone(name),
            })
        })
        .collect()
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Access {
    pub(crate) allowed: bool,
    pub(crate) requestor_events: EventMask,
}

fn open_pipe() -> std::io::Result<(OwnedFd, OwnedFd)> {
    let (read, write) = pipe_with(PipeFlags::CLOEXEC)?;
    let flags = fcntl_getfl(&read)?;
    fcntl_setfl(&read, flags | OFlags::NONBLOCK)?;
    Ok((read, write))
}

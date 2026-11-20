use std::borrow::Cow;

use bitflags::bitflags;
use x11rb_protocol::protocol::res::{
    ClientIdMask, ClientIdSpec, QueryClientIdsReply, QueryClientIdsRequest,
};
use x11rb_protocol::protocol::xproto::{
    Atom, AtomEnum, GetPropertyReply, GetPropertyRequest, Window,
};

use crate::atoms::Atoms;
use crate::event::{WindowProperty, XwmEvent};
use crate::ewmh::{states_from, types_from};
use crate::fault::{Outcome, describe};
use crate::hints::{SizeHints, WmHints, decode_text, motif_decorated};
use crate::pending::ExpectedReply;
use crate::record::{Record, current};
use crate::window::{Protocols, XWindowId};
use crate::wire::{NONE, Reply, parse};
use crate::xwm::Xwm;

const PROPERTY_LIMIT: u32 = 2048;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Field {
    NetName,
    Name,
    Class,
    Hints,
    NormalHints,
    Protocols,
    TransientFor,
    WindowType,
    Motif,
    Pid,
    StartupId,
    Opacity,
    States,
    XdndAware,
    XdndProxy,
}

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct Fields: u16 {
        const NET_NAME = 1;
        const NAME = 1 << 1;
        const CLASS = 1 << 2;
        const HINTS = 1 << 3;
        const NORMAL_HINTS = 1 << 4;
        const PROTOCOLS = 1 << 5;
        const TRANSIENT_FOR = 1 << 6;
        const WINDOW_TYPE = 1 << 7;
        const MOTIF = 1 << 8;
        const PID = 1 << 9;
        const STARTUP_ID = 1 << 10;
        const OPACITY = 1 << 11;
        const STATES = 1 << 12;
        const XDND_AWARE = 1 << 13;
        const XDND_PROXY = 1 << 14;
    }
}

impl Field {
    const ALL: [Self; 15] = [
        Self::NetName,
        Self::Name,
        Self::Class,
        Self::Hints,
        Self::NormalHints,
        Self::Protocols,
        Self::TransientFor,
        Self::WindowType,
        Self::Motif,
        Self::Pid,
        Self::StartupId,
        Self::Opacity,
        Self::States,
        Self::XdndAware,
        Self::XdndProxy,
    ];

    fn bit(self) -> Fields {
        Fields::from_bits_truncate(1 << self as u16)
    }

    fn atom(self, atoms: &Atoms) -> Atom {
        match self {
            Self::NetName => atoms._NET_WM_NAME,
            Self::Name => AtomEnum::WM_NAME.into(),
            Self::Class => AtomEnum::WM_CLASS.into(),
            Self::Hints => AtomEnum::WM_HINTS.into(),
            Self::NormalHints => AtomEnum::WM_NORMAL_HINTS.into(),
            Self::Protocols => atoms.WM_PROTOCOLS,
            Self::TransientFor => AtomEnum::WM_TRANSIENT_FOR.into(),
            Self::WindowType => atoms._NET_WM_WINDOW_TYPE,
            Self::Motif => atoms._MOTIF_WM_HINTS,
            Self::Pid => atoms._NET_WM_PID,
            Self::StartupId => atoms._NET_STARTUP_ID,
            Self::Opacity => atoms._NET_WM_WINDOW_OPACITY,
            Self::States => atoms._NET_WM_STATE,
            Self::XdndAware => atoms.XdndAware,
            Self::XdndProxy => atoms.XdndProxy,
        }
    }

    fn from_atom(atom: Atom, atoms: &Atoms) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|field| field.atom(atoms) == atom)
    }

    pub(crate) fn request(self, window: Window, atoms: &Atoms) -> GetPropertyRequest {
        let (type_, length) = match self {
            Self::XdndAware => (AtomEnum::ANY.into(), 1),
            Self::XdndProxy => (AtomEnum::WINDOW.into(), 1),
            _ => (AtomEnum::ANY.into(), PROPERTY_LIMIT),
        };
        GetPropertyRequest {
            delete: false,
            window,
            property: self.atom(atoms),
            type_,
            long_offset: 0,
            long_length: length,
        }
    }
}

impl Xwm {
    pub(crate) fn read_all(&mut self, window: XWindowId) -> Outcome {
        let Some(record) = self.windows.get_mut(&window) else {
            return Ok(());
        };
        if self.resources {
            let sequence = self.link.wire.ask(QueryClientIdsRequest {
                specs: Cow::Owned(vec![ClientIdSpec {
                    client: window.raw(),
                    mask: ClientIdMask::LOCAL_CLIENT_PID,
                }]),
            })?;
            self.pending.expect(
                sequence,
                ExpectedReply::Pid {
                    window,
                    epoch: record.epoch,
                },
            );
            record.reading += 1;
        }
        self.request_fields(window, Fields::all(), true)
    }

    pub(crate) fn property_changed(&mut self, window: XWindowId, atom: Atom) {
        let Some(field) = Field::from_atom(atom, &self.link.atoms) else {
            return;
        };
        let Some(record) = self.windows.get_mut(&window) else {
            return;
        };
        let owned = match field {
            Field::States => record.window.mapped,
            Field::Pid => record.window.pid.is_some(),
            _ => false,
        };
        if owned {
            return;
        }
        if record.dirty.is_empty() {
            self.dirty.push(window);
        }
        record.dirty |= field.bit();
    }

    pub(crate) fn send_reads(&mut self) -> Outcome {
        let mut dirty = std::mem::take(&mut self.dirty);
        for window in dirty.drain(..) {
            let Some(record) = self.windows.get_mut(&window) else {
                continue;
            };
            let fields = std::mem::replace(&mut record.dirty, Fields::empty());
            self.request_fields(window, fields, false)?;
        }
        self.dirty = dirty;
        Ok(())
    }

    pub(crate) fn property_read(
        &mut self,
        window: XWindowId,
        epoch: u64,
        field: Field,
        quiet: bool,
        reply: Reply,
    ) -> Outcome {
        let Some(record) = current(&mut self.windows, window, epoch) else {
            return Ok(());
        };
        record.reading -= 1;
        let reply: GetPropertyReply = match reply {
            Ok(packet) => parse(&packet)?,
            Err(error) => {
                tracing::debug!("failed to read {window} properties: {}", describe(&error));
                return Ok(());
            }
        };
        let changed = apply(&self.link.atoms, record, field, &reply);
        if let Some(property) = changed.filter(|_| !quiet) {
            self.announce_change(window, property);
        }
        Ok(())
    }

    pub(crate) fn pid_read(&mut self, window: XWindowId, epoch: u64, reply: Reply) -> Outcome {
        let Some(record) = current(&mut self.windows, window, epoch) else {
            return Ok(());
        };
        record.reading -= 1;
        match reply {
            Ok(packet) => {
                let reply: QueryClientIdsReply = parse(&packet)?;
                if let Some(pid) = reply.ids.iter().find_map(|id| id.value.first().copied()) {
                    record.window.pid = Some(pid);
                }
            }
            Err(error) => {
                tracing::debug!("X-Resource query for {window} failed: {}", describe(&error))
            }
        }
        Ok(())
    }

    fn request_fields(&mut self, window: XWindowId, fields: Fields, quiet: bool) -> Outcome {
        let Some(record) = self.windows.get_mut(&window) else {
            return Ok(());
        };
        for field in Field::ALL {
            if !fields.contains(field.bit()) {
                continue;
            }
            let sequence = self
                .link
                .wire
                .ask(field.request(window.raw(), &self.link.atoms))?;
            self.pending.expect(
                sequence,
                ExpectedReply::Property {
                    window,
                    epoch: record.epoch,
                    field,
                    quiet,
                },
            );
            record.reading += 1;
        }
        Ok(())
    }

    fn announce_change(&mut self, window: XWindowId, property: WindowProperty) {
        let announced = self.events.iter().any(|event| {
            matches!(event, XwmEvent::PropertyChanged { window: known, property: kind }
                if *known == window && *kind == property)
        });
        if !announced {
            self.events
                .push(XwmEvent::PropertyChanged { window, property });
        }
    }
}

fn apply(
    atoms: &Atoms,
    record: &mut Record,
    field: Field,
    reply: &GetPropertyReply,
) -> Option<WindowProperty> {
    let present = reply.type_ != NONE;
    let words = || -> Vec<u32> { reply.value32().map(Iterator::collect).unwrap_or_default() };
    let text = || present.then(|| decode_text(reply.type_ == atoms.UTF8_STRING, &reply.value));
    let window = &mut record.window;
    let changed = match field {
        Field::NetName | Field::Name => {
            let title = text();
            match field {
                Field::NetName => record.titles.net = title,
                _ => record.titles.legacy = title,
            }
            let current = record.titles.current();
            let changed = window.title != current;
            if changed {
                current.clone_into(&mut window.title);
            }
            changed
        }
        Field::Class => {
            let mut parts = reply.value.split(|byte| *byte == 0);
            let instance = decode_text(false, parts.next().unwrap_or_default());
            let class = decode_text(false, parts.next().unwrap_or_default());
            let changed = window.instance != instance || window.class != class;
            window.instance = instance;
            window.class = class;
            changed
        }
        Field::Hints => replace(&mut window.hints, WmHints::parse(&words())),
        Field::NormalHints => replace(&mut window.size_hints, SizeHints::parse(&words())),
        Field::Protocols => replace(&mut window.protocols, protocols_from(atoms, words())),
        Field::TransientFor => {
            let own = window.id.raw();
            let parent = words()
                .first()
                .copied()
                .filter(|parent| *parent != NONE && *parent != own)
                .map(XWindowId::new);
            replace(&mut window.transient_for, parent)
        }
        Field::WindowType => replace(&mut window.declared_types, types_from(atoms, words())),
        Field::Motif => replace(&mut window.decorated, motif_decorated(&words())),
        Field::Pid => {
            window.pid.is_none() && {
                let pid = words().first().copied().filter(|pid| *pid != 0);
                replace(&mut window.pid, pid)
            }
        }
        Field::StartupId => replace(&mut window.startup_id, text()),
        Field::Opacity => {
            let opacity = words()
                .first()
                .map(|opacity| (f64::from(*opacity) / f64::from(u32::MAX)) as f32);
            replace(&mut window.opacity, opacity)
        }
        Field::States => replace(&mut window.states, states_from(atoms, words())),
        Field::XdndAware => {
            record.xdnd.version = words().first().copied();
            false
        }
        Field::XdndProxy => {
            record.xdnd.proxy = words().first().copied().filter(|proxy| *proxy != NONE);
            false
        }
    };
    if changed { property_of(field) } else { None }
}

fn replace<T: PartialEq>(slot: &mut T, value: T) -> bool {
    let changed = *slot != value;
    *slot = value;
    changed
}

fn protocols_from(atoms: &Atoms, words: Vec<u32>) -> Protocols {
    words
        .into_iter()
        .fold(Protocols::empty(), |protocols, atom| {
            protocols
                | match atom {
                    atom if atom == atoms.WM_TAKE_FOCUS => Protocols::TAKE_FOCUS,
                    atom if atom == atoms.WM_DELETE_WINDOW => Protocols::DELETE_WINDOW,
                    atom if atom == atoms._NET_WM_PING => Protocols::PING,
                    atom if atom == atoms._NET_WM_SYNC_REQUEST => Protocols::SYNC_REQUEST,
                    _ => Protocols::empty(),
                }
        })
}

fn property_of(field: Field) -> Option<WindowProperty> {
    Some(match field {
        Field::NetName | Field::Name => WindowProperty::Title,
        Field::Class => WindowProperty::Class,
        Field::Hints => WindowProperty::Hints,
        Field::NormalHints => WindowProperty::SizeHints,
        Field::Protocols => WindowProperty::Protocols,
        Field::TransientFor => WindowProperty::TransientFor,
        Field::WindowType => WindowProperty::WindowType,
        Field::Motif => WindowProperty::Decorations,
        Field::Pid => WindowProperty::Pid,
        Field::StartupId => WindowProperty::StartupId,
        Field::Opacity => WindowProperty::Opacity,
        Field::States => WindowProperty::States,
        Field::XdndAware | Field::XdndProxy => return None,
    })
}

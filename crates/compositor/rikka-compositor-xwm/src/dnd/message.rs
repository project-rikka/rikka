use x11rb_protocol::protocol::xproto::{Atom, Timestamp, Window};

use crate::wire::NONE;

pub(crate) const VERSION: u32 = 5;
pub(crate) const INLINE_TYPES: usize = 3;

const MORE_TYPES: u32 = 1;
const ACCEPTS: u32 = 1;
const WANTS_POSITIONS: u32 = 2;
const FINISHED_ACCEPTED: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Enter {
    pub(crate) source: Window,
    pub(crate) version: u32,
    pub(crate) listed: bool,
    pub(crate) types: Vec<Atom>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Status {
    pub(crate) target: Window,
    pub(crate) accepted: bool,
    pub(crate) action: Atom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Finished {
    pub(crate) target: Window,
    pub(crate) accepted: bool,
    pub(crate) action: Atom,
}

pub(crate) fn enter(source: Window, version: u32, types: &[Atom]) -> [u32; 5] {
    let listed = types.len() > INLINE_TYPES;
    let inline = |at: usize| match listed {
        true => NONE,
        false => types.get(at).copied().unwrap_or(NONE),
    };
    let flags = (version << 24) | if listed { MORE_TYPES } else { 0 };
    [source, flags, inline(0), inline(1), inline(2)]
}

pub(crate) fn read_enter(data: [u32; 5]) -> Enter {
    Enter {
        source: data[0],
        version: data[1] >> 24,
        listed: data[1] & MORE_TYPES != 0,
        types: data[2..]
            .iter()
            .copied()
            .filter(|atom| *atom != NONE)
            .collect(),
    }
}

pub(crate) fn position(source: Window, x: i32, y: i32, time: Timestamp, action: Atom) -> [u32; 5] {
    [source, 0, pack(x, y), time, action]
}

pub(crate) fn status(target: Window, accepted: bool, action: Atom) -> [u32; 5] {
    let flags = WANTS_POSITIONS | if accepted { ACCEPTS } else { 0 };
    let action = if accepted { action } else { NONE };
    [target, flags, 0, 0, action]
}

pub(crate) fn read_status(data: [u32; 5]) -> Status {
    let accepted = data[1] & ACCEPTS != 0;
    Status {
        target: data[0],
        accepted,
        action: if accepted { data[4] } else { NONE },
    }
}

pub(crate) fn leave(source: Window) -> [u32; 5] {
    [source, 0, 0, 0, 0]
}

pub(crate) fn drop(source: Window, time: Timestamp) -> [u32; 5] {
    [source, 0, time, 0, 0]
}

pub(crate) fn finished(target: Window, version: u32, accepted: bool, action: Atom) -> [u32; 5] {
    if version < VERSION {
        return [target, 0, 0, 0, 0];
    }
    let flags = if accepted { FINISHED_ACCEPTED } else { 0 };
    let action = if accepted { action } else { NONE };
    [target, flags, action, 0, 0]
}

pub(crate) fn read_finished(data: [u32; 5], version: u32) -> Finished {
    let accepted = version < VERSION || data[1] & FINISHED_ACCEPTED != 0;
    Finished {
        target: data[0],
        accepted,
        action: if version < VERSION { NONE } else { data[2] },
    }
}

fn pack(x: i32, y: i32) -> u32 {
    let x = x.clamp(0, i32::from(u16::MAX)) as u32;
    let y = y.clamp(0, i32::from(u16::MAX)) as u32;
    (x << 16) | y
}

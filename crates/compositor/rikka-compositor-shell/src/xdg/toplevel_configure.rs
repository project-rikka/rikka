use bitflags::bitflags;
use rikka_compositor_geometry::{Logical, Size};
use rikka_compositor_protocols::xdg_toplevel::{self, State};

use super::decoration::DecorationMode;
use super::toplevel::Toplevel;
use crate::ids::ToplevelId;
use crate::shell::Shell;

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub struct ToplevelStates: u32 {
        const MAXIMIZED = 1;
        const FULLSCREEN = 1 << 1;
        const RESIZING = 1 << 2;
        const ACTIVATED = 1 << 3;
        const TILED_LEFT = 1 << 4;
        const TILED_RIGHT = 1 << 5;
        const TILED_TOP = 1 << 6;
        const TILED_BOTTOM = 1 << 7;
        const SUSPENDED = 1 << 8;
        const CONSTRAINED_LEFT = 1 << 9;
        const CONSTRAINED_RIGHT = 1 << 10;
        const CONSTRAINED_TOP = 1 << 11;
        const CONSTRAINED_BOTTOM = 1 << 12;
    }
}

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub struct WmCapabilities: u32 {
        const WINDOW_MENU = 1;
        const MAXIMIZE = 1 << 1;
        const FULLSCREEN = 1 << 2;
        const MINIMIZE = 1 << 3;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ToplevelConfigure {
    pub size: Option<Size<i32, Logical>>,
    pub states: ToplevelStates,
    pub bounds: Option<Size<i32, Logical>>,
    pub capabilities: Option<WmCapabilities>,
    pub decoration: Option<DecorationMode>,
}

const STATES: [(ToplevelStates, State, u32); 13] = [
    (ToplevelStates::MAXIMIZED, State::Maximized, 1),
    (ToplevelStates::FULLSCREEN, State::Fullscreen, 1),
    (ToplevelStates::RESIZING, State::Resizing, 1),
    (ToplevelStates::ACTIVATED, State::Activated, 1),
    (ToplevelStates::TILED_LEFT, State::TiledLeft, 2),
    (ToplevelStates::TILED_RIGHT, State::TiledRight, 2),
    (ToplevelStates::TILED_TOP, State::TiledTop, 2),
    (ToplevelStates::TILED_BOTTOM, State::TiledBottom, 2),
    (ToplevelStates::SUSPENDED, State::Suspended, 6),
    (ToplevelStates::CONSTRAINED_LEFT, State::ConstrainedLeft, 7),
    (
        ToplevelStates::CONSTRAINED_RIGHT,
        State::ConstrainedRight,
        7,
    ),
    (ToplevelStates::CONSTRAINED_TOP, State::ConstrainedTop, 7),
    (
        ToplevelStates::CONSTRAINED_BOTTOM,
        State::ConstrainedBottom,
        7,
    ),
];

const CAPABILITIES: [(WmCapabilities, xdg_toplevel::WmCapabilities); 4] = [
    (
        WmCapabilities::WINDOW_MENU,
        xdg_toplevel::WmCapabilities::WindowMenu,
    ),
    (
        WmCapabilities::MAXIMIZE,
        xdg_toplevel::WmCapabilities::Maximize,
    ),
    (
        WmCapabilities::FULLSCREEN,
        xdg_toplevel::WmCapabilities::Fullscreen,
    ),
    (
        WmCapabilities::MINIMIZE,
        xdg_toplevel::WmCapabilities::Minimize,
    ),
];

impl Shell {
    pub fn configure_toplevel(
        &mut self,
        toplevel: ToplevelId,
        configure: ToplevelConfigure,
    ) -> Option<u32> {
        if !self.toplevels.contains_key(toplevel) {
            return None;
        }
        let serial = self.clock.serial();
        self.toplevels.get_mut(toplevel)?.send(serial, configure);
        Some(serial)
    }

    pub fn close_toplevel(&self, toplevel: ToplevelId) {
        if let Some(record) = self.toplevels.get(toplevel) {
            record.resource.close();
        }
    }
}

impl Toplevel {
    fn send(&mut self, serial: u32, configure: ToplevelConfigure) {
        let resource = &self.resource;
        if let Some(bounds) = configure.bounds {
            resource.configure_bounds(bounds.w, bounds.h);
        }
        if let Some(capabilities) = configure.capabilities
            && self.sent_capabilities != Some(capabilities)
        {
            resource.wm_capabilities(encode_capabilities(capabilities).as_bytes());
            self.sent_capabilities = Some(capabilities);
        }
        if let Some(mode) = configure.decoration
            && let Some(decoration) = &mut self.decoration
        {
            decoration.offer(mode);
        }
        let size = configure.size.unwrap_or_default();
        resource.configure(
            size.w,
            size.h,
            encode_states(configure.states, resource.version()).as_bytes(),
        );
        self.xdg_surface.configure(serial);
        self.lifecycle.sent(serial, configure);
    }
}

fn encode_states(states: ToplevelStates, version: u32) -> Words<{ STATES.len() * 4 }> {
    Words::collect(
        STATES
            .iter()
            .filter(|(flag, _, since)| states.contains(*flag) && version >= *since)
            .map(|(_, state, _)| u32::from(*state)),
    )
}

fn encode_capabilities(capabilities: WmCapabilities) -> Words<{ CAPABILITIES.len() * 4 }> {
    Words::collect(
        CAPABILITIES
            .iter()
            .filter(|(flag, _)| capabilities.contains(*flag))
            .map(|(_, capability)| u32::from(*capability)),
    )
}

struct Words<const N: usize> {
    bytes: [u8; N],
    len: usize,
}

impl<const N: usize> Words<N> {
    fn collect(words: impl Iterator<Item = u32>) -> Self {
        let mut encoded = Self {
            bytes: [0; N],
            len: 0,
        };
        for word in words {
            encoded.bytes[encoded.len..encoded.len + 4].copy_from_slice(&word.to_ne_bytes());
            encoded.len += 4;
        }
        encoded
    }

    fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.len]
    }
}

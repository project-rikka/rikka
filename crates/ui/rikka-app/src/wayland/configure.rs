use rikka_wayland_client::xdg_toplevel::State;
use rikka_wayland_client::zxdg_toplevel_decoration_v1::Mode;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct States {
    pub(crate) maximized: bool,
    pub(crate) fullscreen: bool,
    pub(crate) resizing: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Configured {
    pub(crate) serial: u32,
    pub(crate) size: Option<(u32, u32)>,
    pub(crate) states: States,
    pub(crate) bounds: Option<(u32, u32)>,
    pub(crate) server_side: Option<bool>,
}

#[derive(Debug, Default)]
pub(crate) struct Configuring {
    size: Option<(u32, u32)>,
    states: States,
    bounds: Option<(u32, u32)>,
    server_side: Option<bool>,
}

impl Configuring {
    pub(crate) fn toplevel(&mut self, width: i32, height: i32, states: &[u8]) {
        self.size = match (u32::try_from(width), u32::try_from(height)) {
            (Ok(width), Ok(height)) if width > 0 && height > 0 => Some((width, height)),
            _ => None,
        };
        self.states = States::default();
        for raw in states.as_chunks::<4>().0 {
            let raw = u32::from_ne_bytes(*raw);
            match State::try_from(raw) {
                Ok(State::Maximized) => self.states.maximized = true,
                Ok(State::Fullscreen) => self.states.fullscreen = true,
                Ok(State::Resizing) => self.states.resizing = true,
                _ => {}
            }
        }
    }

    pub(crate) fn bounded(&mut self, width: i32, height: i32) {
        self.bounds = match (u32::try_from(width), u32::try_from(height)) {
            (Ok(width), Ok(height)) if width > 0 && height > 0 => Some((width, height)),
            _ => None,
        };
    }

    pub(crate) fn decorated(&mut self, mode: Result<Mode, rikka_wayland_client::UnknownValue>) {
        self.server_side = match mode {
            Ok(Mode::ServerSide) => Some(true),
            Ok(Mode::ClientSide) => Some(false),
            _ => self.server_side,
        };
    }

    pub(crate) fn done(&self, serial: u32) -> Configured {
        Configured {
            serial,
            size: self.size,
            states: self.states,
            bounds: self.bounds,
            server_side: self.server_side,
        }
    }
}

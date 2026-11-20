use rikka_compositor_seat::Seats;
use rikka_compositor_shell::Shell;
use rikka_compositor_surface::Surfaces;

use crate::cursor::Cursor;
use crate::input::Input;
use crate::outputs::Outputs;
use crate::windows::Windows;
use crate::x11::X11;

pub(crate) struct WindowsRef<'a> {
    pub(crate) outputs: &'a Outputs,
    pub(crate) surfaces: &'a Surfaces,
    pub(crate) shell: &'a Shell,
    pub(crate) seats: &'a Seats,
    pub(crate) windows: &'a Windows,
    pub(crate) cursor: &'a Cursor,
    pub(crate) input: &'a Input,
    pub(crate) x11: &'a X11,
}

macro_rules! windows_ref {
    ($host:expr) => {
        $crate::context::WindowsRef {
            outputs: &$host.outputs,
            surfaces: &$host.surfaces,
            shell: &$host.shell,
            seats: &$host.seats,
            windows: &$host.windows,
            cursor: &$host.cursor,
            input: &$host.input,
            x11: &$host.x11,
        }
    };
}

pub(crate) use windows_ref;

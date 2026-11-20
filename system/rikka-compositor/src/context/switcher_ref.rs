use rikka_compositor_seat::Seats;
use rikka_compositor_shell::Shell;

use crate::input::Input;
use crate::windows::Windows;

pub(crate) struct SwitcherRef<'a> {
    pub(crate) shell: &'a Shell,
    pub(crate) seats: &'a Seats,
    pub(crate) windows: &'a Windows,
    pub(crate) input: &'a Input,
}

macro_rules! switcher_ref {
    ($host:expr) => {
        $crate::context::SwitcherRef {
            shell: &$host.shell,
            seats: &$host.seats,
            windows: &$host.windows,
            input: &$host.input,
        }
    };
}

pub(crate) use switcher_ref;

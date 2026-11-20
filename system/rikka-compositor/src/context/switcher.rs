use rikka_compositor_seat::Seats;
use rikka_compositor_shell::Shell;
use rikka_compositor_surface::Surfaces;

use crate::cursor::Cursor;
use crate::input::Input;
use crate::outputs::Outputs;
use crate::render::FrameScheduler;
use crate::session::Session;
use crate::state::Core;
use crate::switcher::Switcher;
use crate::windows::Windows;
use crate::x11::X11;

pub(crate) struct SwitcherCtx<'a> {
    pub(crate) core: &'a mut Core,
    pub(crate) outputs: &'a mut Outputs,
    pub(crate) surfaces: &'a mut Surfaces,
    pub(crate) shell: &'a mut Shell,
    pub(crate) seats: &'a mut Seats,
    pub(crate) windows: &'a mut Windows,
    pub(crate) frames: &'a mut FrameScheduler,
    pub(crate) cursor: &'a mut Cursor,
    pub(crate) input: &'a mut Input,
    pub(crate) switcher: &'a mut Switcher,
    pub(crate) session: &'a mut Session,
    pub(crate) x11: &'a mut X11,
}

macro_rules! switcher_ctx {
    ($host:expr) => {
        $crate::context::SwitcherCtx {
            core: &mut $host.core,
            outputs: &mut $host.outputs,
            surfaces: &mut $host.surfaces,
            shell: &mut $host.shell,
            seats: &mut $host.seats,
            windows: &mut $host.windows,
            frames: &mut $host.frames,
            cursor: &mut $host.cursor,
            input: &mut $host.input,
            switcher: &mut $host.switcher,
            session: &mut $host.session,
            x11: &mut $host.x11,
        }
    };
}

pub(crate) use switcher_ctx;

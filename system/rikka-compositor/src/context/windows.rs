use rikka_compositor_seat::Seats;
use rikka_compositor_shell::Shell;
use rikka_compositor_surface::Surfaces;

use crate::animation::Animations;
use crate::cursor::Cursor;
use crate::input::Input;
use crate::outputs::Outputs;
use crate::policy::Policy;
use crate::render::{FrameScheduler, Scenes};
use crate::selection::Selection;
use crate::session::Session;
use crate::snap::Snapping;
use crate::state::Core;
use crate::switcher::Switcher;
use crate::windows::Windows;
use crate::x11::X11;

pub(crate) struct WindowsCtx<'a> {
    pub(crate) core: &'a mut Core,
    pub(crate) outputs: &'a mut Outputs,
    pub(crate) surfaces: &'a mut Surfaces,
    pub(crate) shell: &'a mut Shell,
    pub(crate) seats: &'a mut Seats,
    pub(crate) windows: &'a mut Windows,
    pub(crate) frames: &'a mut FrameScheduler,
    pub(crate) scenes: &'a mut Scenes,
    pub(crate) cursor: &'a mut Cursor,
    pub(crate) input: &'a mut Input,
    pub(crate) selection: &'a mut Selection,
    pub(crate) policy: &'a mut Policy,
    pub(crate) animations: &'a mut Animations,
    pub(crate) snap: &'a mut Snapping,
    pub(crate) switcher: &'a mut Switcher,
    pub(crate) session: &'a mut Session,
    pub(crate) x11: &'a mut X11,
}

macro_rules! windows_ctx {
    ($host:expr) => {
        $crate::context::WindowsCtx {
            core: &mut $host.core,
            outputs: &mut $host.outputs,
            surfaces: &mut $host.surfaces,
            shell: &mut $host.shell,
            seats: &mut $host.seats,
            windows: &mut $host.windows,
            frames: &mut $host.frames,
            scenes: &mut $host.scenes,
            cursor: &mut $host.cursor,
            input: &mut $host.input,
            selection: &mut $host.selection,
            policy: &mut $host.policy,
            animations: &mut $host.animations,
            snap: &mut $host.snap,
            switcher: &mut $host.switcher,
            session: &mut $host.session,
            x11: &mut $host.x11,
        }
    };
}

pub(crate) use windows_ctx;

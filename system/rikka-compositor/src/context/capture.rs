use rikka_compositor_desktop::WindowId;
use rikka_compositor_output::Captures;
use rikka_compositor_shell::Shell;
use rikka_compositor_surface::Surfaces;

use crate::backend::Backend;
use crate::capture::Capture;
use crate::cursor::Cursor;
use crate::input::Input;
use crate::outputs::Outputs;
use crate::render::{FrameScheduler, Scenes};
use crate::selection::Selection;
use crate::state::Core;
use crate::windows::Windows;

pub(crate) struct CaptureCtx<'a> {
    pub(crate) core: &'a mut Core,
    pub(crate) backend: &'a mut dyn Backend,
    pub(crate) outputs: &'a mut Outputs,
    pub(crate) surfaces: &'a mut Surfaces,
    pub(crate) shell: &'a mut Shell,
    pub(crate) windows: &'a mut Windows,
    pub(crate) frames: &'a mut FrameScheduler,
    pub(crate) scenes: &'a mut Scenes,
    pub(crate) captures: &'a mut Captures<WindowId>,
    pub(crate) capture: &'a mut Capture,
    pub(crate) cursor: &'a mut Cursor,
    pub(crate) input: &'a mut Input,
    pub(crate) selection: &'a mut Selection,
}

macro_rules! capture_ctx {
    ($host:expr) => {
        $crate::context::CaptureCtx {
            core: &mut $host.core,
            backend: &mut *$host.backend,
            outputs: &mut $host.outputs,
            surfaces: &mut $host.surfaces,
            shell: &mut $host.shell,
            windows: &mut $host.windows,
            frames: &mut $host.frames,
            scenes: &mut $host.scenes,
            captures: &mut $host.captures,
            capture: &mut $host.capture,
            cursor: &mut $host.cursor,
            input: &mut $host.input,
            selection: &mut $host.selection,
        }
    };
}

pub(crate) use capture_ctx;

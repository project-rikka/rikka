use rikka_compositor_shell::Shell;
use rikka_compositor_surface::Surfaces;

use crate::input::Input;
use crate::outputs::Outputs;
use crate::render::Scenes;
use crate::selection::Selection;
use crate::windows::Windows;

pub(crate) struct CaptureRef<'a> {
    pub(crate) outputs: &'a Outputs,
    pub(crate) surfaces: &'a Surfaces,
    pub(crate) shell: &'a Shell,
    pub(crate) windows: &'a Windows,
    pub(crate) scenes: &'a Scenes,
    pub(crate) input: &'a Input,
    pub(crate) selection: &'a Selection,
}

macro_rules! capture_ref {
    ($host:expr) => {
        $crate::context::CaptureRef {
            outputs: &$host.outputs,
            surfaces: &$host.surfaces,
            shell: &$host.shell,
            windows: &$host.windows,
            scenes: &$host.scenes,
            input: &$host.input,
            selection: &$host.selection,
        }
    };
}

pub(crate) use capture_ref;

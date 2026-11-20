use rikka_compositor_surface::SurfaceHost;

use crate::event::ShellEvent;
use crate::shell::Shell;
use crate::xdg::TokenRequest;

pub trait ShellHost: SurfaceHost {
    fn shell(&mut self) -> &mut Shell;

    fn shell_event(&mut self, event: ShellEvent);

    fn admit_activation(&mut self, request: &TokenRequest) -> bool;
}

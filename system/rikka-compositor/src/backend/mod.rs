mod extent;
mod headless;
mod kms;
mod leases;
mod nested;

use std::any::Any;
use std::os::fd::OwnedFd;
use std::time::Instant;

use rikka_compositor_input::{DeviceSettings, Leds};
use rikka_renderer::Renderer;

pub use extent::Extent;
pub(crate) use headless::Headless;
pub(crate) use kms::Kms;
pub(crate) use leases::{LeaseCard, Leases};
pub(crate) use nested::Nested;

use rikka_compositor_types::OutputId;

use crate::outputs::OutputConfig;
use crate::render::Snapshot;
use crate::state::Parts;

pub(crate) trait Backend: Any {
    fn configure(&mut self, parts: &mut Parts<'_>, configs: &[(OutputId, OutputConfig)]) -> bool;

    fn schedule(&mut self, output: OutputId);

    fn configure_devices(&mut self, settings: DeviceSettings);

    fn show_leds(&mut self, _leds: Leds) {}

    fn cursor_changed(&mut self, parts: &mut Parts<'_>);

    fn switches_vt(&self) -> bool;

    fn snapshot(&mut self, output: OutputId) -> Option<Snapshot>;

    fn renderer(&mut self) -> &mut Renderer;

    fn rest(&mut self) {
        self.renderer().trim();
    }

    fn syncobj_device(&self) -> Option<OwnedFd> {
        None
    }

    fn leases(&mut self) -> Option<&mut dyn Leases> {
        None
    }

    fn switch_vt(&mut self, _vt: i32) {}

    fn route_changed(&mut self, _parts: &mut Parts<'_>) {}

    fn pointer_warped(&mut self, _parts: &mut Parts<'_>) {}

    fn gpu_reached(&mut self, _finished: u64, _at: Instant) {}

    fn sleep_displays(&mut self, _parts: &mut Parts<'_>) -> bool {
        false
    }

    fn wake_displays(&mut self, _parts: &mut Parts<'_>) {}

    fn interacted(&mut self) {}
}

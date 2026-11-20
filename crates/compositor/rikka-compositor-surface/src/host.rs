use rikka_compositor_protocols::wl_output::WlOutput;
use rikka_compositor_types::OutputId;

use crate::committed::Committed;
use crate::dmabuf::Dmabuf;
use crate::ids::SurfaceId;
use crate::surfaces::Surfaces;

pub trait SurfaceHost: Sized + 'static {
    fn surfaces(&mut self) -> &mut Surfaces;

    fn committing(&mut self, _surface: SurfaceId) {}

    fn committed(&mut self, _commit: &Committed) {}

    fn destroyed(&mut self, _surface: SurfaceId) {}

    fn import_dmabuf(&mut self, _dmabuf: &Dmabuf) -> bool {
        true
    }

    fn wants_refresh(&mut self, _output: OutputId) {}

    fn output_of(&mut self, _output: &WlOutput) -> Option<OutputId> {
        None
    }
}

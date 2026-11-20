use rikka_compositor_protocols::wl_output::WlOutput;
use rikka_compositor_types::OutputId;

use super::store::Outputs;

pub trait OutputHost: Sized + 'static {
    fn outputs(&mut self) -> &mut Outputs;

    fn output_bound(&mut self, _output: OutputId, _resource: &WlOutput) {}
}

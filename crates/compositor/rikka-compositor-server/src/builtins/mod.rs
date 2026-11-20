mod display;
mod fixes;
mod registry;

use rikka_compositor_protocols::wl_display::WlDisplay;
use rikka_compositor_protocols::wl_fixes::WlFixes;
use rikka_compositor_protocols::wl_registry::WlRegistry;

use crate::display::DisplayHandle;
use crate::module::Module;

const FIXES_VERSION: u32 = 2;

pub(crate) struct CoreProtocol;

impl<S: 'static> Module<S> for CoreProtocol {
    fn install(self, display: &DisplayHandle<S>) {
        display.serve::<WlDisplay>(display::request);
        display
            .serve::<WlRegistry>(registry::request)
            .destroyed(registry::destroyed);
        display.serve::<WlFixes>(fixes::request);
        display.global::<WlFixes>().version(FIXES_VERSION).publish();
    }
}

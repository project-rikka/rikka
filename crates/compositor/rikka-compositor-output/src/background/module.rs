use rikka_compositor_protocols::ext_background_effect_manager_v1::ExtBackgroundEffectManagerV1;
use rikka_compositor_protocols::ext_background_effect_surface_v1::ExtBackgroundEffectSurfaceV1;
use rikka_compositor_server::{DisplayHandle, Module};

use super::host::BackgroundEffectHost;
use super::protocol;

#[derive(Debug, Clone, Copy, Default)]
pub struct BackgroundEffectProtocols;

impl<S: BackgroundEffectHost> Module<S> for BackgroundEffectProtocols {
    fn install(self, display: &DisplayHandle<S>) {
        display
            .serve::<ExtBackgroundEffectManagerV1>(protocol::manager)
            .bound(protocol::bound);
        display
            .serve::<ExtBackgroundEffectSurfaceV1>(protocol::request)
            .destroyed(protocol::destroyed);
        display.global::<ExtBackgroundEffectManagerV1>().publish();
    }
}

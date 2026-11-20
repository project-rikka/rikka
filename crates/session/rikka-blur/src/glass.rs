use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use rikka_wayland::Host;
use rikka_wayland_client::Route;
use rikka_wayland_client::ext_background_effect_manager_v1::{
    Capability, Event, ExtBackgroundEffectManagerV1,
};
use rikka_wayland_client::wl_compositor::WlCompositor;
use rikka_wayland_client::wl_surface::WlSurface;

use crate::blurred_surface::BlurredSurface;

#[must_use]
#[derive(Debug)]
pub struct Glass {
    blurs: Arc<AtomicBool>,
    compositor: WlCompositor,
    manager: ExtBackgroundEffectManagerV1,
}

impl Glass {
    pub fn bind(host: &Host) -> Option<Glass> {
        let display = host.display();
        let blurs = Arc::new(AtomicBool::new(false));
        let announced = Arc::clone(&blurs);
        let capabilities = Route::tap::<ExtBackgroundEffectManagerV1>(move |_, event| {
            if let Event::Capabilities { flags } = event {
                announced.store(flags.contains(Capability::BLUR), Ordering::Release);
            }
        });

        let compositor: WlCompositor = display.bind(1..=6).ok()?;
        let manager: ExtBackgroundEffectManagerV1 =
            display.bind_optional_on(&capabilities, 1..=1)?;
        Some(Glass {
            blurs,
            compositor,
            manager,
        })
    }

    pub fn for_surface(&self, surface: &WlSurface) -> BlurredSurface {
        BlurredSurface::new(
            self.manager
                .via(&Route::discard())
                .get_background_effect(surface),
            self.compositor.clone(),
            Arc::clone(&self.blurs),
        )
    }
}

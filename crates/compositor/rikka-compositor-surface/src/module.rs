use rikka_compositor_protocols::wl_buffer::WlBuffer;
use rikka_compositor_protocols::wl_compositor::WlCompositor;
use rikka_compositor_protocols::wl_region::WlRegion;
use rikka_compositor_protocols::wl_shm::WlShm;
use rikka_compositor_protocols::wl_shm_pool::WlShmPool;
use rikka_compositor_protocols::wl_subcompositor::WlSubcompositor;
use rikka_compositor_protocols::wl_subsurface::WlSubsurface;
use rikka_compositor_protocols::wl_surface::WlSurface;
use rikka_compositor_protocols::wp_alpha_modifier_surface_v1::WpAlphaModifierSurfaceV1;
use rikka_compositor_protocols::wp_alpha_modifier_v1::WpAlphaModifierV1;
use rikka_compositor_protocols::wp_content_type_manager_v1::WpContentTypeManagerV1;
use rikka_compositor_protocols::wp_content_type_v1::WpContentTypeV1;
use rikka_compositor_protocols::wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1;
use rikka_compositor_protocols::wp_fractional_scale_v1::WpFractionalScaleV1;
use rikka_compositor_protocols::wp_presentation::WpPresentation;
use rikka_compositor_protocols::wp_single_pixel_buffer_manager_v1::WpSinglePixelBufferManagerV1;
use rikka_compositor_protocols::wp_tearing_control_manager_v1::WpTearingControlManagerV1;
use rikka_compositor_protocols::wp_tearing_control_v1::WpTearingControlV1;
use rikka_compositor_protocols::wp_viewport::WpViewport;
use rikka_compositor_protocols::wp_viewporter::WpViewporter;
use rikka_compositor_server::{DisplayHandle, Module};

use crate::host::SurfaceHost;
use crate::module_parts::{color, synchronization};
use crate::protocol::{
    alpha, compositor, content_type, fractional, presentation, region, shm, single_pixel,
    subsurface, surface, tearing, viewporter,
};
use crate::turn;

#[derive(Debug, Clone, Copy, Default)]
pub struct SurfaceProtocols;

impl<S: SurfaceHost> Module<S> for SurfaceProtocols {
    fn install(self, display: &DisplayHandle<S>) {
        display.serve::<WlCompositor>(compositor::request);
        display
            .serve::<WlSurface>(surface::request)
            .destroyed(surface::destroyed);
        display
            .serve::<WlRegion>(region::request)
            .destroyed(region::destroyed);
        display.serve::<WlSubcompositor>(subsurface::manager);
        display
            .serve::<WlSubsurface>(subsurface::request)
            .destroyed(subsurface::destroyed);
        display.serve::<WlShm>(shm::request).bound(shm::bound);
        display
            .serve::<WlShmPool>(shm::pool)
            .destroyed(shm::pool_destroyed);
        display
            .serve::<WlBuffer>(shm::buffer)
            .destroyed(shm::buffer_destroyed);
        display.serve::<WpSinglePixelBufferManagerV1>(single_pixel::request);
        display.serve::<WpViewporter>(viewporter::manager);
        display
            .serve::<WpViewport>(viewporter::request)
            .destroyed(viewporter::destroyed);
        display.serve::<WpFractionalScaleManagerV1>(fractional::manager);
        display
            .serve::<WpFractionalScaleV1>(fractional::request)
            .destroyed(fractional::destroyed);
        display
            .serve::<WpPresentation>(presentation::request)
            .bound(presentation::bound);
        display.serve::<WpContentTypeManagerV1>(content_type::manager);
        display
            .serve::<WpContentTypeV1>(content_type::request)
            .destroyed(content_type::destroyed);
        display.serve::<WpAlphaModifierV1>(alpha::manager);
        display
            .serve::<WpAlphaModifierSurfaceV1>(alpha::request)
            .destroyed(alpha::destroyed);
        display.serve::<WpTearingControlManagerV1>(tearing::manager);
        display
            .serve::<WpTearingControlV1>(tearing::request)
            .destroyed(tearing::destroyed);

        display.global::<WlCompositor>().publish();
        display.global::<WlSubcompositor>().publish();
        display.global::<WlShm>().publish();
        display.global::<WpSinglePixelBufferManagerV1>().publish();
        display.global::<WpViewporter>().publish();
        display.global::<WpFractionalScaleManagerV1>().publish();
        display.global::<WpPresentation>().publish();
        display.global::<WpContentTypeManagerV1>().publish();
        display.global::<WpAlphaModifierV1>().publish();
        display.global::<WpTearingControlManagerV1>().publish();

        synchronization::install(display);
        color::install(display);
        let reactor = display.reactor().downgrade();
        display
            .reactor()
            .each_turn(move |state: &mut S| turn::service(state, &reactor));
    }
}

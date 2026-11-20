use rikka_compositor_protocols::wp_color_management_output_v1::WpColorManagementOutputV1;
use rikka_compositor_protocols::wp_color_management_surface_feedback_v1::WpColorManagementSurfaceFeedbackV1;
use rikka_compositor_protocols::wp_color_management_surface_v1::WpColorManagementSurfaceV1;
use rikka_compositor_protocols::wp_color_manager_v1::WpColorManagerV1;
use rikka_compositor_protocols::wp_color_representation_manager_v1::WpColorRepresentationManagerV1;
use rikka_compositor_protocols::wp_color_representation_surface_v1::WpColorRepresentationSurfaceV1;
use rikka_compositor_protocols::wp_image_description_creator_params_v1::WpImageDescriptionCreatorParamsV1;
use rikka_compositor_protocols::wp_image_description_v1::WpImageDescriptionV1;
use rikka_compositor_server::DisplayHandle;

use crate::host::SurfaceHost;
use crate::protocol::{color_management, color_representation, image_description};

pub(crate) fn install<S: SurfaceHost>(display: &DisplayHandle<S>) {
    display
        .serve::<WpColorManagerV1>(color_management::manager)
        .bound(color_management::bound);
    display.serve::<WpColorManagementOutputV1>(color_management::output);
    display
        .serve::<WpColorManagementSurfaceV1>(color_management::surface)
        .destroyed(color_management::surface_destroyed);
    display.serve::<WpColorManagementSurfaceFeedbackV1>(color_management::feedback);
    display
        .serve::<WpImageDescriptionCreatorParamsV1>(color_management::creator)
        .destroyed(color_management::creator_destroyed);
    display
        .serve::<WpImageDescriptionV1>(image_description::request)
        .destroyed(image_description::destroyed);
    display
        .serve::<WpColorRepresentationManagerV1>(color_representation::manager)
        .bound(color_representation::bound);
    display
        .serve::<WpColorRepresentationSurfaceV1>(color_representation::surface)
        .destroyed(color_representation::destroyed);
    display.global::<WpColorRepresentationManagerV1>().publish();
}

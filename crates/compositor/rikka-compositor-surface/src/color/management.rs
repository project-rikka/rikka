use std::collections::HashMap;

use rikka_compositor_protocols::wp_color_management_output_v1::WpColorManagementOutputV1;
use rikka_compositor_protocols::wp_color_management_surface_feedback_v1::WpColorManagementSurfaceFeedbackV1;
use rikka_compositor_types::OutputId;

use super::description::{Descriptions, ImageDescription, Parametric};
use super::representation::RepresentationSupport;
use super::support::ColorSupport;
use crate::ids::SurfaceId;
use crate::surfaces::Surfaces;

const IDENTITY64_SINCE: u32 = 2;

#[derive(Debug, Default)]
pub(crate) struct ColorManagement {
    pub(crate) support: ColorSupport,
    pub(crate) descriptions: Descriptions,
    pub(crate) preferred: Option<ImageDescription>,
    pub(crate) outputs: HashMap<OutputId, ImageDescription>,
    pub(crate) output_objects: Vec<(WpColorManagementOutputV1, OutputId)>,
    pub(crate) representation: RepresentationSupport,
}

impl Surfaces {
    pub fn describe(&mut self, parametric: Parametric) -> ImageDescription {
        self.color.descriptions.describe(parametric)
    }

    pub fn set_output_image_description(
        &mut self,
        output: OutputId,
        description: ImageDescription,
    ) {
        if self.color.outputs.get(&output) == Some(&description) {
            return;
        }
        self.color.outputs.insert(output, description);
        self.color
            .output_objects
            .retain(|(object, _)| object.alive());
        for (object, _) in self
            .color
            .output_objects
            .iter()
            .filter(|(_, key)| *key == output)
        {
            object.image_description_changed();
        }
    }

    pub fn output_image_description(&self, output: OutputId) -> Option<&ImageDescription> {
        self.color.outputs.get(&output)
    }

    pub fn forget_output(&mut self, output: OutputId) {
        self.color.outputs.remove(&output);
        self.color.output_objects.retain(|(_, key)| *key != output);
        self.forget_horizon(output);
    }

    pub fn set_preferred_image_description(
        &mut self,
        surface: SurfaceId,
        description: ImageDescription,
    ) {
        let default = self.color.preferred.clone();
        let Some(surface) = self.surfaces.get_mut(surface) else {
            return;
        };
        let before = surface.hints.description.clone().or(default);
        if before.as_ref() == Some(&description) {
            surface.hints.description = Some(description);
            return;
        }
        surface
            .extensions
            .color_feedback
            .retain(WpColorManagementSurfaceFeedbackV1::alive);
        for object in &surface.extensions.color_feedback {
            announce_preferred(object, &description);
        }
        surface.hints.description = Some(description);
    }

    pub fn set_representation_support(&mut self, support: RepresentationSupport) {
        self.color.representation = support;
    }

    pub(crate) fn preferred_description(&self, surface: SurfaceId) -> Option<ImageDescription> {
        self.surfaces
            .get(surface)
            .and_then(|surface| surface.hints.description.clone())
            .or_else(|| self.color.preferred.clone())
    }
}

fn announce_preferred(object: &WpColorManagementSurfaceFeedbackV1, description: &ImageDescription) {
    let identity = description.identity();
    if object.version() >= IDENTITY64_SINCE {
        object.preferred_changed2((identity >> 32) as u32, identity as u32);
    } else {
        object.preferred_changed(identity as u32);
    }
}

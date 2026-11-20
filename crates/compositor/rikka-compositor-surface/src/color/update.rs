use rikka_compositor_protocols::wp_color_manager_v1::RenderIntent;
use rikka_compositor_protocols::wp_color_representation_surface_v1::{
    AlphaMode, ChromaLocation, Coefficients, Range,
};

use super::description::ImageDescription;
use super::representation::ColorRepresentation;

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct ColorUpdate {
    pub(crate) description: Option<Option<(ImageDescription, RenderIntent)>>,
    pub(crate) alpha_mode: Option<Option<AlphaMode>>,
    pub(crate) coefficients: Option<Option<(Coefficients, Range)>>,
    pub(crate) chroma_location: Option<Option<ChromaLocation>>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct ColorState {
    pub(crate) description: Option<(ImageDescription, RenderIntent)>,
    pub(crate) representation: ColorRepresentation,
}

impl ColorUpdate {
    pub(crate) fn absorb(&mut self, later: &mut ColorUpdate) {
        take_over(&mut self.description, &mut later.description);
        take_over(&mut self.alpha_mode, &mut later.alpha_mode);
        take_over(&mut self.coefficients, &mut later.coefficients);
        take_over(&mut self.chroma_location, &mut later.chroma_location);
    }

    pub(crate) fn unset_representation(&mut self) {
        self.alpha_mode = Some(None);
        self.coefficients = Some(None);
        self.chroma_location = Some(None);
    }

    pub(crate) fn representation_over(&self, current: &ColorRepresentation) -> ColorRepresentation {
        ColorRepresentation {
            alpha_mode: self.alpha_mode.unwrap_or(current.alpha_mode),
            coefficients: self.coefficients.unwrap_or(current.coefficients),
            chroma_location: self.chroma_location.unwrap_or(current.chroma_location),
        }
    }

    pub(crate) fn apply(&mut self, state: &mut ColorState) -> bool {
        let representation = self.representation_over(&state.representation);
        let mut changed = representation != state.representation;
        state.representation = representation;
        if let Some(description) = self.description.take() {
            changed |= description != state.description;
            state.description = description;
        }
        changed
    }
}

fn take_over<T>(earlier: &mut Option<T>, later: &mut Option<T>) {
    if later.is_some() {
        *earlier = later.take();
    }
}

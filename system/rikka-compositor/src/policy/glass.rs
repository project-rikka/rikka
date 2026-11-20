use rikka_theme::Translucency;
use rikka_types::Lens;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Glass {
    radius: f32,
    lens: Option<Lens>,
}

impl Glass {
    pub(crate) fn new(radius: f32, lens: Option<Lens>) -> Glass {
        Glass {
            radius,
            lens: lens.filter(|_| radius > 0.0),
        }
    }

    pub(crate) fn none() -> Glass {
        Glass::default()
    }

    pub(crate) fn scrim() -> Glass {
        Glass {
            radius: Translucency::SCRIM_BLUR,
            lens: None,
        }
    }

    pub(crate) fn draws(&self) -> bool {
        self.radius > 0.0 || self.lens.is_some()
    }

    pub(crate) fn radius(&self) -> f32 {
        self.radius * self.lens.map_or(1.0, Lens::frost)
    }

    pub(crate) fn lens(&self, scale: f32) -> Option<Lens> {
        Some(self.lens?.scaled(scale))
    }
}

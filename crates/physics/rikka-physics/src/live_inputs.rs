use rikka_model_core::Instance;
use rikka_physics_tailor::{Grab, Wind};

#[must_use]
#[derive(Clone, Debug)]
pub struct LiveInputs<'a> {
    pub(crate) wind: Wind,
    pub(crate) figures: Vec<(usize, &'a Instance)>,
    pub(crate) grabs: Vec<(usize, Grab)>,
}

impl Default for LiveInputs<'_> {
    fn default() -> Self {
        Self {
            wind: Wind::CALM,
            figures: Vec::new(),
            grabs: Vec::new(),
        }
    }
}

impl<'a> LiveInputs<'a> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn wind(mut self, wind: Wind) -> Self {
        self.wind = wind;
        self
    }

    pub fn figure(mut self, figure: usize, instance: &'a Instance) -> Self {
        self.figures.push((figure, instance));
        self
    }

    pub fn figures(mut self, figures: impl IntoIterator<Item = (usize, &'a Instance)>) -> Self {
        self.figures.extend(figures);
        self
    }

    pub fn grabs(mut self, grabs: impl IntoIterator<Item = (usize, Grab)>) -> Self {
        self.grabs.extend(grabs);
        self
    }
}

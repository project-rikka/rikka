use rikka_model_core::Instance;
use rikka_physics_cloth::{FigureId, Inputs};
use rikka_physics_tailor::{Grab, Wind};

#[derive(Debug, Default)]
pub(crate) struct ClothFigures {
    entries: Vec<(usize, FigureId)>,
}

impl ClothFigures {
    pub(crate) fn add(&mut self, figure: usize, id: FigureId) {
        self.entries.push((figure, id));
    }

    pub(crate) fn remove(&mut self, figure: usize) -> Option<FigureId> {
        let at = self.entries.iter().position(|(key, _)| *key == figure)?;
        Some(self.entries.remove(at).1)
    }

    pub(crate) fn id(&self, figure: usize) -> Option<FigureId> {
        self.entries
            .iter()
            .find(|(key, _)| *key == figure)
            .map(|(_, id)| *id)
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (usize, FigureId)> + '_ {
        self.entries.iter().copied()
    }

    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub(crate) fn inputs<'a>(
        &self,
        wind: Wind,
        instance: impl Fn(usize) -> Option<&'a Instance>,
        grabs: &[(usize, Grab)],
    ) -> Inputs {
        let mut inputs = Inputs::new().wind(wind);
        for (figure, id) in self.iter() {
            if let Some(instance) = instance(figure) {
                inputs = inputs.figure(id, instance);
            }
            for (grabbed, grab) in grabs {
                if *grabbed == figure {
                    inputs = inputs.grab(id, grab.clone());
                }
            }
        }
        inputs
    }
}

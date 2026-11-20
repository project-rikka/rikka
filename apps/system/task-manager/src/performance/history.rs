use std::rc::Rc;

pub(crate) const SPAN: usize = 60;

#[derive(Clone, Debug, Default)]
pub(crate) struct History {
    samples: Rc<Vec<f32>>,
}

impl History {
    pub(crate) fn push(&mut self, value: f32) {
        let samples = Rc::make_mut(&mut self.samples);
        if samples.len() == SPAN {
            samples.remove(0);
        }
        samples.push(value);
    }

    pub(crate) fn samples(&self) -> Rc<Vec<f32>> {
        Rc::clone(&self.samples)
    }

    pub(crate) fn peak(&self) -> f32 {
        self.samples.iter().copied().fold(0.0, f32::max)
    }
}

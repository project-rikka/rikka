pub(crate) struct Planes {
    planes: Vec<Vec<f32>>,
}

impl Planes {
    pub(crate) fn new(channels: usize) -> Planes {
        Planes {
            planes: vec![Vec::new(); channels],
        }
    }

    pub(crate) fn as_slice(&self) -> &[Vec<f32>] {
        &self.planes
    }

    pub(crate) fn frames(&self) -> usize {
        self.planes[0].len()
    }

    pub(crate) fn restart(&mut self, silence: usize) {
        for plane in &mut self.planes {
            plane.clear();
            plane.resize(silence, 0.0);
        }
    }

    pub(crate) fn push_interleaved(&mut self, samples: &[f32]) {
        let channels = self.planes.len();
        for (channel, plane) in self.planes.iter_mut().enumerate() {
            plane.extend(samples.iter().skip(channel).step_by(channels));
        }
    }

    pub(crate) fn pad(&mut self, silence: usize) {
        for plane in &mut self.planes {
            plane.resize(plane.len() + silence, 0.0);
        }
    }

    pub(crate) fn discard(&mut self, frames: usize) {
        for plane in &mut self.planes {
            plane.drain(..frames);
        }
    }
}

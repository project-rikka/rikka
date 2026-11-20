use cut_project::{Transition, TransitionKind};
use rikka_image_edit::{Depth, Imaging, Picture, Result};
use rikka_types::Color;

use super::{Backdrops, Mix, Timing};

#[derive(Clone, Debug)]
pub struct Swatch {
    imaging: Imaging,
    transition: Transition,
    frames: i64,
    scale: f32,
    seed: u32,
}

impl Swatch {
    #[must_use]
    pub fn new(imaging: &Imaging, transition: Transition) -> Swatch {
        Swatch {
            imaging: imaging.clone(),
            frames: transition.length().max(1),
            transition,
            scale: 1.0,
            seed: 0,
        }
    }

    #[must_use]
    pub fn frames(mut self, frames: i64) -> Swatch {
        self.frames = frames.max(1);
        self
    }

    #[must_use]
    pub fn scale(mut self, scale: f32) -> Swatch {
        self.scale = scale;
        self
    }

    #[must_use]
    pub fn seed(mut self, seed: u32) -> Swatch {
        self.seed = seed;
        self
    }

    pub fn render(&self, frame: i64, out: &Picture, into: &Picture) -> Result<Picture> {
        let (width, height) = out.dimensions();
        let blank = |depth| self.imaging.blank(width, height, depth, Color::TRANSPARENT);
        let backdrops = Backdrops {
            clear: blank(Depth::Eight)?,
            deep: match self.transition.kind {
                TransitionKind::FilmDissolve => Some(blank(Depth::Float)?),
                _ => None,
            },
        };
        let timing = Timing {
            step: frame.clamp(0, self.frames - 1),
            length: self.frames,
            seed: self.seed,
        };
        Mix::new(
            &self.transition,
            timing,
            self.scale,
            [true, true],
            backdrops,
        )
        .render(out, into)
    }
}

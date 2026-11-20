use rikka_svg::ColorScheme;
use rikka_types::Source;

use crate::budget::{Budget, Reservation};
use crate::loader::handle::{Decoded, JobState};
use crate::raster::Image;
use crate::read;
use crate::vector;

#[derive(Debug)]
pub(crate) enum Decode {
    Raster,
    Vector {
        width: u32,
        height: u32,
        scheme: ColorScheme,
    },
}

#[derive(Debug)]
pub(crate) struct Task {
    pub(crate) source: Source<'static>,
    pub(crate) decode: Decode,
    pub(crate) thumbnail: Option<(u32, u32)>,
}

impl Task {
    pub(crate) fn run(self, budget: &Budget, state: &JobState) -> Option<Decoded> {
        let read = match &self.source {
            Source::Bytes(bytes) => bytes.len(),
            Source::File(_) => 0,
        };
        let mut reservation = Reservation::empty();
        let mut admit = |decoded: usize| {
            reservation = budget.reserve(read.saturating_add(decoded));
            !state.is_cancelled()
        };
        let mut follows_scheme = false;
        let image = match self.decode {
            Decode::Raster => read::read(self.source).decode_admitted(admit),
            Decode::Vector {
                width,
                height,
                scheme,
            } => {
                let decoded = (width as usize)
                    .saturating_mul(height as usize)
                    .saturating_mul(4);
                match admit(decoded) {
                    false => Ok(None),
                    true => {
                        vector::rasterize(&self.source, width, height, scheme).map(|rasterized| {
                            follows_scheme = rasterized.follows_scheme;
                            Some(rasterized.image)
                        })
                    }
                }
            }
        };
        let image = match (image, self.thumbnail) {
            (Ok(None), _) => return None,
            (Ok(Some(image)), Some((width, height))) => image.thumbnail(width, height),
            (Ok(Some(image)), None) => Ok::<Image, _>(image),
            (Err(error), _) => Err(error),
        };
        Some(Decoded {
            image,
            follows_scheme,
            reservation,
        })
    }
}

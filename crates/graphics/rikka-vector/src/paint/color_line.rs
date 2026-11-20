use crate::color::{Encoder, Rgba, TRANSPARENT, linear_premultiplied, straight};
use crate::paint::gradient::{Interpolation, Spread, Stop};

#[derive(Clone, Copy, Debug)]
struct Resolved {
    offset: f32,
    color: Rgba,
}

#[derive(Debug)]
pub(crate) struct ColorLine {
    stops: Vec<Resolved>,
    spread: Spread,
    interpolation: Interpolation,
    encoder: Encoder,
}

impl Default for ColorLine {
    fn default() -> Self {
        Self {
            stops: Vec::new(),
            spread: Spread::Pad,
            interpolation: Interpolation::Srgb,
            encoder: Encoder::new(),
        }
    }
}

impl ColorLine {
    pub(crate) fn load(
        &mut self,
        stops: &[Stop],
        spread: Spread,
        interpolation: Interpolation,
    ) -> bool {
        self.stops.clear();
        self.spread = spread;
        self.interpolation = interpolation;
        self.stops.extend(stops.iter().map(|stop| Resolved {
            offset: stop.offset,
            color: match interpolation {
                Interpolation::Srgb => straight(stop.color),
                Interpolation::LinearLight => linear_premultiplied(stop.color),
            },
        }));
        self.stops
            .sort_by(|left, right| left.offset.total_cmp(&right.offset));
        !self.stops.is_empty()
    }

    #[inline]
    pub(crate) fn at(&self, t: f32) -> Rgba {
        let color = self.interpolated(t);
        match self.interpolation {
            Interpolation::Srgb => [
                color[0] * color[3],
                color[1] * color[3],
                color[2] * color[3],
                color[3],
            ],
            Interpolation::LinearLight => self.encoder.srgb_premultiplied(color),
        }
    }

    fn interpolated(&self, t: f32) -> Rgba {
        let (Some(first), Some(last)) = (self.stops.first(), self.stops.last()) else {
            return TRANSPARENT;
        };
        if !t.is_finite() {
            return if t > 0.0 { last.color } else { first.color };
        }
        let span = last.offset - first.offset;
        let t = if span <= 0.0 {
            t
        } else {
            match self.spread {
                Spread::Pad => t,
                Spread::Repeat => first.offset + (t - first.offset).rem_euclid(span),
                Spread::Reflect => {
                    let folded = (t - first.offset).rem_euclid(2.0 * span);
                    first.offset
                        + if folded > span {
                            2.0 * span - folded
                        } else {
                            folded
                        }
                }
            }
        };
        if t < first.offset {
            return first.color;
        }
        for pair in self.stops.windows(2) {
            let (low, high) = (pair[0], pair[1]);
            if t < high.offset {
                let fraction = (t - low.offset) / (high.offset - low.offset);
                return std::array::from_fn(|channel| {
                    low.color[channel] + (high.color[channel] - low.color[channel]) * fraction
                });
            }
        }
        last.color
    }
}

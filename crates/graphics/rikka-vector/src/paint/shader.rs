use rikka_types::Point;

use crate::color::{Rgba, TRANSPARENT, load, premultiplied};
use crate::paint::Paint;
use crate::paint::color_line::ColorLine;
use crate::paint::gradient::GradientShape;
use crate::paint::pattern::{Pattern, Sampling, Tiling};
use crate::pixmap::Pixmap;
use crate::transform::Transform;

const DEGENERATE: f32 = 1.0e-6;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Shading<'a> {
    shader: Shader<'a>,
    inverse: Transform,
}

impl<'a> Shading<'a> {
    pub(crate) fn new(
        paint: &Paint<'a>,
        device: &Transform,
        line: &mut ColorLine,
    ) -> Option<Shading<'a>> {
        match *paint {
            Paint::Solid(color) => Some(Shading {
                shader: Shader::Solid(premultiplied(color)),
                inverse: Transform::IDENTITY,
            }),
            Paint::Gradient(gradient) => {
                let inverse = gradient.transform.then(*device).inverse()?;
                if !line.load(gradient.stops, gradient.spread, gradient.interpolation) {
                    return None;
                }
                let shader = match gradient.shape {
                    GradientShape::Linear { start, end } => Shader::linear(start, end)?,
                    GradientShape::Radial {
                        start_center,
                        start_radius,
                        end_center,
                        end_radius,
                    } => Shader::radial(start_center, start_radius, end_center, end_radius)?,
                    GradientShape::Sweep {
                        center,
                        start_degrees,
                        end_degrees,
                    } => Shader::sweep(center, start_degrees, end_degrees),
                };
                Some(Shading { shader, inverse })
            }
            Paint::Pattern(pattern) => {
                let inverse = pattern.transform.then(*device).inverse()?;
                let usable = pattern.pixmap.width() > 0 && pattern.pixmap.height() > 0;
                usable.then_some(Shading {
                    shader: Shader::Pattern(pattern),
                    inverse,
                })
            }
        }
    }

    #[inline]
    pub(crate) fn solid(&self) -> Option<Rgba> {
        match self.shader {
            Shader::Solid(color) => Some(color),
            _ => None,
        }
    }

    #[inline]
    pub(crate) fn at(&self, line: &ColorLine, x: f32, y: f32) -> Rgba {
        self.shader
            .shade(line, self.inverse.apply(Point::new(x, y)))
    }
}

#[derive(Clone, Copy, Debug)]
enum Shader<'a> {
    Pattern(Pattern<'a>),
    Solid(Rgba),
    Linear {
        start: Point,
        direction: Point,
    },
    Radial {
        center: Point,
        radius: f32,
        center_delta: Point,
        radius_delta: f32,
        quadratic: f32,
    },
    Sweep {
        center: Point,
        start_degrees: f32,
        span_degrees: f32,
    },
}

impl Shader<'_> {
    fn linear(start: Point, end: Point) -> Option<Self> {
        let delta = Point::new(end.x - start.x, end.y - start.y);
        let length = delta.x * delta.x + delta.y * delta.y;
        (length > DEGENERATE).then(|| Self::Linear {
            start,
            direction: Point::new(delta.x / length, delta.y / length),
        })
    }

    fn radial(c0: Point, r0: f32, c1: Point, r1: f32) -> Option<Self> {
        let center_delta = Point::new(c1.x - c0.x, c1.y - c0.y);
        let radius_delta = r1 - r0;
        if center_delta.x.abs() < DEGENERATE
            && center_delta.y.abs() < DEGENERATE
            && radius_delta.abs() < DEGENERATE
        {
            return None;
        }
        Some(Self::Radial {
            center: c0,
            radius: r0,
            center_delta,
            radius_delta,
            quadratic: center_delta.x * center_delta.x + center_delta.y * center_delta.y
                - radius_delta * radius_delta,
        })
    }

    fn sweep(center: Point, start_degrees: f32, end_degrees: f32) -> Self {
        Self::Sweep {
            center,
            start_degrees,
            span_degrees: end_degrees - start_degrees,
        }
    }

    #[inline]
    fn shade(&self, line: &ColorLine, point: Point) -> Rgba {
        match *self {
            Self::Solid(color) => color,
            Self::Pattern(pattern) => sample(&pattern, point),
            Self::Linear { start, direction } => {
                line.at((point.x - start.x) * direction.x + (point.y - start.y) * direction.y)
            }
            Self::Radial {
                center,
                radius,
                center_delta,
                radius_delta,
                quadratic,
            } => {
                let offset = Point::new(point.x - center.x, point.y - center.y);
                let linear =
                    offset.x * center_delta.x + offset.y * center_delta.y + radius * radius_delta;
                let constant = offset.x * offset.x + offset.y * offset.y - radius * radius;
                match radial_parameter(quadratic, linear, constant, radius, radius_delta) {
                    Some(t) => line.at(t),
                    None => TRANSPARENT,
                }
            }
            Self::Sweep {
                center,
                start_degrees,
                span_degrees,
            } => {
                let angle = (point.y - center.y)
                    .atan2(point.x - center.x)
                    .to_degrees()
                    .rem_euclid(360.0);
                if span_degrees.abs() < DEGENERATE {
                    let t = if angle < start_degrees {
                        f32::NEG_INFINITY
                    } else {
                        f32::INFINITY
                    };
                    return line.at(t);
                }
                line.at((angle - start_degrees) / span_degrees)
            }
        }
    }
}

#[inline]
fn radial_parameter(a: f32, b: f32, c: f32, radius: f32, radius_delta: f32) -> Option<f32> {
    let valid = |t: f32| radius + t * radius_delta >= 0.0;
    if a.abs() < DEGENERATE {
        if b.abs() < DEGENERATE {
            return None;
        }
        let t = c / (2.0 * b);
        return valid(t).then_some(t);
    }
    let discriminant = b * b - a * c;
    if discriminant < 0.0 {
        return None;
    }
    let root = discriminant.sqrt();
    let (first, second) = ((b + root) / a, (b - root) / a);
    let (high, low) = if first > second {
        (first, second)
    } else {
        (second, first)
    };
    if valid(high) {
        Some(high)
    } else if valid(low) {
        Some(low)
    } else {
        None
    }
}

fn sample(pattern: &Pattern<'_>, point: Point) -> Rgba {
    let pixmap = pattern.pixmap;
    let (width, height) = (pixmap.width() as i64, pixmap.height() as i64);
    let inside =
        point.x >= 0.0 && point.y >= 0.0 && point.x < width as f32 && point.y < height as f32;
    if pattern.tiling == Tiling::Once && !inside {
        return TRANSPARENT;
    }
    if !(point.x + point.y).is_finite() {
        return TRANSPARENT;
    }
    let texel = |x: i64, y: i64| -> Rgba {
        let (x, y) = match pattern.tiling {
            Tiling::Once => (x.clamp(0, width - 1), y.clamp(0, height - 1)),
            Tiling::Repeat => (x.rem_euclid(width), y.rem_euclid(height)),
        };
        load(texel_at(pixmap, x as u32, y as u32))
    };
    let color = match pattern.sampling {
        Sampling::Nearest => texel(point.x.floor() as i64, point.y.floor() as i64),
        Sampling::Bilinear => {
            let (x, y) = (point.x - 0.5, point.y - 0.5);
            let (left, top) = (x.floor(), y.floor());
            let (fx, fy) = (x - left, y - top);
            let (left, top) = (left as i64, top as i64);
            let corners = [
                (texel(left, top), (1.0 - fx) * (1.0 - fy)),
                (texel(left + 1, top), fx * (1.0 - fy)),
                (texel(left, top + 1), (1.0 - fx) * fy),
                (texel(left + 1, top + 1), fx * fy),
            ];
            let mut mixed = TRANSPARENT;
            for (corner, weight) in corners {
                for (channel, value) in mixed.iter_mut().zip(corner) {
                    *channel += value * weight;
                }
            }
            mixed
        }
    };
    color.map(|channel| channel * pattern.opacity)
}

#[inline]
fn texel_at(pixmap: &Pixmap, x: u32, y: u32) -> [u8; 4] {
    let at = (y as usize * pixmap.width() as usize + x as usize) * 4;
    let data = pixmap.data();
    [data[at], data[at + 1], data[at + 2], data[at + 3]]
}

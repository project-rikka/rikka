use std::f32::consts::SQRT_2;

use rikka_types::Point;
use rikka_vector::Transform;

use crate::render::filters::{Image, premultiplied};
use crate::tree::filters::{Light, Lighting, Reflection};

#[derive(Clone, Copy, Debug)]
struct Vector {
    x: f32,
    y: f32,
    z: f32,
}

impl Vector {
    fn dot(self, other: Vector) -> f32 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    fn length(self) -> f32 {
        self.dot(self).sqrt()
    }

    fn normalized(self) -> Vector {
        let length = self.length();
        match length > 0.0 {
            true => Vector {
                x: self.x / length,
                y: self.y / length,
                z: self.z / length,
            },
            false => self,
        }
    }

    fn minus(self, other: Vector) -> Vector {
        Vector {
            x: self.x - other.x,
            y: self.y - other.y,
            z: self.z - other.z,
        }
    }
}

#[derive(Clone, Copy)]
enum Source {
    Distant(Vector),
    Point(Vector),
    Spot {
        position: Vector,
        direction: Vector,
        exponent: f32,
        cosine: Option<f32>,
    },
}

fn placed(light: Light, local: &Transform) -> Source {
    let depth = (local.xx * local.xx + local.yy * local.yy).sqrt() / SQRT_2;
    let point = |position: [f32; 3]| {
        let at = local.apply(Point::new(position[0], position[1]));
        Vector {
            x: at.x,
            y: at.y,
            z: position[2] * depth,
        }
    };
    match light {
        Light::Distant { azimuth, elevation } => {
            let (azimuth, elevation) = (azimuth.to_radians(), elevation.to_radians());
            Source::Distant(Vector {
                x: azimuth.cos() * elevation.cos(),
                y: azimuth.sin() * elevation.cos(),
                z: elevation.sin(),
            })
        }
        Light::Point { position } => Source::Point(point(position)),
        Light::Spot {
            position,
            target,
            exponent,
            cone,
        } => {
            let position = point(position);
            Source::Spot {
                position,
                direction: point(target).minus(position).normalized(),
                exponent,
                cosine: cone.map(|angle| angle.to_radians().cos()),
            }
        }
    }
}

fn alpha(image: &Image, x: usize, y: usize) -> f32 {
    f32::from(image.data[(y * image.width + x) * 4 + 3])
}

fn normal(image: &Image, x: usize, y: usize) -> (f32, f32) {
    let (width, height) = (image.width, image.height);
    let columns = match (x > 0, x + 1 < width) {
        (true, true) => (x - 1, x + 1),
        (false, true) => (x, x + 1),
        (true, false) => (x - 1, x),
        (false, false) => (x, x),
    };
    let rows = match (y > 0, y + 1 < height) {
        (true, true) => (y - 1, y + 1),
        (false, true) => (y, y + 1),
        (true, false) => (y - 1, y),
        (false, false) => (y, y),
    };
    let weighted = |first: usize, last: usize, center: usize| -> Vec<(usize, f32)> {
        match (first < center, last > center) {
            (true, true) => vec![(first, 1.0), (center, 2.0), (last, 1.0)],
            (false, true) => vec![(center, 2.0), (last, 1.0)],
            (true, false) => vec![(first, 1.0), (center, 2.0)],
            (false, false) => vec![(center, 1.0)],
        }
    };
    let horizontal = match columns.1 > columns.0 {
        true => {
            let taps = weighted(rows.0, rows.1, y);
            let total: f32 = taps.iter().map(|(_, weight)| weight).sum();
            let difference: f32 = taps
                .iter()
                .map(|(row, weight)| {
                    weight * (alpha(image, columns.1, *row) - alpha(image, columns.0, *row))
                })
                .sum();
            difference * 2.0 / (total * (columns.1 - columns.0) as f32)
        }
        false => 0.0,
    };
    let vertical = match rows.1 > rows.0 {
        true => {
            let taps = weighted(columns.0, columns.1, x);
            let total: f32 = taps.iter().map(|(_, weight)| weight).sum();
            let difference: f32 = taps
                .iter()
                .map(|(column, weight)| {
                    weight * (alpha(image, *column, rows.1) - alpha(image, *column, rows.0))
                })
                .sum();
            difference * 2.0 / (total * (rows.1 - rows.0) as f32)
        }
        false => 0.0,
    };
    (horizontal, vertical)
}

pub(crate) fn lit(input: &Image, lighting: &Lighting, local: &Transform, linear: bool) -> Image {
    let (width, height) = (input.width, input.height);
    let mut out = Image::blank(width, height, linear);
    if width < 2 || height < 2 {
        return out;
    }
    let source = placed(lighting.light, local);
    let color = premultiplied(lighting.color, linear).map(|channel| channel * 255.0);
    let scale = lighting.surface_scale;
    for (index, pixel) in out.data.as_chunks_mut::<4>().0.iter_mut().enumerate() {
        let (x, y) = (index % width, index / width);
        let surface = Vector {
            x: x as f32,
            y: y as f32,
            z: alpha(input, x, y) / 255.0 * scale,
        };
        let light = match source {
            Source::Distant(direction) => direction,
            Source::Point(position) | Source::Spot { position, .. } => {
                position.minus(surface).normalized()
            }
        };
        let tint = match source {
            Source::Spot {
                direction,
                exponent,
                cosine,
                ..
            } => {
                let facing = -light.dot(direction);
                match facing > 0.0 && cosine.is_none_or(|cosine| facing >= cosine) {
                    true => {
                        let factor = facing.powf(exponent);
                        color.map(|channel| (channel * factor).clamp(0.0, 255.0).round())
                    }
                    false => [0.0; 4],
                }
            }
            _ => color,
        };
        let (nx, ny) = normal(input, x, y);
        let surface_normal = Vector {
            x: -scale / 255.0 * nx,
            y: -scale / 255.0 * ny,
            z: 1.0,
        };
        let flat = nx == 0.0 && ny == 0.0;
        let factor = match lighting.reflection {
            Reflection::Diffuse { constant } => {
                let cosine = match flat {
                    true => light.z,
                    false => surface_normal.dot(light) / surface_normal.length(),
                };
                constant * cosine
            }
            Reflection::Specular { constant, exponent } => {
                let halfway = Vector {
                    x: light.x,
                    y: light.y,
                    z: light.z + 1.0,
                };
                let length = halfway.length();
                if length == 0.0 {
                    0.0
                } else {
                    let cosine = match flat {
                        true => halfway.z / length,
                        false => surface_normal.dot(halfway) / surface_normal.length() / length,
                    };
                    constant * cosine.powf(exponent)
                }
            }
        };
        let channel = |value: f32| (value * factor).clamp(0.0, 255.0).round() as u8;
        let (red, green, blue) = (channel(tint[0]), channel(tint[1]), channel(tint[2]));
        let alpha = match lighting.reflection {
            Reflection::Diffuse { .. } => 255,
            Reflection::Specular { .. } => red.max(green).max(blue),
        };
        *pixel = [red, green, blue, alpha];
    }
    out
}

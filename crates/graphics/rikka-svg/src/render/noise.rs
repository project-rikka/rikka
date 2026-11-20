use rikka_types::{Point, Rect};
use rikka_vector::Transform;

use crate::render::filters::Image;

const MODULUS: i32 = 2_147_483_647;
const MULTIPLIER: i32 = 16_807;
const QUOTIENT: i32 = 127_773;
const REMAINDER: i32 = 2_836;
const LATTICE: usize = 0x100;
const MASK: i32 = 0xFF;
const PERLIN: i32 = 0x1000;

struct Lattice {
    selector: Vec<usize>,
    gradients: [Vec<[f64; 2]>; 4],
}

#[derive(Clone, Copy)]
struct Stitch {
    width: i32,
    height: i32,
    wrap_x: i32,
    wrap_y: i32,
}

fn random(seed: i32) -> i32 {
    let next = MULTIPLIER * (seed % QUOTIENT) - REMAINDER * (seed / QUOTIENT);
    if next <= 0 { next + MODULUS } else { next }
}

impl Lattice {
    fn seeded(seed: i32) -> Lattice {
        let mut seed = match seed <= 0 {
            true => -(seed % (MODULUS - 1)) + 1,
            false => seed,
        }
        .min(MODULUS - 1);
        let mut selector = vec![0usize; LATTICE * 2 + 2];
        let mut gradients: [Vec<[f64; 2]>; 4] =
            std::array::from_fn(|_| vec![[0.0; 2]; LATTICE * 2 + 2]);
        for gradient in &mut gradients {
            for (index, slot) in selector.iter_mut().enumerate().take(LATTICE) {
                *slot = index;
                let mut vector = [0.0; 2];
                for component in &mut vector {
                    seed = random(seed);
                    *component =
                        f64::from((seed % (2 * LATTICE as i32)) - LATTICE as i32) / LATTICE as f64;
                }
                let length = vector[0].hypot(vector[1]);
                gradient[index] = [vector[0] / length, vector[1] / length];
            }
        }
        for index in (1..LATTICE).rev() {
            seed = random(seed);
            let other = (seed % LATTICE as i32) as usize;
            selector.swap(index, other);
        }
        for index in 0..LATTICE + 2 {
            selector[LATTICE + index] = selector[index];
            for gradient in &mut gradients {
                gradient[LATTICE + index] = gradient[index];
            }
        }
        Lattice {
            selector,
            gradients,
        }
    }

    fn noise(&self, channel: usize, x: f64, y: f64, stitch: Option<Stitch>) -> f64 {
        let shifted_x = x + f64::from(PERLIN);
        let shifted_y = y + f64::from(PERLIN);
        let (mut x0, mut y0) = (shifted_x as i32, shifted_y as i32);
        let (mut x1, mut y1) = (x0 + 1, y0 + 1);
        let fraction_x = shifted_x - f64::from(shifted_x as i32);
        let fraction_y = shifted_y - f64::from(shifted_y as i32);
        if let Some(stitch) = stitch {
            if x0 >= stitch.wrap_x {
                x0 -= stitch.width;
            }
            if x1 >= stitch.wrap_x {
                x1 -= stitch.width;
            }
            if y0 >= stitch.wrap_y {
                y0 -= stitch.height;
            }
            if y1 >= stitch.wrap_y {
                y1 -= stitch.height;
            }
        }
        let (x0, x1, y0, y1) = (
            (x0 & MASK) as usize,
            (x1 & MASK) as usize,
            (y0 & MASK) as usize,
            (y1 & MASK) as usize,
        );
        let left = self.selector[x0];
        let right = self.selector[x1];
        let corners = [
            self.selector[left + y0],
            self.selector[right + y0],
            self.selector[left + y1],
            self.selector[right + y1],
        ];
        let gradients = &self.gradients[channel];
        let curve = |t: f64| t * t * (3.0 - 2.0 * t);
        let mix = |t: f64, from: f64, to: f64| from + t * (to - from);
        let dot = |corner: usize, dx: f64, dy: f64| {
            let gradient = gradients[corner];
            dx * gradient[0] + dy * gradient[1]
        };
        let (sx, sy) = (curve(fraction_x), curve(fraction_y));
        let top = mix(
            sx,
            dot(corners[0], fraction_x, fraction_y),
            dot(corners[1], fraction_x - 1.0, fraction_y),
        );
        let bottom = mix(
            sx,
            dot(corners[2], fraction_x, fraction_y - 1.0),
            dot(corners[3], fraction_x - 1.0, fraction_y - 1.0),
        );
        mix(sy, top, bottom)
    }
}

fn stitched(frequency: f64, size: f64) -> f64 {
    if frequency == 0.0 {
        return frequency;
    }
    let low = (size * frequency).floor() / size;
    let high = (size * frequency).ceil() / size;
    match frequency / low < high / frequency {
        true => low,
        false => high,
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Noise {
    pub(crate) frequency: (f32, f32),
    pub(crate) octaves: u32,
    pub(crate) seed: i32,
    pub(crate) tile: Option<Rect>,
    pub(crate) fractal: bool,
}

pub(crate) fn turbulence(
    width: usize,
    height: usize,
    inverse: &Transform,
    noise: Noise,
    linear: bool,
) -> Image {
    let Noise {
        frequency,
        octaves,
        seed,
        tile,
        fractal,
    } = noise;
    let lattice = Lattice::seeded(seed);
    let mut image = Image::blank(width, height, linear);
    let (mut base_x, mut base_y) = (f64::from(frequency.0), f64::from(frequency.1));
    let stitch = tile.map(|tile| {
        let (tile_width, tile_height) = (f64::from(tile.width()), f64::from(tile.height()));
        base_x = stitched(base_x, tile_width);
        base_y = stitched(base_y, tile_height);
        let width = (tile_width * base_x + 0.5) as i32;
        let height = (tile_height * base_y + 0.5) as i32;
        Stitch {
            width,
            height,
            wrap_x: (f64::from(tile.min_x()) * base_x + f64::from(PERLIN) + f64::from(width))
                as i32,
            wrap_y: (f64::from(tile.min_y()) * base_y + f64::from(PERLIN) + f64::from(height))
                as i32,
        }
    });
    for (index, pixel) in image.data.as_chunks_mut::<4>().0.iter_mut().enumerate() {
        let device = Point::new((index % width) as f32, (index / width) as f32);
        let user = inverse.apply(device);
        let mut channels = [0.0f32; 4];
        for (channel, value) in channels.iter_mut().enumerate() {
            let (mut x, mut y) = (f64::from(user.x) * base_x, f64::from(user.y) * base_y);
            let mut stitch = stitch;
            let (mut sum, mut ratio) = (0.0, 1.0);
            for _ in 0..octaves {
                let noise = lattice.noise(channel, x, y, stitch);
                sum += match fractal {
                    true => noise,
                    false => noise.abs(),
                } / ratio;
                x *= 2.0;
                y *= 2.0;
                ratio *= 2.0;
                if let Some(stitch) = &mut stitch {
                    stitch.width *= 2;
                    stitch.wrap_x = 2 * stitch.wrap_x - PERLIN;
                    stitch.height *= 2;
                    stitch.wrap_y = 2 * stitch.wrap_y - PERLIN;
                }
            }
            let scaled = match fractal {
                true => (sum * 255.0 + 255.0) / 2.0,
                false => sum * 255.0,
            };
            *value = ((scaled as f32).clamp(0.0, 255.0) + 0.5).floor() / 255.0;
        }
        let alpha = channels[3];
        *pixel = [
            (channels[0] * alpha * 255.0 + 0.5) as u8,
            (channels[1] * alpha * 255.0 + 0.5) as u8,
            (channels[2] * alpha * 255.0 + 0.5) as u8,
            (alpha * 255.0 + 0.5) as u8,
        ];
    }
    image
}

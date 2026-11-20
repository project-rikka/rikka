use std::collections::BTreeMap;

use glam::Vec3;

use crate::body::Region;
use crate::emission::{Emission, Nozzle};
use crate::lattice::filled_sphere;
use crate::random::Random;

const BLOB_JITTER: f32 = 0.02;
const SURFACE: u32 = 1;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Carry {
    pub(crate) fraction: f64,
    pub(crate) units: u64,
}

pub(crate) struct Nozzles<'a> {
    pub(crate) spacing: f32,
    pub(crate) volume: f32,
    pub(crate) lift: f32,
    pub(crate) region: &'a dyn Fn(crate::body::RegionId) -> Option<(&'a Region, u32)>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct SpawnList {
    pub(crate) records: Vec<[u32; 12]>,
    pub(crate) ranges: Vec<(u32, u32)>,
}

struct NozzlePattern {
    layers: [Vec<(f32, f32)>; 3],
    drop: Vec<Vec3>,
}

impl NozzlePattern {
    fn new(nozzle: Nozzle, spacing: f32) -> NozzlePattern {
        NozzlePattern {
            layers: match nozzle {
                Nozzle::Stream { radius } => stacked(spacing, radius),
                _ => Default::default(),
            },
            drop: match nozzle {
                Nozzle::Drip { radius } => filled_sphere(spacing, radius),
                _ => Vec::new(),
            },
        }
    }
}

pub(crate) struct Window {
    pub(crate) start: f64,
    pub(crate) span: f64,
    pub(crate) substeps: u32,
    pub(crate) dt: f64,
}

impl SpawnList {
    pub(crate) fn fill(
        emissions: &[Emission],
        window: &Window,
        nozzles: &Nozzles<'_>,
        carries: &mut BTreeMap<u32, Carry>,
        room: u32,
    ) -> SpawnList {
        let mut book = SpawnList::default();
        let rounds = window.substeps.max(1);
        let patterns: Vec<NozzlePattern> = emissions
            .iter()
            .map(|emission| NozzlePattern::new(emission.nozzle, nozzles.spacing))
            .collect();
        for round in 0..rounds {
            let first = book.records.len() as u32;
            let round_start = window.start + window.dt * f64::from(round);
            for (emission, pattern) in emissions.iter().zip(&patterns) {
                let carry = carries.entry(emission.emitter).or_default();
                let mut writer = Writer {
                    book: &mut book,
                    emission,
                    room,
                };
                match emission.nozzle {
                    Nozzle::Blob => {
                        if round == 0 {
                            writer.blob(nozzles, carry);
                        }
                    }
                    _ if window.substeps == 0 => {}
                    nozzle => writer.flowing(nozzle, pattern, nozzles, carry, window, round_start),
                }
            }
            book.ranges.push((first, book.records.len() as u32 - first));
        }
        book
    }

    pub(crate) fn bytes(&self) -> &[u8] {
        bytemuck::cast_slice(&self.records)
    }
}

struct Writer<'a> {
    book: &'a mut SpawnList,
    emission: &'a Emission,
    room: u32,
}

impl Writer<'_> {
    fn full(&self) -> bool {
        self.book.records.len() as u32 >= self.room
    }

    fn direct(&mut self, position: Vec3, velocity: Vec3) {
        if self.full() {
            return;
        }
        let life = self.emission.lifetime.unwrap_or(0.0);
        self.book.records.push([
            position.x.to_bits(),
            position.y.to_bits(),
            position.z.to_bits(),
            life.to_bits(),
            velocity.x.to_bits(),
            velocity.y.to_bits(),
            velocity.z.to_bits(),
            0,
            self.emission.substance,
            0,
            0,
            0,
        ]);
    }

    fn blob(&mut self, nozzles: &Nozzles<'_>, carry: &mut Carry) {
        let radius = (3.0 * self.emission.amount.max(0.0) / (4.0 * std::f32::consts::PI)).cbrt();
        let mut random = Random::new(&[u64::from(self.emission.emitter), carry.units, 0xb10b]);
        carry.units += 1;
        let jitter = BLOB_JITTER * nozzles.spacing;
        for point in filled_sphere(nozzles.spacing, radius) {
            let shaken =
                point + Vec3::new(random.signed(), random.signed(), random.signed()) * jitter;
            self.direct(self.emission.to + shaken, self.emission.velocity);
        }
    }

    fn flowing(
        &mut self,
        nozzle: Nozzle,
        pattern: &NozzlePattern,
        nozzles: &Nozzles<'_>,
        carry: &mut Carry,
        window: &Window,
        round_start: f64,
    ) {
        let volume = f64::from(nozzles.volume);
        let rate = f64::from(self.emission.amount.max(0.0));
        let per_unit = match nozzle {
            Nozzle::Stream { .. } => volume * pattern.layers[0].len() as f64,
            Nozzle::Drip { radius } => {
                f64::from(4.0 / 3.0 * std::f32::consts::PI * radius.max(0.0).powi(3)).max(volume)
            }
            _ => volume,
        };
        let per_second = rate / per_unit;
        if per_second <= 0.0 || !per_second.is_finite() {
            return;
        }
        let total = carry.fraction + window.dt * per_second;
        let whole = total.floor();
        let units = whole as u64;
        let pending = carry.fraction;
        carry.fraction = total - whole;
        for unit in 0..units {
            if self.full() {
                carry.units += units - unit;
                return;
            }
            let delay = ((unit as f64 + 1.0 - pending) / per_second).clamp(0.0, window.dt);
            let at = round_start + delay;
            let mix = if window.span > 0.0 {
                ((at - window.start) / window.span).clamp(0.0, 1.0) as f32
            } else {
                1.0
            };
            let mouth = self.emission.from.lerp(self.emission.to, mix);
            let behind = self.emission.velocity * delay as f32;
            let base = mouth - behind;
            let index = carry.units;
            carry.units += 1;
            let mut random = Random::new(&[u64::from(self.emission.emitter), index]);
            match nozzle {
                Nozzle::Stream { .. } => self.layer(&pattern.layers[(index % 3) as usize], base),
                Nozzle::Shower { radius, spread } => self.shower(base, radius, spread, &mut random),
                Nozzle::Drip { .. } => {
                    for point in &pattern.drop {
                        let shaken = *point
                            + Vec3::new(random.signed(), random.signed(), random.signed())
                                * (BLOB_JITTER * nozzles.spacing);
                        self.direct(base + shaken, self.emission.velocity);
                    }
                }
                Nozzle::Surface { region, speed } => {
                    if let Some((region, base_triangle)) = (nozzles.region)(region) {
                        self.surface(region, base_triangle, speed, nozzles.lift, &mut random);
                    }
                }
                Nozzle::Blob => {}
            }
        }
    }

    fn layer(&mut self, layer: &[(f32, f32)], base: Vec3) {
        let (e1, e2) = orthonormal_basis(self.emission.velocity);
        for &(x, y) in layer {
            self.direct(base + e1 * x + e2 * y, self.emission.velocity);
        }
    }

    fn shower(&mut self, base: Vec3, radius: f32, spread: f32, random: &mut Random) {
        let speed = self.emission.velocity.length();
        let axis = if speed > 0.0 {
            self.emission.velocity / speed
        } else {
            Vec3::NEG_Y
        };
        let (e1, e2) = orthonormal_basis(axis);
        let reach = radius.max(0.0) * random.unit().sqrt();
        let angle = std::f32::consts::TAU * random.unit();
        let place = base + (e1 * angle.cos() + e2 * angle.sin()) * reach;
        let cone = 1.0 - random.unit() * (1.0 - spread.clamp(0.0, std::f32::consts::PI).cos());
        let side = (1.0 - cone * cone).max(0.0).sqrt();
        let around = std::f32::consts::TAU * random.unit();
        let direction = axis * cone + (e1 * around.cos() + e2 * around.sin()) * side;
        self.direct(place, direction * speed);
    }

    fn surface(
        &mut self,
        region: &Region,
        base_triangle: u32,
        speed: f32,
        lift: f32,
        random: &mut Random,
    ) {
        if self.full() {
            return;
        }
        let triangle = region.pick(random.unit()) + base_triangle;
        let root = random.unit().sqrt();
        let second = random.unit();
        let u = root * (1.0 - second);
        let v = root * second;
        let velocity = self.emission.velocity;
        let life = self.emission.lifetime.unwrap_or(0.0);
        self.book.records.push([
            u.to_bits(),
            v.to_bits(),
            lift.to_bits(),
            life.to_bits(),
            velocity.x.to_bits(),
            velocity.y.to_bits(),
            velocity.z.to_bits(),
            speed.to_bits(),
            self.emission.substance,
            SURFACE,
            triangle,
            0,
        ]);
    }
}

fn orthonormal_basis(direction: Vec3) -> (Vec3, Vec3) {
    let axis = direction.try_normalize().unwrap_or(Vec3::NEG_Y);
    let helper = if axis.y.abs() < 0.9 { Vec3::Y } else { Vec3::X };
    let e1 = axis.cross(helper).normalize();
    (e1, axis.cross(e1))
}

fn stacked(spacing: f32, radius: f32) -> [Vec<(f32, f32)>; 3] {
    let cell = spacing * spacing * 3f32.sqrt() * 0.5;
    let target_count = ((std::f32::consts::PI * radius * radius / cell).round() as usize).max(1);
    let row = spacing * 3f32.sqrt() * 0.5;
    let reach = (target_count as f32).sqrt().ceil() as i32 + 2;
    let shift = (0.5 * spacing, row / 3.0);
    [0.0, 1.0, 2.0].map(|step| {
        let mut points = Vec::new();
        for y in -reach..=reach {
            for x in -reach..=reach {
                let px = (x as f32 + 0.5 * y as f32) * spacing + shift.0 * step;
                let py = y as f32 * row + shift.1 * step;
                points.push((px, py));
            }
        }
        points.sort_by(|a, b| {
            (a.0 * a.0 + a.1 * a.1)
                .total_cmp(&(b.0 * b.0 + b.1 * b.1))
                .then(a.0.total_cmp(&b.0))
                .then(a.1.total_cmp(&b.1))
        });
        points.truncate(target_count);
        points
    })
}

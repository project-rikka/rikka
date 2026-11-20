use glam::Vec3;

use super::Emitter;
use super::appearance::Coloring;
use super::emission::{MAX_BURST_COUNT, MAX_BURSTS};
use super::forces::{Ground, GroundContact};
use super::noise::{Stream, curl, noise_offset};
use super::track::rotate;

const MAX_BOUNCES: u32 = 8;
const REST_SPEED: f32 = 0.05;
const NEWTON_STEPS: u32 = 12;
const SERIES_LIMIT: f32 = 0.1;
const STEEP: f32 = -1e-6;
const BURST_KEY: u32 = 0x8000_0000;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ParticleState {
    pub position: Vec3,
    pub velocity: Vec3,
    pub age: f32,
    pub lifetime: f32,
    pub size: f32,
    pub color: Vec3,
    pub opacity: f32,
    pub emission: f32,
    pub rotation: f32,
    pub frame: u32,
}

#[derive(Clone, Copy)]
struct Draws {
    life: f32,
    shape: [f32; 3],
    direction: [f32; 2],
    speed: f32,
    drag: f32,
    size: f32,
    rotation: f32,
    spin: f32,
    turn: f32,
    twinkle: f32,
    frame: f32,
    color: f32,
}

impl Draws {
    fn of(seed: u32, key: u32) -> Self {
        let mut stream = Stream::new(seed, key);
        stream.next();
        Self {
            life: stream.next(),
            shape: [stream.next(), stream.next(), stream.next()],
            direction: [stream.next(), stream.next()],
            speed: stream.next(),
            drag: stream.next(),
            size: stream.next(),
            rotation: stream.next(),
            spin: stream.next(),
            turn: stream.next(),
            twinkle: stream.next(),
            frame: stream.next(),
            color: stream.next(),
        }
    }
}

#[derive(Clone, Copy)]
struct Flight {
    position: Vec3,
    velocity: Vec3,
    rest: f32,
}

impl Emitter {
    pub fn capacity(&self) -> u32 {
        self.emission.slots() + self.emission.burst_particles()
    }

    pub fn particle(&self, index: u32, time: f32) -> Option<ParticleState> {
        let (key, age) = self.birth(index, time)?;
        let draws = Draws::of(self.seed, key);
        let emission = &self.emission;
        let lifetime =
            emission.lifetime * (1.0 - emission.lifetime_variance.clamp(0.0, 1.0) * draws.life);
        if !(age >= 0.0 && age <= lifetime) {
            return None;
        }
        let placement = self.track.placement(time - age);
        let (local, direction) = self.shape.sample(draws.shape, draws.direction);
        let origin = placement.translation + rotate(placement.rotation, placement.scale * local);
        let heading = rotate(placement.rotation, direction);
        let launch = &self.launch;
        let speed = launch.speed * (1.0 + launch.speed_variance * (2.0 * draws.speed - 1.0));
        let start = heading * speed + launch.velocity + placement.velocity * launch.inherit;
        let forces = &self.forces;
        let drag = (forces.drag * (1.0 + forces.drag_variance * (2.0 * draws.drag - 1.0))).max(0.0);
        let mut flight = fly(
            origin,
            start,
            forces.gravity,
            forces.wind,
            drag,
            self.ground.as_ref(),
            age,
        )?;

        let turbulence = forces.turbulence;
        if turbulence.strength != 0.0 {
            let active = age.min(flight.rest);
            let sample = origin * turbulence.frequency
                + noise_offset(self.seed)
                + Vec3::new(0.0, active * turbulence.flow, 0.0);
            let swirl = curl(sample);
            flight.position += swirl * (turbulence.strength * active);
            if age < flight.rest {
                flight.velocity += swirl * turbulence.strength;
            }
            if let Some(ground) = &self.ground {
                flight.position.y = flight.position.y.max(ground.height);
            }
        }
        if let Some(attractor) = forces.attractor {
            let fall = (-attractor.rate * age).exp();
            let pull = attractor.strength * (1.0 - fall);
            let toward = attractor.position - flight.position;
            flight.velocity = flight.velocity * (1.0 - pull)
                + toward * (attractor.strength * attractor.rate * fall);
            flight.position += toward * pull;
        }

        let look = &self.appearance;
        let normalized = (age / lifetime.max(1e-6)).clamp(0.0, 1.0);
        let wave =
            0.5 + 0.5 * (std::f32::consts::TAU * (look.twinkle.rate * age + draws.twinkle)).sin();
        let blink = wave * wave;
        let twinkle = 1.0 - look.twinkle.depth * (1.0 - blink * blink);
        let fade = match &self.ground {
            Some(ground) if ground.fade > 0.0 && flight.rest <= age => {
                (1.0 - (age - flight.rest) / ground.fade).clamp(0.0, 1.0)
            }
            _ => 1.0,
        };
        let spin = look.spin
            * (1.0 + look.spin_variance * (2.0 * draws.spin - 1.0))
            * if draws.turn < 0.5 { -1.0 } else { 1.0 };
        let frame = match look.sprite {
            Some(sprite) => {
                let frames = sprite.frames();
                let first = match sprite.random_start {
                    true => (draws.frame * frames as f32) as u32,
                    false => 0,
                };
                let step = match sprite.frame_rate > 0.0 {
                    true => (age * sprite.frame_rate) as u32,
                    false => ((normalized * frames as f32) as u32).min(frames - 1),
                };
                (first + step) % frames
            }
            None => 0,
        };
        Some(ParticleState {
            position: flight.position,
            velocity: flight.velocity,
            age,
            lifetime,
            size: look.size.sample(normalized)
                * (1.0 + look.size_variance * (2.0 * draws.size - 1.0)),
            color: look.color.sample(match look.coloring {
                Coloring::OverAge => normalized,
                Coloring::PerParticle => draws.color,
            }),
            opacity: look.opacity.sample(normalized) * twinkle * fade,
            emission: look.emission.sample(normalized) * twinkle,
            rotation: look.rotation * (2.0 * draws.rotation - 1.0) + spin * age,
            frame,
        })
    }

    fn birth(&self, index: u32, time: f32) -> Option<(u32, f32)> {
        let emission = &self.emission;
        let slots = emission.slots();
        if index < slots {
            let rate = emission.rate;
            let emitted = (time - emission.start) * rate;
            let mut key = latest(self.seed, index, slots, emitted)?;
            if emission.end.is_finite() {
                let last = latest(
                    self.seed,
                    index,
                    slots,
                    (emission.end - emission.start) * rate,
                )?;
                key = key.min(last);
            }
            let jitter = Stream::new(self.seed, key).next();
            return Some((key, (emitted - key as f32 - jitter) / rate));
        }
        let mut rest = index - slots;
        for (at, burst) in emission.bursts.iter().take(MAX_BURSTS).enumerate() {
            let count = burst.count.min(MAX_BURST_COUNT);
            if rest < count {
                let key = BURST_KEY | ((at as u32) << 24) | rest;
                let jitter = Stream::new(self.seed, key).next();
                let born = burst.time + burst.duration * (rest as f32 + jitter) / count as f32;
                return Some((key, time - born));
            }
            rest -= count;
        }
        None
    }
}

fn latest(seed: u32, slot: u32, slots: u32, emitted: f32) -> Option<u32> {
    let ahead = emitted - slot as f32;
    if ahead.is_nan() || ahead < 0.0 {
        return None;
    }
    let cycle = (ahead / slots as f32).floor() as u32;
    let key = cycle.wrapping_mul(slots).wrapping_add(slot);
    let jitter = Stream::new(seed, key).next();
    if key as f32 + jitter <= emitted {
        return Some(key);
    }
    match cycle {
        0 => None,
        _ => Some(key - slots),
    }
}

fn phi(x: f32) -> f32 {
    match x < SERIES_LIMIT {
        true => 1.0 + x * (-0.5 + x * (1.0 / 6.0 + x * (-1.0 / 24.0 + x * (1.0 / 120.0)))),
        false => (1.0 - (-x).exp()) / x,
    }
}

fn psi(x: f32) -> f32 {
    match x < SERIES_LIMIT {
        true => 0.5 + x * (-1.0 / 6.0 + x * (1.0 / 24.0 + x * (-1.0 / 120.0 + x * (1.0 / 720.0)))),
        false => (1.0 - phi(x)) / x,
    }
}

fn ballistic(x: Vec3, v: Vec3, gravity: Vec3, wind: Vec3, drag: f32, t: f32) -> (Vec3, Vec3) {
    let k = drag * t;
    let travel = t * phi(k);
    let fall = t * t * psi(k);
    (
        x + wind * t + (v - wind) * travel + gravity * fall,
        wind + (v - wind) * (-k).exp() + gravity * travel,
    )
}

fn height_at(height: f32, vy: f32, gy: f32, wy: f32, drag: f32, t: f32) -> f32 {
    let k = drag * t;
    height + wy * t + (vy - wy) * (t * phi(k)) + gy * (t * t * psi(k))
}

fn rise_at(vy: f32, gy: f32, wy: f32, drag: f32, t: f32) -> f32 {
    let k = drag * t;
    wy + (vy - wy) * (-k).exp() + gy * (t * phi(k))
}

fn contact(height: f32, vy: f32, gy: f32, wy: f32, drag: f32, span: f32) -> Option<f32> {
    if height < 0.0 || (height == 0.0 && vy <= 0.0) {
        return Some(0.0);
    }
    if span.is_nan() || span <= 0.0 {
        return None;
    }
    let bend = gy + drag * (wy - vy);
    let descend = |from: f32, limit: f32| {
        let mut t = from;
        for _ in 0..NEWTON_STEPS {
            let slope = rise_at(vy, gy, wy, drag, t);
            if slope >= STEEP {
                break;
            }
            t = (t - height_at(height, vy, gy, wy, drag, t) / slope).clamp(0.0, limit);
        }
        t
    };
    if bend <= 0.0 {
        if height_at(height, vy, gy, wy, drag, span) > 0.0 {
            return None;
        }
        return Some(descend(span, span));
    }
    if vy >= 0.0 {
        return None;
    }
    let mut lowest = span;
    if rise_at(vy, gy, wy, drag, span) > 0.0 {
        let mut t = 0.0;
        for _ in 0..NEWTON_STEPS {
            let slope = bend * (-drag * t).exp();
            t = (t - rise_at(vy, gy, wy, drag, t) / slope).clamp(0.0, span);
        }
        lowest = t;
    }
    if height_at(height, vy, gy, wy, drag, lowest) > 0.0 {
        return None;
    }
    Some(descend(0.0, lowest))
}

fn fly(
    origin: Vec3,
    start: Vec3,
    gravity: Vec3,
    wind: Vec3,
    drag: f32,
    ground: Option<&Ground>,
    age: f32,
) -> Option<Flight> {
    let Some(ground) = ground else {
        let (position, velocity) = ballistic(origin, start, gravity, wind, drag, age);
        return Some(Flight {
            position,
            velocity,
            rest: f32::INFINITY,
        });
    };
    let mut x = origin;
    let mut v = start;
    let mut begun = 0.0;
    let keep = 1.0 - ground.friction.clamp(0.0, 1.0);
    for bounce in 0..=MAX_BOUNCES {
        let span = age - begun;
        let Some(hit) = contact(x.y - ground.height, v.y, gravity.y, wind.y, drag, span) else {
            let (position, velocity) = ballistic(x, v, gravity, wind, drag, span);
            return Some(Flight {
                position,
                velocity,
                rest: f32::INFINITY,
            });
        };
        if ground.contact == GroundContact::Die {
            return None;
        }
        let (mut position, velocity) = ballistic(x, v, gravity, wind, drag, hit);
        position.y = ground.height;
        let up = -ground.restitution * velocity.y;
        if up < REST_SPEED || bounce == MAX_BOUNCES {
            return Some(Flight {
                position,
                velocity: Vec3::ZERO,
                rest: begun + hit,
            });
        }
        x = position;
        v = Vec3::new(velocity.x * keep, up, velocity.z * keep);
        begun += hit;
    }
    None
}

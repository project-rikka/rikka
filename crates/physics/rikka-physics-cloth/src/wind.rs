use glam::Vec3;
use rikka_physics_tailor::Wind;

const GUST_RATE: f64 = 0.35;
const GUST_SEED: u64 = 0x9e37_79b9_7f4a_7c15;

pub(crate) fn blowing(wind: &Wind, time: f64) -> (Vec3, f32) {
    let gust = 1.0 + wind.gust * noise(time * GUST_RATE);
    let velocity = wind.velocity() * gust.max(0.0);
    if velocity.is_finite() && wind.turbulence.is_finite() {
        (velocity, wind.turbulence.max(0.0))
    } else {
        (Vec3::ZERO, 0.0)
    }
}

fn noise(at: f64) -> f32 {
    if !at.is_finite() {
        return 0.0;
    }
    let cell = at.floor();
    let t = (at - cell) as f32;
    let eased = t * t * (3.0 - 2.0 * t);
    let left = lattice(cell as i64);
    let right = lattice(cell as i64 + 1);
    left + (right - left) * eased
}

fn lattice(cell: i64) -> f32 {
    let mut h = (cell as u64).wrapping_mul(GUST_SEED);
    h ^= h >> 31;
    h = h.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    h ^= h >> 29;
    (h >> 40) as f32 / (1u64 << 23) as f32 - 1.0
}

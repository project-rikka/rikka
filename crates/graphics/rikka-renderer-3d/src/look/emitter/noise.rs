use glam::{IVec3, Vec3};

const UNIT: f32 = 1.0 / 16_777_216.0;
const STREAM_SALT: u32 = 0x632B_E5AB;
const KEY_SPREAD: u32 = 0x9E37_79B9;
const OFFSET_SALT: u32 = 0x51ED_270B;
const OFFSET_REACH: f32 = 64.0;
const SECOND_FIELD: Vec3 = Vec3::new(31.416, 47.853, 12.679);
const THIRD_FIELD: Vec3 = Vec3::new(73.156, 9.731, 58.367);

const GRADIENTS: [Vec3; 12] = [
    Vec3::new(1.0, 1.0, 0.0),
    Vec3::new(-1.0, 1.0, 0.0),
    Vec3::new(1.0, -1.0, 0.0),
    Vec3::new(-1.0, -1.0, 0.0),
    Vec3::new(1.0, 0.0, 1.0),
    Vec3::new(-1.0, 0.0, 1.0),
    Vec3::new(1.0, 0.0, -1.0),
    Vec3::new(-1.0, 0.0, -1.0),
    Vec3::new(0.0, 1.0, 1.0),
    Vec3::new(0.0, -1.0, 1.0),
    Vec3::new(0.0, 1.0, -1.0),
    Vec3::new(0.0, -1.0, -1.0),
];

pub(crate) fn pcg(value: u32) -> u32 {
    let state = value.wrapping_mul(747_796_405).wrapping_add(2_891_336_453);
    let word = ((state >> ((state >> 28) + 4)) ^ state).wrapping_mul(277_803_737);
    (word >> 22) ^ word
}

fn unit(value: u32) -> f32 {
    (value >> 8) as f32 * UNIT
}

pub(crate) struct Stream(u32);

impl Stream {
    pub(crate) fn new(seed: u32, key: u32) -> Self {
        Self(pcg(
            key.wrapping_mul(KEY_SPREAD) ^ pcg(seed.wrapping_add(STREAM_SALT))
        ))
    }

    pub(crate) fn next(&mut self) -> f32 {
        self.0 = pcg(self.0);
        unit(self.0)
    }
}

pub(crate) fn noise_offset(seed: u32) -> Vec3 {
    let first = pcg(seed ^ OFFSET_SALT);
    let second = pcg(first);
    let third = pcg(second);
    Vec3::new(unit(first), unit(second), unit(third)) * OFFSET_REACH
}

fn gradient(cell: IVec3) -> Vec3 {
    let hash = pcg(cell.x as u32 ^ pcg(cell.y as u32 ^ pcg(cell.z as u32)));
    GRADIENTS[(hash % 12) as usize]
}

fn noise_gradient(p: Vec3) -> Vec3 {
    let floor = p.floor();
    let cell = floor.as_ivec3();
    let f = p - floor;
    let u = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);
    let du = 30.0 * f * f * (f * (f - 2.0) + 1.0);
    let ga = gradient(cell);
    let gb = gradient(cell + IVec3::new(1, 0, 0));
    let gc = gradient(cell + IVec3::new(0, 1, 0));
    let gd = gradient(cell + IVec3::new(1, 1, 0));
    let ge = gradient(cell + IVec3::new(0, 0, 1));
    let gf = gradient(cell + IVec3::new(1, 0, 1));
    let gg = gradient(cell + IVec3::new(0, 1, 1));
    let gh = gradient(cell + IVec3::new(1, 1, 1));
    let va = ga.dot(f);
    let vb = gb.dot(f - Vec3::new(1.0, 0.0, 0.0));
    let vc = gc.dot(f - Vec3::new(0.0, 1.0, 0.0));
    let vd = gd.dot(f - Vec3::new(1.0, 1.0, 0.0));
    let ve = ge.dot(f - Vec3::new(0.0, 0.0, 1.0));
    let vf = gf.dot(f - Vec3::new(1.0, 0.0, 1.0));
    let vg = gg.dot(f - Vec3::new(0.0, 1.0, 1.0));
    let vh = gh.dot(f - Vec3::new(1.0, 1.0, 1.0));
    let k1 = vb - va;
    let k2 = vc - va;
    let k3 = ve - va;
    let k4 = va - vb - vc + vd;
    let k5 = va - vc - ve + vg;
    let k6 = va - vb - ve + vf;
    let k7 = -va + vb + vc - vd + ve - vf - vg + vh;
    let blended = ga
        + u.x * (gb - ga)
        + u.y * (gc - ga)
        + u.z * (ge - ga)
        + u.x * u.y * (ga - gb - gc + gd)
        + u.y * u.z * (ga - gc - ge + gg)
        + u.z * u.x * (ga - gb - ge + gf)
        + u.x * u.y * u.z * (-ga + gb + gc - gd + ge - gf - gg + gh);
    let slope = Vec3::new(
        k1 + k4 * u.y + k6 * u.z + k7 * u.y * u.z,
        k2 + k5 * u.z + k4 * u.x + k7 * u.z * u.x,
        k3 + k6 * u.x + k5 * u.y + k7 * u.x * u.y,
    );
    blended + du * slope
}

pub(crate) fn curl(p: Vec3) -> Vec3 {
    let a = noise_gradient(p);
    let b = noise_gradient(p + SECOND_FIELD);
    let c = noise_gradient(p + THIRD_FIELD);
    Vec3::new(c.y - b.z, a.z - c.x, b.x - a.y)
}

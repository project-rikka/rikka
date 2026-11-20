use glam::{Vec2, Vec3};
use smallvec::SmallVec;

const EPSILON: f64 = f64::EPSILON * 0.5;
const ORIENT2D_BOUND: f64 = 8.0 * EPSILON;
const ORIENT3D_BOUND: f64 = 16.0 * EPSILON;

pub fn orient2d(a: Vec2, b: Vec2, c: Vec2) -> f64 {
    let terms = [
        f64::from(a.x) * f64::from(b.y),
        -(f64::from(a.x) * f64::from(c.y)),
        -(f64::from(b.x) * f64::from(a.y)),
        f64::from(b.x) * f64::from(c.y),
        f64::from(c.x) * f64::from(a.y),
        -(f64::from(c.x) * f64::from(b.y)),
    ];
    let estimate = terms.iter().sum::<f64>();
    let magnitude = terms.iter().map(|term| term.abs()).sum::<f64>();
    if estimate.abs() > ORIENT2D_BOUND * magnitude {
        return estimate;
    }
    exact_sum(&terms)
}

pub fn orient3d(a: Vec3, b: Vec3, c: Vec3, d: Vec3) -> f64 {
    let [ax, ay, az] = a.to_array().map(f64::from);
    let [bx, by, bz] = b.to_array().map(f64::from);
    let [cx, cy, cz] = c.to_array().map(f64::from);
    let [dx, dy, dz] = d.to_array().map(f64::from);
    let (adx, ady, adz) = (ax - dx, ay - dy, az - dz);
    let (bdx, bdy, bdz) = (bx - dx, by - dy, bz - dz);
    let (cdx, cdy, cdz) = (cx - dx, cy - dy, cz - dz);
    let estimate = adx * (bdy * cdz - bdz * cdy)
        + bdx * (cdy * adz - cdz * ady)
        + cdx * (ady * bdz - adz * bdy);
    let permanent = adx.abs() * ((bdy * cdz).abs() + (bdz * cdy).abs())
        + bdx.abs() * ((cdy * adz).abs() + (cdz * ady).abs())
        + cdx.abs() * ((ady * bdz).abs() + (adz * bdy).abs());
    if estimate.abs() > ORIENT3D_BOUND * permanent {
        return estimate;
    }
    orient3d_exact([ax, ay, az], [bx, by, bz], [cx, cy, cz], [dx, dy, dz])
}

fn orient3d_exact(a: [f64; 3], b: [f64; 3], c: [f64; 3], d: [f64; 3]) -> f64 {
    let mut terms: SmallVec<[f64; 48]> = SmallVec::new();
    let mut push_det = |p: [f64; 3], q: [f64; 3], r: [f64; 3], sign: f64| {
        let products = [
            (p[0] * q[1], r[2], 1.0),
            (p[0] * q[2], r[1], -1.0),
            (p[1] * q[2], r[0], 1.0),
            (p[1] * q[0], r[2], -1.0),
            (p[2] * q[0], r[1], 1.0),
            (p[2] * q[1], r[0], -1.0),
        ];
        for (pair, third, orientation) in products {
            let (high, low) = two_product(pair, third);
            terms.push(high * orientation * sign);
            terms.push(low * orientation * sign);
        }
    };
    push_det(a, b, c, 1.0);
    push_det(d, b, c, -1.0);
    push_det(a, d, c, -1.0);
    push_det(a, b, d, -1.0);
    exact_sum(&terms)
}

fn two_product(a: f64, b: f64) -> (f64, f64) {
    let product = a * b;
    (product, a.mul_add(b, -product))
}

fn two_sum(a: f64, b: f64) -> (f64, f64) {
    let sum = a + b;
    let virtual_b = sum - a;
    let virtual_a = sum - virtual_b;
    (sum, (a - virtual_a) + (b - virtual_b))
}

fn exact_sum(terms: &[f64]) -> f64 {
    let mut expansion: SmallVec<[f64; 64]> = SmallVec::new();
    for &term in terms {
        let mut carry = term;
        for component in expansion.iter_mut() {
            let (sum, error) = two_sum(carry, *component);
            *component = error;
            carry = sum;
        }
        expansion.push(carry);
        expansion.retain(|component| *component != 0.0);
    }
    expansion.last().copied().unwrap_or(0.0)
}

use std::cmp::Ordering;

use glam::{DVec3, Vec2, Vec3};

use crate::exact::approx::Approx;
use crate::exact::int::{Int, det3, det4};
use crate::exact::point::{Exact, Point};

#[derive(Clone, Copy, Debug)]
pub(crate) struct Space {
    shift: u32,
    f32_input: bool,
}

pub(crate) fn projection(axis: usize) -> (usize, usize) {
    ((axis + 1) % 3, (axis + 2) % 3)
}

fn lowest_bit_exponent(value: f64) -> i32 {
    let bits = value.to_bits();
    let exponent_bits = ((bits >> 52) & 0x7ff) as i32;
    let fraction = bits & ((1 << 52) - 1);
    let (mantissa, exponent) = match exponent_bits {
        0 => (fraction, -1074),
        _ => (fraction | (1 << 52), exponent_bits - 1075),
    };
    exponent + mantissa.trailing_zeros() as i32
}

impl Space {
    pub(crate) fn new(values: impl IntoIterator<Item = f64>, f32_input: bool) -> Space {
        let lowest = values
            .into_iter()
            .filter(|value| *value != 0.0 && value.is_finite())
            .map(lowest_bit_exponent)
            .min()
            .unwrap_or(0);
        Space {
            shift: (-lowest).max(0) as u32,
            f32_input,
        }
    }

    pub(crate) fn one(&self) -> Int {
        Int::power_of_two(self.shift)
    }

    fn int(&self, value: f64) -> Int {
        Int::from_scaled_f64(value, self.shift)
    }

    pub(crate) fn coords(&self, point: &Point) -> [Int; 4] {
        match &point.exact {
            Exact::Input => [
                self.int(point.approx.x),
                self.int(point.approx.y),
                self.int(point.approx.z),
                Int::from_i64(1),
            ],
            Exact::Homogeneous(coords) => (**coords).clone(),
        }
    }

    fn input_ints(&self, point: &Point) -> [Int; 3] {
        debug_assert!(point.is_input());
        [
            self.int(point.approx.x),
            self.int(point.approx.y),
            self.int(point.approx.z),
        ]
    }

    pub(crate) fn make(&self, coords: [Int; 4]) -> Point {
        Point::homogeneous(coords, self.shift)
    }

    fn approx(point: &Point, axis: usize) -> Approx {
        Approx::new(point.approx[axis], point.error)
    }

    pub(crate) fn orient3d(&self, a: &Point, b: &Point, c: &Point, d: &Point) -> i32 {
        let all_input = a.is_input() && b.is_input() && c.is_input() && d.is_input();
        if all_input && self.f32_input {
            let cast = |point: &Point| point.approx.as_vec3();
            return sign(rikka_spatial::orient3d(cast(a), cast(b), cast(c), cast(d)));
        }
        let diff = |p: &Point, axis: usize| Self::approx(p, axis) - Self::approx(d, axis);
        let (adx, ady, adz) = (diff(a, 0), diff(a, 1), diff(a, 2));
        let (bdx, bdy, bdz) = (diff(b, 0), diff(b, 1), diff(b, 2));
        let (cdx, cdy, cdz) = (diff(c, 0), diff(c, 1), diff(c, 2));
        let estimate = adx * (bdy * cdz - bdz * cdy)
            + bdx * (cdy * adz - cdz * ady)
            + cdx * (ady * bdz - adz * bdy);
        if let Some(result) = estimate.sign() {
            return result;
        }
        if estimate.exact_zero() {
            return 0;
        }
        det4(&[
            self.coords(a),
            self.coords(b),
            self.coords(c),
            self.coords(d),
        ])
        .signum()
    }

    pub(crate) fn orient2d(&self, axis: usize, a: &Point, b: &Point, c: &Point) -> i32 {
        let (u, v) = projection(axis);
        if a.is_input() && b.is_input() && c.is_input() && self.f32_input {
            let cast = |point: &Point| Vec2::new(point.approx[u] as f32, point.approx[v] as f32);
            return sign(rikka_spatial::orient2d(cast(a), cast(b), cast(c)));
        }
        let acx = Self::approx(a, u) - Self::approx(c, u);
        let acy = Self::approx(a, v) - Self::approx(c, v);
        let bcx = Self::approx(b, u) - Self::approx(c, u);
        let bcy = Self::approx(b, v) - Self::approx(c, v);
        let estimate = acx * bcy - acy * bcx;
        if let Some(result) = estimate.sign() {
            return result;
        }
        if estimate.exact_zero() {
            return 0;
        }
        let rows = [self.coords(a), self.coords(b), self.coords(c)];
        det3([
            [&rows[0][u], &rows[0][v], &rows[0][3]],
            [&rows[1][u], &rows[1][v], &rows[1][3]],
            [&rows[2][u], &rows[2][v], &rows[2][3]],
        ])
        .signum()
    }

    pub(crate) fn incircle(&self, axis: usize, a: &Point, b: &Point, c: &Point, d: &Point) -> i32 {
        let (u, v) = projection(axis);
        let diff = |p: &Point, k: usize| Self::approx(p, k) - Self::approx(d, k);
        let (adx, ady) = (diff(a, u), diff(a, v));
        let (bdx, bdy) = (diff(b, u), diff(b, v));
        let (cdx, cdy) = (diff(c, u), diff(c, v));
        let alift = adx * adx + ady * ady;
        let blift = bdx * bdx + bdy * bdy;
        let clift = cdx * cdx + cdy * cdy;
        let estimate = alift * (bdx * cdy - bdy * cdx)
            + blift * (cdx * ady - cdy * adx)
            + clift * (adx * bdy - ady * bdx);
        if let Some(result) = estimate.sign() {
            return result;
        }
        if estimate.exact_zero() {
            return 0;
        }
        let row = |point: &Point| {
            let [x, y, z, w] = self.coords(point);
            let coords = [x, y, z];
            let (pu, pv) = (&coords[u], &coords[v]);
            [pu * &w, pv * &w, &(pu * pu) + &(pv * pv), &w * &w]
        };
        det4(&[row(a), row(b), row(c), row(d)]).signum()
    }

    pub(crate) fn compare(&self, axis: usize, a: &Point, b: &Point) -> Ordering {
        let gap = a.approx[axis] - b.approx[axis];
        let error = a.error + b.error;
        if gap.abs() > error * 1.000001 {
            return gap.partial_cmp(&0.0).unwrap_or(Ordering::Equal);
        }
        if a.is_input() && b.is_input() {
            return a.approx[axis]
                .partial_cmp(&b.approx[axis])
                .unwrap_or(Ordering::Equal);
        }
        let first = self.coords(a);
        let second = self.coords(b);
        (&first[axis] * &second[3]).cmp(&(&second[axis] * &first[3]))
    }

    pub(crate) fn equal(&self, a: &Point, b: &Point) -> bool {
        let gap = (a.approx - b.approx).abs().max_element();
        if gap > (a.error + b.error) * 1.000001 {
            return false;
        }
        if a.is_input() && b.is_input() {
            return a.approx == b.approx;
        }
        (0..3).all(|axis| self.compare(axis, a, b) == Ordering::Equal)
    }

    pub(crate) fn lpi(&self, p: &Point, q: &Point, triangle: [&Point; 3]) -> Point {
        let [p, q] = [self.input_ints(p), self.input_ints(q)];
        let [a, b, c] = triangle.map(|point| self.input_ints(point));
        let normal = cross(&sub(&b, &a), &sub(&c, &a));
        let dp = dot(&normal, &sub(&p, &a));
        let dq = dot(&normal, &sub(&q, &a));
        let coord = |axis: usize| &(&q[axis] * &dp) - &(&p[axis] * &dq);
        self.make([coord(0), coord(1), coord(2), &dp - &dq])
    }

    pub(crate) fn tpi(&self, triangles: [[&Point; 3]; 3]) -> Point {
        let planes: Vec<([Int; 3], Int)> = triangles
            .iter()
            .map(|triangle| {
                let [a, b, c] = triangle.map(|point| self.input_ints(point));
                let normal = cross(&sub(&b, &a), &sub(&c, &a));
                let offset = dot(&normal, &a);
                (normal, offset)
            })
            .collect();
        let row = |index: usize, replace: Option<usize>| -> [&Int; 3] {
            let (normal, offset) = &planes[index];
            let mut entries = [&normal[0], &normal[1], &normal[2]];
            if let Some(column) = replace {
                entries[column] = offset;
            }
            entries
        };
        let solve =
            |replace: Option<usize>| det3([row(0, replace), row(1, replace), row(2, replace)]);
        self.make([solve(Some(0)), solve(Some(1)), solve(Some(2)), solve(None)])
    }

    pub(crate) fn ssi(&self, axis: usize, p1: &Point, q1: &Point, p2: &Point, q2: &Point) -> Point {
        let (u, v) = projection(axis);
        let [p1, q1, p2, q2] = [p1, q1, p2, q2].map(|point| self.input_ints(point));
        let r = sub(&q1, &p1);
        let s = sub(&q2, &p2);
        let w = sub(&p2, &p1);
        let denominator = &(&r[u] * &s[v]) - &(&r[v] * &s[u]);
        let numerator = &(&w[u] * &s[v]) - &(&w[v] * &s[u]);
        let coord = |k: usize| &(&p1[k] * &denominator) + &(&r[k] * &numerator);
        self.make([coord(0), coord(1), coord(2), denominator.clone()])
    }

    pub(crate) fn plane_line_axis(&self, a: [&Point; 3], b: [&Point; 3]) -> usize {
        let normal = |t: [&Point; 3]| (t[1].approx - t[0].approx).cross(t[2].approx - t[0].approx);
        let (na, nb) = (normal(a), normal(b));
        let direction = na.cross(nb);
        let magnitude = direction.abs();
        let axis = (0..3)
            .max_by(|&x, &y| magnitude[x].total_cmp(&magnitude[y]))
            .unwrap_or(0);
        if magnitude[axis] > 1.0e-6 * na.length() * nb.length() {
            return axis;
        }
        let exact_normal = |t: [&Point; 3]| {
            let [p, q, r] = t.map(|point| self.input_ints(point));
            cross(&sub(&q, &p), &sub(&r, &p))
        };
        let exact = cross(&exact_normal(a), &exact_normal(b));
        (0..3)
            .filter(|&k| !exact[k].is_zero())
            .max_by(|&x, &y| magnitude[x].total_cmp(&magnitude[y]))
            .unwrap_or(axis)
    }

    pub(crate) fn offset_x(&self, point: &Point, amount: i64) -> Point {
        let [x, y, z, w] = self.coords(point);
        let step = &self.one().mul_i64(amount) * &w;
        self.make([&x + &step, y, z, w])
    }

    pub(crate) fn input_point(&self, position: Vec3) -> Point {
        Point::input(position.as_dvec3())
    }

    pub(crate) fn input_point_f64(&self, position: DVec3) -> Point {
        Point::input(position)
    }
}

fn sign(value: f64) -> i32 {
    if value > 0.0 {
        1
    } else if value < 0.0 {
        -1
    } else {
        0
    }
}

fn sub(a: &[Int; 3], b: &[Int; 3]) -> [Int; 3] {
    [&a[0] - &b[0], &a[1] - &b[1], &a[2] - &b[2]]
}

fn cross(a: &[Int; 3], b: &[Int; 3]) -> [Int; 3] {
    [
        &(&a[1] * &b[2]) - &(&a[2] * &b[1]),
        &(&a[2] * &b[0]) - &(&a[0] * &b[2]),
        &(&a[0] * &b[1]) - &(&a[1] * &b[0]),
    ]
}

fn dot(a: &[Int; 3], b: &[Int; 3]) -> Int {
    &(&(&a[0] * &b[0]) + &(&a[1] * &b[1])) + &(&a[2] * &b[2])
}

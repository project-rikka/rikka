use std::cmp::Ordering;
use std::ops::{Add, Div, Mul, Sub};

use crate::exact::int::Int;
use crate::exact::{Point, Space};

#[derive(Clone, Debug)]
pub(crate) struct Rational {
    num: Int,
    den: Int,
}

impl Rational {
    pub(crate) fn new(num: Int, den: Int) -> Rational {
        match den.is_negative() {
            true => Rational {
                num: -num,
                den: -den,
            },
            false => Rational { num, den },
        }
    }

    pub(crate) fn zero() -> Rational {
        Rational::new(Int::zero(), Int::from_i64(1))
    }

    pub(crate) fn is_zero(&self) -> bool {
        self.num.is_zero()
    }

    pub(crate) fn abs(&self) -> Rational {
        match self.num.is_negative() {
            true => Rational {
                num: -self.num.clone(),
                den: self.den.clone(),
            },
            false => self.clone(),
        }
    }
}

impl Add for &Rational {
    type Output = Rational;

    fn add(self, other: &Rational) -> Rational {
        Rational::new(
            &(&self.num * &other.den) + &(&other.num * &self.den),
            &self.den * &other.den,
        )
    }
}

impl Sub for &Rational {
    type Output = Rational;

    fn sub(self, other: &Rational) -> Rational {
        Rational::new(
            &(&self.num * &other.den) - &(&other.num * &self.den),
            &self.den * &other.den,
        )
    }
}

impl Mul for &Rational {
    type Output = Rational;

    fn mul(self, other: &Rational) -> Rational {
        Rational::new(&self.num * &other.num, &self.den * &other.den)
    }
}

impl Div for &Rational {
    type Output = Rational;

    fn div(self, other: &Rational) -> Rational {
        Rational::new(&self.num * &other.den, &self.den * &other.num)
    }
}

impl PartialEq for Rational {
    fn eq(&self, other: &Rational) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Rational {}

impl PartialOrd for Rational {
    fn partial_cmp(&self, other: &Rational) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Rational {
    fn cmp(&self, other: &Rational) -> Ordering {
        (&self.num * &other.den).cmp(&(&other.num * &self.den))
    }
}

pub(crate) type RationalVector = [Rational; 3];

pub(crate) fn vector(space: &Space, point: &Point) -> RationalVector {
    let [x, y, z, w] = space.coords(point);
    [
        Rational::new(x, w.clone()),
        Rational::new(y, w.clone()),
        Rational::new(z, w),
    ]
}

pub(crate) fn sub(a: &RationalVector, b: &RationalVector) -> RationalVector {
    [&a[0] - &b[0], &a[1] - &b[1], &a[2] - &b[2]]
}

pub(crate) fn dot(a: &RationalVector, b: &RationalVector) -> Rational {
    &(&(&a[0] * &b[0]) + &(&a[1] * &b[1])) + &(&a[2] * &b[2])
}

pub(crate) fn scale(a: &RationalVector, factor: &Rational) -> RationalVector {
    [&a[0] * factor, &a[1] * factor, &a[2] * factor]
}

pub(crate) fn add(a: &RationalVector, b: &RationalVector) -> RationalVector {
    [&a[0] + &b[0], &a[1] + &b[1], &a[2] + &b[2]]
}

pub(crate) fn cross(a: &RationalVector, b: &RationalVector) -> RationalVector {
    [
        &(&a[1] * &b[2]) - &(&a[2] * &b[1]),
        &(&a[2] * &b[0]) - &(&a[0] * &b[2]),
        &(&a[0] * &b[1]) - &(&a[1] * &b[0]),
    ]
}

pub(crate) fn distance_squared(a: &RationalVector, b: &RationalVector) -> Rational {
    let d = sub(a, b);
    dot(&d, &d)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Closest {
    Vertex(usize),
    Edge(usize),
    Interior,
}

pub(crate) fn closest_on_triangle(
    p: &RationalVector,
    a: &RationalVector,
    b: &RationalVector,
    c: &RationalVector,
) -> (Rational, Closest) {
    let zero = Rational::zero();
    let ab = sub(b, a);
    let ac = sub(c, a);
    let ap = sub(p, a);
    let d1 = dot(&ab, &ap);
    let d2 = dot(&ac, &ap);
    if d1 <= zero && d2 <= zero {
        return (distance_squared(p, a), Closest::Vertex(0));
    }
    let bp = sub(p, b);
    let d3 = dot(&ab, &bp);
    let d4 = dot(&ac, &bp);
    if d3 >= zero && d4 <= d3 {
        return (distance_squared(p, b), Closest::Vertex(1));
    }
    let vc = &(&d1 * &d4) - &(&d3 * &d2);
    if vc <= zero && d1 >= zero && d3 <= zero {
        let v = &d1 / &(&d1 - &d3);
        let point = add(a, &scale(&ab, &v));
        return (distance_squared(p, &point), Closest::Edge(0));
    }
    let cp = sub(p, c);
    let d5 = dot(&ab, &cp);
    let d6 = dot(&ac, &cp);
    if d6 >= zero && d5 <= d6 {
        return (distance_squared(p, c), Closest::Vertex(2));
    }
    let vb = &(&d5 * &d2) - &(&d1 * &d6);
    if vb <= zero && d2 >= zero && d6 <= zero {
        let w = &d2 / &(&d2 - &d6);
        let point = add(a, &scale(&ac, &w));
        return (distance_squared(p, &point), Closest::Edge(2));
    }
    let va = &(&d3 * &d6) - &(&d5 * &d4);
    let d43 = &d4 - &d3;
    let d56 = &d5 - &d6;
    if va <= zero && d43 >= zero && d56 >= zero {
        let w = &d43 / &(&d43 + &d56);
        let bc = sub(c, b);
        let point = add(b, &scale(&bc, &w));
        return (distance_squared(p, &point), Closest::Edge(1));
    }
    let denominator = &(&va + &vb) + &vc;
    let v = &vb / &denominator;
    let w = &vc / &denominator;
    let point = add(&add(a, &scale(&ab, &v)), &scale(&ac, &w));
    (distance_squared(p, &point), Closest::Interior)
}

use glam::DVec3;

use crate::exact::int::Int;

const CONSTRUCTED_ERROR: f64 = 1.0 / (1u64 << 48) as f64;

#[derive(Clone, Debug)]
pub(crate) enum Exact {
    Input,
    Homogeneous(Box<[Int; 4]>),
}

#[derive(Clone, Debug)]
pub(crate) struct Point {
    pub(crate) approx: DVec3,
    pub(crate) error: f64,
    pub(crate) exact: Exact,
}

impl Point {
    pub(crate) fn input(position: DVec3) -> Point {
        Point {
            approx: position,
            error: 0.0,
            exact: Exact::Input,
        }
    }

    pub(crate) fn homogeneous(coords: [Int; 4], shift: u32) -> Point {
        let [x, y, z, w] = coords;
        let (x, y, z, w) = match w.is_negative() {
            true => (-x, -y, -z, -w),
            false => (x, y, z, w),
        };
        let approx = DVec3::new(
            crate::exact::int::ratio(&x, &w, shift),
            crate::exact::int::ratio(&y, &w, shift),
            crate::exact::int::ratio(&z, &w, shift),
        );
        Point {
            approx,
            error: approx.abs().max_element() * CONSTRUCTED_ERROR + f64::MIN_POSITIVE,
            exact: Exact::Homogeneous(Box::new([x, y, z, w])),
        }
    }

    pub(crate) fn is_input(&self) -> bool {
        matches!(self.exact, Exact::Input)
    }
}

mod decimate;
mod graph;
mod grid;
mod growth;
mod principal;
mod shell;
mod tree;
mod union;

use glam::{Vec2, Vec3};

pub(crate) use decimate::Decimator;
pub(crate) use graph::Graph;
pub(crate) use grid::PointGrid;
pub(crate) use growth::{Growth, followed};
pub(crate) use principal::Principal;
pub(crate) use shell::Shell;
pub(crate) use tree::{Hit, TriangleTree};
pub(crate) use union::{Union, groups};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Feature {
    Corner(usize),
    Edge(usize),
    Face,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Closest {
    pub(crate) point: Vec3,
    pub(crate) uv: Vec2,
    pub(crate) feature: Feature,
}

pub(crate) fn closest_on_triangle(p: Vec3, [a, b, c]: [Vec3; 3]) -> Closest {
    let at = |uv: Vec2, feature| Closest {
        point: a + (b - a) * uv.x + (c - a) * uv.y,
        uv,
        feature,
    };
    let ab = b - a;
    let ac = c - a;
    let ap = p - a;
    let d1 = ab.dot(ap);
    let d2 = ac.dot(ap);
    if d1 <= 0.0 && d2 <= 0.0 {
        return at(Vec2::ZERO, Feature::Corner(0));
    }
    let bp = p - b;
    let d3 = ab.dot(bp);
    let d4 = ac.dot(bp);
    if d3 >= 0.0 && d4 <= d3 {
        return at(Vec2::X, Feature::Corner(1));
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        let v = d1 / (d1 - d3);
        return at(Vec2::new(v, 0.0), Feature::Edge(0));
    }
    let cp = p - c;
    let d5 = ab.dot(cp);
    let d6 = ac.dot(cp);
    if d6 >= 0.0 && d5 <= d6 {
        return at(Vec2::Y, Feature::Corner(2));
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        let w = d2 / (d2 - d6);
        return at(Vec2::new(0.0, w), Feature::Edge(2));
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        let w = (d4 - d3) / ((d4 - d3) + (d5 - d6));
        return at(Vec2::new(1.0 - w, w), Feature::Edge(1));
    }
    let denominator = 1.0 / (va + vb + vc);
    at(Vec2::new(vb * denominator, vc * denominator), Feature::Face)
}

pub(crate) fn doubled_area_normal([a, b, c]: [Vec3; 3]) -> Vec3 {
    (b - a).cross(c - a)
}

pub(crate) fn area(corners: [Vec3; 3]) -> f32 {
    doubled_area_normal(corners).length() * 0.5
}

pub(crate) fn is_proper(corners: [Vec3; 3]) -> bool {
    let longest = corners[0]
        .distance_squared(corners[1])
        .max(corners[1].distance_squared(corners[2]))
        .max(corners[2].distance_squared(corners[0]));
    let doubled = doubled_area_normal(corners).length();
    corners.iter().all(|corner| corner.is_finite()) && doubled > 1e-7 * longest && doubled > 1e-14
}

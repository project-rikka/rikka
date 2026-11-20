use rikka_model_core::{Bounds, HumanBone};

use crate::figure::Figure;

use super::hard::is_hard;
use super::inner::innermost;

const SWUNG: f32 = 0.25;
const ON_HEAD: f32 = 0.8;
const SKIN: f32 = 0.6;
const SKIN_LEAST: usize = 24;
const DETAIL: f32 = 0.8;
const ON_FACE: f32 = 0.8;
const FACE_MARGIN: f32 = 0.02;
const CROWN: f32 = 0.015;
const BELOW_NECK: f32 = 0.05;
const SKULL: [HumanBone; 4] = [
    HumanBone::Head,
    HumanBone::Jaw,
    HumanBone::LeftEye,
    HumanBone::RightEye,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Class {
    Detail,
    Hair,
    Garment,
    Solid,
    Skin,
    Unknown,
}

impl Class {
    pub(crate) const RANKS: usize = 6;

    pub(crate) fn rank(self) -> u8 {
        match self {
            Class::Detail => 5,
            Class::Hair => 4,
            Class::Garment => 3,
            Class::Solid => 2,
            Class::Skin => 1,
            Class::Unknown => 0,
        }
    }

    pub(crate) fn is_hair(self) -> bool {
        self == Class::Hair
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct Tally {
    vertices: usize,
    hair: usize,
    garment: usize,
    head: usize,
    inner: usize,
}

pub(crate) fn classify(figure: &Figure<'_>, carried: &[bool]) -> Vec<Class> {
    let skeleton = &figure.skeleton;
    let anatomy = &figure.anatomy;
    let inner = innermost(figure, carried);
    let members = members(figure);
    let tallies: Vec<Tally> = members
        .iter()
        .map(|vertices| {
            let mut tally = Tally {
                vertices: vertices.len(),
                ..Tally::default()
            };
            for vertex in vertices {
                let strongest = figure.vertices[*vertex as usize].strongest();
                match figure.hung(strongest) {
                    Some(swing)
                        if swing
                            .anchor()
                            .is_some_and(|anchor| skeleton.under_head[anchor.index()]) =>
                    {
                        tally.hair += 1;
                    }
                    Some(_) => tally.garment += 1,
                    None if skeleton.under_head[strongest.index()] => tally.head += 1,
                    None => {}
                }
                tally.inner += usize::from(inner[*vertex as usize]);
            }
            tally
        })
        .collect();
    let skull = Bounds::from_points(
        figure
            .vertices
            .iter()
            .zip(&inner)
            .filter(|(vertex, inner)| {
                **inner
                    && anatomy
                        .part(vertex.strongest())
                        .is_some_and(|bone| SKULL.contains(&bone))
            })
            .map(|(vertex, _)| vertex.position),
    );
    let low = skull.min - FACE_MARGIN;
    let high = skull.max + FACE_MARGIN;
    let detail = |vertices: &[u32]| {
        let points = || {
            vertices
                .iter()
                .map(|vertex| figure.vertices[*vertex as usize].position)
        };
        let inside = points()
            .filter(|at| at.cmpge(low).all() && at.cmple(high).all())
            .count();
        !skull.is_empty()
            && Bounds::from_points(points()).longest() < DETAIL * skull.longest()
            && inside as f32 >= ON_FACE * vertices.len() as f32
    };
    let crown = figure
        .vertices
        .iter()
        .filter(|vertex| skeleton.under_head[vertex.strongest().index()])
        .map(|vertex| vertex.position.y)
        .fold(f32::MIN, f32::max);
    let neck = anatomy
        .node(HumanBone::Head)
        .map(|head| skeleton.origins[head.index()].y);
    let hairlike = |vertices: &[u32]| {
        let bounds = Bounds::from_points(
            vertices
                .iter()
                .map(|vertex| figure.vertices[*vertex as usize].position),
        );
        neck.is_some_and(|neck| bounds.max.y >= crown - CROWN || bounds.min.y < neck - BELOW_NECK)
    };
    figure
        .surfaces
        .iter()
        .zip(&tallies)
        .zip(&members)
        .map(|((surface, tally), vertices)| {
            let share = |count: usize| count as f32 / tally.vertices.max(1) as f32;
            let on_head = share(tally.head) >= ON_HEAD;
            let skin = tally.vertices >= SKIN_LEAST && share(tally.inner) >= SKIN;
            if is_hard(
                surface
                    .names
                    .iter()
                    .chain(&surface.mesh)
                    .map(String::as_str),
            ) {
                Class::Solid
            } else if share(tally.hair + tally.garment) >= SWUNG {
                if tally.hair >= tally.garment {
                    Class::Hair
                } else {
                    Class::Garment
                }
            } else if on_head && detail(vertices) {
                Class::Detail
            } else if skin {
                Class::Skin
            } else if on_head && hairlike(vertices) {
                Class::Hair
            } else {
                Class::Unknown
            }
        })
        .collect()
}

fn members(figure: &Figure<'_>) -> Vec<Vec<u32>> {
    let mut members: Vec<Vec<u32>> = vec![Vec::new(); figure.surfaces.len()];
    for triangle in &figure.triangles {
        members[triangle.surface as usize].extend(triangle.corners);
    }
    for vertices in &mut members {
        vertices.sort_unstable();
        vertices.dedup();
    }
    members
}

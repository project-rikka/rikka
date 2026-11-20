use rikka_physics_math::Vec3;

use super::boxed::{BOX_CORNERS, BOX_EDGES, BOX_FACES, corner_index};

#[derive(Clone, Copy, Debug)]
pub(crate) struct SidePlane {
    pub(crate) edge: u16,
    pub(crate) axis: usize,
    pub(crate) sign: f32,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct FaceLoop {
    pub(crate) edges: [u16; 4],
    pub(crate) corners: [usize; 4],
    pub(crate) sides: [SidePlane; 4],
}

const fn corner(index: u8) -> [i32; 3] {
    let point = BOX_CORNERS[index as usize];
    [point[0] as i32, point[1] as i32, point[2] as i32]
}

const fn face_loop(face: usize) -> FaceLoop {
    let axis = face / 2;
    let sign = if face % 2 == 1 { 1 } else { -1 };
    let mut normal = [0; 3];
    normal[axis] = sign;
    let mut out = FaceLoop {
        edges: [0; 4],
        corners: [0; 4],
        sides: [SidePlane {
            edge: 0,
            axis: 0,
            sign: 0.0,
        }; 4],
    };
    let mut edge = BOX_FACES[face];
    let mut slot = 0;
    while slot < 4 {
        let next = BOX_EDGES[edge as usize].next;
        let from = corner(BOX_EDGES[edge as usize].origin);
        let to = corner(BOX_EDGES[next as usize].origin);
        let d = [to[0] - from[0], to[1] - from[1], to[2] - from[2]];
        let binormal = [
            d[1] * normal[2] - d[2] * normal[1],
            d[2] * normal[0] - d[0] * normal[2],
            d[0] * normal[1] - d[1] * normal[0],
        ];
        let mut side = 0;
        while binormal[side] == 0 {
            side += 1;
        }
        out.edges[slot] = edge;
        out.corners[slot] = BOX_EDGES[next as usize].origin as usize;
        out.sides[slot] = SidePlane {
            edge,
            axis: side,
            sign: if binormal[side] > 0 { 1.0 } else { -1.0 },
        };
        edge = next;
        slot += 1;
    }
    out
}

pub(crate) const BOX_FACE_LOOPS: [FaceLoop; 6] = [
    face_loop(0),
    face_loop(1),
    face_loop(2),
    face_loop(3),
    face_loop(4),
    face_loop(5),
];

const AXIS_EDGES: [[u16; 8]; 3] = {
    let mut out = [[0; 8]; 3];
    let mut pair = 0;
    while pair < 12 {
        let from = corner(BOX_EDGES[2 * pair].origin);
        let to = corner(BOX_EDGES[2 * pair + 1].origin);
        let mut axis = 0;
        while from[axis] == to[axis] {
            axis += 1;
        }
        out[axis][BOX_EDGES[2 * pair].origin as usize] = 2 * pair as u16;
        out[axis][BOX_EDGES[2 * pair + 1].origin as usize] = 2 * pair as u16;
        pair += 1;
    }
    out
};

pub(crate) const BOX_EDGE_AXES: [usize; 12] = {
    let mut out = [0; 12];
    let mut pair = 0;
    while pair < 12 {
        let from = corner(BOX_EDGES[2 * pair].origin);
        let to = corner(BOX_EDGES[2 * pair + 1].origin);
        let mut axis = 0;
        while from[axis] == to[axis] {
            axis += 1;
        }
        out[pair] = axis;
        pair += 1;
    }
    out
};

#[inline]
pub(crate) fn box_edge(axis: usize, signs: Vec3) -> usize {
    AXIS_EDGES[axis][corner_index(signs) as usize] as usize
}

#[inline]
pub(crate) fn corner_point(corner: usize, half_extents: Vec3) -> Vec3 {
    Vec3::from(BOX_CORNERS[corner]) * half_extents
}

#[inline]
pub(crate) fn box_edge_corners(edge: usize) -> [usize; 2] {
    [
        BOX_EDGES[edge].origin as usize,
        BOX_EDGES[edge ^ 1].origin as usize,
    ]
}

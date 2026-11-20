use glam::{Vec2, Vec3};
use rikka_polymesh::Polymesh;

pub(crate) const CUBE_POINTS: [[f32; 3]; 8] = [
    [-1.0, -1.0, -1.0],
    [1.0, -1.0, -1.0],
    [1.0, 1.0, -1.0],
    [-1.0, 1.0, -1.0],
    [-1.0, -1.0, 1.0],
    [1.0, -1.0, 1.0],
    [1.0, 1.0, 1.0],
    [-1.0, 1.0, 1.0],
];

pub(crate) const CUBE_FACES: [[u32; 4]; 6] = [
    [0, 3, 2, 1],
    [4, 5, 6, 7],
    [0, 1, 5, 4],
    [1, 2, 6, 5],
    [2, 3, 7, 6],
    [3, 0, 4, 7],
];

pub(crate) fn points(list: &[[f32; 3]]) -> Vec<Vec3> {
    list.iter().copied().map(Vec3::from_array).collect()
}

pub(crate) fn cube() -> Polymesh {
    Polymesh::from_polygons(points(&CUBE_POINTS), CUBE_FACES)
}

pub(crate) fn prism(sides: u32) -> Polymesh {
    let mut positions = Vec::new();
    for z in [0.0, 1.0] {
        for side in 0..sides {
            let angle = std::f32::consts::TAU * side as f32 / sides as f32;
            positions.push(Vec3::new(angle.cos(), angle.sin(), z));
        }
    }
    let mut faces: Vec<Vec<u32>> = vec![(0..sides).rev().collect(), (sides..2 * sides).collect()];
    for side in 0..sides {
        let next = (side + 1) % sides;
        faces.push(vec![side, next, sides + next, sides + side]);
    }
    Polymesh::from_polygons(positions, faces)
}

pub(crate) fn vec3(values: &[[f32; 3]]) -> Vec<Vec3> {
    values.iter().copied().map(Vec3::from_array).collect()
}

pub(crate) fn vec2(values: &[[f32; 2]]) -> Vec<Vec2> {
    values.iter().copied().map(Vec2::from_array).collect()
}

pub(crate) fn assert_close(actual: &[Vec3], expected: &[Vec3], tolerance: f32) {
    assert_eq!(actual.len(), expected.len(), "lengths differ");
    for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
        assert!(
            actual.abs_diff_eq(*expected, tolerance),
            "element {index}: {actual} is not {expected}"
        );
    }
}

pub(crate) fn pair_set(mesh: &Polymesh, edges: impl Iterator<Item = usize>) -> Vec<[u32; 2]> {
    let mut pairs: Vec<[u32; 2]> = edges
        .map(|edge| rikka_polymesh::ordered(mesh.edges[edge]))
        .collect();
    pairs.sort_unstable();
    pairs
}

use std::sync::Arc;

use glam::{Mat4, Vec2, Vec3};
use rikka_mesh_modifiers::{
    CollectionRef, Context, Evaluated, MaterialRef, Modifier, ObjectRef, Purpose, Texture,
    TextureRef, evaluate,
};
use rikka_polymesh::Polymesh;

pub(crate) const TOLERANCE: f32 = 1.0e-5;

pub(crate) struct Oracle {
    pub(crate) positions: &'static [[f32; 3]],
    pub(crate) edges: &'static [[u32; 2]],
    pub(crate) faces: &'static [&'static [u32]],
    pub(crate) uvs: &'static [[f32; 2]],
    pub(crate) groups: &'static [&'static [(u32, f32)]],
}

pub(crate) struct Fixture {
    pub(crate) positions: &'static [[f32; 3]],
    pub(crate) edges: &'static [[u32; 2]],
    pub(crate) faces: &'static [&'static [u32]],
}

pub(crate) const CUBE: Fixture = Fixture {
    positions: &[
        [-1.0, -1.0, -1.0],
        [1.0, -1.0, -1.0],
        [1.0, 1.0, -1.0],
        [-1.0, 1.0, -1.0],
        [-1.0, -1.0, 1.0],
        [1.0, -1.0, 1.0],
        [1.0, 1.0, 1.0],
        [-1.0, 1.0, 1.0],
    ],
    edges: &[
        [0, 1],
        [0, 3],
        [2, 3],
        [1, 2],
        [4, 7],
        [4, 5],
        [5, 6],
        [6, 7],
        [0, 4],
        [1, 5],
        [2, 6],
        [3, 7],
    ],
    faces: &[
        &[0, 3, 2, 1],
        &[4, 5, 6, 7],
        &[0, 1, 5, 4],
        &[1, 2, 6, 5],
        &[2, 3, 7, 6],
        &[3, 0, 4, 7],
    ],
};

pub(crate) const GRID: Fixture = Fixture {
    positions: &[
        [0.0, 0.0, 0.0],
        [0.5, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [0.0, 0.5, 0.0],
        [0.5, 0.5, 0.0],
        [1.0, 0.5, 0.0],
        [0.0, 1.0, 0.0],
        [0.5, 1.0, 0.0],
        [1.0, 1.0, 0.0],
    ],
    edges: &[
        [0, 3],
        [0, 1],
        [1, 4],
        [3, 4],
        [1, 2],
        [2, 5],
        [4, 5],
        [3, 6],
        [4, 7],
        [6, 7],
        [5, 8],
        [7, 8],
    ],
    faces: &[&[0, 1, 4, 3], &[1, 2, 5, 4], &[3, 4, 7, 6], &[4, 5, 8, 7]],
};

pub(crate) const CYLINDER: Fixture = Fixture {
    positions: &[],
    edges: &[
        [0, 8],
        [0, 1],
        [1, 9],
        [8, 9],
        [1, 2],
        [2, 10],
        [9, 10],
        [2, 3],
        [3, 11],
        [10, 11],
        [3, 4],
        [4, 12],
        [11, 12],
        [4, 5],
        [5, 13],
        [12, 13],
        [5, 6],
        [6, 14],
        [13, 14],
        [6, 7],
        [7, 15],
        [14, 15],
        [0, 7],
        [8, 15],
    ],
    faces: &[
        &[0, 1, 9, 8],
        &[1, 2, 10, 9],
        &[2, 3, 11, 10],
        &[3, 4, 12, 11],
        &[4, 5, 13, 12],
        &[5, 6, 14, 13],
        &[6, 7, 15, 14],
        &[7, 0, 8, 15],
    ],
};

pub(crate) fn build(positions: Vec<Vec3>, edges: &[[u32; 2]], faces: &[&[u32]]) -> Polymesh {
    let mut mesh = Polymesh {
        positions,
        edges: edges.to_vec(),
        ..Polymesh::default()
    };
    for face in faces {
        mesh.corner_verts.extend_from_slice(face);
        mesh.face_offsets.push(mesh.corner_verts.len() as u32);
    }
    mesh.ensure_edges();
    assert_eq!(
        mesh.edges.len(),
        edges.len(),
        "fixture edges cover every face"
    );
    mesh
}

pub(crate) fn fixture(fixture: &Fixture) -> Polymesh {
    let positions = match fixture.positions.is_empty() {
        true => (0..16)
            .map(|index| {
                let angle = (index % 8) as f32 * std::f32::consts::FRAC_PI_4;
                Vec3::new(angle.cos(), angle.sin(), (index / 8) as f32)
            })
            .collect(),
        false => fixture.positions.iter().copied().map(Vec3::from).collect(),
    };
    build(positions, fixture.edges, fixture.faces)
}

pub(crate) fn moved(mut mesh: Polymesh, offset: Vec3, scale: Vec3) -> Polymesh {
    for position in &mut mesh.positions {
        *position = *position * scale + offset;
    }
    mesh
}

pub(crate) fn with_uvs(mut mesh: Polymesh) -> Polymesh {
    let uvs = mesh
        .corner_verts
        .iter()
        .map(|&point| {
            let position = mesh.positions[point as usize];
            Vec2::new(position.x * 0.4 + 0.1, position.y * 0.3 + 0.2)
        })
        .collect();
    mesh.add_uv_map("UVMap", uvs);
    mesh.uv_maps.set_active("UVMap");
    mesh
}

pub(crate) fn with_side_groups(mut mesh: Polymesh) -> Polymesh {
    let left = mesh.vertex_groups.add("Arm.L");
    let right = mesh.vertex_groups.add("Arm.R");
    mesh.vertex_groups.ensure_weights(mesh.point_count());
    for point in 0..mesh.point_count() {
        mesh.vertex_groups
            .set_weight(point, left, 0.1 * (point + 1) as f32);
        if point % 2 == 0 {
            mesh.vertex_groups
                .set_weight(point, right, 0.05 * (point + 1) as f32);
        }
    }
    mesh
}

#[derive(Default)]
pub(crate) struct Scene {
    pub(crate) matrix: Mat4,
    pub(crate) materials: usize,
    pub(crate) objects: Vec<(Mat4, Option<Arc<Polymesh>>)>,
    pub(crate) textures: Vec<Box<dyn Texture>>,
    pub(crate) slots: Option<Vec<Option<MaterialRef>>>,
    pub(crate) object_slots: Vec<Option<Vec<Option<MaterialRef>>>>,
    pub(crate) collections: Vec<Vec<ObjectRef>>,
}

impl Scene {
    pub(crate) fn with_object(mut self, matrix: Mat4, mesh: Option<Polymesh>) -> Scene {
        self.objects.push((matrix, mesh.map(Arc::new)));
        self
    }
}

impl Context for Scene {
    fn object_matrix(&self) -> Mat4 {
        self.matrix
    }

    fn material_count(&self) -> usize {
        self.materials
    }

    fn object(&self) -> Option<ObjectRef> {
        Some(ObjectRef::new(u64::MAX))
    }

    fn matrix(&self, object: ObjectRef) -> Option<Mat4> {
        self.objects
            .get(object.get() as usize)
            .map(|(matrix, _)| *matrix)
    }

    fn mesh(&self, object: ObjectRef) -> Option<Arc<Polymesh>> {
        self.objects
            .get(object.get() as usize)
            .and_then(|(_, mesh)| mesh.clone())
    }

    fn texture(&self, texture: TextureRef) -> Option<&dyn Texture> {
        self.textures
            .get(texture.get() as usize)
            .map(|texture| texture.as_ref())
    }

    fn materials(&self) -> Vec<Option<MaterialRef>> {
        self.slots
            .clone()
            .unwrap_or_else(|| vec![None; self.materials])
    }

    fn object_materials(&self, object: ObjectRef) -> Option<Vec<Option<MaterialRef>>> {
        self.object_slots
            .get(object.get() as usize)
            .cloned()
            .flatten()
    }

    fn collection_objects(&self, collection: CollectionRef) -> Option<Vec<ObjectRef>> {
        self.collections.get(collection.get() as usize).cloned()
    }
}

pub(crate) fn run(
    mesh: &Polymesh,
    modifier: impl Into<rikka_mesh_modifiers::ModifierKind>,
) -> Evaluated {
    run_in(mesh, modifier, &Scene::default())
}

pub(crate) fn run_in(
    mesh: &Polymesh,
    modifier: impl Into<rikka_mesh_modifiers::ModifierKind>,
    scene: &dyn Context,
) -> Evaluated {
    let stack = [Modifier::new("Modifier", modifier)];
    let evaluated = evaluate(mesh, &stack, scene, Purpose::Viewport);
    assert!(evaluated.notes.is_empty(), "{:?}", evaluated.notes);
    evaluated
}

pub(crate) fn assert_close(actual: &[Vec3], expected: &[[f32; 3]]) {
    assert_eq!(actual.len(), expected.len(), "point count");
    for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
        let expected = Vec3::from(*expected);
        assert!(
            actual.abs_diff_eq(expected, TOLERANCE),
            "point {index}: {actual} != {expected}"
        );
    }
}

pub(crate) fn assert_matches(mesh: &Polymesh, oracle: &Oracle) {
    assert_close(&mesh.positions, oracle.positions);
    let faces: Vec<Vec<u32>> = (0..mesh.face_count())
        .map(|face| mesh.face_verts(face).to_vec())
        .collect();
    let expected: Vec<Vec<u32>> = oracle.faces.iter().map(|face| face.to_vec()).collect();
    assert_eq!(faces, expected, "faces");
}

pub(crate) fn assert_edges(mesh: &Polymesh, oracle: &Oracle) {
    let mut actual: Vec<[u32; 2]> = mesh
        .edges
        .iter()
        .map(|&edge| rikka_polymesh::ordered(edge))
        .collect();
    let mut expected: Vec<[u32; 2]> = oracle
        .edges
        .iter()
        .map(|&edge| rikka_polymesh::ordered(edge))
        .collect();
    actual.sort_unstable();
    expected.sort_unstable();
    assert_eq!(actual, expected, "edges");
}

pub(crate) fn assert_uvs(mesh: &Polymesh, name: &str, expected: &[[f32; 2]]) {
    let uvs = mesh.uv_map(name).expect("the UV map survives");
    assert_eq!(uvs.len(), expected.len(), "corner count");
    for (index, (actual, expected)) in uvs.iter().zip(expected).enumerate() {
        let expected = Vec2::from(*expected);
        assert!(
            actual.abs_diff_eq(expected, TOLERANCE),
            "corner {index}: {actual} != {expected}"
        );
    }
}

pub(crate) struct Probe {
    pub(crate) positions: &'static [[f32; 3]],
    pub(crate) groups: &'static [&'static [(u32, f32)]],
    pub(crate) uvs: &'static [[f32; 2]],
    pub(crate) sharp_edges: &'static [bool],
    pub(crate) bevel_weights: &'static [f32],
    pub(crate) sharp_faces: &'static [bool],
    pub(crate) colors: &'static [[f32; 4]],
    pub(crate) normals: &'static [[f32; 3]],
}

pub(crate) fn placement(location: [f32; 3], rotation: [f32; 3], scale: [f32; 3]) -> Mat4 {
    let rotation = glam::Mat3::from_rotation_z(rotation[2])
        * glam::Mat3::from_rotation_y(rotation[1])
        * glam::Mat3::from_rotation_x(rotation[0]);
    Mat4::from_translation(Vec3::from(location))
        * Mat4::from_mat3(rotation)
        * Mat4::from_scale(Vec3::from(scale))
}

pub(crate) fn assert_groups(mesh: &Polymesh, expected: &[&[(u32, f32)]]) {
    assert_eq!(mesh.point_count(), expected.len(), "point count");
    for (point, expected) in expected.iter().enumerate() {
        let actual = mesh
            .vertex_groups
            .weights
            .get(point)
            .cloned()
            .unwrap_or_default();
        assert_eq!(
            actual.len(),
            expected.len(),
            "point {point}: {actual:?} != {expected:?}"
        );
        for &(group, weight) in expected.iter() {
            let found = mesh.vertex_groups.weight(point, group);
            assert!(
                (found - weight).abs() < TOLERANCE,
                "point {point} group {group}: {found} != {weight}"
            );
        }
    }
}

pub(crate) fn assert_floats(actual: &[f32], expected: &[f32]) {
    assert_eq!(actual.len(), expected.len(), "length");
    for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
        assert!(
            (actual - expected).abs() < TOLERANCE,
            "{index}: {actual} != {expected}"
        );
    }
}

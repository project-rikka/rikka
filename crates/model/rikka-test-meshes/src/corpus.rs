use std::f32::consts::TAU;

use glam::{Mat4, Vec2, Vec3, Vec4};
use rikka_mesh_edit::{EditMesh, NgonMethod, Primitive, QuadMethod};
use rikka_polymesh::{
    Attribute, BEVEL_WEIGHT_EDGE, BEVEL_WEIGHT_VERT, CREASE_EDGE, CREASE_VERT, CUSTOM_NORMAL,
    Domain, HIDE_EDGE, HIDE_POLY, HIDE_VERT, Influence, MATERIAL_INDEX, Polymesh, SHARP_EDGE,
    SHARP_FACE, ShapeKey, UV_SEAM, Values, VertexGroup,
};

use crate::rng::Rng;

#[derive(Clone, Debug)]
pub struct Fixture {
    pub name: String,
    pub mesh: Polymesh,
}

impl Fixture {
    pub fn new(name: impl Into<String>, mesh: Polymesh) -> Fixture {
        if let Err(error) = mesh.validate() {
            panic!("invalid fixture mesh: {error}");
        }
        Fixture {
            name: name.into(),
            mesh,
        }
    }

    #[must_use]
    pub fn edit_mesh(&self) -> EditMesh {
        let mut mesh = match EditMesh::from_polymesh(&self.mesh) {
            Ok(mesh) => mesh,
            Err(error) => panic!("fixture {} does not convert: {error}", self.name),
        };
        mesh.update_normals();
        mesh
    }
}

#[must_use]
pub fn corpus() -> Vec<Fixture> {
    vec![
        Fixture::new("empty", Polymesh::new()),
        Fixture::new("lone_vertex", loose(&[[0.0, 0.0, 0.0]], &[])),
        Fixture::new("wires", wires()),
        Fixture::new("triangle_sphere", triangle_sphere()),
        Fixture::new("character_blob", character_blob()),
        Fixture::new("mixed", mixed()),
        Fixture::new("concave_ngons", concave_ngons()),
        Fixture::new("fin", fin()),
        Fixture::new("bowtie", bowtie()),
        Fixture::new("doubled_faces", doubled_faces()),
        Fixture::new("shared_vertices", shared_vertices()),
        Fixture::new("open_grid", open_grid()),
        Fixture::new("degenerate", degenerate()),
        Fixture::new("coincident", coincident()),
        Fixture::new("layers", layers()),
    ]
}

fn mesh(positions: &[[f32; 3]], faces: &[&[u32]]) -> Polymesh {
    Polymesh::from_polygons(
        positions.iter().copied().map(Vec3::from_array).collect(),
        faces.iter().copied(),
    )
}

fn loose(positions: &[[f32; 3]], wires: &[[u32; 2]]) -> Polymesh {
    let mut result = mesh(positions, &[]);
    add_wires(&mut result, wires);
    result
}

fn add_wires(mesh: &mut Polymesh, wires: &[[u32; 2]]) {
    mesh.edges.extend_from_slice(wires);
    mesh.attributes.resize(Domain::Edge, mesh.edges.len());
}

fn add_points(mesh: &mut Polymesh, points: &[[f32; 3]]) -> u32 {
    let first = mesh.positions.len() as u32;
    mesh.positions
        .extend(points.iter().copied().map(Vec3::from_array));
    mesh.attributes.resize(Domain::Point, mesh.positions.len());
    first
}

fn from_edit(mesh: &EditMesh) -> Polymesh {
    mesh.to_polymesh()
}

fn wires() -> Polymesh {
    loose(
        &[
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.5],
            [2.0, 2.0, 2.0],
            [3.0, 0.0, 0.0],
        ],
        &[[0, 1], [1, 2], [2, 3], [3, 0], [0, 2], [4, 5]],
    )
}

fn triangle_sphere() -> Polymesh {
    let mut mesh = EditMesh::new();
    let added = mesh.add_primitive(
        &Primitive::IcoSphere {
            subdivisions: 2,
            radius: 1.0,
        },
        Mat4::IDENTITY,
        true,
    );
    if let Err(error) = added {
        panic!("ico sphere: {error}");
    }
    from_edit(&mesh)
}

fn character_blob() -> Polymesh {
    let mut mesh = EditMesh::new();
    let added = mesh.add_primitive(
        &Primitive::UvSphere {
            segments: 16,
            rings: 10,
            radius: 1.0,
        },
        Mat4::IDENTITY,
        true,
    );
    if let Err(error) = added {
        panic!("uv sphere: {error}");
    }
    let faces: Vec<_> = mesh.faces().collect();
    for face in faces {
        if mesh.face_len(face) > 3
            && let Err(error) = mesh.triangulate_face(face, QuadMethod::Beauty, NgonMethod::Beauty)
        {
            panic!("triangulate: {error}");
        }
    }
    let verts: Vec<_> = mesh.verts().collect();
    for vert in verts {
        let p = mesh.position(vert);
        let arm = (p.x.abs() * 3.0).min(1.0) * (1.0 - (p.z * 4.0).abs()).max(0.0);
        let shaped = Vec3::new(
            p.x * (0.6 + arm * 1.4),
            p.y * 0.45,
            p.z * 1.8 + (p.x * 5.0).sin() * 0.05,
        );
        mesh.set_position(vert, shaped);
    }
    from_edit(&mesh)
}

fn mixed() -> Polymesh {
    mesh(
        &[
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [2.0, 0.0, 0.1],
            [3.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [1.0, 1.0, 0.2],
            [2.0, 1.0, 0.0],
            [3.0, 1.2, 0.0],
            [0.5, 2.0, 0.0],
            [1.5, 2.2, 0.1],
            [2.5, 2.0, 0.0],
            [3.5, 2.5, 0.0],
            [1.8, 1.5, 0.0],
            [4.0, 1.0, 0.0],
            [4.5, 2.0, 0.6],
            [4.0, 3.0, -0.4],
        ],
        &[
            &[0, 1, 5, 4],
            &[1, 2, 5],
            &[2, 6, 5],
            &[2, 3, 7, 6],
            &[3, 13, 7],
            &[4, 5, 12, 9, 8],
            &[5, 6, 7, 11, 10, 12],
            &[7, 13, 14, 15, 11],
        ],
    )
}

fn concave_ngons() -> Polymesh {
    let mut positions = Vec::new();
    for index in 0..10 {
        let angle = TAU * index as f32 / 10.0;
        let radius = if index % 2 == 0 { 1.0 } else { 0.4 };
        positions.push([angle.cos() * radius, angle.sin() * radius, 0.0]);
    }
    for index in 0..8 {
        let angle = TAU * index as f32 / 8.0;
        let z = if index % 2 == 0 { 0.5 } else { -0.5 };
        positions.push([3.0 + angle.cos(), angle.sin(), z]);
    }
    positions.extend_from_slice(&[
        [5.0, 0.0, 0.0],
        [7.0, 0.0, 0.0],
        [7.0, 2.0, 0.0],
        [6.0, 0.3, 0.0],
        [5.0, 2.0, 0.0],
    ]);
    let star: Vec<u32> = (0..10).collect();
    let zigzag: Vec<u32> = (10..18).collect();
    mesh(&positions, &[&star, &zigzag, &[18, 19, 20, 21, 22]])
}

fn fin() -> Polymesh {
    mesh(
        &[
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.5, 1.0, 0.0],
            [0.5, -1.0, 0.0],
            [0.5, 0.0, 1.0],
            [1.5, 1.0, 0.0],
        ],
        &[&[0, 1, 2], &[1, 0, 3], &[0, 1, 4], &[1, 5, 2]],
    )
}

fn bowtie() -> Polymesh {
    let mut result = mesh(
        &[
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [2.0, 1.0, 0.0],
            [2.0, 2.0, 0.0],
            [1.0, 2.0, 0.0],
            [3.0, 3.0, 0.0],
        ],
        &[&[0, 1, 2, 3], &[2, 4, 5, 6]],
    );
    add_wires(&mut result, &[[5, 7], [3, 6]]);
    add_points(&mut result, &[[-1.0, -1.0, 0.0], [4.0, 0.0, 1.0]]);
    result
}

fn doubled_faces() -> Polymesh {
    mesh(
        &[
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.3, 1.0, 0.2],
            [2.0, 0.0, 0.0],
            [3.0, 0.0, 0.0],
            [3.0, 1.0, 0.0],
            [2.0, 1.0, 0.0],
        ],
        &[
            &[0, 1, 2],
            &[0, 2, 1],
            &[3, 4, 5, 6],
            &[3, 6, 5, 4],
            &[1, 3, 6, 2],
        ],
    )
}

fn shared_vertices() -> Polymesh {
    mesh(
        &[
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [2.0, 0.5, 0.3],
        ],
        &[&[0, 1, 2, 3], &[0, 2, 1, 3], &[1, 4, 2]],
    )
}

fn grid(columns: u32, rows: u32, size: f32) -> (Vec<[f32; 3]>, Vec<Vec<u32>>) {
    let mut positions = Vec::new();
    for row in 0..=rows {
        for column in 0..=columns {
            positions.push([
                column as f32 * size,
                row as f32 * size,
                ((column * 7 + row * 3) % 5) as f32 * 0.02,
            ]);
        }
    }
    let width = columns + 1;
    let mut faces = Vec::new();
    for row in 0..rows {
        for column in 0..columns {
            let a = row * width + column;
            faces.push(vec![a, a + 1, a + width + 1, a + width]);
        }
    }
    (positions, faces)
}

fn open_grid() -> Polymesh {
    let (positions, mut faces) = grid(5, 4, 0.5);
    faces.remove(12);
    faces.remove(6);
    let quad = faces.remove(0);
    faces.push(vec![quad[0], quad[1], quad[2]]);
    let slices: Vec<&[u32]> = faces.iter().map(Vec::as_slice).collect();
    mesh(&positions, &slices)
}

fn degenerate() -> Polymesh {
    mesh(
        &[
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [3.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [1.0, 1.0, 0.0],
            [1.000_001, 1.0, 0.0],
            [1.0, 1.000_001, 0.0],
            [5.0, 5.0, 5.0],
            [5.0, 5.0, 5.0],
            [5.0, 5.0, 5.0],
            [2.0, 2.0, 0.0],
        ],
        &[
            &[0, 1, 2, 3],
            &[1, 0, 4],
            &[4, 6, 7],
            &[8, 9, 10],
            &[2, 11, 5, 1],
            &[3, 2, 1, 0, 11],
        ],
    )
}

fn coincident() -> Polymesh {
    mesh(
        &[
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [1.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [2.0, 1.0, 0.0],
            [1.0, 1.0, 0.0],
            [1.0, 1.0, 0.0],
            [1.0, 2.0, 0.0],
            [0.0, 2.0, 0.0],
        ],
        &[&[0, 1, 2, 3], &[4, 5, 6, 7], &[3, 8, 9, 10]],
    )
}

fn layers() -> Polymesh {
    let (positions, mut faces) = grid(4, 3, 1.0);
    let quad = faces.remove(5);
    faces.push(vec![quad[0], quad[1], quad[2]]);
    faces.push(vec![quad[0], quad[2], quad[3]]);
    let slices: Vec<&[u32]> = faces.iter().map(Vec::as_slice).collect();
    let mut result = mesh(&positions, &slices);
    let mut rng = Rng::new(17);
    let points = result.point_count();
    let edges = result.edge_count();
    let face_count = result.face_count();
    let corners = result.corner_count();
    let first: Vec<Vec2> = result
        .corner_verts
        .iter()
        .map(|&point| result.positions[point as usize].truncate() * 0.2)
        .collect();
    let second: Vec<Vec2> = (0..corners)
        .map(|_| Vec2::new(rng.unit(), rng.unit()))
        .collect();
    result.add_uv_map("UVMap", first);
    result.add_uv_map("Second", second);
    for (index, pinned) in result.uv_pins_mut("UVMap").iter_mut().enumerate() {
        *pinned = index % 7 == 0;
    }
    for (index, pinned) in result.uv_pins_mut("Second").iter_mut().enumerate() {
        *pinned = index % 5 == 1;
    }
    let seams: Vec<bool> = (0..edges).map(|index| index % 4 == 0).collect();
    result
        .attributes
        .insert(Attribute::new(UV_SEAM, Domain::Edge, Values::Bool(seams)));
    let point_colors: Vec<Vec4> = (0..points)
        .map(|_| Vec4::new(rng.unit(), rng.unit(), rng.unit(), 1.0))
        .collect();
    let corner_colors: Vec<Vec4> = (0..corners)
        .map(|_| Vec4::new(rng.unit(), rng.unit(), rng.unit(), rng.unit()))
        .collect();
    result.add_color_attribute("Col", Domain::Point, point_colors);
    result.add_color_attribute("Paint", Domain::Corner, corner_colors);
    let floats = |rng: &mut Rng, count: usize| -> Values {
        Values::Float((0..count).map(|_| rng.unit()).collect())
    };
    let crease_vert = floats(&mut rng, points);
    let crease_edge = floats(&mut rng, edges);
    let bevel_vert = floats(&mut rng, points);
    let bevel_edge = floats(&mut rng, edges);
    for (name, domain, values) in [
        (CREASE_VERT, Domain::Point, crease_vert),
        (CREASE_EDGE, Domain::Edge, crease_edge),
        (BEVEL_WEIGHT_VERT, Domain::Point, bevel_vert),
        (BEVEL_WEIGHT_EDGE, Domain::Edge, bevel_edge),
    ] {
        result
            .attributes
            .insert(Attribute::new(name, domain, values));
    }
    let normals: Vec<Vec3> = (0..corners)
        .map(|_| (Vec3::Z + rng.direction() * 0.3).normalize())
        .collect();
    result.attributes.insert(Attribute::new(
        CUSTOM_NORMAL,
        Domain::Corner,
        Values::Vec3(normals),
    ));
    let materials: Vec<i32> = (0..face_count).map(|index| (index % 3) as i32).collect();
    result.attributes.insert(Attribute::new(
        MATERIAL_INDEX,
        Domain::Face,
        Values::Int(materials),
    ));
    let sharp_edges: Vec<bool> = (0..edges).map(|index| index % 5 == 2).collect();
    let sharp_faces: Vec<bool> = (0..face_count).map(|index| index % 2 == 0).collect();
    result.attributes.insert(Attribute::new(
        SHARP_EDGE,
        Domain::Edge,
        Values::Bool(sharp_edges),
    ));
    result.attributes.insert(Attribute::new(
        SHARP_FACE,
        Domain::Face,
        Values::Bool(sharp_faces),
    ));
    result.vertex_groups.groups = vec![VertexGroup::new("Upper"), VertexGroup::new("Left")];
    result.vertex_groups.active = Some(1);
    result.vertex_groups.weights = result
        .positions
        .iter()
        .map(|position| {
            let mut influences = vec![Influence {
                group: 0,
                weight: (position.y / 3.0).clamp(0.0, 1.0),
            }];
            if position.x < 2.0 {
                influences.push(Influence {
                    group: 1,
                    weight: 1.0 - position.x * 0.5,
                });
            }
            influences
        })
        .collect();
    let basis = result.positions.clone();
    let raised: Vec<Vec3> = basis.iter().map(|&p| p + Vec3::Z * p.x * 0.1).collect();
    let twisted: Vec<Vec3> = basis
        .iter()
        .map(|&p| p + Vec3::new(-p.y, p.x, 0.0) * 0.05)
        .collect();
    result.shape_keys.keys = vec![
        ShapeKey::new("Basis", basis),
        ShapeKey::new("Raise", raised),
        {
            let mut key = ShapeKey::new("Twist", twisted);
            key.relative_to = 1;
            key.value = 0.5;
            key.vertex_group = Some("Upper".to_owned());
            key
        },
    ];
    result.shape_keys.active = Some(0);
    hide_faces(&mut result, &[0, 9]);
    result
}

fn hide_faces(mesh: &mut Polymesh, hidden: &[usize]) {
    let points = mesh.point_count();
    let edges = mesh.edge_count();
    let mut visible_points = vec![false; points];
    let mut visible_edges = vec![false; edges];
    let faces: Vec<bool> = (0..mesh.face_count())
        .map(|face| hidden.contains(&face))
        .collect();
    for face in (0..mesh.face_count()).filter(|&face| !faces[face]) {
        for corner in mesh.face(face) {
            visible_points[mesh.corner_verts[corner] as usize] = true;
            visible_edges[mesh.corner_edges[corner] as usize] = true;
        }
    }
    let used_points: Vec<bool> = {
        let mut used = vec![false; points];
        for &point in &mesh.corner_verts {
            used[point as usize] = true;
        }
        used
    };
    let hidden_points: Vec<bool> = (0..points)
        .map(|point| used_points[point] && !visible_points[point])
        .collect();
    let hidden_edges: Vec<bool> = mesh
        .edges
        .iter()
        .enumerate()
        .map(|(edge, pair)| {
            !visible_edges[edge] && pair.iter().all(|&point| hidden_points[point as usize])
        })
        .collect();
    mesh.attributes.insert(Attribute::new(
        HIDE_VERT,
        Domain::Point,
        Values::Bool(hidden_points),
    ));
    mesh.attributes.insert(Attribute::new(
        HIDE_EDGE,
        Domain::Edge,
        Values::Bool(hidden_edges),
    ));
    mesh.attributes
        .insert(Attribute::new(HIDE_POLY, Domain::Face, Values::Bool(faces)));
}

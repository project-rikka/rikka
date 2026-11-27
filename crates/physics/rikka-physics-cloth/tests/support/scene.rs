use std::sync::Arc;

use glam::{Vec2, Vec3, Vec4};
use rikka_model_core::{Mesh, Model, ModelBuilder, Node, NodeId};
use rikka_physics_tailor::{
    Anchor, Bend, BindTarget, Body, ClothAsset, Curve, Edge, EdgeKind, Flutter, Frame, Inertia,
    Influence, MAX_INFLUENCES, Particle, Piece, PieceKey, PieceKind, Preset, Style, Tether,
    VertexBinding, particle_normals,
};

pub(crate) fn style() -> Style {
    Style {
        gravity: 1.0,
        gravity_falloff: 0.0,
        damping: Curve::flat(0.02),
        settling: 0.0,
        drag: 0.0,
        lift: 0.0,
        stretch: 1.0,
        compression: 0.6,
        bend: 0.4,
        buckling: 0.5,
        restore: Curve::flat(0.0),
        limit: Curve::flat(0.0),
        limit_stiffness: 0.0,
        pull: Curve::flat(0.0),
        max_distance: Curve::flat(1.0),
        backstop: None,
        tether: 1.0,
        extent: 0.0,
        inertia: Inertia {
            carried: 0.0,
            carried_turn: 0.0,
            depth_carried: 0.0,
            centrifugal: 0.0,
            speed_limit: f32::INFINITY,
            turn_limit: f32::INFINITY,
            smoothing: 0.0,
        },
        max_speed: 20.0,
        attenuation: 0.3,
        friction: 0.0,
        thickness: 0.002,
        self_collision: false,
        flutter: Flutter {
            amplitude: 0.0,
            frequency: 1.0,
            wavelength: 1.0,
        },
    }
}

pub(crate) fn sheet_asset(
    columns: u32,
    rows: u32,
    spacing: f32,
    kind: PieceKind,
) -> (Arc<Model>, Arc<ClothAsset>) {
    let at = |row: u32, column: u32| row * columns + column;
    let width = (columns - 1) as f32 * spacing;
    let rest: Vec<Vec3> = (0..rows)
        .flat_map(|row| {
            (0..columns).map(move |column| {
                Vec3::new(
                    column as f32 * spacing - 0.5 * width,
                    -(row as f32) * spacing,
                    0.0,
                )
            })
        })
        .collect();
    let mut triangles = Vec::new();
    for row in 0..rows - 1 {
        for column in 0..columns - 1 {
            triangles.push([at(row, column), at(row + 1, column), at(row, column + 1)]);
            triangles.push([
                at(row, column + 1),
                at(row + 1, column),
                at(row + 1, column + 1),
            ]);
        }
    }
    let mut edges = Vec::new();
    let link = |edges: &mut Vec<Edge>, a: u32, b: u32, kind: EdgeKind| {
        edges.push(Edge {
            ends: [a, b],
            rest: rest[a as usize].distance(rest[b as usize]),
            kind,
        });
    };
    for row in 0..rows {
        for column in 0..columns {
            if column + 1 < columns {
                link(
                    &mut edges,
                    at(row, column),
                    at(row, column + 1),
                    EdgeKind::Structural,
                );
            }
            if row + 1 < rows {
                link(
                    &mut edges,
                    at(row, column),
                    at(row + 1, column),
                    EdgeKind::Structural,
                );
            }
            if row + 1 < rows && column + 1 < columns {
                link(
                    &mut edges,
                    at(row, column + 1),
                    at(row + 1, column),
                    EdgeKind::Shear,
                );
            }
        }
    }
    let bends = hinges(&triangles, &rest);
    let particles: Vec<Particle> = (0..rows)
        .flat_map(|row| {
            let rest = &rest;
            (0..columns).map(move |column| Particle {
                rest: rest[at(row, column) as usize],
                anchor: Anchor::Skinned {
                    joints: [NodeId(0); 4],
                    weights: [1.0, 0.0, 0.0, 0.0],
                },
                pinned: row == 0,
                mass: 0.004,
                depth: row as f32 / (rows - 1) as f32,
                reach: 2.0,
                parent: (row > 0).then(|| at(row - 1, column)),
                radius: 0.005,
                clearance: 0.0,
                relief_height: 0.0,
                relief_depth: 0.0,
                spread: 1.0,
            })
        })
        .collect();
    let tethers = (1..rows)
        .flat_map(|row| {
            (0..columns).map(move |column| Tether {
                particle: at(row, column),
                anchor: at(0, column),
                length: row as f32 * spacing,
            })
        })
        .collect();
    let normals = particle_normals(&rest, &triangles);
    let bindings = (0..rest.len() as u32)
        .map(|vertex| bind(vertex, &rest, &normals, &triangles))
        .collect();
    let piece = Piece {
        key: PieceKey {
            node: NodeId(0),
            vertex: 0,
        },
        name: "sheet".into(),
        kind,
        preset: match kind {
            PieceKind::Sheet => Preset::Skirt,
            PieceKind::Strands => Preset::Ribbon,
        },
        style: style(),
        layer: 0,
        particles,
        edges,
        bends,
        tethers,
        triangles: triangles.clone(),
        strand_nodes: Vec::new(),
        bindings,
    };
    let mut mesh = Mesh::new("sheet");
    mesh.vertices.positions = rest.clone();
    mesh.vertices.normals = vec![Vec3::Z; rest.len()];
    mesh.vertices.tangents = vec![Vec4::new(1.0, 0.0, 0.0, 1.0); rest.len()];
    mesh.indices = triangles.iter().flatten().copied().collect();
    let mut builder = ModelBuilder::new();
    let mesh = builder.add_mesh(mesh);
    builder.add_node(Node::new("root").mesh(mesh));
    let model = Arc::new(builder.finish().expect("a sheet model"));
    let asset = ClothAsset {
        rest_worlds: model.world_transforms(),
        pieces: vec![piece],
        body: Body::default(),
    };
    (model, Arc::new(asset))
}

fn hinges(triangles: &[[u32; 3]], rest: &[Vec3]) -> Vec<Bend> {
    let mut bends = Vec::new();
    for (index, first) in triangles.iter().enumerate() {
        for second in &triangles[index + 1..] {
            let shared: Vec<u32> = first
                .iter()
                .copied()
                .filter(|corner| second.contains(corner))
                .collect();
            if shared.len() != 2 {
                continue;
            }
            let wing = |triangle: &[u32; 3]| {
                triangle
                    .iter()
                    .copied()
                    .find(|corner| !shared.contains(corner))
                    .expect("a wing")
            };
            let (a, b) = (shared[0], shared[1]);
            let (c, d) = (wing(first), wing(second));
            let mut bend = Bend {
                hinge: [a, b],
                wings: [c, d],
                rest_angle: 0.0,
            };
            bend.rest_angle = bend.angle(rest);
            bends.push(bend);
        }
    }
    bends
}

fn bind(vertex: u32, rest: &[Vec3], normals: &[Vec3], triangles: &[[u32; 3]]) -> VertexBinding {
    let (triangle, corners) = triangles
        .iter()
        .enumerate()
        .find(|(_, corners)| corners.contains(&vertex))
        .expect("every vertex has a triangle");
    let corner = corners
        .iter()
        .position(|&at| at == vertex)
        .expect("a corner");
    let uv = [Vec2::ZERO, Vec2::X, Vec2::Y][corner];
    let frame = Frame::from_triangle(
        corners.map(|at| rest[at as usize]),
        corners.map(|at| normals[at as usize]),
        uv,
    );
    let mut influences = [Influence::default(); MAX_INFLUENCES];
    influences[0] = Influence {
        triangle: triangle as u32,
        uv,
        offset: frame.local_point(rest[vertex as usize]),
        normal: frame.local_vector(Vec3::Z),
        tangent: frame.local_vector(Vec3::X),
        weight: 1.0,
    };
    VertexBinding {
        node: NodeId(0),
        vertex,
        blend: 1.0,
        gap: 0.001,
        target: BindTarget::Triangles(influences),
    }
}

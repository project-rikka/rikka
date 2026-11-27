use std::sync::Arc;
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

use glam::{Vec2, Vec3, Vec4};
use rikka_gpu::{Compute, ComputeBuffer, Gpu};
use rikka_model_core::{Mesh, Model, ModelBuilder, Node, NodeId, Vertices};
use rikka_physics::{Bake, BakeView, Drape, FigureKey, Progress, Scene, SceneFrame, SceneSource};
use rikka_physics_tailor::{
    Anchor, Bend, BindTarget, Body, ClothAsset, Edge, EdgeKind, Frame, Influence, MAX_INFLUENCES,
    Particle, Piece, PieceKey, PieceKind, Preset, Tether, VertexBinding, particle_normals,
};
use rikka_renderer_3d::DRAPE_VERTEX_BYTES;

pub(crate) const PATIENCE: Duration = Duration::from_secs(600);

pub(crate) fn compute() -> Compute {
    Gpu::builder()
        .app_name("rikka-physics")
        .compute_support()
        .build()
        .expect("a Vulkan 1.3 device that can compute is required")
        .compute()
        .expect("compute work")
}

pub(crate) fn plank() -> Arc<Model> {
    let mut builder = ModelBuilder::new();
    let mut mesh = Mesh::new("plank");
    let half = 0.3;
    mesh.vertices = Vertices {
        positions: vec![
            Vec3::new(-half, 0.25, -half),
            Vec3::new(half, 0.25, -half),
            Vec3::new(half, 0.25, half),
            Vec3::new(-half, 0.25, half),
        ],
        normals: vec![Vec3::Y; 4],
        tangents: vec![Vec4::new(1.0, 0.0, 0.0, 1.0); 4],
        uvs: vec![Vec2::ZERO; 4],
        ..Vertices::default()
    };
    mesh.indices = vec![0, 2, 1, 0, 3, 2];
    let mesh = builder.add_mesh(mesh);
    builder.add_node(Node::new("plank").mesh(mesh));
    Arc::new(builder.finish().expect("a plank"))
}

pub(crate) struct Curtain {
    pub(crate) model: Arc<Model>,
    pub(crate) asset: Arc<ClothAsset>,
}

pub(crate) fn curtain(columns: u32, rows: u32, spacing: f32, top: f32) -> Curtain {
    let at = |row: u32, column: u32| row * columns + column;
    let width = (columns - 1) as f32 * spacing;
    let rest: Vec<Vec3> = (0..rows)
        .flat_map(|row| {
            (0..columns).map(move |column| {
                Vec3::new(
                    column as f32 * spacing - 0.5 * width,
                    top - row as f32 * spacing,
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
    for row in 0..rows {
        for column in 0..columns {
            let mut link = |b: u32, kind: EdgeKind| {
                let a = at(row, column);
                edges.push(Edge {
                    ends: [a, b],
                    rest: rest[a as usize].distance(rest[b as usize]),
                    kind,
                });
            };
            if column + 1 < columns {
                link(at(row, column + 1), EdgeKind::Structural);
            }
            if row + 1 < rows {
                link(at(row + 1, column), EdgeKind::Structural);
            }
            if row + 1 < rows && column > 0 {
                link(at(row + 1, column - 1), EdgeKind::Shear);
            }
        }
    }
    let particles = (0..rows)
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
        name: "curtain".into(),
        kind: PieceKind::Sheet,
        preset: Preset::Skirt,
        style: Preset::Skirt.style(),
        layer: 0,
        particles,
        edges,
        bends: hinges(&triangles, &rest),
        tethers,
        triangles: triangles.clone(),
        strand_nodes: Vec::new(),
        bindings,
    };
    let mut mesh = Mesh::new("curtain");
    mesh.vertices = Vertices {
        positions: rest.clone(),
        normals: vec![Vec3::Z; rest.len()],
        tangents: vec![Vec4::new(1.0, 0.0, 0.0, 1.0); rest.len()],
        uvs: vec![Vec2::ZERO; rest.len()],
        ..Vertices::default()
    };
    mesh.indices = triangles.iter().flatten().copied().collect();
    let mut builder = ModelBuilder::new();
    let mesh = builder.add_mesh(mesh);
    builder.add_node(Node::new("curtain").mesh(mesh));
    let model = Arc::new(builder.finish().expect("a curtain"));
    let asset = ClothAsset {
        rest_worlds: model.world_transforms(),
        pieces: vec![piece],
        body: Body::default(),
    };
    Curtain {
        model,
        asset: Arc::new(asset),
    }
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
        gap: 0.0,
        target: BindTarget::Triangles(influences),
    }
}

pub(crate) fn read(compute: &Compute, buffer: &ComputeBuffer, bytes: u64) -> Vec<u8> {
    if bytes == 0 {
        return Vec::new();
    }
    let mut batch = compute.batch();
    let download = batch.download_range(buffer, 0, bytes);
    batch.submit().expect("the readback is sent");
    download.wait().expect("the readback comes back")
}

pub(crate) fn particles(compute: &Compute, scene_frame: &SceneFrame) -> Vec<u8> {
    let surface = scene_frame.surface();
    surface.ready.wait().expect("the surface is ready");
    read(
        compute,
        &surface.particles,
        u64::from(surface.particle_count) * 64,
    )
}

pub(crate) fn drapes(compute: &Compute, drapes: &[Drape]) -> Vec<Vec<u8>> {
    drapes
        .iter()
        .map(|drape| {
            drape.ready.wait().expect("the drape is ready");
            read(
                compute,
                &drape.buffer,
                u64::from(drape.vertices) * DRAPE_VERTEX_BYTES,
            )
        })
        .collect()
}

pub(crate) fn positions(bytes: &[u8]) -> Vec<Vec3> {
    bytes
        .as_chunks::<48>()
        .0
        .iter()
        .map(|vertex| {
            let word = |at: usize| {
                f32::from_le_bytes([
                    vertex[at * 4],
                    vertex[at * 4 + 1],
                    vertex[at * 4 + 2],
                    vertex[at * 4 + 3],
                ])
            };
            Vec3::new(word(0), word(1), word(2))
        })
        .collect()
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Look {
    pub(crate) particles: Vec<u8>,
    pub(crate) cloth: Vec<Vec<u8>>,
}

pub(crate) fn look(compute: &Compute, scene_frame: &SceneFrame, figure: FigureKey) -> Look {
    Look {
        particles: particles(compute, scene_frame),
        cloth: drapes(compute, scene_frame.cloth(figure)),
    }
}

pub(crate) fn start(compute: &Compute) -> (Bake, Receiver<()>) {
    start_within(compute, usize::MAX)
}

pub(crate) fn start_within(compute: &Compute, budget: usize) -> (Bake, Receiver<()>) {
    let (notifier, notify) = channel();
    let bake = Bake::builder()
        .checkpoint_every(8)
        .budget(budget)
        .spawn(Arc::new(move || {
            let _ = notifier.send(());
        }))
        .expect("the bake starts");
    bake.set_compute(compute);
    (bake, notify)
}

pub(crate) fn settled(bake: &Bake, notify: &Receiver<()>, done: impl Fn(&Progress) -> bool) {
    loop {
        let progress = bake.progress();
        if let Some(error) = bake.take_error() {
            panic!("the bake failed: {error}");
        }
        if done(&progress) {
            return;
        }
        notify
            .recv_timeout(PATIENCE)
            .expect("the bake reports its progress");
    }
}

pub(crate) fn baked(bake: &Bake, notify: &Receiver<()>, last: u32) {
    let whole = 0..=last;
    settled(bake, notify, |progress| {
        !progress.working && progress.cached == [whole.clone()]
    });
}

pub(crate) fn shown(
    compute: &Compute,
    bake: &Bake,
    display: &mut BakeView,
    step: u32,
) -> SceneFrame {
    display
        .frame(bake, compute, step)
        .expect("the display mirrors the bake")
        .expect("a baked step is shown")
}

pub(crate) fn simulated<T>(
    compute: &Compute,
    source: &Arc<dyn SceneSource>,
    steps: &[u32],
    observe: impl Fn(&SceneFrame) -> T,
) -> Vec<T> {
    let mut scene = Scene::build(compute, Arc::clone(source)).expect("a scene");
    steps
        .iter()
        .map(|step| {
            while scene.step().is_none_or(|at| at < *step) {
                scene.advance().expect("a step");
            }
            observe(&scene.frame())
        })
        .collect()
}

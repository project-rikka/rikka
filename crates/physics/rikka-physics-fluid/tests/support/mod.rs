use rikka_sync::{Mutex, MutexGuard};
use std::sync::Arc;

use glam::{Mat4, Vec3};
use rikka_gpu::{Compute, ComputeBuffer, Gpu};
use rikka_model_core::{Mesh, Model, ModelBuilder, Node};
use rikka_physics_fluid::{Fluid, FluidBuilder, Particle, Substance, Surface};

static TURN: Mutex<()> = Mutex::new(());

struct Device {
    compute: Compute,
    _turn: MutexGuard<'static, ()>,
}

thread_local! {
    static DEVICE: Device = {
        let _ = tracing_subscriber::fmt().with_test_writer().try_init();
        let turn = TURN.lock();
        let compute = Gpu::builder()
            .app_name("rikka-physics-fluid")
            .compute_support()
            .build()
            .expect("a Vulkan 1.3 device that can compute is required")
            .compute()
            .expect("compute");
        Device {
            compute,
            _turn: turn,
        }
    };
}

pub(crate) fn compute() -> Compute {
    DEVICE.with(|device| device.compute.clone())
}

pub(crate) fn fluid(builder: FluidBuilder) -> Fluid {
    builder.build(&compute()).expect("a fluid")
}

pub(crate) fn small() -> FluidBuilder {
    Fluid::builder()
        .radius(0.0025)
        .capacity(8192)
        .spray_capacity(2048)
        .substeps(2, 12)
}

pub(crate) fn water() -> Substance {
    Substance::default()
}

pub(crate) fn honey() -> Substance {
    Substance {
        viscosity: 0.75,
        elasticity: 0.0,
        relaxation: 1.0,
        cohesion: 0.5,
        adhesion: 0.6,
        friction: 0.6,
        spray: 0.0,
        wetting: 0.6,
        tint: [0.9, 0.6, 0.1],
    }
}

pub(crate) fn slime() -> Substance {
    Substance {
        viscosity: 0.6,
        elasticity: 0.6,
        relaxation: 0.2,
        cohesion: 0.8,
        adhesion: 0.7,
        friction: 0.8,
        spray: 0.0,
        wetting: 0.3,
        tint: [0.3, 0.9, 0.4],
    }
}

pub(crate) fn lotion() -> Substance {
    Substance {
        viscosity: 0.6,
        elasticity: 0.45,
        relaxation: 0.7,
        cohesion: 0.7,
        adhesion: 0.85,
        friction: 0.7,
        spray: 0.0,
        wetting: 0.8,
        tint: [0.95, 0.95, 0.92],
    }
}

pub(crate) fn read(buffer: &ComputeBuffer, bytes: u64) -> Vec<u8> {
    if bytes == 0 {
        return Vec::new();
    }
    let mut batch = compute().batch();
    let download = batch.download_range(buffer, 0, bytes);
    batch.submit().expect("a readback");
    download.wait().expect("readback bytes")
}

pub(crate) fn surface_bytes(surface: &Surface) -> (Vec<u8>, Vec<u8>) {
    surface.ready.wait().expect("the surface is made");
    (
        read(&surface.particles, u64::from(surface.particle_count) * 64),
        read(&surface.spray, u64::from(surface.spray_count) * 32),
    )
}

pub(crate) fn floats(bytes: &[u8]) -> Vec<f32> {
    bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|word| f32::from_le_bytes(*word))
        .collect()
}

pub(crate) fn sheet(name: &str, half: f32, divisions: u32) -> Arc<Model> {
    model_of(vec![grid(
        name,
        Vec3::new(-half, 0.0, -half),
        Vec3::X * 2.0 * half,
        Vec3::Z * 2.0 * half,
        divisions,
    )])
}

pub(crate) fn grid(name: &str, origin: Vec3, across: Vec3, along: Vec3, divisions: u32) -> Mesh {
    let mut mesh = Mesh::new(name);
    let side = divisions + 1;
    for row in 0..side {
        for column in 0..side {
            let u = column as f32 / divisions as f32;
            let v = row as f32 / divisions as f32;
            mesh.vertices
                .positions
                .push(origin + across * u + along * v);
        }
    }
    for row in 0..divisions {
        for column in 0..divisions {
            let at = row * side + column;
            mesh.indices.extend_from_slice(&[
                at,
                at + side,
                at + 1,
                at + 1,
                at + side,
                at + side + 1,
            ]);
        }
    }
    mesh
}

pub(crate) fn cup(half: f32, height: f32, divisions: u32) -> Arc<Model> {
    let corners = [
        Vec3::new(-half, 0.0, -half),
        Vec3::new(half, 0.0, -half),
        Vec3::new(half, 0.0, half),
        Vec3::new(-half, 0.0, half),
    ];
    let mut meshes = vec![grid(
        "bottom",
        corners[0],
        Vec3::X * 2.0 * half,
        Vec3::Z * 2.0 * half,
        divisions,
    )];
    for side in 0..4 {
        let from = corners[side];
        let to = corners[(side + 1) % 4];
        meshes.push(grid("wall", from, to - from, Vec3::Y * height, divisions));
    }
    model_of(meshes)
}

pub(crate) fn model_of(meshes: Vec<Mesh>) -> Arc<Model> {
    let mut builder = ModelBuilder::new();
    for mesh in meshes {
        let name = mesh.name.clone();
        let id = builder.add_mesh(mesh);
        let mut node = Node::new(name);
        node.mesh = Some(id);
        builder.add_node(node);
    }
    Arc::new(builder.finish().expect("a model"))
}

pub(crate) fn placed(model: &Arc<Model>, transform: Mat4) -> rikka_model_core::Instance {
    let mut instance = rikka_model_core::Instance::new(Arc::clone(model));
    instance.set_transform(transform);
    instance
}

pub(crate) fn lowest(particles: &[Particle]) -> f32 {
    particles
        .iter()
        .map(|particle| particle.position.y)
        .fold(f32::INFINITY, f32::min)
}

pub(crate) fn relative_densities(
    particles: &[Particle],
    radius: f32,
    fewest: usize,
    keep: impl Fn(&Particle) -> bool,
) -> Vec<f32> {
    let spacing = 2.0 * radius;
    let h = 1.7 * spacing;
    let side = spacing * std::f32::consts::SQRT_2;
    let mut rest = 0.0;
    for z in -3i32..=3 {
        for y in -3i32..=3 {
            for x in -3i32..=3 {
                for offset in [
                    Vec3::ZERO,
                    Vec3::new(0.5, 0.5, 0.0),
                    Vec3::new(0.5, 0.0, 0.5),
                    Vec3::new(0.0, 0.5, 0.5),
                ] {
                    let r = ((Vec3::new(x as f32, y as f32, z as f32) + offset) * side).length();
                    if r > 0.0 && r < h {
                        rest += (1.0 - r / h).powi(2);
                    }
                }
            }
        }
    }
    particles
        .iter()
        .filter(|particle| keep(particle))
        .filter_map(|particle| {
            let mut density = 0.0;
            let mut neighbours = 0;
            for other in particles {
                let r = particle.position.distance(other.position);
                if r > 0.0 && r < h {
                    density += (1.0 - r / h).powi(2);
                    neighbours += 1;
                }
            }
            (neighbours >= fewest).then_some(density / rest)
        })
        .collect()
}

pub(crate) fn connected(
    particles: &[Particle],
    reach: f32,
    from: impl Fn(&Particle) -> bool,
    to: impl Fn(&Particle) -> bool,
) -> bool {
    particles
        .iter()
        .zip(reached(particles, reach, from))
        .any(|(particle, connected)| connected && to(particle))
}

pub(crate) fn reached(
    particles: &[Particle],
    reach: f32,
    from: impl Fn(&Particle) -> bool,
) -> Vec<bool> {
    let mut connected: Vec<bool> = particles.iter().map(&from).collect();
    let mut queue: Vec<usize> = (0..particles.len()).filter(|&at| connected[at]).collect();
    while let Some(at) = queue.pop() {
        for (other, particle) in particles.iter().enumerate() {
            if !connected[other] && particle.position.distance(particles[at].position) < reach {
                connected[other] = true;
                queue.push(other);
            }
        }
    }
    connected
}

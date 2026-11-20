use rikka_gpu::Compute;
use rikka_gpu::{ComputeBuffer, Ticket};
use rikka_renderer::{Node, Renderer, Scene};
use rikka_renderer_3d::Error;
use rikka_renderer_3d::Three;
use rikka_renderer_3d::glam::Vec3;
use rikka_renderer_3d::{
    Background, Camera, FluidLayer, FluidMaterial, Frame, Outline, ViewportConfig,
};
use rikka_test_renderer::TestRenderer;
use rikka_types::{Color, Rect};

const SIZE: u32 = 96;
const RADIUS: f32 = 0.05;
const PARTICLE_BYTES: usize = 64;
const LEFT: Vec3 = Vec3::new(-0.5, 0.35, 0.0);
const RIGHT: Vec3 = Vec3::new(0.5, 0.35, 0.0);
const BELOW: Vec3 = Vec3::new(0.0, -0.45, 0.0);

const RED: FluidMaterial = FluidMaterial {
    color: Vec3::new(0.85, 0.04, 0.04),
    depth: 0.01,
    cloudiness: 1.0,
    roughness: 0.3,
    ior: 1.45,
};

struct Particle {
    center: Vec3,
    radii: Vec3,
    substance: u32,
}

fn renderer() -> TestRenderer {
    rikka_test_renderer::fluid_renderer("rikka-fluid-test")
        .expect("a Vulkan 1.3 device with mesh and compute support is required for these tests")
}

fn compute(renderer: &TestRenderer) -> Compute {
    renderer
        .gpu()
        .compute()
        .expect("the test device can run compute work")
}

fn ball(centre: Vec3, reach: f32, substance: u32) -> Vec<Particle> {
    let spacing = 1.4 * RADIUS;
    let cells = (reach / spacing).ceil() as i32;
    let mut particles = Vec::new();
    for x in -cells..=cells {
        for y in -cells..=cells {
            for z in -cells..=cells {
                let offset = Vec3::new(x as f32, y as f32, z as f32) * spacing;
                if offset.length() <= reach {
                    particles.push(Particle {
                        center: centre + offset,
                        radii: Vec3::splat(RADIUS),
                        substance,
                    });
                }
            }
        }
    }
    particles
}

fn dead(count: usize, at: Vec3) -> Vec<Particle> {
    (0..count)
        .map(|index| Particle {
            center: at + Vec3::X * (index % 5) as f32 * RADIUS,
            radii: Vec3::ZERO,
            substance: 0,
        })
        .collect()
}

fn bytes(particles: &[Particle]) -> Vec<u8> {
    particles
        .iter()
        .flat_map(|particle| {
            let c = particle.center;
            let r = particle.radii;
            [
                c.x,
                c.y,
                c.z,
                f32::from_bits(particle.substance),
                0.0,
                0.0,
                0.0,
                1.0,
                r.x,
                r.y,
                r.z,
                0.0,
                0.0,
                0.0,
                0.0,
                0.0,
            ]
        })
        .flat_map(f32::to_le_bytes)
        .collect()
}

fn upload(compute: &Compute, particles: &[Particle]) -> (ComputeBuffer, Ticket) {
    let bytes = bytes(particles);
    let buffer = compute
        .buffer(bytes.len().max(PARTICLE_BYTES) as u64)
        .expect("a particle buffer");
    let mut batch = compute.batch();
    if !bytes.is_empty() {
        batch.write(&buffer, 0, &bytes);
    }
    let ticket = batch.submit().expect("the particles go up");
    (buffer, ticket)
}

fn layer(compute: &Compute, particles: &[Particle], materials: &[FluidMaterial]) -> FluidLayer {
    let (buffer, ticket) = upload(compute, particles);
    FluidLayer::new(&buffer, particles.len() as u32, RADIUS, &ticket)
        .materials(materials.iter().copied())
}

fn camera() -> Camera {
    Camera::look_at(
        Vec3::new(0.0, 0.0, 2.2),
        Vec3::ZERO,
        Vec3::Y,
        1.0,
        1.0,
        0.05,
    )
}

fn frame(looked: bool) -> Frame {
    let mut frame = Frame::new(camera());
    frame
        .background(Background::Color(Vec3::splat(0.5)))
        .ambient(Vec3::splat(0.8))
        .outline(Outline {
            enabled: false,
            ..Outline::default()
        });
    if looked {
        frame.look(rikka_test_renderer_3d::minimal_look());
    }
    frame
}

fn render(renderer: &mut Renderer, frame: &Frame) -> Result<Vec<u8>, Error> {
    let mut viewport = renderer
        .three()
        .create_viewport(ViewportConfig::new(SIZE, SIZE).samples(1))?;
    renderer.three().render(&mut viewport, frame)?;
    let mut target = renderer.create_target(SIZE, SIZE)?;
    let mut scene = Scene::new();
    scene.set_background(Color::rgb(0.0, 0.0, 0.0));
    scene.insert(Node::image(
        Rect::new(0.0, 0.0, SIZE as f32, SIZE as f32),
        viewport.texture(),
    ));
    renderer.draw(&mut scene, &mut target)?;
    Ok(renderer.read_pixels(&mut target)?)
}

fn read(renderer: &mut Renderer, frame: &Frame) -> Vec<u8> {
    render(renderer, frame).expect("the frame draws")
}

fn pixel_of(world: Vec3) -> usize {
    let clip = camera().view_projection() * world.extend(1.0);
    let ndc = clip.truncate() / clip.w;
    let x = ((ndc.x * 0.5 + 0.5) * SIZE as f32) as usize;
    let y = ((ndc.y * 0.5 + 0.5) * SIZE as f32) as usize;
    (y * SIZE as usize + x) * 4
}

fn colour(pixels: &[u8], world: Vec3) -> [i32; 3] {
    let at = pixel_of(world);
    [0, 1, 2].map(|channel| i32::from(pixels[at + channel]))
}

fn dominant(rgb: [i32; 3], channel: usize) -> bool {
    (0..3).all(|other| other == channel || rgb[channel] > rgb[other] + 40)
}

#[test]
fn fluid_particle_covers_only_its_footprint() {
    let mut renderer = renderer();
    let compute = compute(&renderer);
    for looked in [false, true] {
        let dry = read(&mut renderer, &frame(looked));
        let mut wet = frame(looked);
        wet.fluid(layer(&compute, &ball(Vec3::ZERO, 0.3, 0), &[RED]));
        let wet = read(&mut renderer, &wet);

        let middle = colour(&wet, Vec3::new(0.0, 0.0, 0.3));
        assert!(
            dominant(middle, 0),
            "looked {looked}: the middle is {middle:?}"
        );
        let corner = Vec3::new(-0.9, 0.9, 0.0);
        assert_eq!(
            colour(&wet, corner),
            colour(&dry, corner),
            "looked {looked}: the corner changed"
        );
    }
}

#[test]
fn dead_particle_slots_are_skipped() {
    let mut renderer = renderer();
    let compute = compute(&renderer);
    let mut particles = ball(LEFT, 0.25, 0);
    particles.extend(dead(400, RIGHT));
    particles.push(Particle {
        center: Vec3::new(f32::NAN, 0.0, 0.0),
        radii: Vec3::splat(RADIUS),
        substance: 0,
    });
    particles.extend(ball(BELOW, 0.25, 0));
    let dry = read(&mut renderer, &frame(false));
    let mut wet = frame(false);
    wet.fluid(layer(&compute, &particles, &[RED]));
    let wet = read(&mut renderer, &wet);

    assert!(dominant(colour(&wet, LEFT), 0));
    assert!(dominant(colour(&wet, BELOW), 0));
    assert_eq!(colour(&wet, RIGHT), colour(&dry, RIGHT));
}

#[test]
fn rejects_fluid_layer_overflowing_buffer() {
    let mut renderer = renderer();
    let compute = compute(&renderer);
    let (buffer, ticket) = upload(&compute, &ball(Vec3::ZERO, 0.1, 0));
    let capacity = (buffer.size() / PARTICLE_BYTES as u64) as u32;
    for (count, radius) in [
        (capacity + 1, RADIUS),
        (capacity, 0.0),
        (capacity, f32::NAN),
    ] {
        let mut refused = frame(false);
        refused.fluid(FluidLayer::new(&buffer, count, radius, &ticket).material(RED));
        assert!(
            matches!(render(&mut renderer, &refused), Err(Error::Invalid(_))),
            "{count} particles of radius {radius} in room for {capacity}"
        );
    }
    let mut endless = frame(false);
    endless.fluid(
        FluidLayer::new(&buffer, capacity, RADIUS, &ticket).material(FluidMaterial {
            depth: f32::INFINITY,
            ..RED
        }),
    );
    assert!(matches!(
        render(&mut renderer, &endless),
        Err(Error::Invalid(_))
    ));
}

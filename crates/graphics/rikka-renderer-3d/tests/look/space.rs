use rikka_renderer::{Node, Renderer, Scene};
use rikka_renderer_3d::Three;
use rikka_renderer_3d::glam::{Vec2, Vec3, Vec4};
use rikka_renderer_3d::{
    Background, Camera, Draw, Frame, Indices, Look, MaterialDesc, MaterialId, MaterialState,
    MeshData, MeshId, Outline, ParticleKind,
};
use rikka_renderer_3d::{NITS_PER_RADIANCE_UNIT, ToneMapping, ViewportConfig};
use rikka_test_renderer::TestRenderer;
use rikka_types::{Color, Rect};

const SIZE: u32 = 96;

fn renderer() -> TestRenderer {
    rikka_test_renderer::renderer_3d("rikka-space-test")
        .expect("a Vulkan 1.3 device with the mesh feature set is required for these tests")
}

fn quad(renderer: &mut Renderer, corners: [Vec3; 4], normal: Vec3) -> MeshId {
    let uvs = [
        Vec2::new(0.0, 0.0),
        Vec2::new(1.0, 0.0),
        Vec2::new(1.0, 1.0),
        Vec2::new(0.0, 1.0),
    ];
    renderer
        .three()
        .create_mesh(&MeshData {
            positions: &corners,
            normals: &[normal; 4],
            tangents: &[Vec4::new(1.0, 0.0, 0.0, 1.0); 4],
            uvs: &uvs,
            indices: Some(Indices::U32(&[0, 1, 2, 0, 2, 3, 0, 2, 1, 0, 3, 2])),
            ..MeshData::default()
        })
        .expect("a quad")
}

fn floor(renderer: &mut Renderer, half: f32) -> MeshId {
    quad(
        renderer,
        [
            Vec3::new(-half, 0.0, half),
            Vec3::new(half, 0.0, half),
            Vec3::new(half, 0.0, -half),
            Vec3::new(-half, 0.0, -half),
        ],
        Vec3::Y,
    )
}

fn surface(albedo: f32, emissive: f32) -> MaterialDesc {
    rikka_test_renderer_3d::standard(
        Vec4::new(albedo, albedo, albedo, 1.0),
        MaterialState::opaque(),
    )
    .with("specular_roughness", 0.8)
    .and_then(|desc| desc.with("emission_luminance", emissive * NITS_PER_RADIANCE_UNIT))
    .expect("standard parameters")
}

fn material(renderer: &mut Renderer, desc: MaterialDesc) -> MaterialId {
    renderer.three().create_material(desc).expect("a material")
}

fn quiet() -> Look {
    let mut look = rikka_test_renderer_3d::minimal_look();
    look.shadows.screen.strength = 0.0;
    look
}

fn framed(camera: Camera, look: Look) -> Frame {
    let mut frame = Frame::new(camera);
    frame
        .background(Background::Color(Vec3::ZERO))
        .tone_mapping(ToneMapping::Clamp)
        .time(1.0)
        .look(look)
        .outline(Outline {
            enabled: false,
            ..Outline::default()
        });
    frame
}

fn read(renderer: &mut Renderer, frame: &Frame) -> Vec<u8> {
    let mut viewport = renderer
        .three()
        .create_viewport(ViewportConfig::new(SIZE, SIZE).samples(1))
        .expect("a viewport");
    renderer
        .three()
        .render(&mut viewport, frame)
        .expect("a 3D frame draws");
    let mut target = renderer
        .create_target(SIZE, SIZE)
        .expect("an offscreen target");
    let mut scene = Scene::new();
    scene.set_background(Color::rgb(0.0, 0.0, 0.0));
    scene.insert(Node::image(
        Rect::new(0.0, 0.0, SIZE as f32, SIZE as f32),
        viewport.texture(),
    ));
    renderer
        .draw(&mut scene, &mut target)
        .expect("a 2D frame draws");
    renderer.read_pixels(&mut target).expect("pixels come back")
}

fn brightness(pixels: &[u8], (x, y): (usize, usize)) -> f32 {
    let i = (y * SIZE as usize + x) * 4;
    (pixels[i] as f32 + pixels[i + 1] as f32 + pixels[i + 2] as f32) / 3.0
}

fn around(pixels: &[u8], (x, y): (usize, usize)) -> f32 {
    let mut sum = 0.0;
    for dy in 0..3 {
        for dx in 0..3 {
            sum += brightness(pixels, (x + dx - 1, y + dy - 1));
        }
    }
    sum / 9.0
}

fn pixel_of(camera: &Camera, world: Vec3) -> (usize, usize) {
    let clip = camera.view_projection() * world.extend(1.0);
    let ndc = clip.truncate() / clip.w;
    let x = (ndc.x * 0.5 + 0.5) * SIZE as f32;
    let y = (ndc.y * 0.5 + 0.5) * SIZE as f32;
    (x as usize, y as usize)
}

fn corner_scene(renderer: &mut Renderer) -> (Camera, Vec<Draw>) {
    let ground = floor(renderer, 3.0);
    let wall = quad(
        renderer,
        [
            Vec3::new(-3.0, 0.0, -0.5),
            Vec3::new(3.0, 0.0, -0.5),
            Vec3::new(3.0, 3.0, -0.5),
            Vec3::new(-3.0, 3.0, -0.5),
        ],
        Vec3::Z,
    );
    let grey = surface(0.7, 0.0);
    let grey = material(renderer, grey);
    let camera = Camera::look_at(
        Vec3::new(0.0, 1.2, 2.5),
        Vec3::new(0.0, 0.2, -0.3),
        Vec3::Y,
        1.0,
        1.0,
        0.05,
    );
    let draws = vec![
        Draw::new(ground, grey).object_id(1),
        Draw::new(wall, grey).object_id(2),
    ];
    (camera, draws)
}

fn behind_the_wall(look: &mut Look) {
    let sky = rikka_test_renderer_3d::sky_of(look);
    let sun = sky.to_sun();
    sky.heading = std::f32::consts::PI - sun.x.atan2(sun.z);
    assert!(sky.to_sun().z < -0.5, "the sun stands behind the wall");
}

fn colour(pixels: &[u8], (x, y): (usize, usize)) -> Vec3 {
    let mut sum = Vec3::ZERO;
    for dy in 0..3 {
        for dx in 0..3 {
            let i = ((y + dy - 1) * SIZE as usize + x + dx - 1) * 4;
            sum += Vec3::new(
                f32::from(pixels[i]),
                f32::from(pixels[i + 1]),
                f32::from(pixels[i + 2]),
            );
        }
    }
    sum / 9.0
}

fn texels(pixels: &[u8], (x, y): (usize, usize)) -> Vec<u8> {
    (y - 1..=y + 1)
        .flat_map(|row| {
            let start = (row * SIZE as usize + x - 1) * 4;
            pixels[start..start + 12].iter().copied()
        })
        .collect()
}

#[test]
fn gtao_darkens_wall_floor_corner() {
    let mut renderer = renderer();
    let (camera, draws) = corner_scene(&mut renderer);
    let mut look = quiet();
    behind_the_wall(&mut look);
    let mut open = framed(camera, look.clone());
    open.draws(draws.clone());
    look.occlusion.strength = 1.5;
    let mut occluded = framed(camera, look);
    occluded.draws(draws);

    let open = read(&mut renderer, &open);
    let occluded = read(&mut renderer, &occluded);
    let corner = pixel_of(&camera, Vec3::new(0.0, 0.03, -0.45));
    let away = pixel_of(&camera, Vec3::new(0.0, 0.0, 1.4));
    assert!(
        around(&occluded, corner) < around(&open, corner) * 0.92,
        "the corner went from {} to {}",
        around(&open, corner),
        around(&occluded, corner)
    );
    assert!(
        (around(&occluded, away) - around(&open, away)).abs() <= 3.0,
        "the open floor went from {} to {}",
        around(&open, away),
        around(&occluded, away)
    );
}

fn overhang_scene(renderer: &mut Renderer) -> (Camera, Vec<Draw>) {
    let wall = quad(
        renderer,
        [
            Vec3::new(-3.0, 0.0, -0.5),
            Vec3::new(3.0, 0.0, -0.5),
            Vec3::new(3.0, 1.5, -0.5),
            Vec3::new(-3.0, 1.5, -0.5),
        ],
        Vec3::Z,
    );
    let roof = quad(
        renderer,
        [
            Vec3::new(-3.0, 1.5, -0.5),
            Vec3::new(3.0, 1.5, -0.5),
            Vec3::new(3.0, 1.5, 0.9),
            Vec3::new(-3.0, 1.5, 0.9),
        ],
        Vec3::NEG_Y,
    );
    let matte = surface(0.7, 0.0)
        .with("specular_weight", 0.0)
        .expect("the standard template has a specular weight");
    let matte = material(renderer, matte);
    let camera = Camera::look_at(
        Vec3::new(0.0, 0.8, 2.6),
        Vec3::new(0.0, 1.0, -0.5),
        Vec3::Y,
        1.0,
        1.0,
        0.05,
    );
    let draws = vec![
        Draw::new(wall, matte).object_id(1),
        Draw::new(roof, matte).object_id(2),
    ];
    (camera, draws)
}

fn red_ground() -> Look {
    let mut look = quiet();
    rikka_test_renderer_3d::exposure_mut(&mut look).adaptation = 0.0;
    rikka_test_renderer_3d::sky_of(&mut look).ground = Vec3::new(0.9, 0.04, 0.02);
    behind_the_wall(&mut look);
    look
}

fn redness(colour: Vec3) -> f32 {
    colour.x / colour.z.max(1.0)
}

#[test]
fn bent_normals_light_a_crease_from_its_open_side() {
    let mut renderer = renderer();
    let (camera, draws) = overhang_scene(&mut renderer);
    let mut look = red_ground();
    let mut open = framed(camera, look.clone());
    open.draws(draws.clone());
    look.occlusion.strength = 1.0;
    let mut occluded = framed(camera, look);
    occluded.draws(draws);

    let open = read(&mut renderer, &open);
    let occluded = read(&mut renderer, &occluded);
    let crease = pixel_of(&camera, Vec3::new(0.0, 1.42, -0.49));
    let below = pixel_of(&camera, Vec3::new(0.0, 0.35, -0.49));
    let (before, after) = (colour(&open, crease), colour(&occluded, crease));
    assert!(
        after.z < before.z * 0.9,
        "the roof hides the sky above the crease: {before} to {after}"
    );
    assert!(
        redness(after) > redness(before) * 1.15,
        "the crease turned toward the red ground below it from {before} to {after}"
    );
    let (before, after) = (colour(&open, below), colour(&occluded, below));
    assert!(
        (redness(after) - redness(before)).abs() < redness(before) * 0.03,
        "the open wall kept its colour from {before} to {after}"
    );
}

fn metal(renderer: &mut Renderer, albedo: f32) -> MaterialId {
    let desc = surface(albedo, 0.0)
        .with("base_metalness", 1.0)
        .and_then(|desc| desc.with("specular_roughness", 0.45))
        .expect("standard parameters");
    material(renderer, desc)
}

#[test]
fn specular_occlusion_darkens_reflections_into_a_crease_only() {
    let mut renderer = renderer();
    let (camera, mut draws) = corner_scene(&mut renderer);
    let chrome = metal(&mut renderer, 0.9);
    for draw in &mut draws {
        draw.material = chrome;
    }
    let mut look = quiet();
    rikka_test_renderer_3d::exposure_mut(&mut look).adaptation = 0.0;
    behind_the_wall(&mut look);
    let mut open = framed(camera, look.clone());
    open.draws(draws.clone());
    look.occlusion.strength = 1.0;
    let mut occluded = framed(camera, look);
    occluded.draws(draws);

    let open = read(&mut renderer, &open);
    let occluded = read(&mut renderer, &occluded);
    let corner = pixel_of(&camera, Vec3::new(0.0, 0.0, -0.42));
    let away = pixel_of(&camera, Vec3::new(0.0, 0.0, 1.2));
    assert!(
        around(&occluded, corner) < around(&open, corner) * 0.8,
        "the reflection into the wall went from {} to {}",
        around(&open, corner),
        around(&occluded, corner)
    );
    assert!(around(&open, away) > 8.0, "the open floor reflects the sky");
    assert_eq!(
        texels(&occluded, away),
        texels(&open, away),
        "the open floor reflects the sky unchanged"
    );
}

#[test]
fn multiple_bounces_lift_occlusion_on_bright_surfaces() {
    let mut renderer = renderer();
    let (camera, draws) = corner_scene(&mut renderer);
    let mut look = quiet();
    rikka_test_renderer_3d::exposure_mut(&mut look).adaptation = 0.0;
    behind_the_wall(&mut look);
    let corner = pixel_of(&camera, Vec3::new(0.0, 0.03, -0.45));
    let mut kept = Vec::new();
    for albedo in [0.9, 0.2] {
        let painted = material(&mut renderer, surface(albedo, 0.0));
        let draws: Vec<Draw> = draws
            .iter()
            .map(|draw| Draw {
                material: painted,
                ..*draw
            })
            .collect();
        let mut open = framed(camera, look.clone());
        open.draws(draws.clone());
        let mut occluded_look = look.clone();
        occluded_look.occlusion.strength = 1.0;
        let mut occluded = framed(camera, occluded_look);
        occluded.draws(draws);
        let open = read(&mut renderer, &open);
        let occluded = read(&mut renderer, &occluded);
        kept.push(around(&occluded, corner) / around(&open, corner).max(1.0));
    }
    assert!(
        kept[0] > kept[1] + 0.08,
        "a bright corner keeps {} of its light, a dark one {}",
        kept[0],
        kept[1]
    );
}

fn sparkling(time: f32, seed: u32, amount: f32) -> Frame {
    let camera = Camera::look_at(Vec3::ZERO, Vec3::NEG_Z, Vec3::Y, 1.0, 1.0, 0.05);
    let mut look = quiet();
    rikka_test_renderer_3d::sky_of(&mut look).sun = 0.0;
    rikka_test_renderer_3d::sky_of(&mut look).moon = 0.0;
    look.particles.focus = 3.0;
    look.particles.amount = amount;
    look.particles.kind = ParticleKind::Motes;
    look.particles.size = 6.0;
    look.particles.glow = 4.0;
    look.particles.reach = 4.0;
    look.particles.seed = seed;
    let mut frame = framed(camera, look);
    frame.time(time);
    frame
}

#[test]
fn particles_deterministic_for_time_and_seed() {
    let mut renderer = renderer();
    let first = read(&mut renderer, &sparkling(2.5, 7, 0.5));
    let again = read(&mut renderer, &sparkling(2.5, 7, 0.5));
    let reseeded = read(&mut renderer, &sparkling(2.5, 8, 0.5));
    let none = read(&mut renderer, &sparkling(2.5, 7, 0.0));
    let lit = |pixels: &[u8]| pixels.chunks(4).filter(|p| p[0] > 24).count();

    assert_eq!(first, again);
    assert_ne!(first, reseeded);
    assert!(lit(&first) > 40, "{} pixels show particles", lit(&first));
    assert_eq!(lit(&none), 0);
}

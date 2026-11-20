use std::f32::consts::{FRAC_PI_2, PI, TAU};

use rikka_gpu::{Pixels, TextureId};
use rikka_renderer::{Node, Renderer, Scene};
use rikka_renderer_3d::Three;
use rikka_renderer_3d::glam::{Vec2, Vec3, Vec4};
use rikka_renderer_3d::{
    Antialiasing, Background, BuiltinTemplate, Camera, Composite, Draw, Environment,
    EnvironmentImage, Error, Exposure, Frame, Grade, Indices, Look, MaterialDesc, MaterialId,
    MaterialTemplate, MeshData, MeshId, Outline, Panorama, Tier, Tone, ToneMapping, Tonemap,
    ViewportConfig,
};
use rikka_test_renderer::TestRenderer;
use rikka_types::{Color, Rect, srgb};

const SIZE: u32 = 96;
const WIDTH: u32 = 1024;
const HEIGHT: u32 = 512;
const RADIUS: f32 = 0.8;

fn renderer() -> TestRenderer {
    rikka_test_renderer::renderer_3d("rikka-environment-test")
        .expect("a Vulkan 1.3 device with the mesh feature set is required for these tests")
}

fn traced_renderer() -> TestRenderer {
    let renderer = rikka_test_renderer::ray_renderer("rikka-environment-ray-test")
        .expect("a Vulkan 1.3 device with the mesh feature set is required for these tests");
    assert!(
        renderer.gpu().traces_rays(),
        "this test needs a GPU with acceleration structures and ray queries"
    );
    renderer
}

fn texel_direction(x: u32, y: u32) -> Vec3 {
    let polar = PI * (y as f32 + 0.5) / HEIGHT as f32;
    let azimuth = TAU * (x as f32 + 0.5) / WIDTH as f32 - PI;
    Vec3::new(
        polar.sin() * azimuth.sin(),
        polar.cos(),
        -polar.sin() * azimuth.cos(),
    )
}

fn texels(radiance: impl Fn(Vec3) -> Vec3) -> Vec<f32> {
    (0..HEIGHT)
        .flat_map(|y| (0..WIDTH).map(move |x| (x, y)))
        .flat_map(|(x, y)| radiance(texel_direction(x, y)).extend(1.0).to_array())
        .collect()
}

fn upload(renderer: &Renderer, pixels: &[f32]) -> TextureId {
    renderer
        .uploads()
        .create(Pixels::rgba32f(WIDTH, HEIGHT, pixels))
        .expect("the panorama uploads")
}

fn camera() -> Camera {
    Camera::look_at(
        Vec3::new(0.0, 0.0, 4.0),
        Vec3::ZERO,
        Vec3::Y,
        0.6,
        1.0,
        0.05,
    )
}

fn look(environment: Environment) -> Look {
    let mut look = rikka_test_renderer_3d::minimal_look();
    look.environment = environment;
    look.shadows.screen.strength = 0.0;
    let mut composite = Composite::new();
    composite.push(Exposure {
        adaptation: 0.0,
        ..Exposure::default()
    });
    composite.push(Grade {
        contrast: 1.0,
        vibrance: 0.0,
        night: 0.0,
        ..Grade::default()
    });
    composite.push(Tonemap::new(Tone::Clamp));
    look.composite = composite;
    look
}

fn frame(look: Look, background: Background, draws: &[Draw]) -> Frame {
    let mut frame = Frame::new(camera());
    frame
        .background(background)
        .ambient(Vec3::ZERO)
        .tone_mapping(ToneMapping::Clamp)
        .time(1.0)
        .look(look)
        .outline(Outline {
            enabled: false,
            ..Outline::default()
        });
    frame.draws(draws.iter().cloned());
    frame
}

fn shoot(renderer: &mut Renderer, frame: &Frame) -> Vec<u8> {
    let mut viewport = renderer
        .three()
        .create_viewport(
            ViewportConfig::new(SIZE, SIZE)
                .samples(1)
                .antialiasing(Antialiasing::Multisample),
        )
        .expect("a viewport");
    for _ in 0..2 {
        renderer
            .three()
            .render(&mut viewport, frame)
            .expect("a 3D frame draws");
    }
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

fn linear(pixels: &[u8], (x, y): (usize, usize)) -> Vec3 {
    let mut sum = Vec3::ZERO;
    for dy in 0..3 {
        for dx in 0..3 {
            let i = ((y + dy - 1) * SIZE as usize + x + dx - 1) * 4;
            sum += Vec3::from_array(
                [pixels[i], pixels[i + 1], pixels[i + 2]]
                    .map(|level| srgb::to_linear(f32::from(level) / 255.0)),
            );
        }
    }
    sum / 9.0
}

fn pixel_of(world: Vec3) -> (usize, usize) {
    let clip = camera().view_projection() * world.extend(1.0);
    let ndc = clip.truncate() / clip.w;
    (
        ((ndc.x * 0.5 + 0.5) * SIZE as f32) as usize,
        ((ndc.y * 0.5 + 0.5) * SIZE as f32) as usize,
    )
}

fn on_sphere(normal: Vec3) -> (usize, usize) {
    pixel_of(normal.normalize() * RADIUS)
}

const CORNER: (usize, usize) = (4, 4);

fn standard(renderer: &mut Renderer, edit: impl Fn(MaterialDesc) -> MaterialDesc) -> MaterialId {
    let desc = MaterialDesc::new(&MaterialTemplate::builtin(BuiltinTemplate::Standard))
        .with("base_color", Vec3::ONE)
        .expect("standard parameters");
    renderer
        .three()
        .create_material(edit(desc))
        .expect("a material")
}

fn set(desc: MaterialDesc, name: &str, value: f32) -> MaterialDesc {
    desc.with(name, value).expect("standard parameters")
}

fn diffuse(renderer: &mut Renderer) -> MaterialId {
    standard(renderer, |desc| set(desc, "specular_weight", 0.0))
}

fn metal(renderer: &mut Renderer, roughness: f32) -> MaterialId {
    standard(renderer, |desc| {
        set(
            set(desc, "base_metalness", 1.0),
            "specular_roughness",
            roughness,
        )
    })
}

fn ball(renderer: &mut Renderer) -> MeshId {
    rikka_test_renderer_3d::sphere(renderer, RADIUS, 48).expect("a sphere")
}

fn image(texture: TextureId, intensity: f32) -> EnvironmentImage {
    EnvironmentImage {
        intensity,
        ..EnvironmentImage::new(texture)
    }
}

fn close(seen: f32, expected: f32, tolerance: f32) -> bool {
    (seen - expected).abs() <= tolerance
}

#[test]
fn uniform_panorama_lights_white_spheres_to_its_own_radiance() {
    let mut renderer = renderer();
    let ball = ball(&mut renderer);
    let texture = upload(&renderer, &texels(|_| Vec3::ONE));
    let lit = Environment::Image(image(texture, 0.5));
    let materials = [
        ("diffuse", diffuse(&mut renderer)),
        ("mirror", metal(&mut renderer, 0.0)),
        ("polished metal", metal(&mut renderer, 0.3)),
        ("rough metal", metal(&mut renderer, 1.0)),
        (
            "dielectric",
            standard(&mut renderer, |desc| set(desc, "specular_roughness", 0.4)),
        ),
    ];
    let mut failures = Vec::new();
    for (name, material) in materials {
        let pixels = shoot(
            &mut renderer,
            &frame(look(lit), Background::Sky, &[Draw::new(ball, material)]),
        );
        let background = linear(&pixels, CORNER).x;
        if !close(background, 0.5, 0.02) {
            failures.push(format!("{name}: background {background:.3}"));
        }
        for normal in [
            Vec3::Z,
            Vec3::new(0.6, 0.5, 0.62),
            Vec3::new(-0.7, -0.3, 0.65),
        ] {
            let seen = linear(&pixels, on_sphere(normal));
            if !close(seen.x, 0.5, 0.03) || !close(seen.z, 0.5, 0.03) {
                failures.push(format!("{name}: {normal} shows {seen:.3}"));
            }
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");

    renderer.uploads().destroy(texture);
    let mut viewport = renderer
        .three()
        .create_viewport(ViewportConfig::new(SIZE, SIZE))
        .expect("a viewport");
    let refused = renderer
        .three()
        .render(&mut viewport, &frame(look(lit), Background::Sky, &[]));
    assert!(
        matches!(refused, Err(Error::Gpu(rikka_renderer::Error::Unknown(_)))),
        "a destroyed panorama gave {refused:?}"
    );
}

#[test]
fn half_lit_panorama_lights_the_facing_side_and_rotation_turns_it() {
    let mut renderer = renderer();
    let ball = ball(&mut renderer);
    let material = diffuse(&mut renderer);
    let texture = upload(
        &renderer,
        &texels(|direction| Vec3::splat(f32::from(u8::from(direction.x > 0.0)))),
    );
    let right = Vec3::new(0.8, 0.0, 0.6);
    let left = Vec3::new(-0.8, 0.0, 0.6);
    let cases = [
        (0.0, [0.1, 0.5, 0.9]),
        (FRAC_PI_2, [0.2, 0.0, 0.2]),
        (PI, [0.9, 0.5, 0.1]),
    ];
    let mut failures = Vec::new();
    for (rotation, expected) in cases {
        let lit = Environment::Image(EnvironmentImage {
            rotation,
            background: 0.0,
            ..image(texture, 0.8)
        });
        let pixels = shoot(
            &mut renderer,
            &frame(
                look(lit),
                Background::Color(Vec3::ZERO),
                &[Draw::new(ball, material)],
            ),
        );
        for (normal, wanted) in [left, Vec3::Z, right].into_iter().zip(expected) {
            let seen = linear(&pixels, on_sphere(normal)).y;
            if !close(seen, 0.8 * wanted, 0.035) {
                failures.push(format!(
                    "rotation {rotation}: {normal} shows {seen:.3}, expected {:.3}",
                    0.8 * wanted
                ));
            }
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

#[test]
fn mirrors_reflect_the_panorama_and_rough_metal_its_blur() {
    let mut renderer = renderer();
    let ball = ball(&mut renderer);
    let texture = upload(
        &renderer,
        &texels(|direction| match direction.z > 0.0 {
            true => Vec3::X,
            false => Vec3::Z,
        }),
    );
    let lit = Environment::Image(image(texture, 0.6));
    let mirror = metal(&mut renderer, 0.0);
    let rough = metal(&mut renderer, 1.0);
    let centre = on_sphere(Vec3::Z);
    let tilted = on_sphere(Vec3::new(0.38, 0.0, 0.92));

    let mirrored = shoot(
        &mut renderer,
        &frame(look(lit), Background::Sky, &[Draw::new(ball, mirror)]),
    );
    let sky = linear(&mirrored, CORNER);
    assert!(sky.z > 0.55 && sky.x < 0.03, "background behind {sky}");
    let back = linear(&mirrored, centre);
    assert!(
        back.x > 0.55 && back.z < 0.05,
        "the mirror shows {back} where it faces back along +Z"
    );
    let reflected = linear(&mirrored, tilted);
    assert!(
        reflected.x > 0.55 && reflected.z < 0.05,
        "the mirror shows {reflected} where it reflects 45 degrees into the red half"
    );

    let blurred = linear(
        &shoot(
            &mut renderer,
            &frame(look(lit), Background::Sky, &[Draw::new(ball, rough)]),
        ),
        tilted,
    );
    assert!(
        blurred.x > blurred.z && blurred.z > 0.04,
        "rough metal shows {blurred}, a blend leaning to red"
    );
    for (name, seen) in [("mirror", reflected), ("rough", blurred)] {
        assert!(
            close(seen.x + seen.z, 0.6, 0.04),
            "{name} keeps {seen} of the 0.6 it reflects"
        );
    }
}

#[test]
fn background_shows_blends_and_blurs_the_panorama() {
    let mut renderer = renderer();
    let stripes = 64.0;
    let texture = upload(
        &renderer,
        &texels(|direction| {
            let turn = direction.x.atan2(-direction.z) / TAU + 0.5;
            Vec3::splat(f32::from(u8::from((turn * stripes).fract() < 0.5)))
        }),
    );
    let grey = Vec3::splat(0.3);
    let shown = |renderer: &mut Renderer, background: f32, blur: f32| {
        let lit = Environment::Image(EnvironmentImage {
            background,
            blur,
            ..image(texture, 0.6)
        });
        shoot(renderer, &frame(look(lit), Background::Color(grey), &[]))
    };
    let row = |pixels: &[u8]| -> Vec<f32> {
        (2..SIZE as usize - 2)
            .map(|x| linear(pixels, (x, SIZE as usize / 2)).y)
            .collect()
    };
    let spread = |values: &[f32]| {
        let high = values.iter().copied().fold(f32::MIN, f32::max);
        let low = values.iter().copied().fold(f32::MAX, f32::min);
        (low, high)
    };
    let mean = |values: &[f32]| values.iter().sum::<f32>() / values.len() as f32;

    let hidden = row(&shown(&mut renderer, 0.0, 0.0));
    let (low, high) = spread(&hidden);
    assert!(
        close(low, 0.3, 0.02) && close(high, 0.3, 0.02),
        "hidden {low}..{high}"
    );

    let sharp = row(&shown(&mut renderer, 1.0, 0.0));
    let (low, high) = spread(&sharp);
    assert!(low < 0.1 && high > 0.5, "sharp stripes span {low}..{high}");

    let blended = row(&shown(&mut renderer, 0.5, 0.0));
    let off = sharp
        .iter()
        .zip(&blended)
        .map(|(shown, half)| (half - (0.5 * shown + 0.15)).abs())
        .fold(0.0f32, f32::max);
    assert!(off < 0.03, "half shown strays {off} from the even blend");

    let blurred = row(&shown(&mut renderer, 1.0, 1.0));
    let (low, high) = spread(&blurred);
    assert!(
        high - low < 0.06,
        "blurred stripes still span {low}..{high}"
    );
    assert!(
        close(mean(&blurred), 0.3, 0.04),
        "blurred mean {}",
        mean(&blurred)
    );
}

#[test]
fn sun_extraction_finds_the_disc_and_its_light_replaces_the_clamped_energy() {
    let toward = Vec3::new(0.3, 0.6, 0.74).normalize();
    let disc = 3.0f32.to_radians();
    let sky = 0.2;
    let sun = 200.0;
    let threshold = 10.0;
    let pixels = texels(|direction| {
        Vec3::splat(match direction.angle_between(toward) < disc {
            true => sun,
            false => sky,
        })
    });
    let found = Panorama::new(WIDTH, HEIGHT, &pixels)
        .expect("a panorama")
        .sun(threshold)
        .expect("the disc exceeds the threshold");
    assert!(
        found.direction.angle_between(toward) < 0.5f32.to_radians(),
        "found the sun at {}",
        found.direction
    );
    let solid_angle = TAU * (1.0 - disc.cos());
    let expected = (sun - threshold) * solid_angle * (1.0 - 0.25 * disc * disc);
    assert!(
        close(found.illuminance, expected, 0.04 * expected),
        "illuminance {} against {expected}",
        found.illuminance
    );
    assert!(
        close(found.angular_radius, disc, 0.25 * disc),
        "angular radius {} against {disc}",
        found.angular_radius
    );
    assert!(close(found.color.x, 1.0, 1e-3) && close(found.color.z, 1.0, 1e-3));
    assert!(
        Panorama::new(WIDTH, HEIGHT, &pixels)
            .expect("a panorama")
            .sun(sun * 2.0)
            .is_none()
    );

    let mut renderer = renderer();
    let ball = ball(&mut renderer);
    let material = diffuse(&mut renderer);
    let texture = upload(&renderer, &pixels);
    let whole = EnvironmentImage {
        background: 0.0,
        ..image(texture, 0.8)
    };
    let clamped = EnvironmentImage {
        sun_clamp: Some(threshold),
        ..whole
    };
    let draws = [Draw::new(ball, material)];
    let lit = |renderer: &mut Renderer, image: EnvironmentImage, sunlit: bool| {
        let mut shot = frame(
            look(Environment::Image(image)),
            Background::Color(Vec3::ZERO),
            &draws,
        );
        if sunlit {
            shot.light(found.light(&image));
        }
        linear(&shoot(renderer, &shot), on_sphere(toward)).y
    };
    let full = lit(&mut renderer, whole, false);
    let without = lit(&mut renderer, clamped, false);
    let restored = lit(&mut renderer, clamped, true);
    assert!(
        full > 0.4,
        "the unclamped sun lights the facing side to {full}"
    );
    assert!(without < 0.45 * full, "clamping left {without} of {full}");
    assert!(
        close(restored, full, 0.12 * full),
        "the extracted light restores {restored} of {full}"
    );
}

#[test]
fn ray_traced_reflections_escape_into_the_panorama() {
    let mut renderer = traced_renderer();
    let texture = upload(
        &renderer,
        &texels(|direction| match direction.y > 0.0 {
            true => Vec3::new(1.0, 0.1, 0.1),
            false => Vec3::ZERO,
        }),
    );
    let floor = renderer
        .three()
        .create_mesh(&MeshData {
            positions: &[
                Vec3::new(-3.0, -1.0, 3.0),
                Vec3::new(3.0, -1.0, 3.0),
                Vec3::new(3.0, -1.0, -3.0),
                Vec3::new(-3.0, -1.0, -3.0),
            ],
            normals: &[Vec3::Y; 4],
            tangents: &[Vec4::new(1.0, 0.0, 0.0, 1.0); 4],
            uvs: &[Vec2::ZERO; 4],
            indices: Some(Indices::U32(&[0, 1, 2, 0, 2, 3])),
            ..MeshData::default()
        })
        .expect("a floor");
    let mirror = standard(&mut renderer, |desc| {
        let desc = set(set(desc, "base_metalness", 1.0), "specular_roughness", 0.05);
        set(set(desc, "gloss.roughness", 0.05), "gloss.reflection", 1.0)
    });
    let shot = |renderer: &mut Renderer, tier: Tier| {
        let mut look = look(Environment::Image(EnvironmentImage {
            background: 0.0,
            ..image(texture, 0.6)
        }));
        look.reflections.tier = tier;
        look.reflections.strength = 1.0;
        let draw = Draw::new(floor, mirror);
        let pixels = shoot(
            renderer,
            &frame(look, Background::Color(Vec3::ZERO), &[draw]),
        );
        linear(&pixels, pixel_of(Vec3::new(0.0, -1.0, 0.0)))
    };
    let traced = shot(&mut renderer, Tier::RayTraced);
    let rastered = shot(&mut renderer, Tier::Off);
    assert!(
        traced.x > 0.4 && traced.x > 3.0 * traced.z,
        "the traced mirror floor shows {traced}"
    );
    assert!(
        close(traced.x, rastered.x, 0.15 * rastered.x),
        "traced {traced} against image-lit {rastered}"
    );
}

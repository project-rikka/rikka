use rikka_renderer::Renderer;
use rikka_renderer_3d::glam::{Mat4, Vec3, Vec4};
use rikka_renderer_3d::{
    Background, Composite, CompositeInput, Draw, Glow, Grade, Light, MaterialState, RenderLayer,
    ShadowQuality, Tonemap, Value,
};

use crate::support::{Shot, create, encoded, frame, looked, opaque, quad, renderer, texture};

const MMD_LIGHT: f32 = 154.0 / 255.0;
const BASE: Vec3 = Vec3::new(0.5, 0.3, 0.2);
const AMBIENT: Vec3 = Vec3::new(0.1, 0.1, 0.1);
const SHADE: Vec3 = Vec3::new(0.2, 0.1, 0.05);
const FRAME_AMBIENT: f32 = 0.2;
const POWER: f32 = 16.0;
const LIT: Vec3 = Vec3::new(0.8, 0.0, 0.6);
const DARK: Vec3 = Vec3::new(0.866_025_4, 0.0, -0.5);
const TOP: Vec3 = Vec3::ONE;
const BOTTOM: Vec3 = Vec3::new(1.0, 1.0, 0.0);

#[derive(Clone, Copy)]
enum Sphere {
    None,
    Add(Vec3),
    Multiply(Vec3),
}

fn expected(toward: Vec3, ramp: bool, sphere: Sphere) -> Vec3 {
    let normal = Vec3::Z;
    let facing = normal.dot(toward);
    let material = (BASE + AMBIENT).clamp(Vec3::ZERO, Vec3::ONE);
    let (albedo, shade, added) = match sphere {
        Sphere::None => (material, SHADE, Vec3::ZERO),
        Sphere::Add(color) => (material, SHADE, color),
        Sphere::Multiply(color) => (material * color, SHADE * color, Vec3::ZERO),
    };
    let half = (toward + Vec3::Z).normalize();
    let specular = Vec3::ONE * MMD_LIGHT * normal.dot(half).max(0.001).powf(POWER);
    let step = ((facing + 0.05) / 0.1).clamp(0.0, 1.0);
    let direct = match ramp {
        true => {
            let tone = match facing > 0.0 {
                true => TOP,
                false => BOTTOM,
            };
            (albedo + added) * tone
        }
        false => shade.lerp(albedo, step) + added * step,
    };
    direct + specular + (albedo + added) * FRAME_AMBIENT
}

fn drawn(renderer: &mut Renderer, toward: Vec3, ramp: bool, sphere: Sphere) -> Vec3 {
    let mut desc = opaque("mmd")
        .with("base_color", BASE.extend(1.0))
        .and_then(|desc| desc.with("ambient_color", AMBIENT))
        .and_then(|desc| desc.with("shade_color", SHADE))
        .and_then(|desc| desc.with("specular_color", Vec3::ONE))
        .and_then(|desc| desc.with("specular_power", POWER))
        .expect("mmd parameters");
    if ramp {
        let ramp = texture(renderer, 1, 2, |_, y| match y {
            0 => [255, 255, 255, 255],
            _ => [255, 255, 0, 255],
        });
        desc.set("toon_texture", ramp).expect("a toon ramp");
    }
    let (mode, color) = match sphere {
        Sphere::None => ("none", None),
        Sphere::Add(color) => ("add", Some(color)),
        Sphere::Multiply(color) => ("multiply", Some(color)),
    };
    desc.set("sphere_mode", Value::Enum(mode.into()))
        .expect("a sphere mode");
    if let Some(color) = color {
        let texel = color.to_array().map(|channel| (channel * 255.0) as u8);
        let map = texture(renderer, 1, 1, |_, _| [texel[0], texel[1], texel[2], 255]);
        desc.set("sphere_texture", map).expect("a sphere map");
    }
    let material = create(renderer, desc);
    let plane = quad(renderer, 0.5);
    let mut frame = frame(None);
    frame
        .background(Background::Color(Vec3::ZERO))
        .ambient(Vec3::splat(FRAME_AMBIENT))
        .light(
            Light::directional(-toward)
                .intensity(1.0)
                .shadow(ShadowQuality::None),
        )
        .draw(Draw::new(plane, material));
    Shot::take(renderer, &frame).at(64, 64)
}

#[test]
fn mmd_shading_follows_the_mmd_formula() {
    let mut renderer = renderer();
    let blue = Vec3::new(0.0, 0.0, 1.0);
    let magenta = Vec3::new(1.0, 0.0, 1.0);
    let cases = [
        ("toon ramp, lit side", LIT, true, Sphere::None),
        ("toon ramp, shadow side", DARK, true, Sphere::None),
        ("shade colour step, lit side", LIT, false, Sphere::None),
        ("shade colour step, shadow side", DARK, false, Sphere::None),
        ("multiplying sphere", LIT, true, Sphere::Multiply(magenta)),
        (
            "adding sphere in toon shadow",
            DARK,
            true,
            Sphere::Add(blue),
        ),
    ];
    for (name, toward, ramp, sphere) in cases {
        let found = drawn(&mut renderer, toward, ramp, sphere);
        let wanted = encoded(expected(toward, ramp, sphere));
        assert!(
            (found - wanted).abs().max_element() <= 2.0,
            "{name}: drew {found}, the MMD formula gives {wanted}"
        );
    }
}

#[test]
fn light_through_coloured_glass_reaches_an_mmd_floor_red() {
    let mut renderer = renderer();
    let glass = create(
        &mut renderer,
        rikka_test_renderer_3d::standard(
            Vec4::ONE,
            MaterialState::transparent().shadows(true, true),
        )
        .with("transmission_weight", 1.0)
        .and_then(|desc| desc.with("transmission_color", Vec3::new(1.0, 0.0, 0.0)))
        .expect("a red glass"),
    );
    let floor = quad(&mut renderer, 1.5);
    let card = quad(&mut renderer, 0.3);
    let cloth = create(
        &mut renderer,
        opaque("mmd")
            .with("shade_color", Vec3::splat(0.3))
            .expect("cloth parameters"),
    );
    let mut frame = frame(Some(looked()));
    frame.light(
        Light::directional(Vec3::new(1.0, 0.0, -1.0))
            .intensity(3.0)
            .source_size(0.01)
            .shadow(ShadowQuality::High),
    );
    frame.draw(Draw::new(floor, cloth));
    frame.draw(Draw::new(card, glass).transform(Mat4::from_translation(Vec3::new(-0.5, 0.0, 0.5))));
    let shot = Shot::take(&mut renderer, &frame);
    let behind = shot.at(64, 64);
    let open = shot.at(100, 64);
    assert!(
        behind.x > 1.5 * behind.y.max(1.0) && behind.x > 1.5 * behind.z.max(1.0),
        "light through red glass reaches the floor red: {behind}"
    );
    assert!(
        open.y > 1.5 * behind.y.max(1.0),
        "the open floor stays white: {open} vs {behind}"
    );
}

#[test]
fn mmd_emission_reaches_emission_bloom() {
    let mut renderer = renderer();
    let card = quad(&mut renderer, 0.3);
    let glowing = create(
        &mut renderer,
        opaque("mmd")
            .with("base_color", Vec4::new(0.0, 0.0, 0.0, 1.0))
            .and_then(|desc| desc.with("emission_color", Vec3::new(4.0, 0.0, 0.0)))
            .expect("emissive parameters"),
    );
    let mut shoot = |bloom: f32| {
        let mut look = looked();
        let exposure = *rikka_test_renderer_3d::exposure_mut(&mut look);
        let mut composite = Composite::new();
        let exposed = composite.push(exposure);
        composite.node(
            Glow {
                bloom,
                diffusion: 0.0,
                streaks: 0.0,
                flare: 0.0,
                ..Glow::default()
            },
            [
                (Composite::IMAGE, CompositeInput::Node(exposed)),
                (Glow::SOURCE, CompositeInput::Layer(RenderLayer::Emission)),
            ],
        );
        composite.push(Grade::default());
        composite.push(Tonemap::default());
        look.composite = composite;
        let mut frame = frame(Some(look));
        frame.background(Background::Color(Vec3::ZERO));
        frame.draw(Draw::new(card, glowing));
        Shot::take(&mut renderer, &frame)
    };
    let plain = shoot(0.0);
    let bloomed = shoot(1.0);
    assert!(
        bloomed.at(82, 64).x > plain.at(82, 64).x + 8.0,
        "the emitter blooms past its edge: {} vs {}",
        bloomed.at(82, 64),
        plain.at(82, 64)
    );
}

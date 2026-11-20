use rikka_renderer::Renderer;
use rikka_renderer_3d::Error;
use rikka_renderer_3d::Three;
use rikka_renderer_3d::glam::{Mat4, Quat, Vec2, Vec3, Vec4};
use rikka_renderer_3d::rikka_ies::IesProfile;
use rikka_renderer_3d::{
    Background, Camera, Draw, Figure, Frame, Head, Indices, KeyLight, Light, LightRig, Look,
    MaterialDesc, MaterialId, MaterialState, MeshData, MeshId, Outline, ShaderCompiler,
    ShadowQuality, ToneMapping,
};

use crate::support::{SIZE, ambient_frame, flat, quad, read, renderer, viewport};

const IRRADIANCE: &str = r#"
import rikka;

struct Surface : ISurface<RikkaNoData> {
    [Default(1.0)] float gain;

    [constref]
    RikkaSurfaceData<RikkaNoData> evaluate(RikkaPixel pixel, RikkaContext context) {
        RikkaNoData data;
        data.unused = 0u;
        return rikka_surface_data(pixel, data);
    }
};

struct Model : IShadingModel {
    typealias Data = RikkaNoData;
    typealias State = float4;

    [constref]
    State prepare(RikkaPixel pixel, RikkaSurfaceData<Data> surface, RikkaContext context) {
        return float4(normalize(surface.normal), 0.0);
    }

    [constref]
    void direct(inout State state, RikkaLightSample light, inout RikkaShadowQuery shadow, RikkaContext context) {
        state.w += light.irradiance(context.frame, state.xyz) * light.light.intensity;
    }

    [constref]
    void indirect(inout State state, RikkaIndirect indirect, RikkaContext context) {
    }

    [constref]
    float4 resolve(State state, RikkaPixel pixel, RikkaContext context) {
        return float4(float3(state.w), 1.0);
    }
};
"#;

const EYE: Vec3 = Vec3::new(0.0, 0.0, 3.0);
const FOV: f32 = 1.0;

fn screen(point: Vec3) -> (usize, usize) {
    let focal = 1.0 / (0.5 * FOV).tan();
    let depth = EYE.z - point.z;
    let ndc = Vec2::new(point.x, point.y) * focal / depth;
    let x = (ndc.x * 0.5 + 0.5) * SIZE as f32;
    let y = (0.5 - ndc.y * 0.5) * SIZE as f32;
    (x as usize, y as usize)
}

fn texel(pixels: &[u8], (x, y): (usize, usize)) -> [u8; 4] {
    let i = (y * SIZE as usize + x) * 4;
    [pixels[i], pixels[i + 1], pixels[i + 2], pixels[i + 3]]
}

fn luminance(pixels: &[u8], at: (usize, usize)) -> f32 {
    let [r, g, b, _] = texel(pixels, at);
    0.2126 * f32::from(r) + 0.7152 * f32::from(g) + 0.0722 * f32::from(b)
}

fn linear(encoded: u8) -> f32 {
    let value = f32::from(encoded) / 255.0;
    match value <= 0.04045 {
        true => value / 12.92,
        false => ((value + 0.055) / 1.055).powf(2.4),
    }
}

fn lit_frame() -> Frame {
    let mut frame = ambient_frame(0.0);
    frame.tone_mapping(ToneMapping::Clamp);
    frame
}

fn cube(renderer: &mut Renderer, half: f32) -> MeshId {
    let faces = [
        Vec3::X,
        Vec3::NEG_X,
        Vec3::Y,
        Vec3::NEG_Y,
        Vec3::Z,
        Vec3::NEG_Z,
    ];
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut indices = Vec::new();
    for normal in faces {
        let across = normal.any_orthonormal_vector();
        let up = normal.cross(across);
        let first = positions.len() as u32;
        for (s, t) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
            positions.push((normal + across * s + up * t) * half);
            normals.push(normal);
        }
        indices.extend([first, first + 1, first + 2, first, first + 2, first + 3]);
    }
    let tangents = vec![Vec4::new(1.0, 0.0, 0.0, 1.0); positions.len()];
    let uvs = vec![Vec2::ZERO; positions.len()];
    renderer
        .three()
        .create_mesh(&MeshData {
            positions: &positions,
            normals: &normals,
            tangents: &tangents,
            uvs: &uvs,
            indices: Some(Indices::U32(&indices)),
            ..MeshData::default()
        })
        .expect("a cube")
}

fn form_factor_corner(x: f32, y: f32) -> f32 {
    let a = (1.0 + x * x).sqrt();
    let b = (1.0 + y * y).sqrt();
    (x / a * (y / a).atan() + y / b * (x / b).atan()) / (2.0 * std::f32::consts::PI)
}

#[test]
fn area_light_irradiance_matches_the_analytic_integral() {
    let mut renderer = renderer();
    let mut view = viewport(&mut renderer, 1);
    let receiver = quad(&mut renderer, 0.0, 1.0);
    let template = ShaderCompiler::new()
        .and_then(|compiler| compiler.template("irradiance", IRRADIANCE))
        .expect("the irradiance template compiles");
    let material = renderer
        .three()
        .create_material(MaterialDesc::new(&template))
        .expect("a material");
    let pi = std::f32::consts::PI;
    let luminance = 0.4;
    let toward = Vec3::NEG_Z;
    let cases = [
        (
            "rect",
            Light::rect(Vec3::new(0.0, 0.0, 0.5), toward, Vec3::X, 1.0, 0.6, 50.0),
            pi * 4.0 * form_factor_corner(1.0, 0.6),
        ),
        (
            "disc",
            Light::disc(Vec3::new(0.0, 0.0, 0.5), toward, 0.4, 50.0),
            pi * 0.16 / (0.25 + 0.16),
        ),
        (
            "sphere",
            Light::sphere(Vec3::new(0.0, 0.0, 0.6), 0.15, 50.0),
            pi * 0.15 * 0.15 / 0.36,
        ),
    ];
    for (name, light, form) in cases {
        let mut frame = lit_frame();
        frame
            .light(light.intensity(luminance))
            .draw(Draw::new(receiver, material));
        let pixels = read(&mut renderer, &mut view, &frame);
        let measured = linear(texel(&pixels, (64, 64))[0]);
        let expected = luminance * form;
        let error = (measured - expected).abs() / expected;
        assert!(
            error < 0.05,
            "{name}: irradiance {measured} against the analytic {expected}"
        );
    }
}

#[test]
fn openpbr_area_lights_follow_the_ltc_integrals() {
    let mut renderer = renderer();
    let mut view = viewport(&mut renderer, 1);
    let receiver = quad(&mut renderer, 0.0, 1.0);
    let paint = |renderer: &mut Renderer, values: &[(&str, f32)], color: Vec3| {
        let desc = values.iter().try_fold(
            rikka_test_renderer_3d::standard(color.extend(1.0), MaterialState::opaque()),
            |desc, &(name, value)| desc.with(name, value),
        );
        renderer
            .three()
            .create_material(desc.expect("standard parameters"))
            .expect("a material")
    };
    let shine = |renderer: &mut Renderer, view: &mut rikka_renderer_3d::Viewport, material| {
        let mut frame = lit_frame();
        frame
            .light(
                Light::rect(
                    Vec3::new(0.0, 0.0, 1.0),
                    Vec3::NEG_Z,
                    Vec3::X,
                    0.6,
                    0.2,
                    50.0,
                )
                .intensity(0.8),
            )
            .draw(Draw::new(receiver, material));
        read(renderer, view, &frame)
    };

    let matte = paint(
        &mut renderer,
        &[("specular_weight", 0.0), ("base_diffuse_roughness", 0.0)],
        Vec3::ONE,
    );
    let mut frame = lit_frame();
    frame
        .light(
            Light::rect(
                Vec3::new(0.0, 0.0, 0.5),
                Vec3::NEG_Z,
                Vec3::X,
                1.0,
                0.6,
                50.0,
            )
            .intensity(0.4),
        )
        .draw(Draw::new(receiver, matte));
    let pixels = read(&mut renderer, &mut view, &frame);
    let measured = linear(texel(&pixels, (64, 64))[1]);
    let expected = 0.4 * 4.0 * form_factor_corner(1.0, 0.6);
    assert!(
        (measured - expected).abs() < 0.05 * expected,
        "diffuse follows the clipped irradiance: {measured} against {expected}"
    );

    let mirrors = [
        (
            "metal",
            paint(
                &mut renderer,
                &[("base_metalness", 1.0), ("specular_roughness", 0.05)],
                Vec3::ONE,
            ),
        ),
        (
            "coat",
            paint(
                &mut renderer,
                &[("coat_weight", 1.0), ("coat_roughness", 0.05)],
                Vec3::ZERO,
            ),
        ),
    ];
    for (name, material) in mirrors {
        let pixels = shine(&mut renderer, &mut view, material);
        let along = linear(texel(&pixels, screen(Vec3::new(0.15, 0.0, 0.0)))[1]);
        let across = linear(texel(&pixels, screen(Vec3::new(0.0, 0.15, 0.0)))[1]);
        assert!(
            along > 0.02 && across < 0.25 * along,
            "{name}: the reflection takes the rect's shape: {along} along, {across} across"
        );
    }
}

#[test]
fn photometric_units_follow_the_renderer_scale() {
    let sun = Light::directional(Vec3::NEG_Y).lux(32_000.0).expect("lux");
    assert!((sun.intensity - 1.0).abs() < 1e-6);
    let bulb = Light::point(Vec3::ZERO, 10.0)
        .lumens(4.0 * std::f32::consts::PI * 32_000.0)
        .expect("lumens");
    assert!((bulb.intensity - 1.0).abs() < 1e-5);
    let panel = Light::rect(Vec3::ZERO, Vec3::NEG_Y, Vec3::X, 1.0, 1.0, 5.0)
        .lumens(std::f32::consts::PI * 32_000.0)
        .expect("lumens");
    assert!((panel.intensity - 1.0).abs() < 1e-5);
    assert!(Light::point(Vec3::ZERO, 1.0).lux(1.0).is_err());
    assert!(Light::directional(Vec3::NEG_Y).nits(1.0).is_err());
}

#[test]
fn light_channels_isolate_light_and_shadow() {
    let mut renderer = renderer();
    let mut view = viewport(&mut renderer, 1);
    let plate = quad(&mut renderer, 0.0, 0.4);
    let floor = quad(&mut renderer, 0.0, 1.5);
    let blocker = quad(&mut renderer, 0.0, 0.3);
    let white = flat(&mut renderer, [1.0, 1.0, 1.0, 1.0]);

    let mut frame = lit_frame();
    frame
        .light(
            Light::directional(Vec3::NEG_Z)
                .intensity(3.0)
                .channels(2)
                .shadow(ShadowQuality::None),
        )
        .draw(
            Draw::new(plate, white)
                .transform(Mat4::from_translation(Vec3::new(-0.5, 0.0, 0.0)))
                .light_channels(1),
        )
        .draw(
            Draw::new(plate, white)
                .transform(Mat4::from_translation(Vec3::new(0.5, 0.0, 0.0)))
                .light_channels(2),
        );
    let pixels = read(&mut renderer, &mut view, &frame);
    let outside = luminance(&pixels, screen(Vec3::new(-0.5, 0.0, 0.0)));
    let inside = luminance(&pixels, screen(Vec3::new(0.5, 0.0, 0.0)));
    assert!(
        outside < 4.0 && inside > 150.0,
        "only the draw sharing a channel is lit: {outside} vs {inside}"
    );

    let shadowed =
        |channels: u32, renderer: &mut Renderer, view: &mut rikka_renderer_3d::Viewport| {
            let mut frame = lit_frame();
            frame
                .light(
                    Light::directional(Vec3::new(1.0, 0.0, -1.0))
                        .intensity(3.0)
                        .channels(2)
                        .shadow(ShadowQuality::High),
                )
                .draw(Draw::new(floor, white).light_channels(2))
                .draw(
                    Draw::new(blocker, white)
                        .transform(Mat4::from_translation(Vec3::new(-0.5, 0.0, 0.5)))
                        .light_channels(channels),
                );
            let pixels = read(renderer, view, &frame);
            luminance(&pixels, screen(Vec3::new(0.15, 0.0, 0.0)))
        };
    let unlinked = shadowed(1, &mut renderer, &mut view);
    let linked = shadowed(3, &mut renderer, &mut view);
    assert!(
        unlinked > 150.0 && linked < 20.0,
        "a caster shadows only for lights it shares a channel with: {unlinked} vs {linked}"
    );
}

#[test]
fn character_rig_lights_only_its_figure() {
    let mut renderer = renderer();
    let mut view = viewport(&mut renderer, 1);
    let plate = quad(&mut renderer, 0.0, 0.4);
    let white = flat(&mut renderer, [0.8, 0.8, 0.8, 1.0]);
    let left = Vec3::new(-0.5, 0.0, 0.0);
    let right = Vec3::new(0.5, 0.0, 0.0);

    let draw = |rig: LightRig, renderer: &mut Renderer, view: &mut rikka_renderer_3d::Viewport| {
        let mut frame = lit_frame();
        frame.light(
            Light::directional(Vec3::new(0.0, -0.3, -1.0))
                .intensity(1.0)
                .shadow(ShadowQuality::None),
        );
        let rigged = frame.figure(Figure {
            rig,
            ..Figure::default()
        });
        let plain = frame.figure(Figure::default());
        frame
            .draw(
                Draw::new(plate, white)
                    .transform(Mat4::from_translation(left))
                    .figure(rigged),
            )
            .draw(
                Draw::new(plate, white)
                    .transform(Mat4::from_translation(right))
                    .figure(plain),
            );
        read(renderer, view, &frame)
    };
    let base = draw(LightRig::default(), &mut renderer, &mut view);
    let rig = LightRig {
        key: Some(KeyLight {
            color: Some(Vec3::new(1.0, 0.2, 0.2)),
            ..KeyLight::default()
        }),
        lights: vec![
            Light::point(Vec3::new(0.5, 0.0, 0.6), 3.0).intensity(0.4),
            Light::directional(Vec3::new(0.0, 0.0, -1.0))
                .color(Vec3::new(0.0, 0.0, 1.0))
                .intensity(0.5)
                .shadow(ShadowQuality::None),
        ],
    };
    let rigged = draw(rig, &mut renderer, &mut view);
    let [r0, g0, b0, _] = texel(&base, screen(left));
    let [r1, g1, b1, _] = texel(&rigged, screen(left));
    assert!(
        r1 >= r0 && g1 < g0 && b1 > g1 + 20,
        "the rig's red key and blue fill reach their figure: {:?} -> {:?}",
        [r0, g0, b0],
        [r1, g1, b1]
    );
    assert_eq!(
        texel(&base, screen(right)),
        texel(&rigged, screen(right)),
        "the other figure is untouched, even under the rig's point light"
    );
}

#[test]
fn transparent_casters_tint_their_shadows() {
    let mut renderer = renderer();
    let mut view = viewport(&mut renderer, 1);
    let floor = quad(&mut renderer, 0.0, 1.5);
    let pane = quad(&mut renderer, 0.0, 0.3);
    let white = flat(&mut renderer, [1.0, 1.0, 1.0, 1.0]);
    let red = renderer
        .three()
        .create_material(
            rikka_test_renderer_3d::standard(
                Vec4::ONE,
                MaterialState::transparent().shadows(true, true),
            )
            .with("transmission_weight", 1.0)
            .and_then(|desc| desc.with("transmission_color", Vec3::new(1.0, 0.0, 0.0)))
            .expect("the standard template transmits"),
        )
        .expect("a glass material");
    let mut frame = lit_frame();
    frame
        .light(
            Light::directional(Vec3::new(1.0, 0.0, -1.0))
                .intensity(2.0)
                .shadow(ShadowQuality::High),
        )
        .draw(Draw::new(floor, white))
        .draw(Draw::new(pane, red).transform(Mat4::from_translation(Vec3::new(-0.5, 0.0, 0.5))));
    let pixels = read(&mut renderer, &mut view, &frame);
    let [r, g, b, _] = texel(&pixels, screen(Vec3::new(0.1, 0.0, 0.0)));
    let lit = luminance(&pixels, screen(Vec3::new(0.9, 0.0, 0.0)));
    let tinted = luminance(&pixels, screen(Vec3::new(0.1, 0.0, 0.0)));
    assert!(
        f32::from(r) > 1.5 * f32::from(g).max(1.0) && f32::from(r) > 1.5 * f32::from(b).max(1.0),
        "light through red glass falls red: {:?}",
        [r, g, b]
    );
    assert!(
        tinted < 0.9 * lit && tinted > 0.1 * lit,
        "glass dims but does not block: {tinted} of {lit}"
    );
}

#[test]
fn figure_shadow_map_resolves_detail_the_cascades_lose() {
    let mut renderer = renderer();
    let mut view = viewport(&mut renderer, 1);
    let plate = quad(&mut renderer, 0.0, 0.5);
    let white = flat(&mut renderer, [1.0, 1.0, 1.0, 1.0]);
    let camera = Camera::look_at(
        Vec3::new(0.0, 0.0, 30.0),
        Vec3::ZERO,
        Vec3::Y,
        0.02,
        1.0,
        1.0,
    );
    let bar = Mat4::from_translation(Vec3::new(0.0, 0.0, 0.08))
        * Mat4::from_scale(Vec3::new(0.03, 0.6, 1.0));
    let draw = |mapped: ShadowQuality,
                crowd: u32,
                renderer: &mut Renderer,
                view: &mut rikka_renderer_3d::Viewport| {
        let mut frame = Frame::new(camera);
        frame
            .background(Background::Transparent)
            .ambient(Vec3::ZERO)
            .tone_mapping(ToneMapping::Clamp)
            .outline(Outline {
                enabled: false,
                ..Outline::default()
            })
            .light(
                Light::directional(Vec3::new(0.25, 0.0, -1.0))
                    .intensity(2.0)
                    .shadow(ShadowQuality::Low),
            );
        let figure = frame.figure(Figure {
            shadow: mapped,
            head: Some(Head {
                center: Vec3::new(0.0, 0.0, 0.08),
                forward: Vec3::Z,
                up: Vec3::Y,
                radius: 0.3,
            }),
            ..Figure::default()
        });
        frame
            .draw(Draw::new(plate, white).figure(figure))
            .draw(Draw::new(plate, white).transform(bar).figure(figure));
        for index in 0..crowd {
            let far = frame.figure(Figure {
                shadow: ShadowQuality::High,
                ..Figure::default()
            });
            let aside = Mat4::from_translation(Vec3::new(40.0 + index as f32 * 2.0, 0.0, 0.0));
            frame.draw(Draw::new(plate, white).transform(aside).figure(far));
        }
        read(renderer, view, &frame)
    };
    let darkest = |pixels: &[u8]| {
        (66..80)
            .map(|x| luminance(pixels, (x, 64)))
            .fold(f32::MAX, f32::min)
    };
    let lit_point = (110, 64);
    let mapped = draw(ShadowQuality::High, 0, &mut renderer, &mut view);
    let crowded = draw(ShadowQuality::High, 12, &mut renderer, &mut view);
    let cascades = draw(ShadowQuality::None, 0, &mut renderer, &mut view);
    let lit = luminance(&mapped, lit_point);
    for (pixels, case) in [(&mapped, "alone"), (&crowded, "among far figures")] {
        assert!(
            darkest(pixels) < 0.35 * lit,
            "the figure map shows the bar's shadow {case}: {} of {lit}",
            darkest(pixels)
        );
    }
    assert!(
        darkest(&cascades) > 0.7 * lit,
        "a coarse cascade loses it: {} of {lit}",
        darkest(&cascades)
    );
}

#[test]
fn many_figure_maps_leave_the_cascades_their_resolution() {
    let mut renderer = renderer();
    let mut view = viewport(&mut renderer, 1);
    let plate = quad(&mut renderer, 0.0, 0.5);
    let white = flat(&mut renderer, [1.0, 1.0, 1.0, 1.0]);
    let camera = Camera::look_at(
        Vec3::new(0.0, 0.0, 30.0),
        Vec3::ZERO,
        Vec3::Y,
        0.02,
        1.0,
        1.0,
    );
    let bar = Mat4::from_translation(Vec3::new(0.0, 0.0, 0.08))
        * Mat4::from_scale(Vec3::new(0.1, 0.6, 1.0));
    let draw = |figures: u32, renderer: &mut Renderer, view: &mut rikka_renderer_3d::Viewport| {
        let mut frame = Frame::new(camera);
        frame
            .background(Background::Transparent)
            .ambient(Vec3::ZERO)
            .tone_mapping(ToneMapping::Clamp)
            .outline(Outline {
                enabled: false,
                ..Outline::default()
            })
            .light(
                Light::directional(Vec3::new(0.25, 0.0, -1.0))
                    .intensity(2.0)
                    .shadow(ShadowQuality::High),
            )
            .draw(Draw::new(plate, white))
            .draw(Draw::new(plate, white).transform(bar));
        for index in 0..figures {
            let figure = frame.figure(Figure {
                shadow: ShadowQuality::High,
                ..Figure::default()
            });
            let aside = Mat4::from_translation(Vec3::new(40.0 + index as f32 * 2.0, 0.0, 0.0));
            frame.draw(Draw::new(plate, white).transform(aside).figure(figure));
        }
        read(renderer, view, &frame)
    };
    let alone = draw(0, &mut renderer, &mut view);
    let crowded = draw(12, &mut renderer, &mut view);
    let darkest = (60..80)
        .map(|x| luminance(&alone, (x, 64)))
        .fold(f32::MAX, f32::min);
    let lit = luminance(&alone, (110, 64));
    assert!(
        darkest < 0.75 * lit,
        "the cascade shows the bar's shadow: {darkest} of {lit}"
    );
    assert_eq!(alone, crowded, "figure maps take nothing from the cascades");
}

const HALF_PROFILE: &str = "IESNA:LM-63-2002
[TEST] half
TILT=NONE
1 -1 1 3 5 1 2 0 0 0
1 1 10
0 45 90
0 90 180 270 360
1000 1000 0
500 500 0
0 0 0
500 500 0
1000 1000 0
";

#[test]
fn ies_profile_shapes_a_point_light() {
    let mut renderer = renderer();
    let mut view = viewport(&mut renderer, 1);
    let floor = quad(&mut renderer, 0.0, 1.5);
    let white = flat(&mut renderer, [1.0, 1.0, 1.0, 1.0]);
    let profile = IesProfile::parse(HALF_PROFILE.as_bytes()).expect("an LM-63 profile");
    let id = renderer
        .three()
        .create_light_profile(&profile)
        .expect("a light profile");
    let downward = Quat::from_rotation_arc(Vec3::NEG_Y, Vec3::NEG_Z);
    let lamp = Light::point(Vec3::new(0.0, 0.0, 0.8), 4.0).intensity(0.4);
    let frame = |light: Light| {
        let mut frame = lit_frame();
        frame.light(light).draw(Draw::new(floor, white));
        frame
    };
    let green = |pixels: &[u8], at: Vec3| linear(texel(pixels, screen(at))[1]);
    let plain = read(&mut renderer, &mut view, &frame(lamp));
    let shaped = read(
        &mut renderer,
        &mut view,
        &frame(lamp.oriented_profile(id, downward)),
    );
    let cases = [
        (Vec3::new(0.4, 0.0, 0.0), 1.0),
        (Vec3::new(0.0, 0.4, 0.0), 0.5),
        (Vec3::new(0.0, -0.4, 0.0), 0.5),
        (Vec3::new(-0.4, 0.0, 0.0), 0.0),
    ];
    for (at, expected) in cases {
        let unshaped = green(&plain, at);
        assert!(
            unshaped > 0.05 && unshaped < 0.9,
            "the plain light stays in range at {at}: {unshaped}"
        );
        let ratio = green(&shaped, at) / unshaped;
        assert!(
            (ratio - expected).abs() < 0.06,
            "the profile scales the light at {at} by {ratio}, not {expected}"
        );
    }

    assert!(renderer.three().destroy_light_profile(id).expect("destroy"));
    let refused = renderer
        .three()
        .render(&mut view, &frame(lamp.oriented_profile(id, downward)));
    assert!(
        matches!(
            refused,
            Err(Error::Unknown(rikka_renderer_3d::Resource::LightProfile(_)))
        ),
        "a destroyed profile is refused: {refused:?}"
    );
}

#[test]
fn cached_local_shadows_follow_moved_casters() {
    let mut renderer = renderer();
    let mut view = viewport(&mut renderer, 1);
    let floor = quad(&mut renderer, 0.0, 1.5);
    let blocker = quad(&mut renderer, 0.0, 0.15);
    let white = flat(&mut renderer, [1.0, 1.0, 1.0, 1.0]);
    let draw = |at: Vec3, renderer: &mut Renderer, view: &mut rikka_renderer_3d::Viewport| {
        let mut frame = lit_frame();
        frame
            .light(
                Light::spot(Vec3::new(0.0, 0.0, 1.2), Vec3::NEG_Z, 4.0, 0.6, 0.9)
                    .intensity(3.0)
                    .shadow(ShadowQuality::Medium),
            )
            .draw(Draw::new(floor, white))
            .draw(Draw::new(blocker, white).transform(Mat4::from_translation(at)));
        read(renderer, view, &frame)
    };
    let left = Vec3::new(-0.3, 0.0, 0.6);
    let right = Vec3::new(0.3, 0.0, 0.6);
    let first = draw(left, &mut renderer, &mut view);
    let again = draw(left, &mut renderer, &mut view);
    assert_eq!(
        first, again,
        "an unchanged light and caster redraw the same shadow"
    );
    let moved = draw(right, &mut renderer, &mut view);
    let under = |pixels: &[u8], x: f32| luminance(pixels, screen(Vec3::new(x * 2.0, 0.0, 0.0)));
    assert!(
        under(&first, -0.35) < 0.3 * under(&moved, -0.35),
        "the shadow left where the caster was: {} vs {}",
        under(&first, -0.35),
        under(&moved, -0.35)
    );
    assert!(
        under(&moved, 0.35) < 0.3 * under(&first, 0.35),
        "and fell where it went: {} vs {}",
        under(&moved, 0.35),
        under(&first, 0.35)
    );
}

fn looked(screen: f32) -> Look {
    let mut look = rikka_test_renderer_3d::minimal_look();
    let sky = rikka_test_renderer_3d::sky_of(&mut look);
    sky.sun = 0.0;
    sky.moon = 0.0;
    rikka_test_renderer_3d::exposure_mut(&mut look).adaptation = 0.0;
    look.shadows.screen.strength = screen;
    look
}

fn project(camera: &Camera, point: Vec3) -> Vec2 {
    let ndc = camera.view_projection().project_point3(point);
    Vec2::new(ndc.x * 0.5 + 0.5, ndc.y * 0.5 + 0.5) * SIZE as f32
}

const RESTING: Vec3 = Vec3::new(0.0, -0.3, 0.15);
const FLOATING: Vec3 = Vec3::new(0.7, -0.3, 0.9);

struct Blocks {
    floor: MeshId,
    block: MeshId,
    white: MaterialId,
    camera: Camera,
}

impl Blocks {
    fn new(renderer: &mut Renderer) -> Self {
        Self {
            floor: quad(renderer, 0.0, 1.5),
            block: cube(renderer, 0.15),
            white: flat(renderer, [0.8, 0.8, 0.8, 1.0]),
            camera: Camera::look_at(
                Vec3::new(0.0, -2.6, 1.4),
                Vec3::new(0.0, -0.3, 0.0),
                Vec3::Z,
                FOV,
                1.0,
                0.05,
            ),
        }
    }

    fn frame(&self, light: Vec3, look: Look) -> Frame {
        let mut frame = Frame::new(self.camera);
        frame
            .background(Background::Transparent)
            .ambient(Vec3::ZERO)
            .tone_mapping(ToneMapping::Clamp)
            .outline(Outline {
                enabled: false,
                ..Outline::default()
            })
            .look(look)
            .light(
                Light::directional(light)
                    .intensity(2.0)
                    .shadow(ShadowQuality::None),
            )
            .draw(Draw::new(self.floor, self.white))
            .draw(Draw::new(self.block, self.white).transform(Mat4::from_translation(RESTING)))
            .draw(Draw::new(self.block, self.white).transform(Mat4::from_translation(FLOATING)));
        frame
    }
}

#[test]
fn screen_shadows_add_contact_without_leaking() {
    let mut renderer = renderer();
    let mut view = viewport(&mut renderer, 1);
    let blocks = Blocks::new(&mut renderer);
    let camera = blocks.camera;
    let light = Vec3::new(0.0, -1.0, -0.35);
    let plain = read(&mut renderer, &mut view, &blocks.frame(light, looked(0.0)));
    let traced = read(&mut renderer, &mut view, &blocks.frame(light, looked(1.0)));
    let pixel = |point: Vec3| {
        let at = project(&camera, point);
        (at.x as usize, at.y as usize)
    };
    let contact = pixel(Vec3::new(0.0, -0.5, 0.0));
    assert!(
        luminance(&traced, contact) < 0.6 * luminance(&plain, contact),
        "the floor at the resting block's base darkens: {} vs {}",
        luminance(&traced, contact),
        luminance(&plain, contact)
    );
    let corners = (0..8).map(|index| {
        let sign = |bit: usize| if index & bit == 0 { -0.15 } else { 0.15 };
        project(&camera, FLOATING + Vec3::new(sign(1), sign(2), sign(4)))
    });
    let (low, high) = corners.fold((Vec2::MAX, Vec2::MIN), |(low, high), at| {
        (low.min(at), high.max(at))
    });
    let column = ((low.x + high.x) * 0.5) as usize;
    let open = pixel(Vec3::new(-0.9, 0.6, 0.0));
    let below = (4..40).map(|step| (column, high.y as usize + step));
    for at in below.chain([open]) {
        let difference = (luminance(&traced, at) - luminance(&plain, at)).abs();
        assert!(
            difference < 3.0,
            "floor that no surface touches stays lit: {difference} at {at:?}"
        );
    }
}

fn spaced() -> Look {
    let full = Look::default();
    let mut look = looked(1.0);
    look.occlusion = full.occlusion;
    look.reflections = full.reflections;
    look.illumination = full.illumination;
    look
}

fn differing(pixels: &[u8], expected: &[u8]) -> Vec<(usize, usize)> {
    (0..pixels.len() / 4)
        .filter(|&at| pixels[at * 4..at * 4 + 4] != expected[at * 4..at * 4 + 4])
        .map(|at| (at % SIZE as usize, at / SIZE as usize))
        .collect()
}

#[test]
fn screen_shadows_match_in_fresh_and_used_viewports() {
    let mut renderer = renderer();
    let blocks = Blocks::new(&mut renderer);
    let light = Vec3::new(0.3, 1.0, -0.6);
    let traced = blocks.frame(light, looked(1.0));
    let mut first = viewport(&mut renderer, 1);
    let expected = read(&mut renderer, &mut first, &traced);
    for run in 0..3 {
        let mut fresh = viewport(&mut renderer, 1);
        let mut used = viewport(&mut renderer, 1);
        read(&mut renderer, &mut used, &blocks.frame(light, looked(0.0)));
        for (name, view) in [("fresh", &mut fresh), ("used", &mut used)] {
            let pixels = read(&mut renderer, view, &traced);
            let differing = differing(&pixels, &expected);
            assert!(
                differing.is_empty(),
                "run {run}: a {name} viewport differs at {differing:?}"
            );
        }
    }
}

#[test]
fn screen_space_lighting_repeats_exactly_in_new_viewports() {
    let mut renderer = renderer();
    let blocks = Blocks::new(&mut renderer);
    let light = Vec3::new(0.3, 1.0, -0.6);
    let plain = blocks.frame(light, looked(0.0));
    let lit = blocks.frame(light, spaced());
    let mut kept = Vec::new();
    let mut expected = Vec::new();
    for run in 0..4 {
        let mut view = viewport(&mut renderer, 1);
        read(&mut renderer, &mut view, &plain);
        let mut pixels = Vec::new();
        for _ in 0..3 {
            pixels = read(&mut renderer, &mut view, &lit);
        }
        if run == 0 {
            expected = pixels;
        } else {
            let differing = differing(&pixels, &expected);
            assert!(differing.is_empty(), "run {run} differs at {differing:?}");
        }
        kept.push(view);
    }
}

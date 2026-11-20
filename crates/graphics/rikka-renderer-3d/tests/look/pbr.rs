use rikka_renderer::{Node, Renderer, Scene};
use rikka_renderer_3d::Three;
use rikka_renderer_3d::glam::{Mat4, Vec2, Vec3, Vec4};
use rikka_renderer_3d::{
    Antialiasing, Background, BuiltinTemplate, Camera, Draw, Environment, Figure, Frame, Indices,
    Light, Look, MaterialDesc, MaterialId, MaterialState, MaterialTemplate, MeshData, MeshId,
    NITS_PER_RADIANCE_UNIT, Outline, Overrides, ToneMapping, Upscale, ViewportConfig,
};
use rikka_test_renderer::TestRenderer;
use rikka_types::{Color, Rect};

const SIZE: u32 = 96;
const GREY: f32 = 0.45;

fn renderer() -> TestRenderer {
    rikka_test_renderer::renderer_3d("rikka-pbr-test")
        .expect("a Vulkan 1.3 device with the mesh feature set is required for these tests")
}

fn standard() -> MaterialDesc {
    MaterialDesc::new(&MaterialTemplate::builtin(BuiltinTemplate::Standard))
}

fn material(renderer: &mut Renderer, desc: MaterialDesc) -> MaterialId {
    renderer.three().create_material(desc).expect("a material")
}

fn look() -> Look {
    let mut look = Look {
        environment: Environment::None,
        ..rikka_test_renderer_3d::minimal_look()
    };
    rikka_test_renderer_3d::exposure_mut(&mut look).adaptation = 0.0;
    look.shadows.screen.strength = 0.0;
    look
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

fn frame(background: Vec3, ambient: Vec3) -> Frame {
    let mut frame = Frame::new(camera());
    frame
        .background(Background::Color(background))
        .ambient(ambient)
        .tone_mapping(ToneMapping::Clamp)
        .time(1.0)
        .look(look())
        .outline(Outline {
            enabled: false,
            ..Outline::default()
        });
    frame
}

const MODES: [(&str, Antialiasing); 3] = [
    ("multisample", Antialiasing::Multisample),
    ("temporal", Antialiasing::Temporal),
    (
        "upscaled",
        Antialiasing::Upscale(Upscale::fixed(0.67).sharpness(0.0)),
    ),
];

const SETTLING_FRAMES: usize = 24;

fn read(renderer: &mut Renderer, frame: &Frame) -> Vec<u8> {
    read_with(renderer, frame, Antialiasing::Multisample)
}

fn read_with(renderer: &mut Renderer, frame: &Frame, antialiasing: Antialiasing) -> Vec<u8> {
    let frames = match antialiasing {
        Antialiasing::Multisample => 2,
        Antialiasing::Temporal | Antialiasing::Upscale(_) => SETTLING_FRAMES,
    };
    read_after(renderer, frame, antialiasing, frames)
}

fn read_after(
    renderer: &mut Renderer,
    frame: &Frame,
    antialiasing: Antialiasing,
    frames: usize,
) -> Vec<u8> {
    let mut viewport = renderer
        .three()
        .create_viewport(
            ViewportConfig::new(SIZE, SIZE)
                .samples(1)
                .antialiasing(antialiasing),
        )
        .expect("a viewport");
    for _ in 0..frames {
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

fn brightness(pixels: &[u8], (x, y): (usize, usize)) -> f32 {
    let i = (y * SIZE as usize + x) * 4;
    (pixels[i] as f32 + pixels[i + 1] as f32 + pixels[i + 2] as f32) / 3.0
}

fn pixel_of(world: Vec3) -> (usize, usize) {
    let clip = camera().view_projection() * world.extend(1.0);
    let ndc = clip.truncate() / clip.w;
    let x = (ndc.x * 0.5 + 0.5) * SIZE as f32;
    let y = (ndc.y * 0.5 + 0.5) * SIZE as f32;
    (x as usize, y as usize)
}

fn plane(renderer: &mut Renderer, center: Vec3, half: Vec2) -> MeshId {
    let corners = [
        center + Vec3::new(-half.x, -half.y, 0.0),
        center + Vec3::new(half.x, -half.y, 0.0),
        center + Vec3::new(half.x, half.y, 0.0),
        center + Vec3::new(-half.x, half.y, 0.0),
    ];
    renderer
        .three()
        .create_mesh(&MeshData {
            positions: &corners,
            normals: &[Vec3::Z; 4],
            tangents: &[Vec4::new(1.0, 0.0, 0.0, 1.0); 4],
            uvs: &[Vec2::ZERO; 4],
            indices: Some(Indices::U32(&[0, 1, 2, 0, 2, 3])),
            ..MeshData::default()
        })
        .expect("a plane")
}

#[test]
fn white_furnace_hides_energy_preserving_standard_materials() {
    let mut renderer = renderer();
    let ball = rikka_test_renderer_3d::sphere(&mut renderer, 0.8, 48).expect("a sphere");
    let white = |desc: MaterialDesc| desc.with("base_color", Vec3::ONE);
    let cases: Vec<(String, MaterialDesc)> =
        [0.0f32, 0.02, 0.05, 0.1, 0.15, 0.2, 0.25, 0.5, 0.75, 1.0]
            .into_iter()
            .flat_map(|roughness| {
                let rough = |desc: MaterialDesc| desc.with("specular_roughness", roughness);
                [
                    ("dielectric", white(standard()).and_then(rough)),
                    (
                        "metal",
                        white(standard())
                            .and_then(rough)
                            .and_then(|desc| desc.with("base_metalness", 1.0)),
                    ),
                    (
                        "rough diffuse",
                        white(standard())
                            .and_then(rough)
                            .and_then(|desc| desc.with("base_diffuse_roughness", 1.0)),
                    ),
                    (
                        "coated",
                        white(standard()).and_then(rough).and_then(|desc| {
                            desc.with("coat_weight", 1.0)?
                                .with("coat_roughness", roughness)?
                                .with("coat_darkening", 0.0)
                        }),
                    ),
                    (
                        "fuzz",
                        white(standard()).and_then(rough).and_then(|desc| {
                            desc.with("fuzz_weight", 1.0)?
                                .with("fuzz_roughness", roughness)
                        }),
                    ),
                ]
                .map(|(name, desc)| {
                    (
                        format!("{name} at roughness {roughness}"),
                        desc.expect("standard parameters"),
                    )
                })
            })
            .collect();
    let background = Vec3::splat(GREY);
    let centre = pixel_of(Vec3::ZERO);
    let rim = pixel_of(Vec3::new(0.55, 0.0, 0.58));
    let mut failures = Vec::new();
    for (name, desc) in cases {
        let id = material(&mut renderer, desc);
        let mut lit = frame(background, background);
        lit.draw(Draw::new(ball, id));
        let pixels = read(&mut renderer, &lit);
        let expected = brightness(&pixels, (2, 2));
        for (place, at) in [("centre", centre), ("rim", rim)] {
            let seen = brightness(&pixels, at);
            if (seen - expected).abs() > 0.035 * expected {
                failures.push(format!("{name}: {place} {seen:.1} against {expected:.1}"));
            }
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

fn side_lit(
    renderer: &mut Renderer,
    ball: MeshId,
    (left, right): (MaterialId, MaterialId),
    antialiasing: Antialiasing,
) -> Vec<u8> {
    let mut lit = frame(Vec3::ZERO, Vec3::splat(0.05));
    lit.light(Light::directional(Vec3::new(-1.0, 0.0, -0.15)).intensity(3.0));
    lit.draw(Draw::new(ball, left).transform(Mat4::from_translation(Vec3::new(-0.6, 0.0, 0.0))));
    lit.draw(Draw::new(ball, right).transform(Mat4::from_translation(Vec3::new(0.6, 0.0, 0.0))));
    read_with(renderer, &lit, antialiasing)
}

#[test]
fn subsurface_scattering_changes_only_marked_pixels() {
    let mut renderer = renderer();
    let ball = rikka_test_renderer_3d::sphere(&mut renderer, 0.5, 48).expect("a sphere");
    let skin = |weight: f32| {
        MaterialDesc::new(&MaterialTemplate::builtin(BuiltinTemplate::Skin))
            .with("subsurface_weight", weight)
            .and_then(|desc| desc.with("subsurface_radius", 0.08))
            .expect("skin parameters")
    };
    let plain = material(
        &mut renderer,
        standard()
            .with("base_color", Vec3::new(0.8, 0.6, 0.5))
            .expect("standard parameters"),
    );
    let scattering = material(&mut renderer, skin(1.0));
    let unmarked = material(&mut renderer, skin(0.0));
    for (mode, antialiasing) in MODES {
        let scattered = side_lit(&mut renderer, ball, (scattering, plain), antialiasing);
        let reference = side_lit(&mut renderer, ball, (unmarked, plain), antialiasing);

        let differs = |range: std::ops::Range<usize>| {
            range
                .flat_map(|x| (0..SIZE as usize).map(move |y| (x, y)))
                .map(|at| (brightness(&scattered, at) - brightness(&reference, at)).abs())
                .fold(0.0f32, f32::max)
        };
        let (middle, _) = pixel_of(Vec3::ZERO);
        assert!(
            differs(0..middle) > 6.0,
            "{mode}: the scattering sphere changes under subsurface scattering"
        );
        assert!(
            differs(middle..SIZE as usize) <= 1.0,
            "{mode}: pixels without a subsurface weight stay exactly as they were"
        );
    }
}

#[test]
fn overrides_route_and_specialize_like_material_values() {
    let mut renderer = renderer();
    let ball = rikka_test_renderer_3d::sphere(&mut renderer, 0.5, 48).expect("a sphere");
    let skin = |weight: f32| {
        MaterialDesc::new(&MaterialTemplate::builtin(BuiltinTemplate::Skin))
            .with("subsurface_weight", weight)
            .and_then(|desc| desc.with("subsurface_radius", 0.08))
            .expect("skin parameters")
    };
    let coated = |weight: f32| {
        standard()
            .with("base_color", Vec3::new(0.6, 0.05, 0.05))
            .and_then(|desc| desc.with("coat_weight", weight))
            .expect("standard parameters")
    };
    let cases = [
        ("subsurface", skin(0.0), skin(1.0), "subsurface_weight"),
        ("coat", coated(0.0), coated(1.0), "coat_weight"),
    ];
    for (name, plain, valued, parameter) in cases {
        let plain = material(&mut renderer, plain);
        let shot = |renderer: &mut Renderer, id: MaterialId, overridden: bool, frames: usize| {
            let mut lit = frame(Vec3::ZERO, Vec3::splat(0.05));
            lit.light(Light::directional(Vec3::new(-1.0, -0.3, -0.4)).intensity(3.0));
            let mut draw = Draw::new(ball, id);
            if overridden {
                draw = draw.figure(lit.figure(Figure::new(Overrides::new().with(parameter, 1.0))));
            }
            lit.draw(draw);
            read_after(renderer, &lit, Antialiasing::Multisample, frames)
        };
        let untouched_first = shot(&mut renderer, plain, false, 1);
        let warming = shot(&mut renderer, plain, true, 1);
        let valued = material(&mut renderer, valued);
        let by_value = shot(&mut renderer, valued, false, 2);
        let by_override = shot(&mut renderer, plain, true, 2);
        let untouched = shot(&mut renderer, plain, false, 2);
        let gap = |a: &[u8], b: &[u8]| {
            (0..(SIZE * SIZE) as usize)
                .map(|at| {
                    (brightness(a, (at % SIZE as usize, at / SIZE as usize))
                        - brightness(b, (at % SIZE as usize, at / SIZE as usize)))
                    .abs()
                })
                .fold(0.0f32, f32::max)
        };
        assert!(
            gap(&by_value, &untouched) > 6.0,
            "{name}: the value changes the image"
        );
        assert!(
            gap(&untouched_first, &warming) <= 1.0,
            "{name}: an override keeps the material's own pipelines while its own compile"
        );
        assert!(
            gap(&by_value, &by_override) <= 1.0,
            "{name}: an override draws exactly what the value draws"
        );
    }
}

#[test]
fn transmission_refracts_the_opaque_scene() {
    let mut renderer = renderer();
    let ball = rikka_test_renderer_3d::sphere(&mut renderer, 0.6, 48).expect("a sphere");
    let dark = plane(
        &mut renderer,
        Vec3::new(-1.5, 0.0, -1.5),
        Vec2::new(1.5, 2.0),
    );
    let bright = plane(
        &mut renderer,
        Vec3::new(1.5, 0.0, -1.5),
        Vec2::new(1.5, 2.0),
    );
    let black = material(
        &mut renderer,
        standard()
            .with("base_color", Vec3::ZERO)
            .and_then(|desc| desc.with("specular_weight", 0.0))
            .expect("standard parameters"),
    );
    let white = material(
        &mut renderer,
        standard()
            .with("base_color", Vec3::ZERO)
            .and_then(|desc| desc.with("specular_weight", 0.0))
            .and_then(|desc| desc.with("emission_luminance", 0.8 * NITS_PER_RADIANCE_UNIT))
            .expect("standard parameters"),
    );
    let glass = |thin: bool| {
        standard()
            .preset("glass")
            .and_then(|desc| desc.with("geometry_thickness", 1.2))
            .and_then(|desc| desc.with("geometry_thin_walled", thin))
            .expect("glass parameters")
            .state(MaterialState::transparent())
    };
    let thick = material(&mut renderer, glass(false));
    let thin = material(&mut renderer, glass(true));
    for (mode, antialiasing) in MODES {
        let render = |renderer: &mut Renderer, glass: MaterialId| {
            let mut lit = frame(Vec3::ZERO, Vec3::splat(0.02));
            lit.draw(Draw::new(dark, black));
            lit.draw(Draw::new(bright, white));
            lit.draw(Draw::new(ball, glass));
            read_with(renderer, &lit, antialiasing)
        };
        let refracted = render(&mut renderer, thick);
        let straight = render(&mut renderer, thin);

        let through = pixel_of(Vec3::new(0.3, 0.0, 0.5));
        assert!(
            brightness(&straight, through) > 40.0,
            "{mode}: thin-walled glass lets the bright backdrop through, {}",
            brightness(&straight, through)
        );
        let (centre_x, centre_y) = pixel_of(Vec3::ZERO);
        let (edge_x, _) = pixel_of(Vec3::new(0.5, 0.0, 0.0));
        let reach = edge_x - centre_x;
        let mut inside = 0.0f32;
        let mut outside = 0.0f32;
        for y in 0..SIZE as usize {
            for x in 0..SIZE as usize {
                let gap = (brightness(&refracted, (x, y)) - brightness(&straight, (x, y))).abs();
                let distance = (x as f32 - centre_x as f32).hypot(y as f32 - centre_y as f32);
                match distance < reach as f32 {
                    true => inside = inside.max(gap),
                    false if distance > reach as f32 * 1.4 => outside = outside.max(gap),
                    false => {}
                }
            }
        }
        assert!(
            inside > 30.0,
            "{mode}: thick glass bends the backdrop seen through it, {inside}"
        );
        assert!(
            outside <= 2.0,
            "{mode}: refraction leaves the scene around the glass alone, {outside}"
        );
    }
}

use rikka_renderer::{Node, Renderer, Scene};
use rikka_renderer_3d::Error;
use rikka_renderer_3d::Three;
use rikka_renderer_3d::glam::{Mat4, Vec3, Vec4};
use rikka_renderer_3d::{
    Background, BuiltinTemplate, Camera, Draw, Figure, Frame, Hint, Literal, MaterialDesc,
    MaterialId, MaterialState, MaterialTemplate, MeshId, Outline, Overrides, ParameterError,
    ParameterKind, ShaderCompiler, TextureKind, ToneMapping, Value, Viewport, ViewportConfig,
};
use rikka_test_renderer::TestRenderer;
use rikka_types::{Color, Rect};

const SIZE: u32 = 32;

const PIPELINE_BUILD: std::time::Duration = std::time::Duration::from_secs(30);

const GLOW: &str = r#"
import rikka;

struct GlowData {
    float glow;
};

struct Surface : ISurface<GlowData> {
    [Color] [Srgb] [Default(0.0, 1.0, 0.0)] [Group("base")] float3 tint;
    [Default(1.0)] [Range(0.0, 4.0)] [SoftRange(0.0, 2.0)] [Group("light")] [Hint("logarithmic")] float glow;
    [Options("flat|banded")] [Default(1)] uint mode;
    [Texture("normal")] RikkaTexture detail;
    [Default(1.0)] bool lit;
    [Default(3)] int bands;

    [constref]
    RikkaSurfaceData<GlowData> evaluate(RikkaPixel pixel, RikkaContext context) {
        GlowData data;
        data.glow = glow;
        RikkaSurfaceData<GlowData> s = rikka_surface_data(pixel, data);
        s.albedo = tint;
        return s;
    }
};

struct Model : IShadingModel {
    [Default(1.0)] float gain;

    typealias Data = GlowData;
    typealias State = float4;

    [constref]
    State prepare(RikkaPixel pixel, RikkaSurfaceData<Data> surface, RikkaContext context) {
        return float4(surface.albedo * surface.model.glow * gain, surface.alpha);
    }

    [constref]
    void direct(inout State state, RikkaLightSample light, inout RikkaShadowQuery shadow, RikkaContext context) {
    }

    [constref]
    void indirect(inout State state, RikkaIndirect indirect, RikkaContext context) {
    }

    [constref]
    float4 resolve(State state, RikkaPixel pixel, RikkaContext context) {
        return float4(state.rgb, rikka_coverage(context, state.a));
    }
};
"#;

const DIMMED: &str = r#"
import rikka;

struct Surface : ISurface<RikkaNoData> {
    [Color] [Default(1.0, 1.0, 1.0)] float3 tint;
    [Default(0.25)] float dim;

    [constref]
    RikkaSurfaceData<RikkaNoData> evaluate(RikkaPixel pixel, RikkaContext context) {
        RikkaNoData data;
        data.unused = 0u;
        RikkaSurfaceData<RikkaNoData> s = rikka_surface_data(pixel, data);
        s.albedo = tint * dim;
        return s;
    }
};

typealias Model = RikkaUnlitModel;
"#;

fn renderer() -> TestRenderer {
    rikka_test_renderer::renderer_3d("rikka-material-test")
        .expect("a Vulkan 1.3 device with the mesh feature set is required for these tests")
}

fn quad(renderer: &mut Renderer) -> MeshId {
    let positions = [
        Vec3::new(-1.0, -1.0, 0.0),
        Vec3::new(1.0, -1.0, 0.0),
        Vec3::new(1.0, 1.0, 0.0),
        Vec3::new(-1.0, 1.0, 0.0),
    ];
    renderer
        .three()
        .create_mesh(&rikka_renderer_3d::MeshData {
            positions: &positions,
            normals: &[Vec3::Z; 4],
            indices: Some(rikka_renderer_3d::Indices::U32(&[0, 1, 2, 0, 2, 3])),
            ..rikka_renderer_3d::MeshData::default()
        })
        .expect("a quad")
}

fn frame() -> Frame {
    let mut frame = Frame::new(Camera::look_at(
        Vec3::new(0.0, 0.0, 4.0),
        Vec3::ZERO,
        Vec3::Y,
        1.0,
        1.0,
        0.05,
    ));
    frame
        .background(Background::Transparent)
        .ambient(Vec3::ONE)
        .tone_mapping(ToneMapping::Clamp)
        .outline(Outline {
            enabled: false,
            ..Outline::default()
        });
    frame
}

fn viewport(renderer: &mut Renderer) -> Viewport {
    renderer
        .three()
        .create_viewport(ViewportConfig::new(SIZE, SIZE).samples(1))
        .expect("a viewport")
}

fn pixels(renderer: &mut Renderer, viewport: &mut Viewport, frame: &Frame) -> Vec<u8> {
    renderer
        .three()
        .render(viewport, frame)
        .expect("a 3D frame draws");
    let mut target = renderer.create_target(SIZE, SIZE).expect("a target");
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

fn at(pixels: &[u8], x: u32, y: u32) -> [u8; 4] {
    let index = ((y * SIZE + x) * 4) as usize;
    [
        pixels[index],
        pixels[index + 1],
        pixels[index + 2],
        pixels[index + 3],
    ]
}

fn centre(renderer: &mut Renderer, material: MaterialId) -> [u8; 4] {
    let mesh = quad(renderer);
    let mut view = viewport(renderer);
    let mut frame = frame();
    frame.draw(Draw::new(mesh, material));
    let pixels = pixels(renderer, &mut view, &frame);
    at(&pixels, SIZE / 2, SIZE / 2)
}

fn compiler() -> ShaderCompiler {
    ShaderCompiler::new().expect("libslang loads")
}

#[test]
fn runtime_surface_and_model_draw() {
    let mut renderer = renderer();
    let template = compiler()
        .template("glow", GLOW)
        .expect("the template compiles against the rikka library");
    let desc = MaterialDesc::new(&template)
        .with("glow", 0.5)
        .and_then(|desc| desc.with("gain", 2.0))
        .expect("glow parameters");
    let material = renderer.three().create_material(desc).expect("a material");
    let [red, green, blue, alpha] = centre(&mut renderer, material);
    assert!(
        green > 240 && red < 10 && blue < 10 && alpha == 255,
        "tint × glow × gain shows full green: {:?}",
        [red, green, blue, alpha]
    );
}

#[test]
fn schema_reflects_declared_parameters() {
    let template = compiler().template("glow", GLOW).expect("a template");
    let schema = template.schema();
    let names: Vec<&str> = schema
        .parameters()
        .iter()
        .map(|parameter| parameter.name.as_str())
        .collect();
    assert_eq!(
        names,
        ["tint", "glow", "mode", "detail", "lit", "bands", "gain"]
    );
    let tint = schema.parameter("tint").expect("tint");
    assert_eq!(
        tint.kind,
        ParameterKind::Color {
            alpha: false,
            srgb: true
        }
    );
    assert_eq!(tint.default, Literal::Vec3([0.0, 1.0, 0.0]));
    assert_eq!(tint.group.as_deref(), Some("base"));
    let glow = schema.parameter("glow").expect("glow");
    assert_eq!(glow.kind, ParameterKind::Float);
    assert_eq!(glow.default, Literal::Float(1.0));
    assert_eq!(
        glow.range.map(|range| (range.low, range.high)),
        Some((0.0, 4.0))
    );
    assert_eq!(
        glow.soft_range.map(|range| (range.low, range.high)),
        Some((0.0, 2.0))
    );
    assert_eq!(glow.hints, [Hint::Logarithmic]);
    let mode = schema.parameter("mode").expect("mode");
    assert_eq!(
        mode.kind,
        ParameterKind::Enum {
            options: vec!["flat".to_owned(), "banded".to_owned()]
        }
    );
    assert_eq!(mode.default, Literal::Enum(1));
    assert_eq!(
        schema.parameter("detail").expect("detail").kind,
        ParameterKind::Texture {
            kind: TextureKind::Normal,
            sampler: true
        }
    );
    assert_eq!(
        schema.parameter("lit").expect("lit").default,
        Literal::Bool(true)
    );
    assert_eq!(
        schema.parameter("bands").expect("bands").default,
        Literal::Int(3)
    );
    let offsets: Vec<usize> = schema
        .parameters()
        .iter()
        .map(|parameter| parameter.offset)
        .collect();
    assert!(
        offsets.windows(2).all(|pair| pair[0] < pair[1]),
        "{offsets:?}"
    );
}

type Expected = fn(&ParameterError) -> bool;

#[test]
fn set_refuses_what_the_schema_does_not_declare() {
    let template = compiler().template("glow", GLOW).expect("a template");
    let mut desc = MaterialDesc::new(&template);
    let cases: [(&str, Value, Expected); 4] = [
        (
            "missing",
            Value::Float(1.0),
            |error| matches!(error, ParameterError::Unknown { name, .. } if name == "missing"),
        ),
        ("tint", Value::Float(1.0), |error| {
            matches!(error, ParameterError::Mismatch { given: "float", .. })
        }),
        (
            "glow",
            Value::Float(5.0),
            |error| matches!(error, ParameterError::OutOfRange { value, .. } if *value == 5.0),
        ),
        (
            "mode",
            Value::Enum("wavy".to_owned()),
            |error| matches!(error, ParameterError::UnknownOption { option, .. } if option == "wavy"),
        ),
    ];
    for (name, value, expected) in cases {
        match desc.set(name, value) {
            Err(Error::Parameter(error)) if expected(&error) => {}
            other => panic!("{name}: unexpected {other:?}"),
        }
    }
    desc.set("mode", Value::Enum("flat".to_owned()))
        .expect("a declared option");
    assert!(matches!(
        MaterialDesc::new(&template).preset("hair"),
        Err(Error::Parameter(ParameterError::UnknownPreset { .. }))
    ));
    let metal = MaterialDesc::new(&MaterialTemplate::builtin(BuiltinTemplate::Standard))
        .preset("brushed_metal")
        .expect("standard has a brushed metal preset");
    assert_eq!(metal.value("base_metalness"), Some(&Value::Float(1.0)));
}

#[test]
fn reload_keeps_values_and_changes_the_image() {
    let mut renderer = renderer();
    let compiler = compiler();
    let template = compiler.template("reloaded", GLOW).expect("a template");
    let desc = MaterialDesc::new(&template)
        .with("tint", Vec3::new(0.0, 0.0, 1.0))
        .expect("a blue tint");
    let material = renderer.three().create_material(desc).expect("a material");
    let mesh = quad(&mut renderer);
    let mut view = viewport(&mut renderer);
    let mut frame = frame();
    frame.draw(Draw::new(mesh, material));
    let before = at(
        &pixels(&mut renderer, &mut view, &frame),
        SIZE / 2,
        SIZE / 2,
    );
    assert!(before[2] > 240, "blue before the reload: {before:?}");

    template
        .reload(&compiler, DIMMED)
        .expect("the new source compiles");
    assert!(template.schema().parameter("dim").is_some());
    assert!(template.schema().parameter("glow").is_none());
    let (ready, built) = std::sync::mpsc::channel();
    template.on_ready(move || ready.send(()).expect("the test waits"));
    let first = at(
        &pixels(&mut renderer, &mut view, &frame),
        SIZE / 2,
        SIZE / 2,
    );
    built
        .recv_timeout(PIPELINE_BUILD)
        .expect("the reloaded template's pipelines are built");
    assert!(template.is_ready());
    let after = at(
        &pixels(&mut renderer, &mut view, &frame),
        SIZE / 2,
        SIZE / 2,
    );
    assert!(
        first == before || first == after,
        "{first:?} is one generation or the other"
    );
    assert!(
        after[0] < 10 && after[1] < 10 && (100..200).contains(&after[2]),
        "the kept blue tint, dimmed by the new default: {after:?}"
    );
    assert!(matches!(
        template.reload(&compiler, "import rikka;\nstruct Surface {};\n"),
        Err(Error::Slang(_))
    ));
}

#[test]
fn figure_overrides_touch_only_their_figure() {
    let mut renderer = renderer();
    let material = renderer
        .three()
        .create_material(
            rikka_test_renderer_3d::standard(Vec4::ONE, MaterialState::opaque())
                .with("specular_weight", 0.0)
                .expect("a specular weight"),
        )
        .expect("a white material");
    let mesh = quad(&mut renderer);
    let mut view = viewport(&mut renderer);
    let mut frame = frame();
    let red = frame.figure(Figure::new(
        Overrides::new().with("base_color", Vec3::new(1.0, 0.0, 0.0)),
    ));
    let skin_only = frame.figure(Figure::new(Overrides::new().with_for(
        BuiltinTemplate::Skin.id(),
        "base_color",
        Vec4::new(0.0, 0.0, 1.0, 1.0),
    )));
    let place = |x: f32| {
        Mat4::from_translation(Vec3::new(x, 0.0, 0.0)) * Mat4::from_scale(Vec3::splat(0.4))
    };
    frame.draw(Draw::new(mesh, material).transform(place(-1.0)).figure(red));
    frame.draw(Draw::new(mesh, material).transform(place(0.0)));
    frame.draw(
        Draw::new(mesh, material)
            .transform(place(1.0))
            .figure(skin_only),
    );
    let pixels = pixels(&mut renderer, &mut view, &frame);
    let left = at(&pixels, SIZE / 2 - 8, SIZE / 2);
    let middle = at(&pixels, SIZE / 2, SIZE / 2);
    let right = at(&pixels, SIZE / 2 + 8, SIZE / 2);
    assert!(
        left[0] > 200 && left[1] < 10 && left[2] < 10,
        "red figure: {left:?}"
    );
    assert!(
        middle.iter().take(3).all(|&channel| channel > 200),
        "plain draw: {middle:?}"
    );
    assert_eq!(
        right, middle,
        "a skin-scoped override leaves the standard material alone"
    );

    let mut wrong = self::frame();
    let bad = wrong.figure(Figure::new(Overrides::new().with("base_color", 1.0)));
    wrong.draw(Draw::new(mesh, material).figure(bad));
    assert!(matches!(
        renderer.three().render(&mut view, &wrong),
        Err(Error::Parameter(ParameterError::Mismatch { .. }))
    ));
}

#[test]
fn compile_failures_are_reported() {
    let broken = compiler().template(
        "broken",
        "import rikka;\nstruct Surface : ISurface<RikkaNoData> { float x; };\ntypealias Model = missing_model;\n",
    );
    let Err(error @ Error::Slang(_)) = broken else {
        panic!("a compile error is reported: {broken:?}");
    };
    let message = error.to_string();
    assert!(
        message.starts_with("failed to compile shader: ") && message.contains("missing_model"),
        "{message}"
    );
}

const SWAYING: &str = r#"
import rikka;

struct Surface : ISurface<RikkaStandardData>, IVertex {
    [Default(1.0)] float sway;
    [Role("displacement")] [Default(1.0)] float reach;

    [constref]
    RikkaSurfaceData<RikkaStandardData> evaluate(RikkaPixel pixel, RikkaContext context) {
        RikkaStandardData data;
        data.metallic = 0.0;
        data.roughness = 1.0;
        RikkaSurfaceData<RikkaStandardData> s = rikka_surface_data(pixel, data);
        s.albedo = float3(1.0, 0.0, 0.0);
        return s;
    }

    [constref]
    void deform(inout RikkaVertex vertex, RikkaContext context, float time) {
        vertex.world_position.x += sway * time;
    }
};

typealias Model = RikkaStandardModel;
"#;

const SHADOWED: usize = 64;

fn luma(pixel: [u8; 4]) -> u32 {
    u32::from(pixel[0]) + u32::from(pixel[1]) + u32::from(pixel[2])
}

fn swayed(times: [f32; 2]) -> [Vec<u8>; 2] {
    let mut renderer = renderer();
    let template = compiler()
        .template("swaying", SWAYING)
        .expect("a vertex hook compiles");
    let caster = renderer
        .three()
        .create_material(MaterialDesc::new(&template))
        .expect("a swaying material");
    let wall = renderer
        .three()
        .create_material(rikka_test_renderer_3d::standard(
            Vec4::ONE,
            MaterialState::opaque(),
        ))
        .expect("a wall");
    let mesh = quad(&mut renderer);
    let mut target = renderer
        .create_target(SHADOWED as u32, SHADOWED as u32)
        .expect("a target");
    times.map(|time| {
        let mut view = renderer
            .three()
            .create_viewport(ViewportConfig::new(SHADOWED as u32, SHADOWED as u32).samples(1))
            .expect("a viewport");
        let mut frame = frame();
        frame.ambient(Vec3::splat(0.2)).time(time).light(
            rikka_renderer_3d::Light::directional(Vec3::new(0.0, -0.5, -1.0))
                .intensity(3.0)
                .shadow(rikka_renderer_3d::ShadowQuality::Medium),
        );
        frame.draw(Draw::new(mesh, wall).transform(
            Mat4::from_translation(Vec3::new(0.0, 0.0, -2.0)) * Mat4::from_scale(Vec3::splat(4.0)),
        ));
        frame.draw(Draw::new(mesh, caster).transform(Mat4::from_scale(Vec3::splat(0.4))));
        renderer
            .three()
            .render(&mut view, &frame)
            .expect("a 3D frame draws");
        let mut scene = Scene::new();
        scene.set_background(Color::rgb(0.0, 0.0, 0.0));
        scene.insert(Node::image(
            Rect::new(0.0, 0.0, SHADOWED as f32, SHADOWED as f32),
            view.texture(),
        ));
        renderer
            .draw(&mut scene, &mut target)
            .expect("a 2D frame draws");
        renderer.read_pixels(&mut target).expect("pixels come back")
    })
}

fn texel(pixels: &[u8], x: usize, y: usize) -> [u8; 4] {
    let index = (y * SHADOWED + x) * 4;
    [
        pixels[index],
        pixels[index + 1],
        pixels[index + 2],
        pixels[index + 3],
    ]
}

#[test]
fn vertex_hook_moves_drawing_shadow_and_depth_together() {
    let [still, moved] = swayed([0.0, 1.0]);
    let centre = SHADOWED / 2;
    let caster_px = SHADOWED as f32 / (2.0 * 4.0 * 0.5f32.tan());
    let wall_px = SHADOWED as f32 / (2.0 * 6.0 * 0.5f32.tan());
    let moved_x = centre + caster_px.round() as usize;
    let half = (0.4 * caster_px) as usize - 2;
    for y in centre - half..=centre + half {
        for x in moved_x - half..=moved_x + half {
            let [red, green, blue, _] = texel(&moved, x, y);
            assert!(
                red > 100 && green < 30 && blue < 30,
                "the displaced caster covers ({x}, {y}) without depth holes: {:?}",
                [red, green, blue]
            );
        }
    }
    let red = |pixel: [u8; 4]| pixel[0] > 100 && pixel[1] < 30;
    assert!(red(texel(&still, centre, centre)) && !red(texel(&moved, centre, centre)));
    let below = centre + wall_px.round() as usize;
    let moved_shadow = centre + wall_px.round() as usize;
    let darker = |pixels: &[u8], x: usize| luma(texel(pixels, x, below));
    assert!(
        darker(&moved, moved_shadow) * 2 < darker(&moved, centre),
        "the moved caster shadows the wall where it moved: {} vs {}",
        darker(&moved, moved_shadow),
        darker(&moved, centre)
    );
    assert!(
        darker(&still, centre) * 2 < darker(&still, moved_shadow),
        "the still caster shadows the wall behind it: {} vs {}",
        darker(&still, centre),
        darker(&still, moved_shadow)
    );
}

#[test]
fn copied_parameters_are_refused() {
    let source = SWAYING.replace(
        "    [constref]\n    RikkaSurfaceData<RikkaStandardData> evaluate",
        "    RikkaSurfaceData<RikkaStandardData> evaluate",
    );
    assert_ne!(source, SWAYING);
    match compiler().template("copied", &source) {
        Err(Error::Invalid(message)) => assert!(message.contains("Surface.evaluate"), "{message}"),
        other => panic!("a by-value method is refused: {other:?}"),
    }
}

#[test]
fn displacement_bound_keeps_moved_draws_from_culling() {
    let mut renderer = renderer();
    let compiler = compiler();
    let template = compiler
        .template("swaying", SWAYING)
        .expect("a vertex hook compiles");
    let mesh = quad(&mut renderer);
    let mut view = viewport(&mut renderer);
    for (reach, shown) in [(0.0, false), (3.5, true), (-1.0, true)] {
        let material = renderer
            .three()
            .create_material(
                MaterialDesc::new(&template)
                    .with("sway", 3.0)
                    .and_then(|desc| desc.with("reach", reach))
                    .expect("sway parameters"),
            )
            .expect("a swaying material");
        let mut frame = frame();
        frame.time(1.0);
        frame.draw(Draw::new(mesh, material).transform(
            Mat4::from_translation(Vec3::new(-3.0, 0.0, 0.0)) * Mat4::from_scale(Vec3::splat(0.4)),
        ));
        let pixel = at(
            &pixels(&mut renderer, &mut view, &frame),
            SIZE / 2,
            SIZE / 2,
        );
        assert_eq!(pixel[0] > 100, shown, "reach {reach}: {pixel:?}");
    }
    let unbounded = SWAYING.replace(
        "    [Role(\"displacement\")] [Default(1.0)] float reach;\n",
        "",
    );
    match compiler.template("unbounded", &unbounded) {
        Err(Error::Invalid(message)) => assert!(message.contains("displacement"), "{message}"),
        other => panic!("a vertex hook without a bound is refused: {other:?}"),
    }
}

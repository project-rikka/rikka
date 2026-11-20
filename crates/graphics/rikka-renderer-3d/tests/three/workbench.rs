use crate::support::{SIZE, ambient_frame, read, renderer};
use rikka_renderer::Renderer;
use rikka_renderer_3d::Three;
use rikka_renderer_3d::glam::{Mat4, Vec2, Vec3, Vec4};
use rikka_renderer_3d::{
    BuiltinTemplate, Draw, Frame, Indices, Light, MaterialDesc, MaterialId, MaterialState,
    MaterialTemplate, MeshData, MeshId, ToneMapping, Value, Viewport, ViewportConfig,
};

const CENTER: usize = SIZE as usize / 2;

fn pixel(pixels: &[u8], x: usize, y: usize) -> [u8; 4] {
    let at = (y * SIZE as usize + x) * 4;
    [pixels[at], pixels[at + 1], pixels[at + 2], pixels[at + 3]]
}

fn viewport(renderer: &mut Renderer) -> Viewport {
    renderer
        .three()
        .create_viewport(ViewportConfig::new(SIZE, SIZE).samples(1))
        .expect("a viewport")
}

fn frame() -> Frame {
    let mut frame = ambient_frame(1.0);
    frame.tone_mapping(ToneMapping::Clamp);
    frame
}

fn material(
    renderer: &mut Renderer,
    builtin: BuiltinTemplate,
    values: Vec<(&str, Value)>,
) -> MaterialId {
    let mut desc = MaterialDesc::new(&MaterialTemplate::builtin(builtin))
        .state(MaterialState::opaque().shadows(false, false));
    for (name, value) in values {
        desc = desc.with(name, value).expect("a template parameter");
    }
    renderer.three().create_material(desc).expect("a material")
}

fn quad(renderer: &mut Renderer, color: [u8; 4]) -> MeshId {
    renderer
        .three()
        .create_mesh(&MeshData {
            positions: &[
                Vec3::new(-0.5, -0.5, 0.0),
                Vec3::new(0.5, -0.5, 0.0),
                Vec3::new(0.5, 0.5, 0.0),
                Vec3::new(-0.5, 0.5, 0.0),
            ],
            normals: &[Vec3::Z; 4],
            uvs: &[Vec2::ZERO; 4],
            colors: &[color; 4],
            indices: Some(Indices::U32(&[0, 1, 2, 0, 2, 3])),
            ..MeshData::default()
        })
        .expect("a quad")
}

fn near(actual: [u8; 4], expected: [u8; 3]) -> bool {
    actual[..3]
        .iter()
        .zip(expected)
        .all(|(&actual, expected)| actual.abs_diff(expected) <= 1)
}

#[test]
fn workbench_lighting_ignores_scene_lights() {
    let mut renderer = renderer();
    let mut view = viewport(&mut renderer);
    let sphere = rikka_test_renderer_3d::sphere(&mut renderer, 0.8, 16).expect("a sphere");
    let studio = material(&mut renderer, BuiltinTemplate::Workbench, Vec::new());

    let mut unlit = frame();
    unlit.draw(Draw::new(sphere, studio));
    let alone = read(&mut renderer, &mut view, &unlit);

    let mut lit = frame();
    lit.draw(Draw::new(sphere, studio));
    lit.light(
        Light::directional(Vec3::new(1.0, -1.0, -1.0))
            .color(Vec3::new(1.0, 0.0, 0.0))
            .intensity(20.0),
    );
    let beside = read(&mut renderer, &mut view, &lit);

    assert_eq!(
        alone, beside,
        "scene lights leave workbench shading unchanged"
    );
    let middle = pixel(&alone, CENTER, CENTER);
    let rim = pixel(&alone, CENTER, CENTER - 25);
    assert!(
        middle[0] > 60,
        "the studio lights the facing surface: {middle:?}"
    );
    assert_ne!(middle, rim, "lighting follows the normal");
}

#[test]
fn workbench_flat_lighting_shows_base_color() {
    let mut renderer = renderer();
    let mut view = viewport(&mut renderer);
    let mesh = quad(&mut renderer, [255; 4]);
    let flat = material(
        &mut renderer,
        BuiltinTemplate::Workbench,
        vec![
            ("lighting", Value::Enum("flat".into())),
            ("base_color", Vec3::new(0.5, 0.25, 0.125).into()),
        ],
    );
    let mut shown = frame();
    shown.draw(Draw::new(mesh, flat));
    let pixels = read(&mut renderer, &mut view, &shown);
    let middle = pixel(&pixels, CENTER, CENTER);
    assert!(near(middle, [188, 137, 99]), "flat base colour: {middle:?}");
}

#[test]
fn unlit_vertex_colors_round_trip_exactly() {
    let mut renderer = renderer();
    let mut view = viewport(&mut renderer);
    let mesh = quad(&mut renderer, [200, 30, 90, 255]);
    let painted = material(
        &mut renderer,
        BuiltinTemplate::Unlit,
        vec![("vertex_color", Value::Enum("srgb".into()))],
    );
    let mut shown = frame();
    shown.draw(Draw::new(mesh, painted).object_id(5));
    let pixels = read(&mut renderer, &mut view, &shown);
    let middle = pixel(&pixels, CENTER, CENTER);
    assert!(near(middle, [200, 30, 90]), "vertex colour: {middle:?}");

    let picked = renderer
        .three()
        .pick(&view, CENTER as i32, CENTER as i32, 1, 1)
        .expect("a pick answers");
    assert_eq!(picked.at(CENTER as i32, CENTER as i32), Some(5));
}

#[test]
fn wireframe_material_follows_skinning() {
    let mut renderer = renderer();
    let mut view = viewport(&mut renderer);
    let mesh = renderer
        .three()
        .create_mesh(&MeshData {
            positions: &[
                Vec3::new(-0.5, -0.5, 0.0),
                Vec3::new(0.5, -0.5, 0.0),
                Vec3::new(0.5, 0.5, 0.0),
                Vec3::new(-0.5, 0.5, 0.0),
            ],
            normals: &[Vec3::Z; 4],
            joints: &[[0, 0, 0, 0]; 4],
            weights: &[Vec4::X; 4],
            indices: Some(Indices::U32(&[0, 1, 2, 0, 2, 3])),
            ..MeshData::default()
        })
        .expect("a skinned quad");
    let wire = renderer
        .three()
        .create_material(
            MaterialDesc::new(&MaterialTemplate::builtin(BuiltinTemplate::Wireframe))
                .state(MaterialState::wireframe()),
        )
        .expect("a wireframe material");
    let lit_columns = |renderer: &mut Renderer, view: &mut Viewport, offset: f32| {
        let mut shown = frame();
        let skin = shown.skin(&[Mat4::from_translation(Vec3::new(offset, 0.0, 0.0))]);
        shown.draw(Draw::new(mesh, wire).skin(skin));
        let pixels = read(renderer, view, &shown);
        (0..SIZE as usize)
            .filter(|&x| pixel(&pixels, x, CENTER)[0] > 128)
            .collect::<Vec<_>>()
    };
    let rest = lit_columns(&mut renderer, &mut view, 0.0);
    let moved = lit_columns(&mut renderer, &mut view, 0.5);
    assert!(!rest.is_empty(), "the wire shows at rest");
    let shift = moved
        .first()
        .zip(rest.first())
        .map(|(moved, rest)| moved - rest);
    assert!(
        shift.is_some_and(|shift| (18..=22).contains(&shift)),
        "the wire follows the bone: {rest:?} to {moved:?}"
    );
}

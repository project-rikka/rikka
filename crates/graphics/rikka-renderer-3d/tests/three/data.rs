use rikka_renderer::{Node, Renderer, Scene};
use rikka_renderer_3d::Error;
use rikka_renderer_3d::Three;
use rikka_renderer_3d::glam::{Mat4, Vec3, Vec4};
use rikka_renderer_3d::{
    Background, Camera, Draw, Frame, Indices, MaterialId, MaterialState, MeshData, MeshId,
    MorphTarget, Viewport, ViewportConfig,
};
use rikka_test_renderer::TestRenderer;
use rikka_types::{Color, Rect};

const SIZE: u32 = 64;

fn renderer() -> TestRenderer {
    rikka_test_renderer::renderer_3d("rikka-three-data-test")
        .expect("a Vulkan 1.3 device with the mesh feature set is required for these tests")
}

fn quad(renderer: &mut Renderer, half: f32, morph: Option<Vec3>) -> MeshId {
    let positions = [
        Vec3::new(-half, -half, 0.0),
        Vec3::new(half, -half, 0.0),
        Vec3::new(half, half, 0.0),
        Vec3::new(-half, half, 0.0),
    ];
    let normals = [Vec3::Z; 4];
    let indices = [0u32, 1, 2, 0, 2, 3];
    let moved = [0u32, 1, 2, 3];
    let shift = [morph.unwrap_or_default(); 4];
    let targets = [MorphTarget {
        vertices: &moved,
        positions: &shift,
        normals: &[],
    }];
    renderer
        .three()
        .create_mesh(&MeshData {
            positions: &positions,
            normals: &normals,
            indices: Some(Indices::U32(&indices)),
            morph_targets: match morph {
                Some(_) => &targets,
                None => &[],
            },
            ..MeshData::default()
        })
        .expect("a quad is a mesh")
}

fn flat(renderer: &mut Renderer) -> MaterialId {
    flat_in(renderer, MaterialState::opaque())
}

fn flat_in(renderer: &mut Renderer, state: MaterialState) -> MaterialId {
    renderer
        .three()
        .create_material(rikka_test_renderer_3d::standard(
            Vec4::new(1.0, 0.0, 0.0, 1.0),
            state,
        ))
        .expect("a material")
}

fn frame() -> Frame {
    let mut frame = Frame::new(Camera::look_at(
        Vec3::new(0.0, 0.0, 3.0),
        Vec3::ZERO,
        Vec3::Y,
        1.0,
        1.0,
        0.05,
    ));
    frame
        .background(Background::Transparent)
        .ambient(Vec3::splat(1.0))
        .outline(rikka_renderer_3d::Outline {
            enabled: false,
            ..Default::default()
        });
    frame
}

fn viewport(renderer: &mut Renderer) -> Viewport {
    renderer
        .three()
        .create_viewport(ViewportConfig::new(SIZE, SIZE))
        .expect("a viewport")
}

fn red_columns(renderer: &mut Renderer, view: &mut Viewport, frame: &Frame) -> Vec<usize> {
    renderer
        .three()
        .render(view, frame)
        .expect("a 3D frame draws");
    let mut target = renderer
        .create_target(SIZE, SIZE)
        .expect("an offscreen target");
    let mut scene = Scene::new();
    scene.set_background(Color::rgb(0.0, 0.0, 0.0));
    scene.insert(Node::image(
        Rect::new(0.0, 0.0, SIZE as f32, SIZE as f32),
        view.texture(),
    ));
    renderer
        .draw(&mut scene, &mut target)
        .expect("a 2D frame draws");
    let pixels = renderer.read_pixels(&mut target).expect("pixels come back");
    let row = SIZE as usize / 2;
    (0..SIZE as usize)
        .filter(|&x| {
            let i = (row * SIZE as usize + x) * 4;
            pixels[i] > 150 && pixels[i + 1] < 80
        })
        .collect()
}

#[test]
fn morph_target_offsets_vertices_by_weight() {
    let mut renderer = renderer();
    let mut view = viewport(&mut renderer);
    let mesh = quad(&mut renderer, 0.4, Some(Vec3::new(1.0, 0.0, 0.0)));
    let material = flat(&mut renderer);

    let mut still = frame();
    let (first, count) = still.morphs(&[0.0]);
    still.draw(Draw::new(mesh, material).morph_weights(first, count));
    let resting = red_columns(&mut renderer, &mut view, &still);

    let mut moved = frame();
    let (first, count) = moved.morphs(&[1.0]);
    moved.draw(Draw::new(mesh, material).morph_weights(first, count));
    let shifted = red_columns(&mut renderer, &mut view, &moved);

    let centre = |columns: &[usize]| columns.iter().sum::<usize>() / columns.len().max(1);
    assert!(!resting.is_empty() && !shifted.is_empty());
    assert!(
        centre(&shifted) > centre(&resting) + 8,
        "resting around {}, moved around {}",
        centre(&resting),
        centre(&shifted)
    );
}

#[test]
fn deformed_mesh_bounds_include_deformation() {
    let mut renderer = renderer();
    let mut view = viewport(&mut renderer);
    let away = Vec3::new(40.0, 0.0, 0.0);
    let half = 0.4;
    let positions = [
        away + Vec3::new(-half, -half, 0.0),
        away + Vec3::new(half, -half, 0.0),
        away + Vec3::new(half, half, 0.0),
        away + Vec3::new(-half, half, 0.0),
    ];
    let normals = [Vec3::Z; 4];
    let indices = [0u32, 1, 2, 0, 2, 3];
    let joints = [[0u16; 4]; 4];
    let weights = [rikka_renderer_3d::glam::Vec4::X; 4];
    let moved = [0u32, 1, 2, 3];
    let back = [-away; 4];
    let targets = [MorphTarget {
        vertices: &moved,
        positions: &back,
        normals: &[],
    }];
    let skinned = renderer
        .three()
        .create_mesh(&MeshData {
            positions: &positions,
            normals: &normals,
            joints: &joints,
            weights: &weights,
            indices: Some(Indices::U32(&indices)),
            ..MeshData::default()
        })
        .expect("a skinned quad");
    let morphed = renderer
        .three()
        .create_mesh(&MeshData {
            positions: &positions,
            normals: &normals,
            indices: Some(Indices::U32(&indices)),
            morph_targets: &targets,
            ..MeshData::default()
        })
        .expect("a morphed quad");

    for (state, pass) in [
        (MaterialState::opaque(), "opaque"),
        (MaterialState::transparent(), "transparent"),
    ] {
        let material = flat_in(&mut renderer, state);

        let mut by_bone = frame();
        let skin = by_bone.skin(&[Mat4::from_translation(-away)]);
        by_bone.draw(Draw::new(skinned, material).skin(skin));
        let boned = red_columns(&mut renderer, &mut view, &by_bone);
        assert!(!boned.is_empty(), "an {pass} quad its bone brings back");

        let mut by_morph = frame();
        let (first, count) = by_morph.morphs(&[1.0]);
        by_morph.draw(Draw::new(morphed, material).morph_weights(first, count));
        let morphing = red_columns(&mut renderer, &mut view, &by_morph);
        assert!(!morphing.is_empty(), "an {pass} quad its morph brings back");
    }
}

#[test]
fn reused_mesh_slot_draws_new_shape() {
    let mut renderer = renderer();
    let mut view = viewport(&mut renderer);
    let material = flat(&mut renderer);
    let wide = quad(&mut renderer, 0.9, None);

    let mut first = frame();
    first.draw(Draw::new(wide, material));
    let before = red_columns(&mut renderer, &mut view, &first);

    assert!(renderer.three().destroy_mesh(wide).expect("meshes"));
    let narrow = quad(&mut renderer, 0.2, None);
    let mut second = frame();
    second.draw(Draw::new(narrow, material));
    let after = red_columns(&mut renderer, &mut view, &second);

    assert!(
        after.len() < before.len() / 2,
        "{} then {}",
        before.len(),
        after.len()
    );
    assert!(!after.is_empty());
}

#[test]
fn rejects_skin_shorter_than_mesh() {
    let mut renderer = renderer();
    let mut view = viewport(&mut renderer);
    let material = flat(&mut renderer);
    let positions = [Vec3::ZERO, Vec3::X, Vec3::Y];
    let joints = [[0u16, 0, 0, 0], [1, 0, 0, 0], [2, 0, 0, 0]];
    let weights = [rikka_renderer_3d::glam::Vec4::X; 3];
    let mesh = renderer
        .three()
        .create_mesh(&MeshData {
            positions: &positions,
            joints: &joints,
            weights: &weights,
            ..MeshData::default()
        })
        .expect("a skinned triangle");

    let mut short = frame();
    let skin = short.skin(&[Mat4::IDENTITY; 2]);
    short.draw(Draw::new(mesh, material).skin(skin));
    assert!(matches!(
        renderer.three().render(&mut view, &short),
        Err(Error::Invalid(_))
    ));

    let mut whole = frame();
    let skin = whole.skin(&[Mat4::IDENTITY; 3]);
    whole.draw(Draw::new(mesh, material).skin(skin));
    renderer
        .three()
        .render(&mut view, &whole)
        .expect("a skin with every bone draws");
}

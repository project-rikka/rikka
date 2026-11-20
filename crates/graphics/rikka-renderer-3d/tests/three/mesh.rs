use crate::support::{SIZE, ambient_frame, flat, quad, read, renderer, viewport};
use rikka_renderer::Renderer;
use rikka_renderer_3d::Three;
use rikka_renderer_3d::glam::{Mat4, Vec3, Vec4};
use rikka_renderer_3d::{
    BuiltinTemplate, Draw, Indices, Light, MaterialDesc, MaterialId, MaterialState,
    MaterialTemplate, MeshData, MeshUpdate, ShadowQuality, ToneMapping, Value, ViewportConfig,
};

fn pixel(pixels: &[u8], width: usize, x: usize, y: usize) -> (u8, u8, u8, u8) {
    let i = (y * width + x) * 4;
    (pixels[i], pixels[i + 1], pixels[i + 2], pixels[i + 3])
}

#[test]
fn quad_rasterizes_at_center() {
    let mut renderer = renderer();
    let mut view = viewport(&mut renderer, 1);
    let mesh = quad(&mut renderer, 0.0, 0.5);
    let material = flat(&mut renderer, [1.0, 0.0, 0.0, 1.0]);

    let mut frame = ambient_frame(1.0);
    frame.draw(Draw::new(mesh, material));
    let pixels = read(&mut renderer, &mut view, &frame);

    let (r, g, b, a) = pixel(&pixels, SIZE as usize, 64, 64);
    assert!(
        r > 200 && g < 60 && b < 60 && a > 200,
        "middle: {r},{g},{b},{a}"
    );
    assert_eq!(
        pixel(&pixels, SIZE as usize, 4, 4),
        (0, 0, 0, 255),
        "corner"
    );
}

#[test]
fn depth_test_keeps_nearest() {
    let mut renderer = renderer();
    let mut view = viewport(&mut renderer, 1);
    let far = quad(&mut renderer, -1.0, 0.9);
    let near = quad(&mut renderer, 1.0, 0.5);
    let blue = flat(&mut renderer, [0.0, 0.0, 1.0, 1.0]);
    let green = flat(&mut renderer, [0.0, 1.0, 0.0, 1.0]);

    let mut frame = ambient_frame(1.0);
    frame.draw(Draw::new(far, blue));
    frame.draw(Draw::new(near, green));
    let pixels = read(&mut renderer, &mut view, &frame);

    let (r, g, b, _) = pixel(&pixels, SIZE as usize, 64, 64);
    assert!(
        g > 180 && b < 60 && r < 60,
        "the near quad wins: {r},{g},{b}"
    );
}

#[test]
fn frustum_culls_outside_object() {
    let mut renderer = renderer();
    let mut view = viewport(&mut renderer, 1);
    let mesh = quad(&mut renderer, 0.0, 0.5);
    let material = flat(&mut renderer, [1.0, 0.0, 0.0, 1.0]);

    let mut frame = ambient_frame(1.0);
    frame.draw(
        Draw::new(mesh, material).transform(Mat4::from_translation(Vec3::new(200.0, 0.0, 0.0))),
    );
    let pixels = read(&mut renderer, &mut view, &frame);

    assert_eq!(pixel(&pixels, SIZE as usize, 64, 64), (0, 0, 0, 255));
}

#[test]
fn light_shades_facing_surface() {
    let mut renderer = renderer();
    let mut view = viewport(&mut renderer, 1);
    let mesh = quad(&mut renderer, 0.0, 0.5);
    let material = flat(&mut renderer, [1.0, 1.0, 1.0, 1.0]);

    let mut lit = ambient_frame(0.0);
    lit.light(
        Light::directional(Vec3::NEG_Z)
            .intensity(4.0)
            .shadow(ShadowQuality::None),
    );
    lit.draw(Draw::new(mesh, material));
    let pixels = read(&mut renderer, &mut view, &lit);
    let (r, _, _, _) = pixel(&pixels, SIZE as usize, 64, 64);
    assert!(r > 60, "a lit surface is not black: {r}");

    let mut unlit = ambient_frame(0.0);
    unlit.light(
        Light::directional(Vec3::Z)
            .intensity(4.0)
            .shadow(ShadowQuality::None),
    );
    unlit.draw(Draw::new(mesh, material));
    let pixels = read(&mut renderer, &mut view, &unlit);
    let (r, g, b, _) = pixel(&pixels, SIZE as usize, 64, 64);
    assert!(
        r < 20 && g < 20 && b < 20,
        "a surface facing away: {r},{g},{b}"
    );
}

#[test]
fn shadow_falls_without_self_shadowing() {
    let mut renderer = renderer();
    let mut view = viewport(&mut renderer, 1);
    let wall = quad(&mut renderer, -2.0, 4.0);
    let blocker = quad(&mut renderer, 0.0, 0.6);
    let white = flat(&mut renderer, [1.0, 1.0, 1.0, 1.0]);

    let build = |with_blocker: bool, shadows: ShadowQuality| {
        let mut frame = ambient_frame(0.0);
        frame.light(
            Light::directional(Vec3::new(0.0, -0.6, -1.0))
                .intensity(4.0)
                .shadow(shadows),
        );
        frame.draw(Draw::new(wall, white));
        if with_blocker {
            frame.draw(Draw::new(blocker, white));
        }
        frame
    };

    let plain = read(&mut renderer, &mut view, &build(false, ShadowQuality::None));
    let alone = read(
        &mut renderer,
        &mut view,
        &build(false, ShadowQuality::Medium),
    );
    let blocked = read(
        &mut renderer,
        &mut view,
        &build(true, ShadowQuality::Medium),
    );

    const IN_SHADOW: usize = 96;
    const BELOW_SHADOW: usize = 120;
    const ABOVE_CASTER: usize = 24;

    let at = |pixels: &[u8], row: usize| pixel(pixels, SIZE as usize, 64, row).0;

    for row in [ABOVE_CASTER, IN_SHADOW, BELOW_SHADOW] {
        assert!(
            at(&plain, row) > 200,
            "row {row} is lit: {}",
            at(&plain, row)
        );
        assert_eq!(
            at(&alone, row),
            at(&plain, row),
            "row {row}: a wall does not shadow itself"
        );
    }

    assert_eq!(at(&blocked, IN_SHADOW), 0, "the band under the caster");
    assert!(
        at(&blocked, BELOW_SHADOW) > 200,
        "past the shadow's reach: {}",
        at(&blocked, BELOW_SHADOW)
    );
    assert!(
        at(&blocked, ABOVE_CASTER) > 200,
        "above the caster, where a mirrored map would put the shadow: {}",
        at(&blocked, ABOVE_CASTER)
    );
}

#[test]
fn pick_returns_frontmost_object() {
    let mut renderer = renderer();
    let mut view = viewport(&mut renderer, 4);
    let far = quad(&mut renderer, -1.0, 0.9);
    let near = quad(&mut renderer, 1.0, 0.5);
    let material = flat(&mut renderer, [1.0, 1.0, 1.0, 1.0]);

    let mut frame = ambient_frame(1.0);
    frame.draw(Draw::new(far, material).object_id(7));
    frame.draw(Draw::new(near, material).object_id(9));
    renderer
        .three()
        .render(&mut view, &frame)
        .expect("a 3D frame draws");

    let middle = renderer
        .three()
        .pick(&view, 64, 64, 1, 1)
        .expect("a pick answers");
    assert_eq!(middle.at(64, 64), Some(9), "the near quad");

    let corner = renderer
        .three()
        .pick(&view, 2, 2, 1, 1)
        .expect("a pick answers");
    assert_eq!(corner.at(2, 2), None, "empty space");

    let outside = renderer
        .three()
        .pick(&view, -4, -4, 1, 1)
        .expect("a pick answers");
    assert_eq!(outside.area().width * outside.area().height, 0, "outside");
    assert_eq!(outside.at(-4, -4), None, "outside");
}

fn glass(renderer: &mut Renderer, color: [f32; 4]) -> MaterialId {
    renderer
        .three()
        .create_material(rikka_test_renderer_3d::standard(
            Vec4::from_array(color),
            MaterialState::transparent(),
        ))
        .expect("a transparent material")
}

#[test]
fn transparent_draw_blends_over_opaque() {
    let mut renderer = renderer();
    let mut view = viewport(&mut renderer, 1);
    let back = quad(&mut renderer, -1.0, 0.9);
    let front = quad(&mut renderer, 1.0, 0.5);
    let red = flat(&mut renderer, [1.0, 0.0, 0.0, 1.0]);
    let glass = glass(&mut renderer, [0.0, 0.0, 0.5, 0.5]);

    let mut frame = ambient_frame(1.0);
    frame.draw(Draw::new(back, red));
    frame.draw(Draw::new(front, glass));
    let pixels = read(&mut renderer, &mut view, &frame);

    let (r, _, b, _) = pixel(&pixels, SIZE as usize, 64, 64);
    assert!(r > 30, "the red behind still shows: {r}");
    assert!(b > 30, "the blue in front is there too: {b}");
}

#[test]
fn transparent_draws_sort_by_order_before_depth() {
    let mut renderer = renderer();
    let mut view = viewport(&mut renderer, 1);
    let near = quad(&mut renderer, 1.0, 0.5);
    let far = quad(&mut renderer, -1.0, 0.5);
    let blue = glass(&mut renderer, [0.0, 0.0, 1.0, 0.5]);
    let red = glass(&mut renderer, [1.0, 0.0, 0.0, 0.5]);

    let mut frame = ambient_frame(1.0);
    frame.draw(Draw::new(far, red).order(1));
    frame.draw(Draw::new(near, blue).order(0));
    let pixels = read(&mut renderer, &mut view, &frame);

    let (r, _, b, _) = pixel(&pixels, SIZE as usize, 64, 64);
    assert!(
        r > b,
        "the later red lies over the nearer blue: {r} against {b}"
    );
}
#[test]
fn destroyed_material_slot_is_reused() {
    let mut renderer = renderer();
    let mut view = viewport(&mut renderer, 1);
    let mesh = quad(&mut renderer, 0.0, 0.3);

    let blue = flat(&mut renderer, [0.0, 0.0, 1.0, 1.0]);
    let mut frame = ambient_frame(1.0);
    frame.draw(Draw::new(mesh, blue));
    let pixels = read(&mut renderer, &mut view, &frame);
    let (r, _, b, _) = pixel(&pixels, SIZE as usize, 64, 64);
    assert!(b > 200 && r < 60, "the quad is blue: {r},{b}");

    assert!(
        renderer
            .three()
            .destroy_material(blue)
            .expect("mesh support"),
        "a material is let go of once"
    );
    assert!(
        !renderer
            .three()
            .destroy_material(blue)
            .expect("mesh support"),
        "and the id that named it names nothing after that"
    );

    let mut stale = ambient_frame(1.0);
    stale.draw(Draw::new(mesh, blue));
    assert!(
        renderer.three().render(&mut view, &stale).is_err(),
        "a draw naming a material that is gone is refused rather than drawn from a stale record"
    );

    let red = flat(&mut renderer, [1.0, 0.0, 0.0, 1.0]);
    let mut again = ambient_frame(1.0);
    again.draw(Draw::new(mesh, red));
    let pixels = read(&mut renderer, &mut view, &again);
    let (r, _, b, _) = pixel(&pixels, SIZE as usize, 64, 64);
    assert!(
        r > 200 && b < 60,
        "the place handed out again holds the new material: {r},{b}"
    );
}

#[test]
fn ten_thousand_draws_in_one_bucket() {
    const SIDE: u32 = 100;
    const WIDE: u32 = 800;

    let mut renderer = renderer();
    let mut view = renderer
        .three()
        .create_viewport(ViewportConfig::new(WIDE, WIDE).samples(1))
        .expect("a viewport");
    let mesh = quad(&mut renderer, 0.0, 0.010);
    let material = flat(&mut renderer, [1.0, 1.0, 1.0, 1.0]);

    let mut frame = ambient_frame(1.0);
    let at = |i: u32| (f64::from(i) - f64::from(SIDE - 1) / 2.0) as f32 * 0.028;
    for y in 0..SIDE {
        for x in 0..SIDE {
            frame.draw(
                Draw::new(mesh, material)
                    .transform(Mat4::from_translation(Vec3::new(at(x), at(y), 0.0)))
                    .object_id(y * SIDE + x),
            );
        }
    }
    renderer
        .three()
        .render(&mut view, &frame)
        .expect("a 3D frame draws");

    let ids = renderer
        .three()
        .pick(&view, 0, 0, WIDE, WIDE)
        .expect("the identity buffer reads back");
    let found: std::collections::HashSet<u32> = ids.ids().flatten().collect();
    assert_eq!(
        found.len() as u32,
        SIDE * SIDE,
        "every draw of the frame is in the picture"
    );
}

#[test]
fn seventeen_directional_lights_cover_frame() {
    let mut renderer = renderer();
    let mut view = viewport(&mut renderer, 1);
    let mesh = quad(&mut renderer, 0.0, 2.0);
    let material = flat(&mut renderer, [1.0, 1.0, 1.0, 1.0]);

    let sunlit = |count: usize| {
        let mut frame = ambient_frame(0.0);
        for _ in 0..count {
            frame.light(
                Light::directional(Vec3::NEG_Z)
                    .intensity(0.1)
                    .shadow(ShadowQuality::None),
            );
        }
        frame.draw(Draw::new(mesh, material));
        frame
    };

    let one = read(&mut renderer, &mut view, &sunlit(1));
    let many = read(&mut renderer, &mut view, &sunlit(17));

    let sampled = (8..SIZE as usize - 8).step_by(8);
    let mut dimmest = u8::MAX;
    let mut brightest = 0;
    for y in sampled.clone() {
        for x in sampled.clone() {
            let (lit, _, _, _) = pixel(&many, SIZE as usize, x, y);
            let (single, _, _, _) = pixel(&one, SIZE as usize, x, y);
            assert!(
                lit > single,
                "seventeen lights are brighter than one at {x},{y}: {lit} against {single}"
            );
            dimmest = dimmest.min(lit);
            brightest = brightest.max(lit);
        }
    }
    assert!(
        u32::from(brightest - dimmest) * 20 < u32::from(brightest),
        "every part of the surface has all seventeen: {dimmest} to {brightest}"
    );
}

fn corners(offset: Vec3) -> [Vec3; 4] {
    [
        Vec3::new(-0.5, -0.5, 0.0) + offset,
        Vec3::new(0.5, -0.5, 0.0) + offset,
        Vec3::new(0.5, 0.5, 0.0) + offset,
        Vec3::new(-0.5, 0.5, 0.0) + offset,
    ]
}

#[test]
fn mesh_update_moves_vertices_bounds_and_colors() {
    let mut renderer = renderer();
    let mut view = viewport(&mut renderer, 1);
    let away = Vec3::new(200.0, 0.0, 0.0);
    let mesh = renderer
        .three()
        .create_mesh(&MeshData {
            positions: &corners(away),
            normals: &[Vec3::Z; 4],
            colors: &[[200, 30, 90, 255]; 4],
            indices: Some(Indices::U32(&[0, 1, 2, 0, 2, 3])),
            ..MeshData::default()
        })
        .expect("a quad");
    let painted = renderer
        .three()
        .create_material(
            MaterialDesc::new(&MaterialTemplate::builtin(BuiltinTemplate::Unlit))
                .with("vertex_color", Value::Enum("srgb".into()))
                .expect("the unlit template reads vertex colours"),
        )
        .expect("a material");
    let middle = |renderer: &mut Renderer, view: &mut rikka_renderer_3d::Viewport| {
        let mut frame = ambient_frame(1.0);
        frame.tone_mapping(ToneMapping::Clamp);
        frame.draw(Draw::new(mesh, painted));
        let pixels = read(renderer, view, &frame);
        pixel(&pixels, SIZE as usize, 64, 64)
    };
    assert_eq!(middle(&mut renderer, &mut view), (0, 0, 0, 255), "far away");

    renderer
        .three()
        .update_mesh(
            mesh,
            &MeshUpdate {
                positions: &corners(Vec3::ZERO),
                ..MeshUpdate::default()
            },
        )
        .expect("a position update");
    let (r, g, b, _) = middle(&mut renderer, &mut view);
    assert!(
        r.abs_diff(200) <= 1 && g.abs_diff(30) <= 1 && b.abs_diff(90) <= 1,
        "moved into view and no longer culled by its old bounds: {r},{g},{b}"
    );

    renderer
        .three()
        .update_mesh(
            mesh,
            &MeshUpdate {
                first: 0,
                colors: &[[10, 220, 140, 255]; 4],
                ..MeshUpdate::default()
            },
        )
        .expect("a colour update");
    let (r, g, b, _) = middle(&mut renderer, &mut view);
    assert!(
        r.abs_diff(10) <= 1 && g.abs_diff(220) <= 1 && b.abs_diff(140) <= 1,
        "recoloured in place: {r},{g},{b}"
    );

    renderer
        .three()
        .replace_mesh_indices(mesh, Indices::U16(&[0, 1, 2]), &[])
        .expect("new topology");
    let mut frame = ambient_frame(1.0);
    frame.draw(Draw::new(mesh, painted));
    let pixels = read(&mut renderer, &mut view, &frame);
    assert_ne!(
        pixel(&pixels, SIZE as usize, 80, 72),
        (0, 0, 0, 255),
        "kept triangle"
    );
    assert_eq!(
        pixel(&pixels, SIZE as usize, 48, 56),
        (0, 0, 0, 255),
        "dropped triangle"
    );
}

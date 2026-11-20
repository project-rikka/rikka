use crate::support::{SIZE, ambient_frame, read, renderer};
use rikka_renderer::Renderer;
use rikka_renderer_3d::Three;
use rikka_renderer_3d::glam::{Vec3, Vec4};
use rikka_renderer_3d::{
    Antialiasing, Dash, Draw, Error, Frame, MaterialId, MaterialState, MeshId, OverlayBatch,
    OverlayData, OverlayDepth, OverlayLines, OverlayPoints, OverlaySmoothing, OverlayTriangles,
    OverlayUpdate, PointShape, Resource, Upscale, Viewport, ViewportConfig,
};

const RED: [u8; 4] = [255, 0, 0, 255];
const GREEN: [u8; 4] = [0, 255, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];
const CENTER: usize = SIZE as usize / 2;

fn pixel(pixels: &[u8], x: usize, y: usize) -> [u8; 4] {
    let at = (y * SIZE as usize + x) * 4;
    [pixels[at], pixels[at + 1], pixels[at + 2], pixels[at + 3]]
}

fn viewport(renderer: &mut Renderer, antialiasing: Antialiasing) -> Viewport {
    renderer
        .three()
        .create_viewport(
            ViewportConfig::new(SIZE, SIZE)
                .samples(1)
                .antialiasing(antialiasing),
        )
        .expect("a viewport")
}

fn sphere(renderer: &mut Renderer) -> (MeshId, MaterialId) {
    let mesh = rikka_test_renderer_3d::sphere(renderer, 0.6, 12).expect("a sphere");
    let material = renderer
        .three()
        .create_material(rikka_test_renderer_3d::standard(
            Vec4::new(0.0, 0.0, 1.0, 1.0),
            MaterialState::opaque(),
        ))
        .expect("a material");
    (mesh, material)
}

fn line(frame: &mut Frame, from: Vec3, to: Vec3, width: f32, depth: OverlayDepth) {
    let source = frame.overlay_data(&OverlayData {
        positions: &[from, to],
        colors: &[RED, RED],
        indices: &[],
    });
    frame.overlay(OverlayBatch::new(source, OverlayLines::new(width)).depth(depth));
}

fn behind_line(frame: &mut Frame, depth: OverlayDepth) {
    line(
        frame,
        Vec3::new(-1.5, 0.0, -1.0),
        Vec3::new(1.5, 0.0, -1.0),
        3.0,
        depth,
    );
}

#[test]
fn primitives_draw_their_colors_and_pixel_sizes() {
    let mut renderer = renderer();
    let mut view = viewport(&mut renderer, Antialiasing::Multisample);
    let mut frame = ambient_frame(1.0);
    line(
        &mut frame,
        Vec3::new(-1.0, 0.3, 0.0),
        Vec3::new(1.0, 0.3, 0.0),
        3.0,
        OverlayDepth::Always,
    );
    let dashed = frame.overlay_data(&OverlayData {
        positions: &[Vec3::new(-1.0, -0.3, 0.0), Vec3::new(1.0, -0.3, 0.0)],
        colors: &[RED, RED],
        indices: &[],
    });
    frame.overlay(
        OverlayBatch::new(dashed, OverlayLines::new(1.0).dash(Dash::new(4.0, 4.0)))
            .depth(OverlayDepth::Always),
    );
    let point = frame.overlay_data(&OverlayData {
        positions: &[Vec3::ZERO],
        colors: &[GREEN],
        indices: &[],
    });
    frame.overlay(
        OverlayBatch::new(point, OverlayPoints::new(4.0).shape(PointShape::Square))
            .depth(OverlayDepth::Always),
    );
    let triangle = frame.overlay_data(&OverlayData {
        positions: &[
            Vec3::new(0.4, 0.6, 0.0),
            Vec3::new(0.9, 0.6, 0.0),
            Vec3::new(0.4, 0.9, 0.0),
        ],
        colors: &[BLUE, BLUE, BLUE],
        indices: &[0, 2, 1],
    });
    frame.overlay(OverlayBatch::new(triangle, OverlayTriangles::new()).depth(OverlayDepth::Always));
    let pixels = read(&mut renderer, &mut view, &frame);

    let solid = (0..SIZE as usize)
        .filter(|&y| pixel(&pixels, 40, y) == RED)
        .count();
    assert!(
        (2..=3).contains(&solid),
        "solid rows of a 3 px line: {solid}"
    );
    let covered = (0..CENTER - 4)
        .map(|y| f32::from(pixel(&pixels, 40, y)[0]) / 255.0)
        .sum::<f32>();
    assert!(
        (covered - 3.0).abs() < 0.15,
        "a 3 px line covers {covered} px"
    );

    let row = (CENTER..SIZE as usize)
        .find(|&y| pixel(&pixels, 30, y)[0] > 0 || pixel(&pixels, 34, y)[0] > 0)
        .expect("the dashed line shows");
    let lit = (8..120)
        .filter(|&x| pixel(&pixels, x, row)[0] > 128)
        .count();
    assert!(
        (34..=44).contains(&lit),
        "half of a 4 on 4 off dash shows: {lit}"
    );

    for (x, y) in [(62, 62), (65, 62), (62, 65), (65, 65)] {
        assert_eq!(pixel(&pixels, x, y), GREEN, "point pixel {x},{y}");
    }
    for (x, y) in [(61, 63), (66, 63), (63, 61), (63, 66)] {
        assert_eq!(
            pixel(&pixels, x, y),
            [0, 0, 0, 255],
            "outside the point {x},{y}"
        );
    }
    assert_eq!(pixel(&pixels, 83, 37), BLUE, "inside the triangle");
}

#[test]
fn aliased_overlays_cover_whole_pixels() {
    let mut renderer = renderer();
    let mut view = viewport(&mut renderer, Antialiasing::Multisample);
    let mut partial = Vec::new();
    for smoothing in [OverlaySmoothing::Smooth, OverlaySmoothing::Aliased] {
        let mut frame = ambient_frame(1.0);
        let slanted = frame.overlay_data(&OverlayData {
            positions: &[Vec3::new(-0.9, -0.2, 0.0), Vec3::new(0.9, 0.5, 0.0)],
            colors: &[RED, RED],
            indices: &[],
        });
        frame.overlay(
            OverlayBatch::new(slanted, OverlayLines::new(1.5).smoothing(smoothing))
                .depth(OverlayDepth::Always),
        );
        let dot = frame.overlay_data(&OverlayData {
            positions: &[Vec3::new(0.3, -0.6, 0.0)],
            colors: &[GREEN],
            indices: &[],
        });
        frame.overlay(
            OverlayBatch::new(dot, OverlayPoints::new(9.0).smoothing(smoothing))
                .depth(OverlayDepth::Always),
        );
        let pixels = read(&mut renderer, &mut view, &frame);
        let fractional = |channel: usize| {
            pixels
                .as_chunks::<4>()
                .0
                .iter()
                .filter(|pixel| pixel[channel] != 0 && pixel[channel] != 255)
                .count()
        };
        let full = |channel: usize| {
            pixels
                .as_chunks::<4>()
                .0
                .iter()
                .filter(|pixel| pixel[channel] == 255)
                .count()
        };
        assert!(full(0) > 20, "{smoothing:?} line shows: {}", full(0));
        assert!(full(1) > 20, "{smoothing:?} point shows: {}", full(1));
        partial.push((fractional(0), fractional(1)));
    }
    let (smooth, aliased) = (partial[0], partial[1]);
    assert!(
        smooth.0 > 20 && smooth.1 > 8,
        "smooth edges blend: {smooth:?}"
    );
    assert_eq!(aliased, (0, 0), "aliased edges are binary");
}

#[test]
fn depth_modes_hide_fade_or_show_behind_sphere() {
    let mut renderer = renderer();
    let mut view = viewport(&mut renderer, Antialiasing::Multisample);
    let (mesh, material) = sphere(&mut renderer);
    let mut plain = ambient_frame(1.0);
    plain.draw(Draw::new(mesh, material));
    let surface = pixel(&read(&mut renderer, &mut view, &plain), CENTER, 64);
    let mut seen = Vec::new();
    for depth in [
        OverlayDepth::Tested,
        OverlayDepth::Faded(0.3),
        OverlayDepth::Always,
    ] {
        let mut frame = ambient_frame(1.0);
        frame.draw(Draw::new(mesh, material));
        behind_line(&mut frame, depth);
        let pixels = read(&mut renderer, &mut view, &frame);
        assert_eq!(pixel(&pixels, 24, 64), RED, "{depth:?} beside the sphere");
        seen.push(pixel(&pixels, CENTER, 64));
    }
    assert_eq!(seen[0], surface, "tested behind the sphere");
    let faded = 0.3 * 255.0 + 0.7 * f32::from(surface[0]);
    assert!(
        (f32::from(seen[1][0]) - faded).abs() <= 2.0,
        "faded behind the sphere: {:?} over {surface:?}",
        seen[1]
    );
    assert_eq!(seen[2], RED, "always in front");
}

#[test]
fn surface_edges_draw_over_their_own_faces() {
    let mut renderer = renderer();
    let mut view = viewport(&mut renderer, Antialiasing::Multisample);
    let (mesh, material) = sphere(&mut renderer);
    let rings = 12u32;
    let segments = rings * 2;
    let polar = std::f32::consts::PI * (rings / 2) as f32 / rings as f32;
    let equator: Vec<Vec3> = (0..=segments)
        .map(|segment| {
            let azimuth = std::f32::consts::TAU * segment as f32 / segments as f32;
            Vec3::new(
                polar.sin() * azimuth.cos(),
                polar.cos(),
                polar.sin() * azimuth.sin(),
            ) * 0.6
        })
        .collect();
    let positions: Vec<Vec3> = equator
        .windows(2)
        .flat_map(|pair| [pair[0], pair[1]])
        .collect();
    let colors = vec![RED; positions.len()];
    let mut frame = ambient_frame(1.0);
    frame.draw(Draw::new(mesh, material));
    let edges = frame.overlay_data(&OverlayData {
        positions: &positions,
        colors: &colors,
        indices: &[],
    });
    frame.overlay(OverlayBatch::new(edges, OverlayLines::new(2.0)));
    let pixels = read(&mut renderer, &mut view, &frame);
    for x in [50, 57, 64, 71, 78] {
        for y in [63, 64] {
            assert_eq!(pixel(&pixels, x, y), RED, "edge pixel {x},{y}");
        }
    }
}

#[test]
fn overlays_hold_still_under_temporal_antialiasing() {
    let mut renderer = renderer();
    for antialiasing in [
        Antialiasing::Temporal,
        Antialiasing::Upscale(Upscale::fixed(0.5)),
    ] {
        let mut view = viewport(&mut renderer, antialiasing);
        let (mesh, material) = sphere(&mut renderer);
        let mut frame = ambient_frame(1.0);
        frame.draw(Draw::new(mesh, material));
        behind_line(&mut frame, OverlayDepth::Tested);
        line(
            &mut frame,
            Vec3::new(-0.3, -0.9, 0.7),
            Vec3::new(0.3, 0.9, 0.7),
            2.0,
            OverlayDepth::Tested,
        );
        let masks: Vec<Vec<bool>> = (0..3)
            .map(|_| {
                let pixels = read(&mut renderer, &mut view, &frame);
                pixels
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|texel| *texel == RED)
                    .collect()
            })
            .collect();
        let shown = masks[2].iter().filter(|&&red| red).count();
        assert!(shown > 100, "{antialiasing:?}: {shown} line pixels");
        assert_eq!(
            masks[1], masks[2],
            "{antialiasing:?}: consecutive frames match"
        );
        let hidden = (44..56)
            .chain(72..84)
            .filter(|&x| masks[2][64 * SIZE as usize + x])
            .count();
        assert!(
            hidden < 4,
            "{antialiasing:?}: {hidden} pixels show through the sphere"
        );
    }
}

#[test]
fn overlay_buffers_update_sub_ranges() {
    let mut renderer = renderer();
    let mut view = viewport(&mut renderer, Antialiasing::Multisample);
    let buffer = renderer
        .three()
        .create_overlay_buffer(&OverlayData {
            positions: &[
                Vec3::new(-0.5, 0.3, 0.0),
                Vec3::new(0.5, 0.3, 0.0),
                Vec3::new(-0.5, -0.3, 0.0),
                Vec3::new(0.5, -0.3, 0.0),
            ],
            colors: &[RED; 4],
            indices: &[],
        })
        .expect("an overlay buffer");
    let draw = |renderer: &mut Renderer, view: &mut Viewport| {
        let mut frame = ambient_frame(1.0);
        frame.overlay(OverlayBatch::new(buffer, OverlayLines::new(3.0)));
        read(renderer, view, &frame)
    };
    let before = draw(&mut renderer, &mut view);
    assert_eq!(pixel(&before, CENTER, 52), RED, "upper line");
    assert_eq!(pixel(&before, CENTER, 75), RED, "lower line");

    renderer
        .three()
        .update_overlay_buffer(
            buffer,
            &OverlayUpdate {
                first: 2,
                colors: &[GREEN; 2],
                ..OverlayUpdate::default()
            },
        )
        .expect("a colour update");
    renderer
        .three()
        .update_overlay_buffer(
            buffer,
            &OverlayUpdate {
                first: 0,
                positions: &[Vec3::new(-0.5, 0.0, 0.0), Vec3::new(0.5, 0.0, 0.0)],
                ..OverlayUpdate::default()
            },
        )
        .expect("a position update");
    let after = draw(&mut renderer, &mut view);
    assert_eq!(pixel(&after, CENTER, 52), [0, 0, 0, 255], "moved away");
    assert_eq!(pixel(&after, CENTER, 64), RED, "moved here");
    assert_eq!(pixel(&after, CENTER, 75), GREEN, "recoloured");

    assert!(
        renderer
            .three()
            .destroy_overlay_buffer(buffer)
            .expect("destroys")
    );
    let mut stale = ambient_frame(1.0);
    stale.overlay(OverlayBatch::new(buffer, OverlayLines::new(1.0)));
    let error = renderer
        .three()
        .render(&mut view, &stale)
        .expect_err("a destroyed buffer refuses");
    assert!(
        matches!(error, Error::Unknown(Resource::OverlayBuffer(id)) if id == buffer),
        "{error}"
    );
}

#[test]
fn overlays_draw_over_look_frames() {
    let mut renderer = renderer();
    let mut view = viewport(&mut renderer, Antialiasing::Temporal);
    let (mesh, material) = sphere(&mut renderer);
    let mut frame = ambient_frame(1.0);
    frame.look(rikka_test_renderer_3d::minimal_look());
    frame.draw(Draw::new(mesh, material));
    behind_line(&mut frame, OverlayDepth::Tested);
    let pixels = read(&mut renderer, &mut view, &frame);
    assert_eq!(pixel(&pixels, 24, 64), RED, "beside the sphere");
    assert_ne!(pixel(&pixels, CENTER, 64), RED, "behind the sphere");
}

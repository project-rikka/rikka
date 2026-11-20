use crate::support::{SIZE, ambient_frame, flat, quad, read, renderer};
use rikka_renderer::Renderer;
use rikka_renderer_3d::Three;
use rikka_renderer_3d::glam::{Mat4, Vec3, Vec4};
use rikka_renderer_3d::{
    ClipBounds, Draw, EditorGrid, Error, GRID_STEPS, GridColors, GridPlane, Indices, MeshData,
    MeshId, OutlineSource, OverlayBatch, OverlayData, OverlayDepth, OverlayLines, SelectionOutline,
    ToneMapping, Viewport, ViewportConfig,
};

const RED: [u8; 4] = [255, 0, 0, 255];
const GREEN: [u8; 4] = [0, 255, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];
const CENTER: usize = SIZE as usize / 2;
const PASS: Vec4 = Vec4::new(0.0, 0.0, 0.0, 1.0);

fn pixel(pixels: &[u8], x: usize, y: usize) -> [u8; 4] {
    let at = (y * SIZE as usize + x) * 4;
    [pixels[at], pixels[at + 1], pixels[at + 2], pixels[at + 3]]
}

fn count(pixels: &[u8], matches: impl Fn([u8; 4]) -> bool) -> usize {
    pixels
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|texel| matches(**texel))
        .count()
}

fn viewport(renderer: &mut Renderer) -> Viewport {
    renderer
        .three()
        .create_viewport(ViewportConfig::new(SIZE, SIZE).samples(4))
        .expect("a viewport")
}

#[test]
fn outlined_draws_keep_their_own_colors() {
    let mut renderer = renderer();
    let mut view = viewport(&mut renderer);
    let mesh = quad(&mut renderer, 0.0, 0.4);
    let material = flat(&mut renderer, [0.2, 0.2, 0.2, 1.0]);
    let mut frame = ambient_frame(1.0);
    frame
        .selection_outline(SelectionOutline::default())
        .draw(
            Draw::new(mesh, material)
                .transform(Mat4::from_translation(Vec3::new(-0.7, 0.0, 0.0)))
                .object_id(1)
                .outlined(RED),
        )
        .draw(
            Draw::new(mesh, material)
                .transform(Mat4::from_translation(Vec3::new(0.7, 0.0, 0.0)))
                .object_id(2)
                .outlined(GREEN),
        );
    let pixels = read(&mut renderer, &mut view, &frame);
    let reddish = |texel: [u8; 4]| texel[0] > 160 && texel[1] < 80 && texel[2] < 80;
    let greenish = |texel: [u8; 4]| texel[1] > 160 && texel[0] < 80 && texel[2] < 80;
    let red_left: usize = (0..SIZE as usize)
        .map(|y| {
            (0..CENTER)
                .filter(|&x| reddish(pixel(&pixels, x, y)))
                .count()
        })
        .sum();
    assert!(
        red_left > 40,
        "the left draw is outlined in red: {red_left}"
    );
    assert!(
        count(&pixels, greenish) > 40,
        "the right draw is outlined in green"
    );
    let inside = pixel(&pixels, CENTER / 2 - 4, CENTER);
    assert!(
        !reddish(inside),
        "the outline stays on the silhouette: {inside:?}"
    );
}

#[test]
fn clip_planes_cut_meshes_and_overlays() {
    let mut renderer = renderer();
    let mut view = viewport(&mut renderer);
    let mesh = quad(&mut renderer, 0.0, 0.8);
    let material = flat(&mut renderer, [1.0, 1.0, 1.0, 1.0]);
    let mut frame = ambient_frame(1.0);
    frame
        .clip_planes([Vec4::new(-1.0, 0.0, 0.0, 0.0), PASS, PASS, PASS, PASS, PASS])
        .draw(Draw::new(mesh, material));
    let line = frame.overlay_data(&OverlayData {
        positions: &[Vec3::new(-1.2, 1.0, 0.0), Vec3::new(1.2, 1.0, 0.0)],
        colors: &[BLUE, BLUE],
        indices: &[],
    });
    frame.overlay(OverlayBatch::new(line, OverlayLines::new(3.0)).depth(OverlayDepth::Always));
    let pixels = read(&mut renderer, &mut view, &frame);
    let lit = |texel: [u8; 4]| texel[0] > 100;
    assert!(
        lit(pixel(&pixels, CENTER - 20, CENTER)),
        "the kept half draws"
    );
    assert!(
        !lit(pixel(&pixels, CENTER + 20, CENTER)),
        "the clipped half is gone"
    );
    let line_row = (0..SIZE as usize)
        .max_by_key(|&y| pixel(&pixels, CENTER - 30, y)[2])
        .expect("a row");
    assert!(
        pixel(&pixels, CENTER - 30, line_row)[2] > 150,
        "the kept overlay draws"
    );
    assert!(
        pixel(&pixels, CENTER + 30, line_row)[2] < 40,
        "the clipped overlay is gone"
    );
}

fn stacked_quads(renderer: &mut Renderer) -> MeshId {
    let corners = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)];
    let mut positions = Vec::new();
    for (half, z) in [(0.25, 0.3), (0.7, -0.3)] {
        for (x, y) in corners {
            positions.push(Vec3::new(x * half, y * half, z));
        }
    }
    let normals = vec![Vec3::Z; positions.len()];
    let indices = [0u32, 1, 2, 0, 2, 3, 4, 5, 6, 4, 6, 7];
    let mesh = renderer
        .three()
        .create_mesh(&MeshData {
            positions: &positions,
            normals: &normals,
            indices: Some(Indices::U32(&indices)),
            ..MeshData::default()
        })
        .expect("stacked quads are a mesh");
    renderer
        .three()
        .set_mesh_edges(mesh, 0, &line_adjacency(&indices))
        .expect("edges upload");
    mesh
}

fn line_adjacency(indices: &[u32]) -> Vec<[u32; 4]> {
    let mut open: Vec<([u32; 2], u32)> = Vec::new();
    let mut quads = Vec::new();
    for triangle in indices.as_chunks::<3>().0 {
        for corner in 0..3 {
            let from = triangle[corner];
            let to = triangle[(corner + 1) % 3];
            let opposite = triangle[(corner + 2) % 3];
            match open.iter().position(|(edge, _)| *edge == [to, from]) {
                Some(at) => {
                    let (_, other) = open.remove(at);
                    quads.push([opposite, from, to, other]);
                }
                None => open.push(([from, to], opposite)),
            }
        }
    }
    quads.extend(
        open.into_iter()
            .map(|([from, to], opposite)| [opposite, from, to, opposite]),
    );
    quads
}

fn outline_runs(pixels: &[u8], reddish: impl Fn([u8; 4]) -> bool) -> usize {
    let mut runs = 0;
    let mut inside = false;
    for x in CENTER..SIZE as usize {
        let hit = reddish(pixel(pixels, x, CENTER));
        if hit && !inside {
            runs += 1;
        }
        inside = hit;
    }
    runs
}

#[test]
fn edge_outlines_follow_every_silhouette_wire() {
    let mut renderer = renderer();
    let mut view = viewport(&mut renderer);
    let mesh = stacked_quads(&mut renderer);
    let material = flat(&mut renderer, [0.2, 0.2, 0.2, 1.0]);
    let reddish = |texel: [u8; 4]| texel[0] > 120 && texel[1] < 90 && texel[2] < 90;
    let mut runs = Vec::new();
    for source in [OutlineSource::Surface, OutlineSource::Edges] {
        let mut frame = ambient_frame(1.0);
        frame.selection_outline(SelectionOutline::default()).draw(
            Draw::new(mesh, material)
                .object_id(1)
                .outlined(RED)
                .outline_source(source),
        );
        let pixels = read(&mut renderer, &mut view, &frame);
        runs.push(outline_runs(&pixels, reddish));
    }
    assert_eq!(runs[0], 1, "the surface outline rings the union only");
    assert_eq!(runs[1], 2, "edge outlines also ring the front quad");
}

fn box_corners(half: f32, near: f32, far: f32) -> [Vec3; 8] {
    let corners = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)];
    let mut out = [Vec3::ZERO; 8];
    for (index, (x, y)) in corners.into_iter().enumerate() {
        out[index] = Vec3::new(x * half, y * half, near);
        out[index + 4] = Vec3::new(x * half, y * half, far);
    }
    out
}

#[test]
fn clip_bounds_tint_the_background_inside_the_box_only() {
    let mut renderer = renderer();
    let mut view = viewport(&mut renderer);
    let mesh = quad(&mut renderer, 0.0, 0.15);
    let material = flat(&mut renderer, [0.0, 0.0, 1.0, 1.0]);
    let gray = Vec3::splat(0.25);
    let mut seen = Vec::new();
    for corners in [box_corners(0.5, 1.0, -1.0), box_corners(6.0, 5.0, -5.0)] {
        let mut frame = ambient_frame(1.0);
        frame
            .tone_mapping(ToneMapping::Clamp)
            .clip_bounds(ClipBounds {
                corners,
                color: gray,
            })
            .draw(Draw::new(mesh, material));
        seen.push(read(&mut renderer, &mut view, &frame));
    }
    let tinted = pixel(&seen[0], CENTER, CENTER - 20);
    assert!(
        tinted[0] > 100 && tinted[0] == tinted[1] && tinted[1] == tinted[2],
        "the clip box background is tinted: {tinted:?}"
    );
    let covered = pixel(&seen[0], CENTER, CENTER);
    assert!(
        covered[2] > 100 && covered[0] < 60,
        "geometry stays in front of the tint: {covered:?}"
    );
    assert_eq!(pixel(&seen[0], 2, 2), [0, 0, 0, 255], "outside the box");
    assert_eq!(
        pixel(&seen[1], CENTER, CENTER - 20),
        [0, 0, 0, 255],
        "a box around the eye shows only back faces"
    );
}

#[test]
fn edge_outlines_need_uploaded_edges() {
    let mut renderer = renderer();
    let mut view = viewport(&mut renderer);
    let mesh = quad(&mut renderer, 0.0, 0.4);
    let material = flat(&mut renderer, [0.2, 0.2, 0.2, 1.0]);
    let mut frame = ambient_frame(1.0);
    frame.selection_outline(SelectionOutline::default()).draw(
        Draw::new(mesh, material)
            .outlined(RED)
            .outline_source(OutlineSource::Edges),
    );
    assert!(matches!(
        renderer.three().render(&mut view, &frame),
        Err(Error::Invalid(_))
    ));
}

fn front_grid() -> EditorGrid {
    let mut steps = [0.0; GRID_STEPS];
    for (index, step) in steps.iter_mut().enumerate() {
        *step = 10f32.powi(index as i32);
    }
    EditorGrid {
        plane: GridPlane::Front,
        show_plane: true,
        axes: [true, true, false],
        aligned: false,
        steps,
        distance: 3.0,
        clip_end: 100.0,
        line_width: 1.0,
        colors: GridColors {
            line: [255, 255, 255, 128],
            emphasis: [255, 255, 255, 255],
            axes: [RED, GREEN, BLUE],
        },
    }
}

#[test]
fn editor_grid_draws_axes_in_their_colors() {
    let mut renderer = renderer();
    let mut view = viewport(&mut renderer);
    let mut frame = ambient_frame(1.0);
    frame.editor_grid(front_grid());
    let pixels = read(&mut renderer, &mut view, &frame);
    let row = (CENTER - 2..CENTER + 2)
        .max_by_key(|&y| pixel(&pixels, CENTER / 2, y)[0])
        .expect("a row");
    let axis = pixel(&pixels, CENTER / 2, row);
    assert!(
        axis[0] > 80 && axis[1] < axis[0] / 2,
        "the X axis is red: {axis:?}"
    );
    let column = (CENTER - 2..CENTER + 2)
        .max_by_key(|&x| pixel(&pixels, x, CENTER / 2)[1])
        .expect("a column");
    let axis = pixel(&pixels, column, CENTER / 2);
    assert!(
        axis[1] > 80 && axis[0] < axis[1] / 2,
        "the Y axis is green: {axis:?}"
    );
    let lines = count(&pixels, |texel| {
        texel[0] > 20 && texel[0].abs_diff(texel[1]) < 8 && texel[1].abs_diff(texel[2]) < 8
    });
    assert!(lines > 200, "grid lines draw: {lines}");
}

#[test]
fn editor_grid_hides_behind_meshes_without_outlines() {
    let mut renderer = renderer();
    let mut view = viewport(&mut renderer);
    let mesh = quad(&mut renderer, 0.4, 0.6);
    let material = flat(&mut renderer, [0.2, 0.2, 0.2, 1.0]);
    let mut frame = ambient_frame(1.0);
    frame
        .editor_grid(front_grid())
        .draw(Draw::new(mesh, material).object_id(1));
    let pixels = read(&mut renderer, &mut view, &frame);
    let covered = (CENTER - 2..CENTER + 2)
        .flat_map(|y| (CENTER + 4..CENTER + 12).map(move |x| (x, y)))
        .map(|(x, y)| pixel(&pixels, x, y))
        .find(|texel| texel[0] > texel[1].saturating_add(40));
    assert_eq!(covered, None, "the X axis stays behind the quad");
}

use std::time::Duration;

use rikka_renderer::{Node, Renderer, Scene};
use rikka_renderer_3d::Three;
use rikka_renderer_3d::glam::{Mat4, Vec3, Vec4};
use rikka_renderer_3d::{
    Antialiasing, Background, Camera, Cull, Draw, DynamicScale, Frame, Light, MaterialDesc,
    MaterialId, MaterialState, MaterialTemplate, MeshId, Outline, RenderScale, Tier, ToneMapping,
    Upscale, Viewport, ViewportConfig,
};
use rikka_test_renderer::TestRenderer;
use rikka_types::{Color, Rect};

use crate::allocations::allocations;

const SIZE: u32 = 64;
const SUPERSAMPLE: u32 = 4;
const STILL_FRAMES: u32 = 300;
const LOWEST_SCALE: f32 = 0.25;

fn renderer() -> TestRenderer {
    rikka_test_renderer::renderer_3d("rikka-temporal-test")
        .expect("a Vulkan 1.3 device with the mesh feature set is required for these tests")
}

fn viewport(
    renderer: &mut Renderer,
    size: u32,
    samples: u32,
    antialiasing: Antialiasing,
) -> Viewport {
    renderer
        .three()
        .create_viewport(
            ViewportConfig::new(size, size)
                .samples(samples)
                .identity(false)
                .antialiasing(antialiasing),
        )
        .expect("a viewport")
}

fn show(renderer: &mut Renderer, viewport: &Viewport) -> Vec<f32> {
    let size = viewport.width();
    let mut target = renderer
        .create_target(size, size)
        .expect("an offscreen target");
    let mut scene = Scene::new();
    scene.set_background(Color::rgb(0.0, 0.0, 0.0));
    scene.insert(Node::image(
        Rect::new(0.0, 0.0, size as f32, size as f32),
        viewport.texture(),
    ));
    renderer
        .draw(&mut scene, &mut target)
        .expect("a 2D frame draws");
    let pixels = renderer.read_pixels(&mut target).expect("pixels come back");
    pixels
        .as_chunks::<4>()
        .0
        .iter()
        .flat_map(|pixel| pixel[..3].iter().map(|&value| linear(value)))
        .collect()
}

fn linear(value: u8) -> f32 {
    let encoded = f32::from(value) / 255.0;
    match encoded <= 0.04045 {
        true => encoded / 12.92,
        false => ((encoded + 0.055) / 1.055).powf(2.4),
    }
}

fn render(renderer: &mut Renderer, viewport: &mut Viewport, frame: &Frame) {
    renderer
        .three()
        .render(viewport, frame)
        .expect("a 3D frame draws");
}

fn still(renderer: &mut Renderer, viewport: &mut Viewport, frame: &Frame, count: u32) -> Vec<f32> {
    for _ in 0..count {
        render(renderer, viewport, frame);
    }
    show(renderer, viewport)
}

fn downsampled(pixels: &[f32], size: u32, factor: u32) -> Vec<f32> {
    let small = size / factor;
    let mut out = vec![0.0; (small * small * 3) as usize];
    for y in 0..size {
        for x in 0..size {
            for channel in 0..3 {
                let from = ((y * size + x) * 3 + channel) as usize;
                let to = (((y / factor) * small + x / factor) * 3 + channel) as usize;
                out[to] += pixels[from] / (factor * factor) as f32;
            }
        }
    }
    out
}

fn mean_error(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(a, b)| (a - b).abs()).sum::<f32>() / a.len() as f32
}

fn luma(pixels: &[f32], size: u32, x: u32, y: u32) -> f32 {
    let at = ((y * size + x) * 3) as usize;
    0.2126 * pixels[at] + 0.7152 * pixels[at + 1] + 0.0722 * pixels[at + 2]
}

fn camera() -> Camera {
    Camera::look_at(
        Vec3::new(0.0, 0.0, 3.0),
        Vec3::ZERO,
        Vec3::Y,
        0.9,
        1.0,
        0.05,
    )
}

fn base_frame(camera: Camera) -> Frame {
    let mut frame = Frame::new(camera);
    frame
        .background(Background::Color(Vec3::ZERO))
        .ambient(Vec3::splat(1.0))
        .tone_mapping(ToneMapping::Clamp)
        .time(1.0)
        .outline(Outline {
            enabled: false,
            ..Outline::default()
        });
    frame
}

fn bright(renderer: &mut Renderer) -> MaterialId {
    grey(renderer, 1.0)
}

fn grey(renderer: &mut Renderer, albedo: f32) -> MaterialId {
    renderer
        .three()
        .create_material(rikka_test_renderer_3d::standard(
            Vec4::new(albedo, albedo, albedo, 1.0),
            MaterialState::opaque(),
        ))
        .expect("a material")
}

fn sphere(renderer: &mut Renderer) -> MeshId {
    rikka_test_renderer_3d::sphere(renderer, 0.6, 24).expect("a sphere")
}

fn tilted(mesh: MeshId, material: MaterialId) -> Draw {
    Draw::new(mesh, material)
        .transform(Mat4::from_rotation_z(0.35) * Mat4::from_scale(Vec3::new(1.3, 0.7, 1.0)))
}

#[test]
fn still_frames_converge_to_the_supersampled_image() {
    let mut renderer = renderer();
    let mesh = sphere(&mut renderer);
    let material = bright(&mut renderer);
    let mut plain = base_frame(camera());
    plain.draw(tilted(mesh, material));
    let mut looked = base_frame(camera());
    looked
        .look(rikka_test_renderer_3d::minimal_look())
        .draw(tilted(mesh, grey(&mut renderer, 0.2)));

    let modes = [
        ("temporal", 1, Antialiasing::Temporal, 0.3),
        (
            "temporal over 4x multisampling",
            4,
            Antialiasing::Temporal,
            0.45,
        ),
        (
            "upscale from 67%",
            1,
            Antialiasing::Upscale(Upscale::fixed(0.67).sharpness(0.0)),
            0.45,
        ),
        (
            "upscale from 50%",
            1,
            Antialiasing::Upscale(Upscale::fixed(0.5).sharpness(0.0)),
            0.5,
        ),
    ];
    for (scene, frame, filmic) in [("plain", &plain, false), ("looked", &looked, true)] {
        let large = SIZE * SUPERSAMPLE;
        let mut reference = viewport(&mut renderer, large, 1, Antialiasing::Multisample);
        let reference = downsampled(
            &still(&mut renderer, &mut reference, frame, 1),
            large,
            SUPERSAMPLE,
        );
        let mut aliased = viewport(&mut renderer, SIZE, 1, Antialiasing::Multisample);
        let aliased = mean_error(&still(&mut renderer, &mut aliased, frame, 1), &reference);
        let mut multisampled = viewport(&mut renderer, SIZE, 4, Antialiasing::Multisample);
        let multisampled = mean_error(
            &still(&mut renderer, &mut multisampled, frame, 1),
            &reference,
        );
        for (name, samples, antialiasing, share) in modes {
            let mut view = viewport(&mut renderer, SIZE, samples, antialiasing);
            let converged = still(&mut renderer, &mut view, frame, STILL_FRAMES);
            let error = mean_error(&converged, &reference);
            let bound = match filmic {
                false => aliased * share,
                true => multisampled,
            };
            assert!(
                error < bound,
                "{scene} {name}: error {error:.5} against the supersampled image, aliased \
                 {aliased:.5}, 4x multisampled {multisampled:.5}"
            );
        }
    }
}

#[test]
fn settled_still_frames_do_not_change() {
    let mut renderer = renderer();
    let mesh = sphere(&mut renderer);
    let material = bright(&mut renderer);
    let mut frame = base_frame(camera());
    frame.draw(tilted(mesh, material));
    let mut view = viewport(&mut renderer, SIZE, 1, Antialiasing::Temporal);
    let settled = still(&mut renderer, &mut view, &frame, STILL_FRAMES);
    let later = still(&mut renderer, &mut view, &frame, 3);
    assert_eq!(settled, later, "a settled still image stays bit-identical");
}

#[test]
fn moving_object_leaves_no_ghost() {
    const STEP: f32 = 0.03;
    const FRAMES: u32 = 40;
    let mut renderer = renderer();
    let mesh = sphere(&mut renderer);
    let material = bright(&mut renderer);
    let at = |frame: u32| {
        Mat4::from_translation(Vec3::new(-1.0 + STEP * frame as f32, 0.0, 0.0))
            * Mat4::from_scale(Vec3::splat(0.5))
    };
    let moving = |index: u32| {
        let mut frame = base_frame(camera());
        frame
            .time(index as f32 / 60.0)
            .draw(Draw::new(mesh, material).transform(at(index)));
        frame
    };

    let mut view = viewport(&mut renderer, SIZE, 1, Antialiasing::Temporal);
    for index in 0..FRAMES {
        render(&mut renderer, &mut view, &moving(index));
    }
    let resolved = show(&mut renderer, &view);
    let mut plain = viewport(&mut renderer, SIZE, 1, Antialiasing::Multisample);
    render(&mut renderer, &mut plain, &moving(FRAMES - 1));
    let truth = show(&mut renderer, &plain);

    let trail_end = (0..SIZE)
        .find(|&x| luma(&truth, SIZE, x, SIZE / 2) > 0.05)
        .expect("the sphere is on screen");
    let mut worst: f32 = 0.0;
    for y in SIZE / 2 - 6..SIZE / 2 + 6 {
        for x in 0..trail_end.saturating_sub(2) {
            worst = worst.max(luma(&resolved, SIZE, x, y) - luma(&truth, SIZE, x, y));
        }
    }
    assert!(
        worst < 0.04,
        "the trail behind the sphere glows by {worst:.4}"
    );
}

#[test]
fn camera_cut_drops_the_history() {
    let mut renderer = renderer();
    let mesh = sphere(&mut renderer);
    let material = bright(&mut renderer);
    let mut before = base_frame(camera());
    before.draw(
        Draw::new(mesh, material).transform(Mat4::from_translation(Vec3::new(-0.7, 0.0, 0.0))),
    );
    let mut after = base_frame(Camera::look_at(
        Vec3::new(0.0, 3.0, 0.5),
        Vec3::ZERO,
        Vec3::Y,
        0.9,
        1.0,
        0.05,
    ));
    after.time(2.0).draw(
        Draw::new(mesh, material).transform(Mat4::from_translation(Vec3::new(0.0, 0.0, 5.0))),
    );
    after.cut();

    let mut view = viewport(&mut renderer, SIZE, 1, Antialiasing::Temporal);
    still(&mut renderer, &mut view, &before, 20);
    let cut = still(&mut renderer, &mut view, &after, 1);
    let brightest = cut.iter().copied().fold(0.0, f32::max);
    assert!(
        brightest < 0.01,
        "the frame after a cut shows {brightest:.4} of the old shot"
    );
}

const OUTLINED: &str = r#"
import rikka;

struct Surface : ISurface<RikkaNoData> {
    [Color] [Default(1.0, 1.0, 1.0)] float3 base_color;

    [constref]
    RikkaSurfaceData<RikkaNoData> evaluate(RikkaPixel pixel, RikkaContext context) {
        RikkaNoData data;
        data.unused = 0u;
        RikkaSurfaceData<RikkaNoData> s = rikka_surface_data(pixel, data);
        s.albedo = pixel.hull ? float3(0.0) : base_color;
        return s;
    }

    [constref]
    override RikkaHull hull(RikkaHullVertex vertex, RikkaContext context, float scale) {
        RikkaHull h;
        h.direction = vertex.smoothed_normal;
        h.width = 0.08 * scale;
        h.depth_offset = 0.0;
        return h;
    }
};

typealias Model = RikkaUnlitModel;
"#;

fn outlined(renderer: &mut Renderer, template: &MaterialTemplate, hull: bool) -> MaterialId {
    let mut desc = MaterialDesc::new(template);
    if hull {
        desc = desc.state(MaterialState::opaque().cull(Cull::Front)).hull();
    }
    renderer
        .three()
        .create_material(desc)
        .expect("an outlined material")
}

fn sharpness(pixels: &[f32], size: u32) -> (f32, usize) {
    let mut steps: Vec<f32> = Vec::new();
    let mut dark = 0;
    for y in 1..size - 1 {
        for x in 1..size - 1 {
            let here = luma(pixels, size, x, y);
            steps.push((luma(pixels, size, x + 1, y) - here).abs());
            steps.push((luma(pixels, size, x, y + 1) - here).abs());
            if here < 0.05 {
                dark += 1;
            }
        }
    }
    steps.sort_by(f32::total_cmp);
    let top = &steps[steps.len() * 98 / 100..];
    (top.iter().sum::<f32>() / top.len() as f32, dark)
}

#[test]
fn hull_outlines_stay_crisp_while_moving() {
    const FRAMES: u32 = 30;
    let mut renderer = renderer();
    let mesh = sphere(&mut renderer);
    let template = rikka_renderer_3d::ShaderCompiler::new()
        .expect("libslang loads")
        .template("outlined", OUTLINED)
        .expect("the outlined template compiles");
    let body = outlined(&mut renderer, &template, false);
    let hull = outlined(&mut renderer, &template, true);
    let moving = |index: u32| {
        let mut frame = base_frame(camera());
        let transform = Mat4::from_translation(Vec3::new(-0.2 + 0.01 * index as f32, 0.0, 0.0));
        frame
            .background(Background::Color(Vec3::splat(0.6)))
            .ambient(Vec3::splat(0.6))
            .light(Light::directional(Vec3::new(0.3, -0.4, -1.0)))
            .time(index as f32 / 60.0)
            .draw(Draw::new(mesh, body).transform(transform))
            .draw(Draw::new(mesh, hull).transform(transform));
        frame
    };
    let mut multisampled = viewport(&mut renderer, SIZE, 4, Antialiasing::Multisample);
    let mut temporal = viewport(&mut renderer, SIZE, 1, Antialiasing::Temporal);
    for index in 0..FRAMES {
        let frame = moving(index);
        render(&mut renderer, &mut multisampled, &frame);
        render(&mut renderer, &mut temporal, &frame);
    }
    let (reference, reference_dark) = sharpness(&show(&mut renderer, &multisampled), SIZE);
    let (resolved, resolved_dark) = sharpness(&show(&mut renderer, &temporal), SIZE);
    assert!(
        resolved > reference * 0.7,
        "outline edges soften: {resolved:.4} against multisampled {reference:.4}"
    );
    assert!(
        resolved_dark as f32 > reference_dark as f32 * 0.7,
        "outline ink fades: {resolved_dark} dark pixels against {reference_dark}"
    );
}

#[test]
fn upscaled_viewports_render_below_display_size() {
    let mut renderer = renderer();
    let view = viewport(
        &mut renderer,
        SIZE,
        4,
        Antialiasing::Upscale(Upscale::fixed(0.5)),
    );
    assert_eq!(view.render_size(), (SIZE / 2, SIZE / 2));
    assert_eq!(view.samples(), 1, "upscaling renders single-sampled");
    assert_eq!((view.width(), view.height()), (SIZE, SIZE));
}

#[test]
fn dynamic_resolution_follows_the_gpu_budget() {
    const FRAMES: u32 = 120;
    let mut renderer = renderer();
    let mesh = sphere(&mut renderer);
    let material = bright(&mut renderer);
    let cases = [
        (
            "unreachable budget",
            Duration::from_nanos(1),
            DynamicScale::MIN,
        ),
        ("generous budget", Duration::from_secs(1), DynamicScale::MAX),
    ];
    for (name, target, scale) in cases {
        let mut view = viewport(
            &mut renderer,
            SIZE * 2,
            1,
            Antialiasing::Upscale(Upscale::dynamic(target)),
        );
        for index in 0..FRAMES {
            let mut frame = base_frame(camera());
            frame.time(index as f32 / 60.0).draw(tilted(mesh, material));
            render(&mut renderer, &mut view, &frame);
        }
        assert_eq!(view.render_scale(), scale, "{name}");
        assert!(view.gpu_time().is_some(), "{name}: the viewport is timed");
        let side = (SIZE as f32 * 2.0 * scale).round() as u32;
        assert_eq!(view.render_size(), (side, side), "{name}");
    }
}

fn stepping() -> Antialiasing {
    Antialiasing::Upscale(Upscale {
        scale: RenderScale::Dynamic(
            DynamicScale::new(Duration::from_nanos(1)).range(LOWEST_SCALE, DynamicScale::MAX),
        ),
        sharpness: 0.0,
    })
}

struct Level {
    scale: f32,
    frames: u32,
    image: Vec<f32>,
}

struct Step {
    allocated: u64,
    change: f32,
}

#[test]
fn dynamic_scale_steps_allocate_nothing_and_keep_the_history() {
    let mut renderer = rikka_test_renderer::ray_renderer("rikka-temporal-test")
        .expect("a Vulkan device that traces rays is required for this test");
    for tier in [Tier::ScreenSpace, Tier::RayTraced] {
        steps_keep_the_history(&mut renderer, tier);
    }
}

fn steps_keep_the_history(renderer: &mut Renderer, tier: Tier) {
    const FRAMES: u32 = 60;
    const WARM_FRAMES: u32 = 6;
    let mesh = sphere(renderer);
    let mut look = rikka_test_renderer_3d::minimal_look();
    look.occlusion.strength = 1.0;
    look.illumination.strength = 1.0;
    look.shadows.tier = tier;
    look.occlusion.tier = tier;
    look.illumination.tier = tier;
    look.reflections.tier = tier;
    let mut frame = base_frame(camera());
    frame
        .look(look)
        .light(Light::directional(Vec3::new(0.3, -0.4, -1.0)))
        .draw(tilted(mesh, grey(renderer, 0.6)));
    let side = SIZE * 2;
    let mut view = viewport(renderer, side, 1, stepping());
    let mut levels: Vec<Level> = Vec::new();
    let mut steps: Vec<Step> = Vec::new();
    let mut steady = 0;
    let mut shown = Vec::new();
    for index in 0..FRAMES {
        let scale = view.render_scale();
        let ((), allocated) = allocations(|| render(renderer, &mut view, &frame));
        let image = show(renderer, &view);
        if view.render_scale() != scale {
            steps.push(Step {
                allocated,
                change: mean_error(&image, &shown),
            });
            levels.push(Level {
                scale,
                frames: index,
                image: std::mem::take(&mut shown),
            });
        } else if index >= WARM_FRAMES {
            steady = steady.max(allocated);
        }
        shown = image;
    }
    levels.push(Level {
        scale: view.render_scale(),
        frames: FRAMES,
        image: shown,
    });
    assert_eq!(view.render_scale(), LOWEST_SCALE, "the scale steps down");
    assert!(steps.len() >= 2, "the scale steps more than once");
    let mut fresh_error = 0.0f32;
    for level in &levels {
        let mut fixed = viewport(
            renderer,
            side,
            1,
            Antialiasing::Upscale(Upscale::fixed(level.scale).sharpness(0.0)),
        );
        let fresh = still(renderer, &mut fixed, &frame, 1);
        let converged = still(renderer, &mut fixed, &frame, level.frames - 1);
        let reset = mean_error(&fresh, &converged);
        let error = mean_error(&level.image, &converged);
        assert!(
            error < reset * 0.5,
            "{tier:?} at scale {}: {error:.5} from the fixed-scale image, a fresh history is \
             {reset:.5} off",
            level.scale
        );
        fresh_error = fresh_error.max(reset);
    }
    for step in steps {
        assert!(
            step.allocated <= steady,
            "{tier:?}: a scale step allocates {} times, a steady frame at most {steady}",
            step.allocated
        );
        assert!(
            step.change < fresh_error * 0.1,
            "{tier:?}: a scale step changes the image by {:.5}, a fresh history is \
             {fresh_error:.5} off",
            step.change
        );
    }
}

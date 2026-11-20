mod wakes;

pub(crate) use wakes::{Wakes, wakes};

use std::time::{Duration, Instant};

use rikka_renderer::Scene;
use rikka_test_renderer::TestRenderer;
use rikka_theme::Theme;
use rikka_types::{Color, Rect, Size};
use rikka_ui::{Rendering, State, TypedUi, ViewFn};

pub(crate) fn dump_rect(line: &str) -> Rect {
    let start = line.find('[').expect("dump line has a rect") + 1;
    let end = start + line[start..].find(']').expect("dump rect is closed");
    let numbers: Vec<f32> = line[start..end]
        .split([' ', 'x'])
        .map(|number| number.parse().expect("dump rect holds numbers"))
        .collect();
    Rect::new(numbers[0], numbers[1], numbers[2], numbers[3])
}

const SETTLED: Duration = Duration::from_secs(2);

pub(crate) fn renderer() -> TestRenderer {
    rikka_test_renderer::renderer("rikka-ui-test").expect("no Vulkan 1.3 device")
}

pub(crate) fn settled_pixels<S: State>(
    state: S,
    view: impl for<'a> ViewFn<'a, S>,
    size: u32,
) -> Vec<u8> {
    settled_pixels_on(state, view, size, Theme::dark())
}

pub(crate) fn settled_pixels_on<S: State>(
    state: S,
    view: impl for<'a> ViewFn<'a, S>,
    size: u32,
    theme: Theme,
) -> Vec<u8> {
    settled_pixels_at(state, view, size, 1.0, theme)
}

pub(crate) fn settled_pixels_scaled<S: State>(
    state: S,
    view: impl for<'a> ViewFn<'a, S>,
    size: u32,
    scale: f32,
) -> Vec<u8> {
    settled_pixels_at(state, view, size, scale, Theme::dark())
}

fn settled_pixels_at<S: State>(
    state: S,
    view: impl for<'a> ViewFn<'a, S>,
    size: u32,
    scale: f32,
    theme: Theme,
) -> Vec<u8> {
    let side = (size as f32 * scale) as u32;
    let mut renderer = renderer();
    let mut target = renderer.create_target(side, side).unwrap();
    let mut scene = Scene::new();

    settled(state, view, size, scale, theme, &mut scene);
    renderer.draw(&mut scene, &mut target).unwrap();
    renderer.read_pixels(&mut target).unwrap()
}

pub(crate) fn settled<S: State>(
    state: S,
    view: impl for<'a> ViewFn<'a, S>,
    size: u32,
    scale: f32,
    theme: Theme,
    scene: &mut Scene,
) -> TypedUi<S> {
    let mut ui = TypedUi::new(state, view, theme);
    ui.set_viewport(Size::new(size as f32, size as f32), scale);
    let start = Instant::now();
    ui.render(Rendering::new(scene).with_time(start));
    ui.render(Rendering::new(scene).with_time(start + SETTLED));
    ui
}

pub(crate) fn pixel(pixels: &[u8], stride: usize, x: usize, y: usize) -> (u8, u8, u8, u8) {
    let i = (y * stride + x) * 4;
    (pixels[i], pixels[i + 1], pixels[i + 2], pixels[i + 3])
}

pub(crate) const UNCOVERED: Color = Color::rgb(1.0, 0.0, 0.0);

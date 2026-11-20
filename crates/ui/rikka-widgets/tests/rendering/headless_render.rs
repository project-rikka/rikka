use crate::common;

use std::time::{Duration, Instant};

use crate::common::{UNCOVERED, renderer, settled_pixels, settled_pixels_scaled};
use rikka_renderer::Scene;
use rikka_theme::Theme;
use rikka_types::{Color, Size};
use rikka_ui::*;
use rikka_widgets::*;

const VIEW: u32 = 256;

fn pixel(pixels: &[u8], x: usize, y: usize) -> (u8, u8, u8, u8) {
    common::pixel(pixels, VIEW as usize, x, y)
}

#[derive(State)]
struct Panel {
    blue: bool,
}

fn panel_view(panel: &Panel) -> Element {
    let color = if panel.blue {
        Color::rgb(0.0, 0.0, 1.0)
    } else {
        Color::rgb(0.0, 1.0, 0.0)
    };
    container(spacer())
        .background(color)
        .width(Length::px(128.0))
        .height(Length::px(128.0))
        .margin(Inset::all(64.0))
        .build()
}

#[test]
fn unchanged_frames_draw_nothing() {
    let mut renderer = renderer();
    let mut target = renderer.create_target(VIEW, VIEW).unwrap();
    let mut scene = Scene::new();

    let mut ui = TypedUi::new(Panel { blue: true }, panel_view, Theme::dark());
    ui.set_background(Some(UNCOVERED));
    ui.set_viewport(Size::new(VIEW as f32, VIEW as f32), 1.0);
    let start = Instant::now();
    ui.render(Rendering::new(&mut scene).with_time(start));

    let out = renderer.draw(&mut scene, &mut target).unwrap();
    assert!(out.rendered());
    let px = renderer.read_pixels(&mut target).unwrap();
    assert_eq!(pixel(&px, 10, 10), (255, 0, 0, 255), "theme background");
    assert_eq!(pixel(&px, 128, 128), (0, 0, 255, 255), "panel fill");

    ui.update(|_| {});
    assert!(ui.needs_render());
    ui.render(Rendering::new(&mut scene).with_time(start));
    let out = renderer.draw(&mut scene, &mut target).unwrap();
    assert!(!out.rendered(), "no-op rebuild damaged scene");

    ui.update(|panel| panel.blue = !panel.blue);
    ui.render(Rendering::new(&mut scene).with_time(start));
    let settled = start + Duration::from_secs(2);
    ui.render(Rendering::new(&mut scene).with_time(settled));
    let out = renderer.draw(&mut scene, &mut target).unwrap();
    assert!(out.rendered());
    assert!(!ui.needs_render(), "fill never settled");
    let px = renderer.read_pixels(&mut target).unwrap();
    assert_eq!(pixel(&px, 128, 128), (0, 255, 0, 255), "toggled fill");
}

const FIELD: f32 = 96.0;
const MARGIN: f32 = 24.0;

#[derive(State)]
struct Selection {
    all: bool,
}

fn selection_view(selection: &Selection) -> Element {
    let document = TextDocument::new("selected");
    if selection.all {
        document.select_all();
    }
    container(
        text_field(&document)
            .autofocus(true)
            .on_change(|| {})
            .width(Length::px(FIELD)),
    )
    .padding(Space::LOOSE)
    .build()
}

fn drag_across_field(all: bool) -> Vec<u8> {
    let pixels = settled_pixels(Selection { all }, selection_view, VIEW);
    let row = MARGIN as usize + 18;
    (MARGIN as usize + 4..(MARGIN + FIELD) as usize - 4)
        .map(|x| pixel(&pixels, x, row))
        .flat_map(|(r, g, b, a)| [r, g, b, a])
        .collect()
}

#[test]
fn selection_draws_behind_text() {
    assert_ne!(
        drag_across_field(false),
        drag_across_field(true),
        "selection not drawn"
    );
}

const FRACTIONAL_SCALE: f32 = 1.5;
const FRACTIONAL_VIEW: u32 = 128;

#[derive(State)]
struct Lowered {
    by: f32,
}

fn lowered_cell(lowered: &Lowered) -> Element {
    let by = lowered.by;
    lazy_column(2, move |slot| match slot.index {
        0 => container(spacer())
            .height(Length::px(by))
            .fill_width()
            .build(),
        _ => text("Appearance").label().build(),
    })
    .fill()
    .build()
}

fn lowered_pixels(by: f32) -> Vec<u8> {
    settled_pixels_scaled(
        Lowered { by },
        lowered_cell,
        FRACTIONAL_VIEW,
        FRACTIONAL_SCALE,
    )
}

fn ink(pixels: &[u8]) -> Vec<&[u8]> {
    let side = (FRACTIONAL_VIEW as f32 * FRACTIONAL_SCALE) as usize;
    let (texels, _) = pixels.as_chunks::<4>();
    let background = texels[texels.len() - 1];
    let (left, right, top, bottom) = texels
        .iter()
        .enumerate()
        .filter(|&(_, rgba)| *rgba != background)
        .map(|(at, _)| (at % side, at / side))
        .fold(
            (usize::MAX, 0, usize::MAX, 0),
            |(left, right, top, bottom), (x, y)| {
                (left.min(x), right.max(x), top.min(y), bottom.max(y))
            },
        );
    assert!(left <= right, "label not drawn");
    (top..=bottom)
        .map(|y| &pixels[(y * side + left) * 4..(y * side + right + 1) * 4])
        .collect()
}

#[test]
fn list_text_off_device_pixels_draws_crisp_at_fractional_scale() {
    let whole = lowered_pixels(2.0);
    let halved = lowered_pixels(1.0);
    assert!(
        ink(&whole) == ink(&halved),
        "label resampled on a half-pixel offset"
    );
}

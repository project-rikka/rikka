use rikka_format::truncate;
use rikka_session::toplevels::Bounds;
use rikka_types::Size;
use rikka_ui::{
    Align, Axis, Element, Label, Length, MaterialSymbol, Space, Styled, TextStyled, View, text, tr,
};
use rikka_widgets::{ControlSize, button, column, container, icon, row};
use task_board::Window;

use crate::axis;
use crate::glide::{glide, settles};
use crate::island::body::MARGIN;
use crate::island::state::Island;
use crate::tasks::{Aim, Tile, thumbnail};

const BOX: Size = Size::new(200.0, 124.0);
const NARROWEST: f32 = 132.0;
const GAP: f32 = 10.0;
const PADDING: f32 = 10.0;
const SMALLEST: f32 = 0.25;
const UNKNOWN_SHAPE: f32 = 16.0 / 10.0;
const CLOSE: f32 = 14.0;
const CLOSE_ROOM: f32 = 28.0;
const TITLE: usize = 80;
const HEADER_GAP: f32 = 4.0;

pub(super) fn view(island: &Island, aim: &Aim) -> Option<(Element, f32)> {
    let (group, tile) = island.tasks().group(aim.space, &aim.key)?;
    let axis = island.dock().axis();
    let shapes: Vec<Size> = group
        .windows
        .iter()
        .map(|window| fitted(window.bounds))
        .collect();
    let widths: Vec<f32> = shapes
        .iter()
        .map(|shape| shape.width.max(NARROWEST))
        .collect();
    let lengths: Vec<f32> = match axis {
        Axis::Horizontal => widths.clone(),
        Axis::Vertical => shapes
            .iter()
            .map(|shape| shape.height + CLOSE_ROOM + HEADER_GAP)
            .collect(),
    };
    let gaps = lengths.len().saturating_sub(1) as f32 * GAP;
    let natural: f32 = lengths.iter().sum();
    let room = island.dock().length() - MARGIN * 2.0 - gaps - PADDING * 2.0;
    let shrink = match natural > room && natural > 0.0 {
        true => (room / natural).max(SMALLEST),
        false => 1.0,
    };
    let width = match axis {
        Axis::Horizontal => natural * shrink + gaps + PADDING * 2.0,
        Axis::Vertical => widths.iter().copied().fold(0.0, f32::max) * shrink + PADDING * 2.0,
    };
    let entries = axis::line(
        axis,
        group
            .windows
            .iter()
            .zip(&shapes)
            .zip(&widths)
            .map(|((window, shape), entry)| {
                glide(
                    axis,
                    entry_view(
                        window,
                        tile,
                        Size::new(shape.width * shrink, shape.height * shrink),
                        entry * shrink,
                    ),
                    settles(),
                    window.id.clone(),
                )
            }),
    )
    .gap(GAP)
    .align_items(match axis {
        Axis::Horizontal => Align::End,
        Axis::Vertical => Align::Center,
    });
    let card = container(entries)
        .padding(PADDING)
        .width(Length::px(width))
        .build();
    Some((card, width))
}

fn fitted(bounds: Bounds) -> Size {
    let shape = match bounds.width > 0 && bounds.height > 0 {
        true => bounds.width as f32 / bounds.height as f32,
        false => UNKNOWN_SHAPE,
    };
    let width = BOX.width.min(BOX.height * shape);
    Size::new(width, width / shape)
}

fn entry_view(window: &Window, tile: &Tile, shape: Size, width: f32) -> impl View {
    let closing = window.id.clone();
    let focusing = window.id.clone();
    column! {
        row! {
            text(title(window, tile))
                .label()
                .no_wrap()
                .max_width(Length::px((width - CLOSE_ROOM).max(0.0)))
                .grow(1.0);
            button(icon(MaterialSymbol::CLOSE).extent(Length::px(CLOSE)))
                .icon_only()
                .ghost()
                .control_size(ControlSize::Small)
                .on_press(move |island: &mut Island| island.close_window(&closing));
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .width(Length::px(width));
        container(thumbnail(&window.id, shape, move |island: &mut Island| {
            island.focus_window(&focusing);
        }))
        .align_x(Align::Center)
        .width(Length::px(width));
    }
    .gap(HEADER_GAP)
}

fn title(window: &Window, tile: &Tile) -> Label {
    let shown = match window.title.trim().is_empty() {
        true => tile.name.as_str(),
        false => window.title.as_str(),
    };
    match shown.trim().is_empty() {
        true => tr!("shell.tasks.untitled"),
        false => Label::from(truncate(shown, TITLE).into_owned()),
    }
}

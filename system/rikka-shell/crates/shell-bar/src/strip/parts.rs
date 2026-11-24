use std::sync::{Arc, LazyLock};

use rikka_types::Rect;
use rikka_ui::{Align, IconSource, ImageSource, Length, View, palette};
use rikka_widgets::{ControlSize, button, container, icon};
use shell_kit::highlight_button;

use crate::axis;
use crate::cluster::{DEPTH, INSET, RADIUS};
use crate::state::Bar;
use crate::status::{self, Status};
use crate::tray;
use crate::workspaces::Slots;

const LOGO: f32 = 18.0;

static SNOWFLAKE: LazyLock<Arc<[u8]>> =
    LazyLock::new(|| Arc::from(*include_bytes!("../../assets/nix-snowflake.svg")));

#[must_use]
pub(crate) fn logo() -> impl View {
    let snowflake = ImageSource::vector(Arc::clone(&SNOWFLAKE));
    button(
        icon(IconSource::Mask(snowflake))
            .extent(Length::px(LOGO))
            .color(palette().accent),
    )
    .icon_only()
    .ghost()
    .control_size(ControlSize::Small)
    .on_press(Bar::assist)
}

#[must_use]
pub(crate) fn workspaces(bar: &Bar) -> impl View {
    Slots::new(
        bar.showing(),
        bar.holding().to_vec(),
        bar.dock().axis(),
        |bar: &mut Bar, number| {
            bar.show(number);
        },
    )
}

#[must_use]
pub(crate) fn cluster(bar: &Bar) -> impl View {
    let axis = bar.dock().axis();
    axis::line(axis, tray(bar))
        .push(indicators(bar))
        .push(status::status(Status {
            daemons: &bar.shared().daemons,
            bell: bar.bell(),
            percentage: bar.shared().percentage.is_shown(),
            hovered: bar.is_hovered(),
            axis,
        }))
        .align_items(Align::Center)
}

fn tray(bar: &Bar) -> Option<impl View> {
    let shared = bar.shared();
    let unfolded = shared.fold.is_unfolded();
    let edge = bar.dock().edge();
    shared.tray.read(|tray| tray::chevron(tray, unfolded, edge))
}

fn indicators(bar: &Bar) -> impl View {
    let shelf = &bar.shared().shelf;
    let axis = bar.dock().axis();
    axis::line(
        axis,
        shelf.marks().into_iter().map(|(kind, mark)| {
            highlight_button(
                container(mark).padding(axis::padding(axis, INSET, DEPTH)),
                move |bar: &mut Bar, at: Rect| bar.indicate(kind, at),
            )
            .pressed(shelf.is_open(kind))
            .radius(RADIUS)
        }),
    )
    .align_items(Align::Center)
}

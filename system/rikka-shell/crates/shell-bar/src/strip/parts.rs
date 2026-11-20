use std::sync::{Arc, LazyLock};

use rikka_types::Rect;
use rikka_ui::{Align, IconSource, ImageSource, Inset, Length, View, palette};
use rikka_widgets::{ControlSize, button, container, icon, row};
use shell_kit::highlight_button;

use crate::cluster::{DEPTH, INSET, RADIUS};
use crate::state::Bar;
use crate::workspaces::Slots;
use crate::{status, tray};

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
        |bar: &mut Bar, number| {
            bar.show(number);
        },
    )
}

#[must_use]
pub(crate) fn cluster(bar: &Bar) -> impl View {
    row! {
        tray(bar);
        indicators(bar);
        status::status(
            &bar.shared().daemons,
            bar.bell(),
            bar.shared().percentage.is_shown(),
            bar.is_hovered(),
        );
    }
    .align_items(Align::Center)
}

fn tray(bar: &Bar) -> impl View {
    let shared = bar.shared();
    let unfolded = shared.fold.is_unfolded();
    shared.tray.read(|tray| tray::chevron(tray, unfolded))
}

fn indicators(bar: &Bar) -> impl View {
    let shelf = &bar.shared().shelf;
    row! {
        for (kind, mark) in shelf.marks() {
            highlight_button(
                container(mark).padding(Inset::xy(INSET, DEPTH)),
                move |bar: &mut Bar, at: Rect| bar.indicate(kind, at),
            )
            .pressed(shelf.is_open(kind))
            .radius(RADIUS);
        }
    }
    .align_items(Align::Center)
}

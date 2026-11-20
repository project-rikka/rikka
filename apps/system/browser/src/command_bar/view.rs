use rikka_app::prelude::*;
use rikka_widgets::{column, row};

use crate::command_bar::state::CommandBar;
use crate::omnibox::rows;
use crate::shell::Shell;
use crate::widgets::{floating, presence};

const WIDTH: f32 = 640.0;
const TOP: f32 = 72.0;
const LIST: f32 = 440.0;

pub(crate) fn view(bar: &CommandBar) -> Element {
    presence(bar.is_open().then(|| panel(bar)))
}

fn panel(bar: &CommandBar) -> Element {
    let listing = bar
        .picker
        .listing()
        .filter(|listing| !listing.rows.is_empty())
        .map(|listing| {
            scrollable(rows(listing, bar.picker.store.as_ref()))
                .max_height(LIST)
                .fill_width()
        });
    let field = row! {
        icon(MaterialSymbol::SEARCH)
            .extent(Length::px(20.0))
            .color(palette().text_muted);
        text_field(bar.field())
            .placeholder(tr!("browser.command.placeholder"))
            .plain(true)
            .autofocus(true)
            .claims_focus(true)
            .on_change(|shell: &mut Shell| shell.command_bar_edited())
            .on_key(|shell: &mut Shell, key: Key, modifiers: Modifiers| {
                shell.command_bar_key(key, modifiers)
            })
            .grow(1.0)
            .min_width(0.0);
    }
    .gap(Space::NORMAL)
    .align_items(Align::Center)
    .padding(Inset::xy(Space::ROOMY, Space::NORMAL))
    .fill_width();
    let divider = listing.is_some().then(|| {
        container(spacer())
            .background(palette().outline.with_alpha(0.4))
            .height(1.0)
            .fill_width()
    });
    let card = container(
        column! {
            field;
            divider;
            listing;
        }
        .width(WIDTH),
    )
    .glass()
    .background(palette().surface.with_alpha(0.92))
    .border(1.0, palette().outline.with_alpha(0.5))
    .radius(Corner::Panel);
    container(floating(card, |shell: &mut Shell| {
        shell.close_command_bar()
    }))
    .align_x(Align::Center)
    .padding(Inset::xy(Space::NONE, TOP))
    .fill_width()
    .transition(fade_scale())
    .build()
    .position(0.0, 0.0)
}

use std::time::Duration;

use rikka_format::format_duration;
use rikka_mpris::{Control, Player, Repeat};
use rikka_ui::{
    Align, Element, Justify, Label, Length, MaterialSymbol, Space, Styled, TextStyled, View,
    palette, theme,
};
use rikka_widgets::{ControlSize, Icon, button, container, icon, row, spacer};

use crate::widgets::kit::figures;
use crate::widgets::live::Live;

const SKIP: f32 = 22.0;
const SIDE: f32 = 18.0;
const DISC_SYMBOL: f32 = 0.56;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Deck {
    Compact,
    Full,
}

#[must_use]
pub(crate) fn controls(player: &Player, deck: Deck, disc: f32) -> Element {
    let abilities = player.abilities;
    let full = deck == Deck::Full && abilities.control;
    let gap = match deck {
        Deck::Compact => Space::HAIR,
        Deck::Full => Space::NORMAL,
    };
    row! {
        if full {
            shuffle(player);
            spacer().grow(1.0);
        }
        skip(player, MaterialSymbol::SKIP_PREVIOUS, abilities.previous, Control::Previous);
        play(player, disc);
        skip(player, MaterialSymbol::SKIP_NEXT, abilities.next, Control::Next);
        if full {
            spacer().grow(1.0);
            repeat(player);
        }
    }
    .gap(gap)
    .justify(Justify::Center)
    .align_items(Align::Center)
    .fill_width()
    .build()
}

#[must_use]
pub(crate) fn play(player: &Player, diameter: f32) -> Element {
    let (control, symbol) = match player.is_playing() {
        true => (Control::Pause, MaterialSymbol::PAUSE),
        false => (Control::Play, MaterialSymbol::PLAY_ARROW),
    };
    let able = player.abilities.play || player.abilities.pause;
    let theme = theme();
    let palette = theme.palette();
    let id = player.id.clone();
    let disc = container(
        icon(symbol)
            .extent(Length::px(diameter * DISC_SYMBOL))
            .filled()
            .color(theme.dim(palette.on_accent, !able)),
    )
    .center()
    .background(theme.dim(palette.accent, !able))
    .radius(diameter / 2.0)
    .width(Length::px(diameter))
    .height(Length::px(diameter));
    match able {
        true => disc
            .on_press(move |live: &mut Live| live.control(&id, control.clone()))
            .build(),
        false => disc.build(),
    }
}

fn shuffle(player: &Player) -> Option<Element> {
    let on = player.shuffle?;
    Some(toggle(
        player,
        MaterialSymbol::SHUFFLE,
        on,
        Control::Shuffle(!on),
    ))
}

fn repeat(player: &Player) -> Option<Element> {
    let (symbol, next) = match player.repeat? {
        Repeat::None => (MaterialSymbol::REPEAT, Repeat::Playlist),
        Repeat::Playlist => (MaterialSymbol::REPEAT, Repeat::Track),
        Repeat::Track => (MaterialSymbol::REPEAT_ONE, Repeat::None),
    };
    let on = player.repeat != Some(Repeat::None);
    Some(toggle(player, symbol, on, Control::Repeat(next)))
}

fn toggle(player: &Player, symbol: MaterialSymbol, on: bool, control: Control) -> Element {
    let tint = match on {
        true => palette().accent,
        false => palette().text_muted,
    };
    press(
        player,
        icon(symbol).extent(Length::px(SIDE)).color(tint),
        true,
        control,
    )
    .build()
}

fn skip(player: &Player, symbol: MaterialSymbol, enabled: bool, control: Control) -> impl View {
    press(
        player,
        icon(symbol).extent(Length::px(SKIP)).filled(),
        enabled,
        control,
    )
}

fn press(player: &Player, glyph: Icon, enabled: bool, control: Control) -> rikka_widgets::Button {
    let id = player.id.clone();
    button(glyph)
        .icon_only()
        .ghost()
        .control_size(ControlSize::Small)
        .enabled(enabled)
        .on_press(move |live: &mut Live| live.control(&id, control.clone()))
}

#[must_use]
pub(super) fn remaining(at: Duration, length: Duration) -> Label {
    Label::from(format!(
        "\u{2212}{}",
        format_duration(length.saturating_sub(at))
    ))
}

#[must_use]
pub(crate) fn times(at: Duration, length: Duration) -> Element {
    row! {
        figures(format_duration(at)).caption().muted().no_wrap();
        spacer().grow(1.0);
        figures(remaining(at, length)).caption().muted().no_wrap();
    }
    .align_items(Align::Center)
    .fill_width()
    .build()
}

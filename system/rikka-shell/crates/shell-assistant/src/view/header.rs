use rikka_types::Color;
use rikka_ui::{
    Align, Inset, IntoHandler, Label, Length, MaterialSymbol, Space, Styled, TextStyled, View,
    palette, text, tr,
};
use rikka_widgets::{ControlSize, button, container, icon, row, spacer};

use crate::panel::{Panel, Tab};

const GLYPH: f32 = 18.0;
const EDGE: f32 = 12.0;
pub(crate) const GAP: f32 = 4.0;

pub(super) fn view(panel: &Panel) -> impl View {
    let palette = palette();
    let tab = panel.tab();

    let line = row! {
        match tab {
            Tab::Conversation => icon(MaterialSymbol::AUTO_AWESOME)
                .extent(Length::px(GLYPH))
                .color(palette.accent)
                .build(),
            Tab::Threads | Tab::Servers => action(
                MaterialSymbol::ARROW_BACK,
                tr!("shell.assistant.back"),
                |panel: &mut Panel| panel.show(Tab::Conversation),
                palette.text_muted,
            )
            .build(),
        };
        text(title(panel))
            .caption()
            .medium()
            .color(palette.text)
            .no_wrap();
        spacer().grow(1.0);
        action(
            MaterialSymbol::ADD_COMMENT,
            tr!("shell.assistant.new"),
            Panel::begin,
            palette.text_muted,
        );
        toggle(tab, Tab::Threads, MaterialSymbol::HISTORY, tr!("shell.assistant.threads"));
        toggle(tab, Tab::Servers, MaterialSymbol::EXTENSION, tr!("shell.assistant.servers"));
        action(
            MaterialSymbol::CLOSE,
            tr!("shell.assistant.close"),
            Panel::close,
            palette.text_muted,
        )
    }
    .gap(Space::TIGHT)
    .align_items(Align::Center)
    .fill_width();

    container(line)
        .padding(Inset::edges(EDGE, EDGE, GAP, EDGE))
        .fill_width()
}

fn title(panel: &Panel) -> Label {
    let message = panel.shown().thread.title();
    match (panel.tab(), message.trim().is_empty()) {
        (Tab::Threads, _) => tr!("shell.assistant.threads"),
        (Tab::Servers, _) => tr!("shell.assistant.servers"),
        (Tab::Conversation, true) => tr!("shell.assistant.title"),
        (Tab::Conversation, false) => Label::from(message),
    }
}

fn toggle(tab: Tab, page: Tab, symbol: MaterialSymbol, named: Label) -> impl View {
    let palette = palette();
    let open = tab == page;
    let showing = match open {
        true => Tab::Conversation,
        false => page,
    };
    action(
        symbol,
        named,
        move |panel: &mut Panel| panel.show(showing),
        match open {
            true => palette.accent,
            false => palette.text_muted,
        },
    )
}

fn action<Marker>(
    symbol: MaterialSymbol,
    named: Label,
    press: impl IntoHandler<(), (), Marker>,
    colour: Color,
) -> impl View {
    button(icon(symbol).extent(Length::px(GLYPH)).color(colour))
        .icon_only()
        .ghost()
        .control_size(ControlSize::Small)
        .on_press(press)
        .tooltip(named)
}

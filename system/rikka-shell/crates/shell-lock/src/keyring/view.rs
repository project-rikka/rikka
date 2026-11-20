use rikka_secrets::KeyringState;
use rikka_theme::Corner;
use rikka_types::{Key, Modifiers};
use rikka_ui::{
    Align, Curve, DurationToken, Element, Justify, Label, Length, MaterialSymbol, Motion, Property,
    Space, Spring, Styled, TextStyled, View, fade, is_exiting, palette, scale, text, tr,
};
use rikka_widgets::{
    ControlSize, button, column, container, icon, row, spacer, spinner, text_field,
};
use shell_kit::glass_scrim;

use crate::keyring::dialog::{Dialog, Phase};

const WIDTH: f32 = 440.0;
const RADIUS: f32 = Corner::Window.radius();
const BORDER: f32 = 1.0;

const SYMBOL: f32 = 28.0;
const MARK: f32 = 16.0;

const ENTER_FROM: f32 = 0.93;

const ASKED: &str = "keyring.asked";
const ACTIONS: &str = "keyring.actions";

pub(super) const EXIT_DURATION: DurationToken = DurationToken::Short4;

pub(super) fn view(dialog: &Dialog) -> impl View {
    glass_scrim(
        container(
            column! {
                if !is_exiting() {
                    panel(dialog)
                        .width(WIDTH)
                        .max_width(Length::portion(0.92))
                        .transition(
                            (fade() + scale(ENTER_FROM))
                                .enter(Curve::Spring(Spring::DEFAULT_SPATIAL))
                                .exit(Motion::exit(EXIT_DURATION).curve(Property::Effects)),
                        );
                }
            }
            .align_items(Align::Center)
            .justify(Justify::Center)
            .fill(),
        )
        .on_chord(Key::Escape, Modifiers::NONE, Dialog::dismiss)
        .fill(),
    )
}

fn panel(dialog: &Dialog) -> Element {
    let palette = palette();
    let panel = column! {
        heading(dialog);
        match dialog.phase() {
            Phase::Resetting => resetting().build().keyed(ASKED),
            _ => asking(dialog).build().keyed(ASKED),
        }
        actions(dialog).build().keyed(ACTIONS)
    }
    .gap(Space::ROOMY)
    .padding(Space::LOOSE)
    .fill_width();

    container(panel)
        .background(palette.surface)
        .border(BORDER, palette.outline)
        .radius(RADIUS)
        .glass()
        .resizing()
        .build()
}

fn heading(dialog: &Dialog) -> impl View {
    row! {
        icon(MaterialSymbol::KEY)
            .extent(Length::px(SYMBOL))
            .color(palette().accent);
        column! {
            text(title(dialog.state())).heading().medium();
            text(message(dialog.state())).label().muted()
        }
        .gap(Space::TIGHT)
        .grow(1.0)
    }
    .gap(Space::ROOMY)
    .align_items(Align::Start)
}

fn asking(dialog: &Dialog) -> impl View {
    column! {
        text_field(dialog.typed())
            .secret(true)
            .placeholder(placeholder(dialog.state()))
            .control_size(ControlSize::Large)
            .autofocus(true)
            .enabled(dialog.phase() != Phase::Checking)
            .on_change(|| {})
            .on_submit(Dialog::submit);
        if let Some(trouble) = trouble(dialog.phase()) {
            trouble;
        }
    }
    .gap(Space::TIGHT)
}

fn trouble(phase: Phase) -> Option<Element> {
    let palette = palette();
    let (symbol, color, said) = match phase {
        Phase::Checking => {
            return Some(
                row! {
                    spinner().extent(Length::px(MARK)).color(palette.text_muted);
                    text(tr!("shell.keyring.checking")).caption().muted()
                }
                .gap(Space::TIGHT)
                .align_items(Align::Center)
                .build(),
            );
        }
        Phase::Refused => (
            MaterialSymbol::ERROR,
            palette.danger,
            tr!("shell.keyring.refused"),
        ),
        Phase::Broken => (
            MaterialSymbol::ERROR,
            palette.danger,
            tr!("shell.keyring.broken"),
        ),
        Phase::Asking | Phase::Resetting => return None,
    };
    Some(
        row! {
            icon(symbol).extent(Length::px(MARK)).color(color);
            text(said).caption().color(color).grow(1.0)
        }
        .gap(Space::TIGHT)
        .align_items(Align::Start)
        .build(),
    )
}

fn resetting() -> impl View {
    let palette = palette();
    row! {
        icon(MaterialSymbol::WARNING)
            .extent(Length::px(MARK))
            .color(palette.danger);
        text(tr!("shell.keyring.reset-warning"))
            .caption()
            .color(palette.danger)
            .grow(1.0)
    }
    .gap(Space::TIGHT)
    .align_items(Align::Start)
}

fn actions(dialog: &Dialog) -> Element {
    if dialog.phase() == Phase::Resetting {
        return row! {
            spacer().grow(1.0);
            button(tr!("shell.keyring.back")).ghost().on_press(Dialog::keep);
            button(tr!("shell.keyring.start-over")).danger().on_press(Dialog::reset)
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .build();
    }
    row! {
        if dialog.state() == Some(KeyringState::Refused) {
            button(tr!("shell.keyring.forgot"))
                .ghost()
                .on_press(Dialog::ask_to_reset);
        }
        spacer().grow(1.0);
        button(tr!("shell.keyring.cancel"))
            .ghost()
            .on_press(Dialog::dismiss);
        button(tr!("shell.keyring.unlock"))
            .primary()
            .enabled(dialog.phase() != Phase::Checking)
            .on_press(Dialog::submit)
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .build()
}

fn title(state: Option<KeyringState>) -> Label {
    match state {
        Some(KeyringState::Absent) => tr!("shell.keyring.create-title"),
        _ => tr!("shell.keyring.title"),
    }
}

fn message(state: Option<KeyringState>) -> Label {
    match state {
        Some(KeyringState::Absent) => tr!("shell.keyring.create"),
        Some(KeyringState::Refused) => tr!("shell.keyring.changed"),
        _ => tr!("shell.keyring.locked"),
    }
}

fn placeholder(state: Option<KeyringState>) -> Label {
    match state {
        Some(KeyringState::Refused) => tr!("shell.keyring.previous-password"),
        _ => tr!("shell.keyring.login-password"),
    }
}

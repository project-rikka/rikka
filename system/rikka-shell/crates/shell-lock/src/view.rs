use jiff::tz::TimeZone;
use rikka_dirs::passwd::Entry;
use rikka_power::Action;
use rikka_types::{Color, CornerRadii};
use rikka_ui::{
    Align, Element, Family, Inset, Justify, Label, Length, MaterialSymbol, Motion, Space, Spring,
    Styled, TextStyled, View, font, text, tr,
};
use rikka_widgets::{
    ControlSize, ImageFit, button, column, column_of, container, icon, image, row, spacer, spinner,
    text_field,
};
use shell_feeds::{dates, power_symbol};
use shell_kit::{Period, scrim};

use crate::backdrop::Backdrop;
use crate::gate::{Gate, Notice, PamQuestion};
use crate::guard::Guard;

const INK: Color = Color::rgba(1.0, 1.0, 1.0, 0.88);
const TYPED_INK: Color = Color::rgba(1.0, 1.0, 1.0, 1.0);
const SOFT_INK: Color = Color::rgba(1.0, 1.0, 1.0, 0.72);
const WARNING_INK: Color = Color::rgba(1.0, 0.78, 0.74, 1.0);
const GLASS: Color = Color::rgba(1.0, 1.0, 1.0, 0.14);
const GLASS_EDGE: Color = Color::rgba(1.0, 1.0, 1.0, 0.22);
const RING: Color = Color::rgba(1.0, 1.0, 1.0, 0.32);

const DAY: &str = "%A, %-d %B";

const TOP: f32 = 0.11;
const CLOCK: f32 = 128.0;
const CLOCK_LINE: f32 = 136.0;
const DATE: f32 = 26.0;
const BETWEEN: f32 = 0.16;

const FACE: f32 = 96.0;
const FACE_RING: f32 = 3.0;
const FIELD_WIDTH: f32 = 280.0;
const FIELD_HEIGHT: f32 = 46.0;
const FIELD_TEXT: f32 = 15.0;
const SUBMIT: f32 = 20.0;
const NOTE: f32 = 14.0;

const POWER_ICON: f32 = 20.0;

const SHAKE: f32 = 18.0;
const SHAKING: Spring = Spring::new(0.16, 2600.0);

pub(crate) fn view(guard: &Guard) -> Element {
    let backdrop = Backdrop {
        picture: guard.media().map(|media| media.picture()),
    };
    let scene = column_of([backdrop.build(), content(guard)])
        .width(Length::FILL)
        .height(Length::FILL);
    scrim(scene, Guard::pressed).build()
}

fn content(guard: &Guard) -> Element {
    let gate = guard.gate();
    column! {
        spacer().height(Length::portion(TOP));
        clock(guard);
        spacer().height(Length::portion(BETWEEN));
        sign_in(guard, gate);
        spacer().grow(1.0);
        footer(guard);
    }
    .align_items(Align::Center)
    .padding(Space::LOOSE)
    .width(Length::FILL)
    .height(Length::FILL)
    .position(0.0, 0.0)
    .build()
}

fn clock(guard: &Guard) -> Element {
    let now = guard
        .wall()
        .now(Period::Minute)
        .to_zoned(TimeZone::system());
    let day = now.strftime(DAY).to_string();
    column! {
        text(dates::time(now.hour(), now.minute()))
            .family(Family::parse(font::INTER_DISPLAY))
            .text_size(CLOCK)
            .line_height(CLOCK_LINE)
            .bold()
            .color(INK)
            .no_wrap();
        text(Label::from(day))
            .family(Family::parse(font::INTER_DISPLAY))
            .text_size(DATE)
            .semibold()
            .color(INK)
            .no_wrap();
    }
    .align_items(Align::Center)
    .build()
}

fn sign_in(guard: &Guard, gate: &Gate) -> Element {
    column! {
        face(guard, gate);
        field(guard, gate);
        notes(guard, gate);
    }
    .gap(Space::LOOSE)
    .align_items(Align::Center)
    .build()
}

fn face(guard: &Guard, gate: &Gate) -> Element {
    let round = CornerRadii::uniform(FACE / 2.0);
    let shown = match guard.face() {
        Some(source) => image(source.clone())
            .fit(ImageFit::Cover)
            .radius(round)
            .width(Length::px(FACE))
            .height(Length::px(FACE))
            .build(),
        None => container(
            text(Label::from(
                gate.account().map(Entry::initial).unwrap_or_default(),
            ))
            .family(Family::parse(font::INTER_DISPLAY))
            .text_size(FACE * 0.42)
            .semibold()
            .color(INK),
        )
        .center()
        .background(GLASS)
        .radius(round)
        .width(Length::px(FACE))
        .height(Length::px(FACE))
        .build(),
    };
    container(shown)
        .padding(Inset::all(FACE_RING))
        .background(RING)
        .radius(CornerRadii::uniform(FACE / 2.0 + FACE_RING))
        .build()
}

fn field(guard: &Guard, gate: &Gate) -> Element {
    let question = gate.question();
    let trailing = match gate.is_checking() {
        true => spinner().extent(Length::px(SUBMIT)).color(SOFT_INK).build(),
        false => button(
            icon(MaterialSymbol::ARROW_FORWARD)
                .extent(Length::px(SUBMIT))
                .color(INK),
        )
        .icon_only()
        .ghost()
        .control_size(ControlSize::Small)
        .on_press(Guard::submit)
        .build(),
    };
    let typing = text_field(gate.typed())
        .plain(true)
        .secret(!question.as_ref().is_some_and(|question| question.echo))
        .font_size(FIELD_TEXT)
        .placeholder(placeholder(question.as_ref()))
        .autofocus(true)
        .claims_focus(true)
        .on_submit(Guard::submit)
        .on_key(Guard::key)
        .grow(1.0)
        .foreground(TYPED_INK)
        .keyed(("lock.field", guard.focus()));
    let shake = match guard.is_shaking() {
        true => SHAKE,
        false => 0.0,
    };
    container(
        row! {
            typing;
            trailing;
        }
        .gap(Space::TIGHT)
        .align_items(Align::Center)
        .fill(),
    )
    .background(GLASS)
    .border(1.0, GLASS_EDGE)
    .radius(CornerRadii::uniform(FIELD_HEIGHT / 2.0))
    .padding(Inset::exact(0.0, Space::SNUG.px(), 0.0, Space::ROOMY.px()))
    .width(Length::px(FIELD_WIDTH))
    .height(Length::px(FIELD_HEIGHT))
    .offset(shake, 0.0)
    .animate(Motion::CustomSpring(SHAKING))
}

fn placeholder(question: Option<&PamQuestion>) -> Label {
    let label = question
        .map(|question| question.text.trim().trim_end_matches([':', '：']).trim())
        .filter(|label| !label.is_empty());
    match label {
        Some(label) => Label::from(label.to_owned()),
        None => tr!("shell.lock.password"),
    }
}

fn notes(guard: &Guard, gate: &Gate) -> Element {
    let typing = guard.keyboard().typing();
    let layout = typing
        .has_multiple_layouts()
        .then(|| typing.layout.clone())
        .flatten();
    let notice = gate.notice().map(|notice| match notice {
        Notice::Refused => (tr!("shell.lock.refused"), WARNING_INK),
        Notice::Said { error, text } => (
            Label::from(text),
            match error {
                true => WARNING_INK,
                false => SOFT_INK,
            },
        ),
        Notice::Broken => (tr!("shell.lock.broken"), WARNING_INK),
    });
    column! {
        if let Some((label, ink)) = notice {
            text(label).text_size(NOTE).medium().color(ink).no_wrap();
        }
        if typing.caps_lock {
            note(MaterialSymbol::KEYBOARD_CAPSLOCK, tr!("shell.lock.caps-lock"));
        }
        if let Some(layout) = layout {
            note(MaterialSymbol::KEYBOARD, Label::from(layout));
        }
    }
    .gap(Space::TIGHT)
    .align_items(Align::Center)
    .build()
}

fn note(symbol: MaterialSymbol, label: Label) -> Element {
    row! {
        icon(symbol).extent(Length::px(NOTE + 2.0)).color(SOFT_INK);
        text(label).text_size(NOTE).color(SOFT_INK).no_wrap();
    }
    .gap(Space::TIGHT)
    .align_items(Align::Center)
    .build()
}

fn footer(guard: &Guard) -> Element {
    row! {
        for action in Action::ALL.into_iter().filter(|action| guard.offers(*action)) {
            power(action);
        }
    }
    .gap(Space::ROOMY)
    .justify(Justify::End)
    .align_items(Align::End)
    .fill_width()
    .build()
}

fn power(action: Action) -> Element {
    let label = match action {
        Action::PowerOff => tr!("shell.lock.power-off"),
        Action::Reboot => tr!("shell.lock.reboot"),
        Action::Suspend => tr!("shell.lock.suspend"),
    };
    column! {
        container(
            button(icon(power_symbol(action)).extent(Length::px(POWER_ICON)).color(INK))
                .icon_only()
                .ghost()
                .control_size(ControlSize::Large)
                .on_press(move |guard: &mut Guard| guard.ask(action)),
        )
        .background(GLASS)
        .border(1.0, GLASS_EDGE)
        .radius(CornerRadii::uniform(ControlSize::Large.height() / 2.0));
        text(label).caption().medium().color(SOFT_INK).no_wrap();
    }
    .gap(Space::TIGHT)
    .align_items(Align::Center)
    .build()
}

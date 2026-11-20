use rikka_polkit::{Ask, NoticeKind, Question, Reply, User};
use rikka_theme::Corner;
use rikka_types::{Key, Modifiers};
use rikka_ui::{
    Align, Curve, DurationToken, Element, Justify, Label, Length, MaterialSymbol, Motion, Property,
    Space, Spring, Styled, TextStyled, View, fade, is_exiting, palette, scale, text, tr,
};
use rikka_widgets::{
    ControlSize, button, column, combo, container, icon, row, spacer, spinner, text_field,
};
use shell_kit::glass_scrim;

use crate::dialog::Dialog;

const WIDTH: f32 = 440.0;
const RADIUS: f32 = Corner::Window.radius();
const BORDER: f32 = 1.0;
const PADDING: f32 = 24.0;

const SYMBOL: f32 = 28.0;
const MARK: f32 = 16.0;

const ENTER_FROM: f32 = 0.93;

const ASKED: &str = "polkit.asked";
const TROUBLE: &str = "polkit.trouble";
const DETAILS: &str = "polkit.details";
const ACTIONS: &str = "polkit.actions";

pub(crate) const EXIT_DURATION: DurationToken = DurationToken::Short4;

pub(crate) fn view(dialog: &Dialog) -> impl View {
    let screen = match dialog.asking() {
        None => column! { spacer() },
        Some(ask) => column! {
            if !is_exiting() {
                panel(dialog, ask)
                    .width(WIDTH)
                    .max_width(Length::portion(0.92))
                    .transition(
                        (fade() + scale(ENTER_FROM))
                            .enter(Curve::Spring(Spring::DEFAULT_SPATIAL))
                            .exit(Motion::exit(EXIT_DURATION).curve(Property::Effects)),
                    );
            }
        },
    };

    glass_scrim(
        container(
            screen
                .align_items(Align::Center)
                .justify(Justify::Center)
                .fill(),
        )
        .on_chord(Key::Escape, Modifiers::NONE, |dialog: &mut Dialog| {
            dialog.answer(Reply::Dismiss);
        })
        .fill(),
    )
}

fn panel(dialog: &Dialog, ask: &Ask) -> Element {
    let palette = palette();
    let panel = column! {
        heading(ask);
        who(ask);
        match &ask.question {
            Some(question) => question_view(dialog, question).build().keyed(ASKED),
            None => working().build().keyed(ASKED),
        }
        if let Some(trouble) = trouble(ask) {
            trouble.build().keyed(TROUBLE);
        }
        details(dialog, ask).build().keyed(DETAILS);
        actions(ask).build().keyed(ACTIONS)
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

fn heading(ask: &Ask) -> impl View {
    row! {
        icon(MaterialSymbol::ENCRYPTED)
            .extent(Length::px(SYMBOL))
            .color(palette().accent);
        column! {
            text(tr!("shell.polkit.title")).heading().medium();
            text(message(ask)).label().muted()
        }
        .gap(Space::TIGHT)
        .grow(1.0)
    }
    .gap(Space::ROOMY)
    .align_items(Align::Start)
}

fn who(ask: &Ask) -> Element {
    let mark = icon(MaterialSymbol::ACCOUNT_CIRCLE)
        .extent(Length::px(MARK))
        .color(palette().text_muted);

    if ask.users.len() > 1 {
        return row! {
            mark;
            combo(ask.chosen)
                .options(ask.users.iter().map(whom))
                .control_size(ControlSize::Medium)
                .enabled(!ask.waiting())
                .on_select(|dialog: &mut Dialog, user| dialog.answer(Reply::Choose(user)))
                .grow(1.0)
        }
        .gap(Space::TIGHT)
        .align_items(Align::Center)
        .build();
    }

    let Some(user) = ask.user() else {
        return spacer().build();
    };
    row! {
        mark;
        text(whom(user)).label().muted()
    }
    .gap(Space::TIGHT)
    .align_items(Align::Center)
    .build()
}

fn question_view(dialog: &Dialog, question: &Question) -> impl View {
    column! {
        text(question.text.trim_end())
            .caption()
            .muted();
        text_field(dialog.typed())
            .secret(!question.echo)
            .control_size(ControlSize::Large)
            .autofocus(true)
            .on_change(|| {})
            .on_submit(Dialog::submit)
    }
    .gap(Space::TIGHT)
}

fn working() -> impl View {
    row! {
        spinner().extent(Length::px(MARK)).color(palette().text_muted);
        text(tr!("shell.polkit.checking")).label().muted()
    }
    .gap(Space::TIGHT)
    .align_items(Align::Center)
}

fn trouble(ask: &Ask) -> Option<impl View> {
    let palette = palette();
    let (symbol, color, shown_text) = match &ask.notice {
        Some(notice) => {
            let (symbol, color) = match notice.kind {
                NoticeKind::Error => (MaterialSymbol::ERROR, palette.danger),
                NoticeKind::Info => (MaterialSymbol::INFO, palette.text_muted),
            };
            (symbol, color, Label::from(notice.text.as_str()))
        }
        None if ask.failures > 0 => (
            MaterialSymbol::ERROR,
            palette.danger,
            tr!("shell.polkit.failed"),
        ),
        None => return None,
    };
    Some(
        row! {
            icon(symbol).extent(Length::px(MARK)).color(color);
            text(shown_text).caption().color(color).grow(1.0)
        }
        .gap(Space::TIGHT)
        .align_items(Align::Start),
    )
}

fn details(dialog: &Dialog, ask: &Ask) -> impl View {
    let palette = palette();
    let shown = dialog.shows_details();
    let chevron = if shown {
        MaterialSymbol::EXPAND_LESS
    } else {
        MaterialSymbol::EXPAND_MORE
    };

    column! {
        button(
            row! {
                text(tr!("shell.polkit.details")).caption();
                icon(chevron).extent(Length::px(MARK))
            }
            .gap(Space::TIGHT)
            .align_items(Align::Center)
        )
        .ghost()
        .small()
        .on_press(move |dialog: &mut Dialog| dialog.show_details(!shown))
        .align_self(Align::Start);
        if shown {
            container(
                column! {
                    detail(tr!("shell.polkit.action"), Label::from(ask.action.as_str()));
                    for (key, value) in &ask.details {
                        detail(Label::from(key.as_str()), Label::from(value.as_str()));
                    }
                }
                .gap(Space::TIGHT),
            )
            .background(palette.surface_variant)
            .radius(RADIUS - PADDING / 2.0)
            .padding(Space::NORMAL)
            .fill_width();
        }
    }
    .gap(Space::TIGHT)
}

fn detail(key: Label, value: Label) -> impl View {
    row! {
        text(key).caption().muted();
        text(value).caption().grow(1.0)
    }
    .gap(Space::TIGHT)
    .align_items(Align::Start)
}

fn actions(ask: &Ask) -> impl View {
    row! {
        spacer().grow(1.0);
        button(tr!("shell.polkit.cancel"))
            .ghost()
            .on_press(|dialog: &mut Dialog| dialog.answer(Reply::Dismiss));
        button(tr!("shell.polkit.authenticate"))
            .primary()
            .enabled(!ask.waiting())
            .on_press(Dialog::submit)
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
}

fn message(ask: &Ask) -> Label {
    match ask.message.trim() {
        "" => tr!("shell.polkit.needed"),
        message => Label::from(message),
    }
}

fn whom(user: &User) -> Label {
    match &user.full_name {
        Some(full_name) => tr!(
            "shell.polkit.user",
            name = full_name.as_str(),
            account = user.name.as_str()
        ),
        None => Label::from(user.name.as_str()),
    }
}

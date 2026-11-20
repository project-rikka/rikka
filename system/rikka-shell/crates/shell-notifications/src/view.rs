use rikka_theme::Corner;
use rikka_ui::{
    Align, Element, Inset, Justify, Space, Styled, TextStyled, View, fade, palette, text, tr,
};
use rikka_widgets::{column, container, dismissible, row};

use crate::card;
use crate::toast::Toast;
use crate::toasts::Toasts;

const WIDTH: f32 = 380.0;

const MARGIN: f32 = 12.0;

const OVERSHOOT: f32 = 16.0;

pub(super) const SPAN: f32 = OVERSHOOT + WIDTH + MARGIN;

const RADIUS: f32 = Corner::Panel.radius();
const BORDER: f32 = 1.0;
const INSET: f32 = 14.0;

const ICON: f32 = 40.0;

pub(crate) fn view(toasts: &Toasts) -> Element {
    toasts.stack().read(pile)
}

fn pile(shown: &[Toast], waiting: usize) -> Element {
    let pile = column! {
        for toast in shown {
            slot(toast);
        }
        if waiting > 0 {
            queued(waiting).transition(fade());
        }
    }
    .width(WIDTH);
    container(pile)
        .padding(Inset::edges(0.0, MARGIN, 0.0, OVERSHOOT))
        .on_hover(|toasts: &mut Toasts, inside: bool| toasts.hover(inside))
        .align_self(Align::Start)
        .build()
}

fn slot(toast: &Toast) -> Element {
    let id = toast.id();
    dismissible(
        container(card(toast))
            .padding(Inset::edges(0.0, 0.0, Space::SNUG, 0.0))
            .fill_width(),
    )
    .arriving(true)
    .dismissed(toast.leaving)
    .on_dismiss(move |toasts: &mut Toasts| toasts.put_away(id))
    .fill_width()
    .build()
    .keyed(id)
}

fn card(toast: &Toast) -> Element {
    let id = toast.id();
    card::card(
        &toast.notification,
        &toast.look,
        move |toasts: &mut Toasts| toasts.activate(id),
    )
    .sized(ICON, RADIUS, INSET)
    .glass()
    .closed(move |toasts: &mut Toasts| toasts.dismiss(id))
    .acted(move |toasts: &mut Toasts, key: String| toasts.act(id, &key))
    .followed(move |toasts: &mut Toasts, at: usize| toasts.follow(id, at))
    .build()
    .fill_width()
}

fn queued(waiting: usize) -> impl View {
    let palette = palette();
    let pill = container(
        text(tr!("shell.notifications.waiting", count = waiting))
            .caption()
            .muted()
            .no_wrap(),
    )
    .padding(Inset::xy(INSET, 5.0))
    .radius(RADIUS)
    .background(palette.surface)
    .border(BORDER, palette.outline);

    row! {
        pill
    }
    .justify(Justify::Center)
    .fill_width()
}

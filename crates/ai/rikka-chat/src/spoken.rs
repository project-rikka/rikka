use std::hash::{Hash, Hasher};

use rikka_thread::Attachment;

use crate::amendment::Amendment;
use crate::attachment;
use crate::chat::{AmendActions, Chat};
use crate::prelude::*;
use crate::transcript::Transcript;
use rikka_widgets::{column, row};

const SAID: f32 = 78.0;
const GLYPH: f32 = 16.0;
const LINE: f32 = 21.0;
const SHORTEST: f32 = 42.0;
const TALLEST: f32 = 320.0;
const NAMED: usize = 3;
const AMENDING: &str = "chat-amending";

#[derive(Clone, Debug, Default)]
pub(crate) struct Amend {
    pub(crate) allowed: bool,
    pub(crate) editing: Option<Amendment>,
}

impl Amend {
    pub(crate) fn editing(&self, stamp: u64) -> Option<&Amendment> {
        self.editing
            .as_ref()
            .filter(|amending| amending.stamp() == stamp)
    }

    pub(crate) fn mark(&self, stamp: u64, hasher: &mut impl Hasher) {
        self.allowed.hash(hasher);
        if let Some(amending) = self.editing(stamp) {
            amending.paths().hash(hasher);
            amending.restoring().hash(hasher);
        }
    }
}

pub(crate) fn person<C: Chat>(bubble: Element, text: &str, stamp: u64, allowed: bool) -> Element {
    let text = text.to_owned();
    local_state(false, move |hovered: bool, set: Setter<bool>| {
        let actions = match hovered {
            true => actions::<C>(&text, stamp, allowed)
                .transition(fade().enter(Curve::Spring(Spring::DEFAULT_SPATIAL)))
                .any(),
            false => spacer().any(),
        };
        container(
            column! {
                row! {
                    spacer().grow(1.0);
                    container(bubble).max_width(Length::portion(SAID / 100.0));
                }
                .fill_width();
                row! {
                    spacer().grow(1.0);
                    actions;
                }
                .align_items(Align::Center)
                .height(ControlSize::Small.height())
                .fill_width();
            }
            .gap(Space::HAIR)
            .fill_width(),
        )
        .on_hover(move |inside: bool| {
            set(inside);
            skip_view();
        })
        .fill_width()
    })
}

fn actions<C: Chat>(text: &str, stamp: u64, allowed: bool) -> Flex {
    let edit = C::amend().filter(|_| allowed).map(|amends| amends.edit);
    row! {
        copy_button(text);
        if let Some(edit) = edit {
            tool(MaterialSymbol::EDIT).on_press(edit.map(move |()| (stamp,)));
        }
    }
    .gap(Space::HAIR)
    .align_items(Align::Center)
}

fn tool(symbol: MaterialSymbol) -> Button {
    button(
        icon(symbol)
            .extent(Length::px(GLYPH))
            .color(palette().text_muted),
    )
    .icon_only()
    .ghost()
    .small()
}

pub(crate) fn editor<C: Chat>(
    amending: &Amendment,
    amends: AmendActions,
    attached: &[Attachment],
    ready: bool,
) -> Element {
    let draft = amending.draft().clone();
    let AmendActions {
        resend,
        cancel,
        send,
        restores,
        ..
    } = amends;
    let editor = text_editor(&draft)
        .autofocus(true)
        .gutter(false)
        .family(Family::parse("sans-serif"))
        .font_size(14.0)
        .line_height(LINE)
        .fits_text(SHORTEST..=TALLEST)
        .on_key(
            |transcript: &mut Transcript, key: Key, modifiers: Modifiers| {
                let leaving = key == Key::Escape && modifiers == Modifiers::NONE;
                if leaving {
                    transcript.stop_amending();
                }
                leaving
            },
        )
        .on_submit(resend.clone());

    container(
        column! {
            if !attached.is_empty() {
                attachment::settled::<C>(attached);
            }
            editor.fill_width().build().keyed((AMENDING, amending.stamp()));
            if !amending.paths().is_empty() {
                restoring(amending, restores, ready);
            }
            row! {
                spacer().grow(1.0);
                button(text(cancel))
                    .ghost()
                    .small()
                    .enabled(ready)
                    .on_press(|transcript: &mut Transcript| transcript.stop_amending());
                button(text(send)).primary().small().enabled(ready).on_press(resend);
            }
            .gap(Space::SNUG)
            .align_items(Align::Center)
            .fill_width();
        }
        .gap(Space::SNUG)
        .fill_width(),
    )
    .surface()
    .radius(Corner::Panel)
    .border(1.0, palette().accent)
    .padding(Inset::xy(14.0, 10.0))
    .fill_width()
    .any()
}

fn restoring(amending: &Amendment, restores: fn(usize) -> Label, ready: bool) -> impl View {
    let paths = amending.paths();
    let mut named: Vec<&str> = paths.iter().take(NAMED).map(String::as_str).collect();
    let rest = paths.len().saturating_sub(NAMED);
    let more = format!("+{rest}");
    if rest > 0 {
        named.push(&more);
    }
    let listed = named.join(", ");
    column! {
        checkbox(amending.restoring())
            .label(text(restores(paths.len())).label())
            .control_size(ControlSize::Small)
            .enabled(ready)
            .on_toggle(|transcript: &mut Transcript, _: bool| transcript.toggle_restore());
        text(listed).caption().muted().no_wrap();
    }
    .gap(Space::HAIR)
    .fill_width()
}

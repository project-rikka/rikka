use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use rikka_symbols::MaterialSymbol;
use rikka_ui::text::{TextStyled, text};
use rikka_ui::{
    Align, Element, Length, Setter, Space, View, after_keyed, clipboard, local_state, palette,
    skip_view, tr,
};

use crate::blocks::flex::row_of;
use crate::controls::button::button;
use crate::media::icon::icon;

const CONFIRMED: Duration = Duration::from_millis(1600);
const GLYPH: f32 = 16.0;
const TIMER: &str = "copy-button";

static COPIES: AtomicU64 = AtomicU64::new(0);

#[must_use]
pub struct CopyButton {
    text: Arc<str>,
    labeled: bool,
}

pub fn copy_button(text: impl Into<Arc<str>>) -> CopyButton {
    CopyButton {
        text: text.into(),
        labeled: false,
    }
}

impl CopyButton {
    pub fn labeled(mut self) -> Self {
        self.labeled = true;
        self
    }
}

#[derive(Clone, Copy)]
struct Confirmation {
    shown: bool,
    key: u64,
}

impl View for CopyButton {
    fn build(self) -> Element {
        let CopyButton {
            text: copied,
            labeled,
        } = self;
        let idle = Confirmation {
            shown: false,
            key: COPIES.fetch_add(1, Ordering::Relaxed),
        };
        local_state(
            idle,
            move |confirmation: Confirmation, set: Setter<Confirmation>| {
                let (symbol, label) = match confirmation.shown {
                    true => (MaterialSymbol::CHECK, tr!("widgets.copy.copied")),
                    false => (MaterialSymbol::CONTENT_COPY, tr!("widgets.copy.copy")),
                };
                let ink = palette().text_muted;
                let glyph = icon(symbol).extent(Length::px(GLYPH)).color(ink);
                let press = move || confirm(&copied, confirmation, &set);
                match labeled {
                    true => button(
                        row_of([glyph.build(), text(label).color(ink).no_wrap().build()])
                            .gap(Space::TIGHT)
                            .align_items(Align::Center),
                    )
                    .ghost()
                    .small()
                    .on_press(press)
                    .build(),
                    false => button(glyph)
                        .icon_only()
                        .ghost()
                        .small()
                        .on_press(press)
                        .tooltip(label),
                }
            },
        )
    }
}

fn confirm(copied: &str, confirmation: Confirmation, set: &Setter<Confirmation>) {
    clipboard::set_text(copied);
    set(Confirmation {
        shown: true,
        ..confirmation
    });
    let unset = Rc::clone(set);
    after_keyed((TIMER, confirmation.key), CONFIRMED, move || {
        unset(Confirmation {
            shown: false,
            ..confirmation
        });
        skip_view();
    });
    skip_view();
}

impl std::fmt::Debug for CopyButton {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CopyButton")
            .field("bytes", &self.text.len())
            .field("labeled", &self.labeled)
            .finish_non_exhaustive()
    }
}

use rikka_app::prelude::*;
use rikka_widgets::{column, row};

use super::Session;
use crate::paint::Paint;
use crate::tab::Tab;
use crate::view::SIDE_PANEL;

const GLYPH: f32 = 20.0;

pub(crate) fn panel(tab: &Tab, session: &Session) -> Element {
    let Some(context) = tab.context() else {
        return spacer().any();
    };
    let dialog = session.dialog();
    let refusal = session.refusal(&context);
    let ready = refusal.is_none() && dialog.outcome(&context).is_some();
    let content = column! {
        row! {
            icon(MaterialSymbol::TUNE).extent(Length::px(GLYPH)).color(palette().accent);
            text(dialog.title()).bold().grow(1.0);
        }
        .gap(Space::SNUG)
        .centered()
        .padding(Space::NORMAL);
        rule();
        scrollable(
            container(dialog.form(&context).keyed(session.resets()))
                .padding(Space::NORMAL)
                .fill_width(),
        )
        .grow(1.0)
        .fill_width();
        if let Some(refusal) = refusal {
            row! {
                icon(MaterialSymbol::WARNING)
                    .extent(Length::px(GLYPH))
                    .color(palette().danger);
                text(refusal).caption().grow(1.0).min_width(0.0);
            }
            .gap(Space::SNUG)
            .centered()
            .padding(Space::NORMAL)
            .fill_width();
        }
        rule();
        column! {
            if session.previews() {
                checkbox(session.is_previewing())
                    .label(text(tr!("paint.session.preview")))
                    .on_toggle(|paint: &mut Paint, on: bool| paint.preview_session(on));
            }
            row! {
                button(text(tr!("paint.session.reset")))
                    .ghost()
                    .on_press(Paint::reset_session);
                spacer().grow(1.0);
                button(text(tr!("paint.session.cancel"))).on_press(Paint::cancel_session);
                button(text(dialog.confirm()))
                    .primary()
                    .enabled(ready)
                    .on_press(Paint::apply_session);
            }
            .gap(Space::SNUG)
            .centered()
            .fill_width();
        }
        .gap(Space::SNUG)
        .padding(Space::NORMAL)
        .fill_width();
    }
    .fill();
    container(content)
        .background(palette().surface)
        .width(SIDE_PANEL)
        .fill_height()
        .any()
}

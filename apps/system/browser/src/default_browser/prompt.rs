use rikka_app::prelude::*;
use rikka_widgets::row;

use crate::default_browser::{Attempt, failure_text};
use crate::shell::Shell;
use crate::widgets::glyph_button;

const GLYPH: f32 = 18.0;

pub(crate) fn view(shell: &Shell) -> Element {
    if !shell
        .browser()
        .services()
        .default_browser
        .prompted(shell.id())
    {
        return Element::empty();
    }
    let attempt = shell.claim.attempt();
    let (symbol, tint, message) = match &attempt {
        Attempt::Failed(failure) => (
            MaterialSymbol::ERROR,
            palette().danger,
            failure_text(failure),
        ),
        Attempt::Idle | Attempt::Making => (
            MaterialSymbol::OPEN_IN_BROWSER,
            palette().accent,
            tr!("browser.default.prompt"),
        ),
    };
    container(
        row! {
            icon(symbol).extent(Length::px(GLYPH)).color(tint);
            text(message).label().grow(1.0);
            button(text(tr!("browser.default.make")))
                .primary()
                .small()
                .enabled(attempt != Attempt::Making)
                .on_press(|shell: &mut Shell| shell.make_default_browser());
            button(text(tr!("browser.default.never-ask")))
                .ghost()
                .small()
                .on_press(|shell: &mut Shell| shell.stop_asking_default());
            glyph_button(MaterialSymbol::CLOSE, GLYPH)
                .on_press(|shell: &mut Shell| shell.dismiss_default_prompt());
        }
        .gap(Space::SNUG)
        .align_items(Align::Center),
    )
    .padding(Inset::xy(Space::NORMAL, Space::SNUG))
    .background(palette().surface)
    .fill_width()
    .any()
}

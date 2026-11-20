use music_core::Fidelity;
use rikka_app::prelude::*;
use rikka_widgets::row;

use crate::wording;

pub(crate) fn badge(fidelity: Fidelity) -> Option<Container> {
    let (message, tint) = match (fidelity.is_high(), fidelity.lossless) {
        (true, _) => (tr!("music.fidelity.high"), palette().accent),
        (false, true) => (tr!("music.fidelity.lossless"), palette().text_muted),
        (false, false) => return None,
    };
    Some(
        container(text(message).caption().semibold().color(tint).no_wrap())
            .background(tint.with_alpha(0.14))
            .radius(Corner::Item)
            .padding(Inset::xy(6.0, 1.0)),
    )
}

pub(crate) fn high(fidelity: Option<Fidelity>) -> Option<Container> {
    fidelity.filter(Fidelity::is_high).and_then(badge)
}

pub(crate) fn fidelity_label(fidelity: Fidelity) -> impl View {
    row! {
        badge(fidelity).map(|badge| badge.shrink(0.0));
        text(wording::fidelity(fidelity))
            .caption()
            .muted()
            .no_wrap()
            .min_width(0.0);
    }
    .gap(Space::TIGHT)
    .align_items(Align::Center)
    .min_width(0.0)
}

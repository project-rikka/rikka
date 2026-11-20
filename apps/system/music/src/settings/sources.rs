use rikka_app::prelude::*;
use rikka_widgets::row;

use crate::marks::mark_of;
use crate::music::Music;
use crate::wording::service;

const SOURCE: f32 = 48.0;

pub(super) fn view(music: &Music) -> impl View {
    let order = music.order();
    let last = order.len().saturating_sub(1);
    card(
        column_of(order.iter().enumerate().map(|(at, slug)| {
            let name = music.providers().find(slug).map_or_else(
                || Label::from(slug.clone()),
                |source| service(&source.service()),
            );
            let (earlier, later) = (slug.clone(), slug.clone());
            row! {
                text((at + 1).to_string()).label().muted().width(20.0);
                icon(mark_of(music.providers(), slug)).extent(Length::px(20.0)).color(palette().accent);
                text(name).label().medium().grow(1.0).min_width(0.0).no_wrap();
                button(icon(MaterialSymbol::ARROW_UPWARD).extent(Length::px(18.0)))
                    .ghost()
                    .icon_only()
                    .small()
                    .enabled(at > 0)
                    .on_press(move |music: &mut Music| music.move_service(&earlier, true));
                button(icon(MaterialSymbol::ARROW_DOWNWARD).extent(Length::px(18.0)))
                    .ghost()
                    .icon_only()
                    .small()
                    .enabled(at < last)
                    .on_press(move |music: &mut Music| music.move_service(&later, false));
            }
            .gap(Space::NORMAL)
            .align_items(Align::Center)
            .height(SOURCE)
            .fill_width()
            .build()
        }))
        .gap(Space::HAIR)
        .fill_width(),
    )
}

use rikka_app::prelude::*;

const HEIGHT: f32 = 16.0;

pub(crate) fn badge(count: u32) -> impl View {
    let shown = match count {
        0..=99 => count.to_string(),
        _ => "99+".to_owned(),
    };
    container(
        text(shown)
            .text_size(Length::px(11.0))
            .bold()
            .no_wrap()
            .color(palette().on_danger),
    )
    .center()
    .padding(Inset::xy(Space::TIGHT, Space::NONE))
    .background(palette().danger)
    .radius(HEIGHT / 2.0)
    .min_width(HEIGHT)
    .height(HEIGHT)
}

pub(crate) fn live_badge() -> Container {
    container(
        text(tr!("discord.voice.live"))
            .caption()
            .bold()
            .no_wrap()
            .color(palette().on_danger),
    )
    .padding(Inset::xy(Space::TIGHT, Space::NONE))
    .background(palette().danger)
    .radius(Corner::Detail)
}

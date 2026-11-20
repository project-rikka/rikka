use rikka_app::prelude::*;

pub const LABEL_WIDTH: f32 = 96.0;
const GLYPH: f32 = 16.0;

pub fn heading(label: Label) -> Element {
    text(label).bold().no_wrap().any()
}

pub fn choice(label: Label, on: bool) -> Button {
    let base = button(text(label).caption().no_wrap()).small().focusless();
    match on {
        true => base.secondary(),
        false => base.ghost(),
    }
}

pub fn toggle<M>(label: Label, on: bool, set: impl IntoHandler<(bool,), (), M>) -> impl View {
    row_of([
        text(label).caption().no_wrap().grow(1.0).any(),
        switch(on)
            .control_size(ControlSize::Small)
            .on_toggle(set)
            .any(),
    ])
    .gap(Space::TIGHT)
    .align_items(Align::Center)
    .fill_width()
}

pub fn framed(parts: Vec<Element>) -> Element {
    container(column_of(parts).gap(Space::TIGHT).fill_width())
        .background(palette().surface_variant)
        .radius(Corner::Item)
        .padding(Space::SNUG)
        .fill_width()
        .any()
}

pub fn labelled(label: Label, control: impl View, trailing: Option<Element>) -> Element {
    let mut parts = vec![
        text(label).caption().no_wrap().width(LABEL_WIDTH).any(),
        container(control).grow(1.0).any(),
    ];
    parts.extend(trailing);
    row_of(parts)
        .gap(Space::TIGHT)
        .align_items(Align::Center)
        .fill_width()
        .any()
}

pub fn glyph(symbol: MaterialSymbol) -> Button {
    button(icon(symbol).extent(Length::px(GLYPH)))
        .icon_only()
        .small()
        .focusless()
}

pub fn key_controls<K, C>(
    here: bool,
    keyed: bool,
    key: impl IntoHandler<(), (), K>,
    clear: impl IntoHandler<(), (), C>,
) -> Element {
    let key = glyph(MaterialSymbol::DIAMOND).on_press(key);
    let key = match (here, keyed) {
        (true, _) => key.primary(),
        (false, true) => key.secondary(),
        (false, false) => key.ghost(),
    };
    let reset = glyph(MaterialSymbol::RESTART_ALT)
        .ghost()
        .enabled(keyed)
        .on_press(clear)
        .tooltip(tr!("animate.tip.clear_keys"));
    row_of([key.tooltip(tr!("animate.tip.add_key")), reset])
        .gap(Space::NONE)
        .any()
}

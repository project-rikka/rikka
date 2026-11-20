use rikka_keyframe::{Ease, Easing, Family};
use rikka_ui::{Align, Element, Label, Space, Styled, TextAlign, TextStyled, View, text, tr};
use rikka_widgets::{column_of, row_of, spacer};

use crate::tile::TILE_WIDTH;

const NAME: f32 = 64.0;

#[derive(Debug)]
pub struct EasingRow {
    pub label: Label,
    pub cells: [Element; 3],
}

impl EasingRow {
    #[must_use]
    pub fn family(family: Family, cell: impl Fn(Easing) -> Element) -> EasingRow {
        EasingRow {
            label: family_label(family),
            cells: Ease::ALL.map(|ease| cell(Easing::new(family, ease))),
        }
    }
}

#[must_use]
pub fn easing_grid(rows: impl IntoIterator<Item = EasingRow>) -> Element {
    let mut heading = vec![spacer().width(NAME).build()];
    heading.extend(Ease::ALL.map(|ease| {
        text(ease_label(ease))
            .caption()
            .muted()
            .no_wrap()
            .align(TextAlign::Center)
            .width(TILE_WIDTH)
            .build()
    }));
    let mut lines = vec![row_of(heading).gap(Space::TIGHT).build()];
    lines.extend(rows.into_iter().map(|row| {
        let mut cells = vec![text(row.label).caption().no_wrap().width(NAME).build()];
        cells.extend(row.cells);
        row_of(cells)
            .gap(Space::TIGHT)
            .align_items(Align::Center)
            .build()
    }));
    column_of(lines).gap(Space::TIGHT).build()
}

#[must_use]
pub fn family_label(family: Family) -> Label {
    match family {
        Family::Linear => tr!("timeline.easing.linear"),
        Family::Sine => tr!("timeline.easing.sine"),
        Family::Quad => tr!("timeline.easing.quad"),
        Family::Cubic => tr!("timeline.easing.cubic"),
        Family::Quart => tr!("timeline.easing.quart"),
        Family::Quint => tr!("timeline.easing.quint"),
        Family::Expo => tr!("timeline.easing.expo"),
        Family::Circ => tr!("timeline.easing.circ"),
        Family::Back => tr!("timeline.easing.back"),
        Family::Elastic => tr!("timeline.easing.elastic"),
        Family::Bounce => tr!("timeline.easing.bounce"),
    }
}

#[must_use]
pub fn ease_label(ease: Ease) -> Label {
    match ease {
        Ease::In => tr!("timeline.easing.in"),
        Ease::Out => tr!("timeline.easing.out"),
        Ease::InOut => tr!("timeline.easing.in_out"),
    }
}

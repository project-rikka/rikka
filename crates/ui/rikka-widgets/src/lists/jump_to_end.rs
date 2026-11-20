use std::hash::Hash;

use rikka_symbols::MaterialSymbol;
use rikka_ui::text::{TextStyled, text};
use rikka_ui::{
    Action, Anchor, AnchorEdge, Element, Handler, Inset, IntoHandler, Length, Place, Setter, Space,
    Style, Styled, View, fade, hash_of, local_state, palette, scale, skip_view,
};

use crate::blocks::container::container;
use crate::blocks::flex::column_of;
use crate::blocks::live::live_observed;
use crate::blocks::spacer::spacer;
use crate::media::icon::icon;

const FACE: f32 = 40.0;
const GLYPH: f32 = 24.0;
const PILL: f32 = 16.0;
const COUNT: f32 = 11.0;
const MOST: usize = 99;
const SHRUNK: f32 = 0.6;

#[must_use]
pub struct JumpToEnd {
    content: Element,
    place: Place,
    unseen: usize,
    more_beyond: bool,
    press: Action,
    above: Option<Anchor>,
    style: Style,
}

pub fn jump_to_end(content: impl View, place: &Place) -> JumpToEnd {
    JumpToEnd {
        content: content.build(),
        place: place.clone(),
        unseen: 0,
        more_beyond: false,
        press: Action::default(),
        above: None,
        style: Style::default(),
    }
}

impl JumpToEnd {
    pub fn unseen(mut self, count: usize) -> Self {
        self.unseen = count;
        self
    }

    pub fn more_beyond(mut self, more: bool) -> Self {
        self.more_beyond = more;
        self
    }

    pub fn on_press<Marker>(mut self, press: impl IntoHandler<(), (), Marker>) -> Self {
        self.press = Action::new(press);
        self
    }

    pub fn above(mut self, key: impl Hash) -> Self {
        self.above = Some(Anchor::new(key, AnchorEdge::AboveEnd).offset(0.0, Space::ROOMY.px()));
        self
    }
}

impl View for JumpToEnd {
    fn build(self) -> Element {
        let JumpToEnd {
            content,
            place,
            unseen,
            more_beyond,
            press,
            above,
            style,
        } = self;
        let press = press.handler().cloned().unwrap_or_else(|| {
            let place = place.clone();
            Handler::new(move || {
                place.end();
                skip_view();
            })
        });
        let mut layers = vec![content];
        let anchor = match above {
            Some(anchor) => anchor,
            None => {
                let end = hash_of(("rikka-jump-to-end", &place));
                layers.push(spacer().height(0.0).fill_width().build().keyed(end));
                Anchor::new(end, AnchorEdge::AboveEnd).offset(-Space::ROOMY.px(), Space::ROOMY.px())
            }
        };
        layers.push(
            live_observed(move || {
                (more_beyond || place.is_scrolled_away())
                    .then(|| face(unseen, press.clone()).anchored(anchor))
            })
            .build(),
        );
        column_of(layers).styled(style).build()
    }
}

fn face(unseen: usize, press: Handler) -> Element {
    local_state(false, move |hovered: bool, set: Setter<bool>| {
        let button = container(
            icon(MaterialSymbol::EXPAND_MORE)
                .extent(Length::px(GLYPH))
                .color(palette().text),
        )
        .center()
        .background(match hovered {
            true => palette().surface_variant,
            false => palette().surface,
        })
        .border(1.0, palette().outline)
        .radius(FACE / 2.0)
        .size(Length::px(FACE), Length::px(FACE))
        .on_press(press.clone())
        .on_hover(move |inside: bool| {
            set(inside);
            skip_view();
        })
        .focusless()
        .build();
        let mut layers = vec![button];
        if unseen > 0 {
            layers.push(
                container(count(unseen))
                    .position(FACE - Space::ROOMY.px(), -Space::TIGHT.px())
                    .build(),
            );
        }
        column_of(layers).size(Length::px(FACE), Length::px(FACE))
    })
    .transition(fade() + scale(SHRUNK))
}

fn count(unseen: usize) -> impl View {
    let label = match unseen {
        0..=MOST => unseen.to_string(),
        _ => format!("{MOST}+"),
    };
    container(
        text(label)
            .text_size(Length::px(COUNT))
            .bold()
            .no_wrap()
            .color(palette().on_danger),
    )
    .center()
    .padding(Inset::xy(Space::TIGHT, Space::NONE))
    .background(palette().danger)
    .radius(PILL / 2.0)
    .min_width(PILL)
    .height(PILL)
}

impl std::fmt::Debug for JumpToEnd {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JumpToEnd")
            .field("unseen", &self.unseen)
            .field("more_beyond", &self.more_beyond)
            .finish_non_exhaustive()
    }
}

impl Styled for JumpToEnd {
    fn style_mut(&mut self) -> &mut Style {
        &mut self.style
    }
}

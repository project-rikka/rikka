use rikka_theme::Corner;
use rikka_types::{Key, Modifiers, Point};
use rikka_ui::{
    Align, Anchor, AnchorEdge, Curve, DurationToken, Element, Handler, IntoHandler, Motion,
    Property, Space, Spring, Styled, View, fade, offset, palette, scale,
};
use rikka_widgets::{column, column_of, container};
use shell_kit::{CURSOR, cursor, sheet};

pub(crate) const EXIT_DURATION: DurationToken = DurationToken::Short4;

pub(super) const ROW_RADIUS: f32 = Corner::Window.nested(PADDING);
pub(super) const ROW_INSET: f32 = 10.0;
pub(super) const ROW_DEPTH: f32 = 6.0;

pub(crate) const DETAIL: f32 = 11.0;

const GAP: f32 = 8.0;

const BORDER: f32 = 1.0;
const PADDING: f32 = 6.0;

const DROP: f32 = 10.0;
const ENTER_FROM: f32 = 0.96;

const ARRIVING: Curve = Curve::Spring(Spring::DEFAULT_SPATIAL);

pub(crate) struct DropdownPanel {
    from: Point,
    width: f32,
    rows: Vec<Element>,
    on_key: Handler<(Key, Modifiers), bool>,
    beside: Vec<Element>,
}

pub(super) fn dropdown_panel<Marker>(
    from: Point,
    width: f32,
    rows: Vec<Element>,
    on_key: impl IntoHandler<(Key, Modifiers), bool, Marker>,
) -> DropdownPanel {
    DropdownPanel {
        from,
        width,
        rows,
        on_key: on_key.into_handler(),
        beside: Vec::new(),
    }
}

impl DropdownPanel {
    #[must_use]
    pub(crate) fn beside(mut self, layers: Vec<Element>) -> Self {
        self.beside = layers;
        self
    }
}

impl View for DropdownPanel {
    fn build(self) -> Element {
        let palette = palette();
        let card = container(
            column_of(self.rows)
                .gap(Space::HAIR)
                .padding(PADDING)
                .align_items(Align::Stretch)
                .fill_width(),
        )
        .background(palette.surface)
        .border(BORDER, palette.outline)
        .radius(Corner::Window)
        .glass()
        .resizing();
        column! {
            cursor(self.from);
            sheet(card, self.on_key)
                .build()
                .width(self.width)
                .transition((scale(ENTER_FROM) + offset(0.0, -DROP)).enter(ARRIVING))
                .anchored(Anchor::new(CURSOR, AnchorEdge::BelowEnd).offset(0.0, GAP));
            for layer in self.beside {
                layer;
            }
        }
        .fill()
        .transition(
            fade()
                .enter(ARRIVING)
                .exit(Motion::exit(EXIT_DURATION).curve(Property::Effects)),
        )
        .build()
    }
}

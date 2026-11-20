use rikka_app::prelude::*;
use rikka_ui::{Accept, Action};

use crate::gestures::inert;
use crate::widgets::row_drag;
use browser_model::Arrangement;
use browser_model::Row;

const EDGE: f32 = 0.25;
const INSET: f32 = 6.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Side {
    Left,
    Right,
    Top,
    Bottom,
}

impl Side {
    pub(crate) fn placement(self) -> (Arrangement, bool) {
        match self {
            Side::Left => (Arrangement::SideBySide, false),
            Side::Right => (Arrangement::SideBySide, true),
            Side::Top => (Arrangement::Stacked, false),
            Side::Bottom => (Arrangement::Stacked, true),
        }
    }

    pub(crate) fn at(bounds: Rect, point: Point) -> Option<Side> {
        let (width, height) = (bounds.width(), bounds.height());
        if width <= 0.0 || height <= 0.0 {
            return None;
        }
        [
            (Side::Left, (point.x - bounds.min_x()) / width),
            (Side::Right, (bounds.max_x() - point.x) / width),
            (Side::Top, (point.y - bounds.min_y()) / height),
            (Side::Bottom, (bounds.max_y() - point.y) / height),
        ]
        .into_iter()
        .filter(|(_, reach)| (0.0..EDGE).contains(reach))
        .min_by(|(_, near), (_, far)| near.total_cmp(far))
        .map(|(side, _)| side)
    }
}

pub(crate) fn edges<M, H>(
    content: impl View,
    hover: impl IntoHandler<(Option<Side>,), (), H>,
    land: impl IntoHandler<(Row, Side), (), M>,
) -> Element {
    Element::with_children(
        Edges {
            hover: Action::new(hover),
            land: Action::new(land),
        },
        vec![content.build()],
    )
}

pub(crate) fn highlight(side: Side) -> Element {
    let half = Length::portion(0.5);
    let mark = container(spacer())
        .background(palette().accent.with_alpha(0.16))
        .border(2.0, palette().accent.with_alpha(0.7))
        .radius(Corner::Panel);
    let mark = match side {
        Side::Left | Side::Right => mark.width(half).fill_height(),
        Side::Top | Side::Bottom => mark.height(half).fill_width(),
    };
    let placed = match side {
        Side::Left => row_of([mark.any()]).justify(Justify::Start).fill().any(),
        Side::Right => row_of([mark.any()]).justify(Justify::End).fill().any(),
        Side::Top => column_of([mark.any()]).justify(Justify::Start).fill().any(),
        Side::Bottom => column_of([mark.any()]).justify(Justify::End).fill().any(),
    };
    inert(container(placed).padding(Inset::all(INSET)).fill()).transition(fade())
}

struct Edges {
    hover: Action<(Option<Side>,)>,
    land: Action<(Row, Side)>,
}

#[derive(Debug, Default)]
struct EdgesState {
    shown: Option<Side>,
}

impl Edges {
    fn show(&self, state: &mut EdgesState, side: Option<Side>, ctx: &mut EventCtx<'_>) {
        if state.shown != side {
            state.shown = side;
            ctx.act(&self.hover, (side,));
        }
    }
}

impl Widget for Edges {
    type State = EdgesState;

    fn layout(&self) -> Layout {
        Layout::column().size(Length::FILL, Length::FILL)
    }

    fn behavior(&self, _: &EdgesState) -> Behavior {
        Behavior::CATCHES_DROPS
    }

    fn event(&self, state: &mut EdgesState, event: &Event, ctx: &mut EventCtx<'_>) {
        match event {
            Event::DragMoved { position, offer } if row_drag::offered(offer) => {
                let side = Side::at(ctx.bounds(), *position);
                if side.is_some() {
                    ctx.accept_drag(Accept::moving(row_drag::mime()));
                    ctx.set_handled();
                }
                self.show(state, side, ctx);
            }
            Event::DragLeft => self.show(state, None, ctx),
            Event::DragDropped {
                position, offer, ..
            } => {
                self.show(state, None, ctx);
                let Some(side) = Side::at(ctx.bounds(), *position) else {
                    return;
                };
                let Some(row) = row_drag::read(offer) else {
                    return;
                };
                ctx.set_handled();
                ctx.act(&self.land, (row, side));
            }
            _ => {}
        }
    }

    fn same(&self, other: &Self) -> bool {
        self.hover == other.hover && self.land == other.land
    }

    fn debug_name(&self) -> &'static str {
        "SplitEdges"
    }
}

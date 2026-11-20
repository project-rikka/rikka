use rikka_theme::caption;
use rikka_types::{CursorShape, Point, PointerButton, Rect, ResizeEdge};
use rikka_ui::{Event, EventCtx, Layout, Length, PaintCtx, Widget, WindowPart, WindowRequest};

const BORDER: f32 = caption::RESIZE_BORDER;

const CORNER: f32 = caption::RESIZE_CORNER;

#[derive(Widget)]
pub(crate) struct ResizeEdges {
    #[debug]
    enabled: bool,
}

pub(crate) fn resize_edges(enabled: bool) -> ResizeEdges {
    ResizeEdges { enabled }
}

impl Widget for ResizeEdges {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::row()
            .size(Length::FILL, Length::FILL)
            .placed_at(Point::ZERO)
    }

    fn paint(&self, _state: &mut (), _ctx: &mut PaintCtx<'_>) {}

    fn hit_test(&self, bounds: Rect, point: Point) -> bool {
        self.enabled && edge_at(bounds, point).is_some()
    }

    fn cursor(&self, _state: &Self::State, bounds: Rect, point: Point) -> Option<CursorShape> {
        edge_at(bounds, point).map(CursorShape::Resize)
    }

    fn window_part(&self, bounds: Rect, point: Point) -> Option<WindowPart> {
        edge_at(bounds, point)
            .filter(|_| self.enabled)
            .map(WindowPart::Resize)
    }

    fn event(&self, _state: &mut (), event: &Event, ctx: &mut EventCtx<'_>) {
        let Event::PointerPressed {
            button: PointerButton::Primary,
            position,
            ..
        } = event
        else {
            return;
        };
        let Some(edge) = edge_at(ctx.bounds(), *position) else {
            return;
        };
        ctx.keep_focus();
        ctx.set_handled();
        ctx.request_window(WindowRequest::Resize(edge));
    }
}

#[derive(Clone, Copy)]
enum Horizontal {
    Left,
    Right,
}

#[derive(Clone, Copy)]
enum Vertical {
    Top,
    Bottom,
}

fn near<T>(a: f32, b: f32, reach: f32, low: T, high: T) -> Option<T> {
    if a <= reach {
        Some(low)
    } else if b <= reach {
        Some(high)
    } else {
        None
    }
}

fn edge_at(bounds: Rect, point: Point) -> Option<ResizeEdge> {
    let left = point.x - bounds.min_x();
    let right = bounds.max_x() - point.x;
    let top = point.y - bounds.min_y();
    let bottom = bounds.max_y() - point.y;
    if left < 0.0 || right < 0.0 || top < 0.0 || bottom < 0.0 {
        return None;
    }

    let x = near(left, right, BORDER, Horizontal::Left, Horizontal::Right);
    let y = near(top, bottom, BORDER, Vertical::Top, Vertical::Bottom);
    if x.is_none() && y.is_none() {
        return None;
    }
    let x = x.or_else(|| near(left, right, CORNER, Horizontal::Left, Horizontal::Right));
    let y = y.or_else(|| near(top, bottom, CORNER, Vertical::Top, Vertical::Bottom));

    Some(match (x, y) {
        (Some(Horizontal::Left), Some(Vertical::Top)) => ResizeEdge::TopLeft,
        (Some(Horizontal::Left), Some(Vertical::Bottom)) => ResizeEdge::BottomLeft,
        (Some(Horizontal::Right), Some(Vertical::Top)) => ResizeEdge::TopRight,
        (Some(Horizontal::Right), Some(Vertical::Bottom)) => ResizeEdge::BottomRight,
        (Some(Horizontal::Left), None) => ResizeEdge::Left,
        (Some(Horizontal::Right), None) => ResizeEdge::Right,
        (None, Some(Vertical::Top)) => ResizeEdge::Top,
        (None, Some(Vertical::Bottom)) => ResizeEdge::Bottom,
        (None, None) => return None,
    })
}

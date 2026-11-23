use rikka_types::{CornerRadii, Rect, RoundedRect};
use rikka_ui::{
    Align, Element, Inset, Layout, Length, PaintCtx, Quad, Space, Styled, View, Widget, theme,
};
use rikka_widgets::{row, spacer};
use shell_kit::taskbar::{BORDER, CORNER, DEPTH};

use crate::island;
use crate::state::Bar;
use crate::strip::parts;

const PADDING: f32 = 12.0;

#[must_use]
pub(crate) fn view(bar: &Bar) -> Element {
    let strip = row! {
        parts::logo();
        parts::workspaces(bar);
        spacer().grow(1.0);
        parts::cluster(bar);
    }
    .gap(Space::TIGHT)
    .align_items(Align::Center)
    .fill_width()
    .height(Length::px(DEPTH))
    .padding(Inset::xy(PADDING, 0.0))
    .build();
    Element::with_children(Strip, vec![strip, island::island(bar.island())])
}

fn hang(bounds: Rect) -> Rect {
    Rect::new(
        bounds.x() + BORDER,
        bounds.y() + DEPTH,
        (bounds.width() - BORDER * 2.0).max(0.0),
        CORNER,
    )
}

fn desktop_below(hang: Rect) -> RoundedRect {
    RoundedRect::new(
        Rect::new(hang.x(), hang.y(), hang.width(), CORNER * 2.0),
        CornerRadii::new(CORNER, CORNER, 0.0, 0.0),
    )
}

struct Strip;

impl Widget for Strip {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::column().size(Length::FILL, Length::FILL)
    }

    fn paint(&self, _state: &mut (), ctx: &mut PaintCtx<'_>) {
        let bounds = ctx.bounds();
        if bounds.is_empty() {
            return;
        }
        let fill = theme().palette().background;
        let band = Rect::new(bounds.x(), bounds.y(), bounds.width(), DEPTH);
        let hang = hang(bounds);
        ctx.draw_quad(Quad::new(band).fill(fill));
        ctx.draw_quad(Quad::new(hang).fill(fill).cut_out(desktop_below(hang)));
        ctx.glass(band);
    }

    fn debug_name(&self) -> &'static str {
        "TaskbarStrip"
    }
}

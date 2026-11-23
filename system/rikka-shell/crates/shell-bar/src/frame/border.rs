use rikka_types::{CornerRadii, Rect, RoundedRect};
use rikka_ui::{Layout, Length, PaintCtx, Quad, Widget, theme};
use shell_kit::taskbar::{BORDER, CORNER};

use crate::frame::Side;

pub(super) struct Border(pub(super) Side);

impl Widget for Border {
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
        match self.0 {
            Side::Left | Side::Right => {
                ctx.draw_quad(Quad::new(bounds).fill(fill));
                ctx.glass(bounds);
            }
            Side::Bottom => {
                let band = Rect::new(bounds.x(), bounds.max_y() - BORDER, bounds.width(), BORDER);
                let hang = Rect::new(
                    bounds.x() + BORDER,
                    band.y() - CORNER,
                    (bounds.width() - BORDER * 2.0).max(0.0),
                    CORNER,
                );
                ctx.draw_quad(Quad::new(band).fill(fill));
                ctx.draw_quad(Quad::new(hang).fill(fill).cut_out(desktop_above(hang)));
                ctx.glass(band);
            }
        }
    }

    fn debug_name(&self) -> &'static str {
        "TaskbarFrame"
    }
}

fn desktop_above(hang: Rect) -> RoundedRect {
    RoundedRect::new(
        Rect::new(hang.x(), hang.y() - CORNER, hang.width(), CORNER * 2.0),
        CornerRadii::new(0.0, 0.0, CORNER, CORNER),
    )
}

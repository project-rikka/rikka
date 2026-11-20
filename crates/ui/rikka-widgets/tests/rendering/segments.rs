use rikka_types::{Color, Point};
use rikka_ui::*;

use crate::common::{pixel, settled_pixels};

const VIEW: u32 = 128;

#[derive(State, Default)]
struct Holder {}

#[derive(Debug)]
struct Diagonal;

impl Widget for Diagonal {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::column().size(Length::FILL, Length::FILL)
    }

    fn paint(&self, _state: &mut (), ctx: &mut PaintCtx<'_>) {
        let origin = ctx.bounds().origin;
        ctx.draw_quad(Quad::new(ctx.bounds()).fill(Color::rgb(0.0, 0.0, 0.0)));
        ctx.draw_segment(
            origin + Point::new(24.0, 24.0),
            origin + Point::new(104.0, 104.0),
            10.0,
            Color::rgb(1.0, 1.0, 1.0),
        );
    }
}

fn view(_: &Holder) -> Element {
    Element::new(Diagonal)
}

#[test]
fn segments_cover_their_turned_capsule() {
    let pixels = settled_pixels(Holder::default(), view, VIEW);
    let at = |x: usize, y: usize| pixel(&pixels, VIEW as usize, x, y);
    let covered = [(64, 64), (24, 24), (104, 104), (66, 62)];
    let bare = [
        (64, 24),
        (24, 104),
        (104, 24),
        (14, 14),
        (114, 114),
        (74, 54),
    ];
    for (x, y) in covered {
        assert!(
            at(x, y).0 > 200,
            "({x}, {y}) is on the segment: {:?}",
            at(x, y)
        );
    }
    for (x, y) in bare {
        assert!(
            at(x, y).0 < 30,
            "({x}, {y}) is off the segment: {:?}",
            at(x, y)
        );
    }
}

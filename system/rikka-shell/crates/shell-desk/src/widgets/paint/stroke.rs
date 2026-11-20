use std::sync::{Arc, LazyLock};

use rikka_types::{Color, Point, Rect, Size};
use rikka_ui::{ImageSource, PaintCtx, Sprite};

static WHITE: LazyLock<Arc<[u8]>> = LazyLock::new(|| Arc::from([u8::MAX; 4].as_slice()));

pub(crate) fn stroke(ctx: &mut PaintCtx<'_>, from: Point, to: Point, width: f32, color: Color) {
    let (across, down) = (to.x - from.x, to.y - from.y);
    let middle = Point::new((from.x + to.x) / 2.0, (from.y + to.y) / 2.0);
    let rect = Rect::from_center(middle, Size::new(across.hypot(down) + width, width));
    let white = ImageSource::Rgba {
        width: 1,
        height: 1,
        pixels: Arc::clone(&WHITE),
    };
    ctx.draw_sprite(
        Sprite::new(rect, white)
            .radius(width / 2.0)
            .turned(down.atan2(across))
            .tinted(color),
    );
}

use rikka_theme::Corner;
use rikka_types::{CornerRadii, Rect, RoundedRect, Size};
use rikka_ui::{ImageSource, PaintCtx};
use shell_kit::taskbar::Shape;

pub const CARD_RADIUS: f32 = 22.0;

pub const THUMB_RADIUS: f32 = Corner::Item.radius();

pub const RING_WIDTH: f32 = 2.0;

#[must_use]
pub fn below_taskbar(bounds: Rect, taskbar: Option<Shape>) -> RoundedRect {
    let strip = taskbar
        .map_or(0.0, Shape::reserved)
        .clamp(0.0, bounds.height());
    let rect = Rect::new(
        bounds.min_x(),
        bounds.min_y() + strip,
        bounds.width(),
        bounds.height() - strip,
    );
    let cut = match strip > 0.0 {
        true => taskbar.map_or(0.0, Shape::corner),
        false => 0.0,
    };
    RoundedRect::new(rect, CornerRadii::new(cut, cut, 0.0, 0.0))
}

#[must_use]
pub fn lerp_rect(from: Rect, to: Rect, progress: f32) -> Rect {
    Rect::new(
        lerp(from.x(), to.x(), progress),
        lerp(from.y(), to.y(), progress),
        lerp(from.width(), to.width(), progress),
        lerp(from.height(), to.height(), progress),
    )
}

#[must_use]
pub fn lerp(from: f32, to: f32, progress: f32) -> f32 {
    from + (to - from) * progress
}

#[must_use]
pub fn aspect(size: Size) -> f32 {
    match size.width >= 1.0 && size.height >= 1.0 {
        true => size.width / size.height,
        false => 16.0 / 9.0,
    }
}

pub fn draw_fitted_image(ctx: &mut PaintCtx<'_>, rect: Rect, source: &ImageSource) {
    let fitted = crate::FIT.place(rect, source.natural_size().unwrap_or(rect.size));
    ctx.draw_image(fitted, source.clone());
}

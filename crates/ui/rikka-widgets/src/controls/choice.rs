use rikka_types::{Rect, RoundedRect};
use rikka_ui::{Align, Inset, Layout, Length, PaintCtx, Style};

use crate::control::ControlSize;
use crate::controls::press::Press;
use crate::controls::ripple::Ripple;

pub(crate) const ROW_PADDING: f32 = 8.0;

const ROW_PADDING_Y: f32 = 6.0;

pub(crate) fn layout(size: ControlSize, props: &Style) -> Layout {
    Layout::row()
        .align_items(Align::Center)
        .padding(Inset::edges(
            ROW_PADDING_Y,
            ROW_PADDING,
            ROW_PADDING_Y,
            ROW_PADDING + size.indicator() + size.gap(),
        ))
        .min_height(Length::px(size.height()))
        .with(props)
}

pub(crate) fn paint_row(
    ctx: &mut PaintCtx<'_>,
    ripple: &mut Ripple,
    press: &Press,
    size: ControlSize,
    interactive: bool,
) -> Rect {
    let bounds = ctx.bounds();
    let side = size.indicator();
    let indicator = Rect::new(
        bounds.x() + ROW_PADDING,
        bounds.center().y - side / 2.0,
        side,
        side,
    );

    let target = size.height().min(side + ROW_PADDING * 2.0);
    ripple.paint(
        ctx,
        press,
        RoundedRect::circle(indicator.center(), target / 2.0),
        interactive,
    );
    indicator
}

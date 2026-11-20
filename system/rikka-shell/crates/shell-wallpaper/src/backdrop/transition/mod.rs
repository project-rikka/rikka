mod circle;

pub(crate) use circle::Circle;
use rikka_types::{Rect, RoundedRect};
use rikka_ui::{Curve, DurationToken, Easing, PaintCtx};

use crate::Picture;

pub const CROSSING: DurationToken = DurationToken::ExtraLong4;

pub const CROSSING_CURVE: Curve = Curve::Tween {
    easing: Easing::Standard,
    duration: CROSSING,
};

pub(crate) trait Transition: std::fmt::Debug + Send + Sync + 'static {
    fn duration(&self) -> DurationToken {
        CROSSING
    }

    fn curve(&self) -> Curve {
        Curve::Tween {
            easing: Easing::Standard,
            duration: self.duration(),
        }
    }

    fn paint(&self, progress: f32, stage: &mut Stage<'_, '_>);
}

#[derive(Debug)]
pub(crate) struct Stage<'a, 'ctx> {
    ctx: &'a mut PaintCtx<'ctx>,
    bounds: Rect,
    outgoing: Option<&'a Picture>,
    incoming: &'a Picture,
}

impl<'a, 'ctx> Stage<'a, 'ctx> {
    pub(crate) fn new(
        ctx: &'a mut PaintCtx<'ctx>,
        bounds: Rect,
        outgoing: Option<&'a Picture>,
        incoming: &'a Picture,
    ) -> Self {
        Self {
            ctx,
            bounds,
            outgoing,
            incoming,
        }
    }

    pub(crate) fn bounds(&self) -> Rect {
        self.bounds
    }

    pub(crate) fn outgoing(&mut self) -> Draw<'_, 'ctx> {
        Draw::new(self.ctx, self.bounds, self.outgoing)
    }

    pub(crate) fn incoming(&mut self) -> Draw<'_, 'ctx> {
        Draw::new(self.ctx, self.bounds, Some(self.incoming))
    }
}

#[derive(Debug)]
#[must_use = "the wallpaper is painted by `draw`"]
pub(crate) struct Draw<'a, 'ctx> {
    ctx: &'a mut PaintCtx<'ctx>,
    bounds: Rect,
    picture: Option<&'a Picture>,
    through: Option<RoundedRect>,
}

impl<'a, 'ctx> Draw<'a, 'ctx> {
    fn new(ctx: &'a mut PaintCtx<'ctx>, bounds: Rect, picture: Option<&'a Picture>) -> Self {
        Self {
            ctx,
            bounds,
            picture,
            through: None,
        }
    }

    pub(crate) fn through(mut self, shape: impl Into<RoundedRect>) -> Self {
        self.through = Some(shape.into());
        self
    }

    pub(crate) fn draw(self) {
        let Some(picture) = self.picture else {
            return;
        };
        let content = picture.place(self.bounds, crate::FIT);
        let source = picture.source();
        let shape = self
            .through
            .unwrap_or_else(|| RoundedRect::rect(self.bounds));
        self.ctx
            .clipped(shape, |ctx| ctx.draw_image(content, source));
    }
}

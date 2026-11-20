use rikka_app::prelude::*;
use rikka_figure::Grips;
use rikka_figure_editing::{paint_band, paint_grips, paint_loop};

use super::{Canvas, CanvasState};
use crate::figures::grips_of;

const MEMBER_ALPHA: f32 = 0.6;

impl Canvas {
    pub(super) fn paint_figures(
        &self,
        state: &CanvasState,
        ctx: &mut PaintCtx<'_>,
        at: &dyn Fn(f32, f32) -> Point,
    ) {
        if !self.tool.picks() {
            return;
        }
        let place = |point: Point| at(point.x, point.y);
        if let Some(band) = state.band {
            paint_band(ctx, place(band.from), place(band.to));
        }
        let (figures, grips) = match state
            .manipulating
            .as_ref()
            .and_then(|manipulating| manipulating.reached.as_ref())
        {
            Some(reached) => (reached.figures.clone(), Some(reached.grips)),
            None => {
                let figures = self.shown_picks();
                let grips = grips_of(&figures);
                (figures, grips)
            }
        };
        let Some(grips) = grips else {
            return;
        };
        if figures.len() > 1 {
            let tint = palette().accent.scale_alpha(MEMBER_ALPHA);
            for figure in &figures {
                let corners: Vec<Point> = Grips::for_forms(&[&figure.form])
                    .map(|member| member.outline())
                    .unwrap_or_default()
                    .into_iter()
                    .map(place)
                    .collect();
                paint_loop(ctx, &corners, tint);
            }
        }
        paint_grips(ctx, &grips, &place, self.camera.scale());
    }
}

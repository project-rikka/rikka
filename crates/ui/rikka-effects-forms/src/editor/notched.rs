use rikka_types::{Color, Size};
use rikka_ui::{PaintCtx, Quad, Rect};

const NOTCH_RADIUS: f32 = 2.0;

#[derive(Clone, Copy, Debug)]
pub(super) struct NotchedTrack {
    pub(super) rect: Rect,
    notch: Size,
}

impl NotchedTrack {
    pub(super) fn new(bounds: Rect, top: f32, height: f32, notch: Size) -> NotchedTrack {
        NotchedTrack {
            rect: Rect::new(
                bounds.x() + notch.width / 2.0,
                top,
                (bounds.width() - notch.width).max(1.0),
                height,
            ),
            notch,
        }
    }

    pub(super) fn x(&self, at: f32) -> f32 {
        self.rect.x() + at * self.rect.width()
    }

    pub(super) fn along(&self, x: f32) -> f32 {
        ((x - self.rect.x()) / self.rect.width()).clamp(0.0, 1.0)
    }

    pub(super) fn nearest(
        &self,
        candidates: impl Iterator<Item = (usize, f32)>,
        x: f32,
        reach: f32,
    ) -> Option<usize> {
        candidates
            .map(|(index, at)| (index, (self.x(at) - x).abs()))
            .filter(|(_, distance)| *distance <= reach)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(index, _)| index)
    }

    pub(super) fn paint_notch(
        &self,
        ctx: &mut PaintCtx<'_>,
        at: f32,
        top: f32,
        fill: Color,
        lit: bool,
    ) {
        let palette = ctx.palette();
        let (width, edge) = match lit {
            true => (2.0, palette.accent),
            false => (1.0, palette.text_muted),
        };
        ctx.draw_quad(
            Quad::new(Rect::new(
                self.x(at) - self.notch.width / 2.0,
                top,
                self.notch.width,
                self.notch.height,
            ))
            .fill(fill)
            .stroke(width, edge)
            .radius(NOTCH_RADIUS),
        );
    }
}

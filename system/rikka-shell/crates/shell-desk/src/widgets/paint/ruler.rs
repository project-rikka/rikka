use rikka_types::{Point, Rect};
use rikka_ui::{Family, Layout, PaintCtx, Quad, Style, TextAlign, TextRun, Widget};

use crate::widgets::paint::track::track;

const HOURS: usize = 24;
const LINE: f32 = 2.0;
const TICK_ALPHA: f32 = 0.32;
const QUARTER_ALPHA: f32 = 0.55;
const LABEL_ALPHA: f32 = 0.5;
const TICK: f32 = 3.0;
const QUARTER: f32 = 6.0;
const NOW: f32 = 7.0;
const LABEL: f32 = 10.0;
const LABEL_LINE: f32 = LABEL * 1.4;
const LABEL_ROOM: f32 = 24.0;
const LABEL_GAP: f32 = 2.0;

#[derive(Widget)]
pub(crate) struct Ruler {
    #[debug]
    through: f32,
    labelled: bool,
    #[style]
    style: Style,
}

#[must_use]
pub(crate) fn ruler(through: f32) -> Ruler {
    Ruler {
        through: through.clamp(0.0, 1.0),
        labelled: false,
        style: Style::default(),
    }
}

impl Ruler {
    #[must_use]
    pub(crate) fn labelled(mut self, labelled: bool) -> Ruler {
        self.labelled = labelled;
        self
    }

    #[must_use]
    pub(crate) fn height(labelled: bool) -> f32 {
        match labelled {
            true => NOW + QUARTER + LABEL_GAP + LABEL_LINE,
            false => NOW + QUARTER,
        }
    }
}

impl Widget for Ruler {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::DEFAULT.with(&self.style)
    }

    fn paint(&self, (): &mut (), ctx: &mut PaintCtx<'_>) {
        let bounds = ctx.bounds();
        if bounds.width() <= NOW {
            return;
        }
        let accent = ctx.palette().accent;
        let ink = ctx.env().text_color;
        let span = bounds.width() - NOW;
        let start = bounds.x() + NOW / 2.0;
        let line = bounds.y() + QUARTER + NOW / 2.0;
        ctx.draw_quad(
            Quad::new(Rect::new(start, line - LINE / 2.0, span, LINE))
                .fill(track(ctx))
                .radius(LINE / 2.0),
        );
        ctx.draw_quad(
            Quad::new(Rect::new(
                start,
                line - LINE / 2.0,
                span * self.through,
                LINE,
            ))
            .fill(accent)
            .radius(LINE / 2.0),
        );
        for hour in 0..=HOURS {
            let at = hour as f32 / HOURS as f32;
            let quarter = hour % 6 == 0;
            let (tall, alpha) = match quarter {
                true => (QUARTER, QUARTER_ALPHA),
                false => (TICK, TICK_ALPHA),
            };
            let tint = match at <= self.through {
                true => accent.scale_alpha(QUARTER_ALPHA.max(alpha)),
                false => ink.scale_alpha(alpha),
            };
            let x = start + span * at;
            ctx.draw_quad(
                Quad::new(Rect::new(
                    x - 0.5,
                    line - LINE / 2.0 - tall - 1.0,
                    1.0,
                    tall,
                ))
                .fill(tint),
            );
            if self.labelled && quarter {
                label(ctx, hour, x, line + NOW / 2.0 + LABEL_GAP);
            }
        }
        ctx.draw_circle(Point::new(start + span * self.through, line), NOW, accent);
    }

    fn same(&self, other: &Self) -> bool {
        self.through == other.through
            && self.labelled == other.labelled
            && self.style == other.style
    }
}

fn label(ctx: &mut PaintCtx<'_>, hour: usize, x: f32, top: f32) {
    let bounds = ctx.bounds();
    let (left, align) = match hour {
        0 => (bounds.x(), TextAlign::Left),
        HOURS => (bounds.max_x() - LABEL_ROOM, TextAlign::End),
        _ => (x - LABEL_ROOM / 2.0, TextAlign::Center),
    };
    let run = TextRun::new(format!("{hour:02}"), LABEL)
        .family(Family::Monospace)
        .no_wrap()
        .align(align);
    let color = ctx.env().text_color.scale_alpha(LABEL_ALPHA);
    ctx.draw_text(Rect::new(left, top, LABEL_ROOM, LABEL_LINE), run, color);
}

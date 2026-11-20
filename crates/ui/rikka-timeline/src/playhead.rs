use std::fmt::Debug;

use rikka_types::{CornerRadii, Rect};
use rikka_ui::{Behavior, Element, PaintCtx, Quad, TextRun, View, Widget, measure_text, palette};

use crate::lane::TimeLane;

const LINE: f32 = 1.5;
const TAB: f32 = 11.0;
const TAB_HEIGHT: f32 = 10.0;
const TAB_LIFT: f32 = 2.0;
const LABEL: f32 = 11.0;
const LABEL_LINE: f32 = 14.0;
const LABEL_PADDING: f32 = 5.0;
const PILL_HEIGHT: f32 = 18.0;

pub trait Cue: PartialEq + Debug + 'static {
    fn is_playing(&self) -> bool;

    fn frame(&self) -> f64;

    fn lane(&self, bounds: Rect) -> TimeLane;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Head {
    Tab,
    Numbered,
}

#[derive(Debug)]
pub struct Playhead<C> {
    cue: C,
    head: Head,
    ruler: f32,
}

impl<C: Cue> Playhead<C> {
    #[must_use]
    pub fn new(cue: C, head: Head, ruler: f32) -> Playhead<C> {
        Playhead { cue, head, ruler }
    }

    fn paint_head(&self, ctx: &mut PaintCtx<'_>, lane: Rect, x: f32, frame: f64) {
        let palette = palette();
        match self.head {
            Head::Tab => ctx.draw_quad(
                Quad::new(Rect::new(
                    x - TAB / 2.0,
                    lane.y() + self.ruler - TAB_HEIGHT - TAB_LIFT,
                    TAB,
                    TAB_HEIGHT,
                ))
                .fill(palette.accent)
                .radius(CornerRadii::new(2.0, 2.0, TAB / 2.0, TAB / 2.0)),
            ),
            Head::Numbered => {
                let label = (frame.floor() as i64).to_string();
                let run = TextRun::new(label, LABEL).medium().no_wrap();
                let width = measure_text(&run, None).width + LABEL_PADDING * 2.0;
                let pill = Rect::new(
                    x - width / 2.0,
                    lane.y() + (self.ruler - PILL_HEIGHT) / 2.0,
                    width,
                    PILL_HEIGHT,
                );
                ctx.draw_quad(
                    Quad::new(pill)
                        .fill(palette.accent)
                        .radius(PILL_HEIGHT / 2.0),
                );
                ctx.draw_text(
                    Rect::new(
                        pill.x() + LABEL_PADDING,
                        pill.y() + (PILL_HEIGHT - LABEL_LINE) / 2.0,
                        width,
                        LABEL_LINE,
                    ),
                    run,
                    palette.on_accent,
                );
            }
        }
    }
}

impl<C: Cue> Widget for Playhead<C> {
    type State = ();

    fn paint(&self, _state: &mut (), ctx: &mut PaintCtx<'_>) {
        let lane = self.cue.lane(ctx.bounds());
        if self.cue.is_playing() {
            ctx.request_frame();
        }
        let frame = self.cue.frame();
        let x = lane.x_of(frame);
        let area = lane.area;
        if x < area.min_x() - TAB || x > area.max_x() + TAB {
            return;
        }
        let accent = palette().accent;
        ctx.clipped(area, |ctx| {
            ctx.draw_quad(
                Quad::new(Rect::new(x - LINE / 2.0, area.y(), LINE, area.height())).fill(accent),
            );
            self.paint_head(ctx, area, x, frame);
        });
    }

    fn behavior(&self, _state: &()) -> Behavior {
        Behavior::INERT | Behavior::TRACKS_PLACE
    }

    fn same(&self, other: &Playhead<C>) -> bool {
        self.cue == other.cue && self.head == other.head && self.ruler == other.ruler
    }

    fn debug_name(&self) -> &'static str {
        "Playhead"
    }
}

impl<C: Cue> View for Playhead<C> {
    fn build(self) -> Element {
        Element::new(self)
    }
}

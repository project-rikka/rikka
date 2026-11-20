use std::rc::Rc;
use std::time::Duration;

use rikka_theme::Corner;
use rikka_types::{CursorShape, Point, PointerButton, Rect};
use rikka_ui::{Element, Event, EventCtx, PaintCtx, Quad, View, Widget, palette};
use rikka_video::Keyframes;

use crate::cut::Cut;
use crate::editor::{Edge, Editor};

pub(crate) const HEIGHT: f32 = 56.0;

const TRACK: f32 = 44.0;

const HANDLE: f32 = 14.0;
const HANDLE_RADIUS: f32 = 7.0;
const GRIP: f32 = 2.0;
const GRIP_TALL: f32 = 18.0;
const GRIP_GAP: f32 = 4.0;

const PLAYHEAD: f32 = 3.0;
const PLAYHEAD_RADIUS: f32 = 1.5;

const MARK: f32 = 1.0;
const MARK_GAP: f32 = 5.0;
const MARK_ALPHA: f32 = 0.4;

const KEPT_ALPHA: f32 = 0.22;
const DROPPED_ALPHA: f32 = 0.5;

const REACH: f32 = 16.0;

pub(super) struct Timeline {
    pub(crate) cut: Cut,
    pub(crate) position: Duration,
    pub(super) keyframes: Rc<Keyframes>,
    pub(crate) enabled: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Grab {
    Start,
    End,
    Playhead,
}

#[derive(Debug, Default)]
pub(crate) struct TimelineState {
    grabbed: Option<Grab>,
}

impl Timeline {
    fn track(&self, bounds: Rect) -> Rect {
        Rect::new(
            bounds.min_x() + HANDLE / 2.0,
            bounds.center().y - TRACK / 2.0,
            (bounds.width() - HANDLE).max(0.0),
            TRACK,
        )
    }

    fn across(&self, track: Rect, at: Duration) -> f32 {
        let whole = self.cut.length().as_secs_f32();
        if whole <= 0.0 {
            return track.min_x();
        }
        let over = (at.as_secs_f32() / whole).clamp(0.0, 1.0);
        track.min_x() + track.width() * over
    }

    fn at(&self, track: Rect, x: f32) -> Duration {
        if track.width() <= 0.0 {
            return Duration::ZERO;
        }
        let over = ((x - track.min_x()) / track.width()).clamp(0.0, 1.0);
        self.cut.length().mul_f32(over)
    }

    fn grab(&self, track: Rect, x: f32) -> Grab {
        let start = (x - self.across(track, self.cut.start())).abs();
        let end = (x - self.across(track, self.cut.end())).abs();
        match start.min(end) <= REACH {
            true if start <= end => Grab::Start,
            true => Grab::End,
            false => Grab::Playhead,
        }
    }

    fn moved(&self, ctx: &mut EventCtx<'_>, grabbed: Grab, track: Rect, x: f32) {
        let at = self.at(track, x);
        ctx.run(move |editor: &mut Editor| match grabbed {
            Grab::Start => editor.moved(Edge::Start, at),
            Grab::End => editor.moved(Edge::End, at),
            Grab::Playhead => editor.scrubbed(at),
        });
    }

    fn paint_marks(&self, ctx: &mut PaintCtx<'_>, track: Rect) {
        let times = self.keyframes.times();
        let room = track.width() / times.len().max(1) as f32;
        if times.len() < 2 || room < MARK_GAP {
            return;
        }
        let ink = palette().outline.scale_alpha(MARK_ALPHA);
        for time in times {
            let x = self.across(track, *time);
            ctx.draw_quad(Quad::new(Rect::new(x, track.min_y(), MARK, track.height())).fill(ink));
        }
    }

    fn paint_handle(&self, ctx: &mut PaintCtx<'_>, track: Rect, at: Duration) {
        let palette = palette();
        let x = self.across(track, at) - HANDLE / 2.0;
        let handle = Rect::new(x, track.min_y(), HANDLE, track.height());
        ctx.draw_quad(Quad::new(handle).fill(palette.accent).radius(HANDLE_RADIUS));

        let grip = GRIP_TALL.min(track.height() - GRIP_GAP * 2.0).max(0.0);
        let top = handle.center().y - grip / 2.0;
        let ink = palette.on_accent;
        for offset in [-GRIP_GAP / 2.0 - GRIP, GRIP_GAP / 2.0] {
            let line = Rect::new(handle.center().x + offset, top, GRIP, grip);
            ctx.draw_quad(Quad::new(line).fill(ink).radius(GRIP / 2.0));
        }
    }
}

impl Widget for Timeline {
    type State = TimelineState;

    fn paint(&self, _state: &mut TimelineState, ctx: &mut PaintCtx<'_>) {
        let track = self.track(ctx.bounds());
        if track.size.is_empty() {
            return;
        }
        let palette = palette();
        ctx.draw_quad(
            Quad::new(track)
                .fill(palette.surface_variant.scale_alpha(DROPPED_ALPHA))
                .radius(Corner::Item),
        );
        self.paint_marks(ctx, track);

        let (start, end) = (
            self.across(track, self.cut.start()),
            self.across(track, self.cut.end()),
        );
        let selection = Rect::new(start, track.min_y(), (end - start).max(0.0), track.height());
        ctx.draw_quad(
            Quad::new(selection)
                .fill(palette.accent.scale_alpha(KEPT_ALPHA))
                .stroke(1.0, palette.accent)
                .radius(Corner::Item),
        );

        let playhead = self.across(track, self.position) - PLAYHEAD / 2.0;
        ctx.draw_quad(
            Quad::new(Rect::new(playhead, track.min_y(), PLAYHEAD, track.height()))
                .fill(palette.text)
                .radius(PLAYHEAD_RADIUS),
        );

        self.paint_handle(ctx, track, self.cut.start());
        self.paint_handle(ctx, track, self.cut.end());
    }

    fn event(&self, state: &mut TimelineState, event: &Event, ctx: &mut EventCtx<'_>) {
        let track = self.track(ctx.bounds());
        match *event {
            Event::PointerPressed {
                position,
                button: PointerButton::Primary,
                ..
            } if self.enabled => {
                let grabbed = self.grab(track, position.x);
                state.grabbed = Some(grabbed);
                ctx.capture_pointer();
                ctx.set_handled();
                ctx.request_paint();
                self.moved(ctx, grabbed, track, position.x);
            }
            Event::PointerMoved(position) => {
                let Some(grabbed) = state.grabbed else {
                    return;
                };
                ctx.set_handled();
                if self.enabled {
                    self.moved(ctx, grabbed, track, position.x);
                }
            }
            Event::PointerReleased {
                button: PointerButton::Primary,
                ..
            } => {
                let Some(grabbed) = state.grabbed.take() else {
                    return;
                };
                ctx.release_pointer();
                ctx.set_handled();
                ctx.request_paint();
                if grabbed == Grab::Start {
                    ctx.run(Editor::settled);
                }
            }
            _ => {}
        }
    }

    fn cursor(&self, _state: &Self::State, bounds: Rect, point: Point) -> Option<CursorShape> {
        if !self.enabled {
            return None;
        }
        match self.grab(self.track(bounds), point.x) {
            Grab::Playhead => Some(CursorShape::Default),
            _ => Some(CursorShape::ResizeColumn),
        }
    }
}

impl View for Timeline {
    fn build(self) -> Element {
        Element::new(self)
    }
}

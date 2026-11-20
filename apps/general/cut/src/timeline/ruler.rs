use std::sync::Arc;

use cut_project::{
    Edit, Marker, MarkerChange, MarkerId, MarkerScope, Marks, Project, Rate, Timecode,
};
use rikka_app::prelude::*;
use rikka_timeline::{Tick, Ticks};

use crate::cut::Cut;
use crate::live::LivePreview;
use crate::timeline::lanes::grab::frames;
use crate::timeline::markers::{self, FLAG};
use crate::timeline::scale::Scale;

pub(crate) const HEIGHT: f32 = 42.0;
pub(crate) const KEY: &str = "cut-ruler";

const MARKER_ROW: f32 = 12.0;
const MARKER_NAME: f32 = 9.5;
const MARKER_REACH: f32 = 6.0;
const END_REACH: f32 = 4.0;
const NAME_GAP: f32 = 3.0;
const LABEL_ROOM: f32 = 96.0;
const LABEL: f32 = 10.5;
const LABEL_LINE: f32 = 14.0;
const LABEL_PAD: f32 = 4.0;
const MAJOR: f32 = 10.0;
const MINOR: f32 = 4.0;
const TICK: f32 = 1.0;
const MARK: f32 = 3.0;
const MARK_FILL: f32 = 0.35;
const PAST_END: f32 = 0.05;

#[derive(Debug)]
pub(crate) struct Ruler {
    pub(crate) project: Arc<Project>,
    pub(crate) live: LivePreview,
    pub(crate) scale: Scale,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Handle {
    Start,
    End,
}

#[derive(Clone, Debug, PartialEq)]
struct Dragging {
    id: MarkerId,
    handle: Handle,
    from: f32,
    original: Marker,
    by: i64,
}

impl Dragging {
    fn marker(&self) -> Marker {
        match self.handle {
            Handle::Start => Marker {
                at: (self.original.at + self.by).max(0),
                ..self.original.clone()
            },
            Handle::End => Marker {
                length: (self.original.length + self.by).max(0),
                ..self.original.clone()
            },
        }
    }

    fn edit(&self) -> Edit {
        Edit::Marker {
            on: MarkerScope::Timeline,
            change: MarkerChange::Set(self.id, self.marker()),
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct PointerState {
    scrubbing: bool,
    dragging: Option<Dragging>,
}

impl Ruler {
    fn rate(&self) -> Rate {
        self.project.rate()
    }

    fn marks(&self) -> Marks {
        self.project.marks()
    }

    fn end(&self) -> i64 {
        self.project.end()
    }

    fn frame_at(&self, bounds: Rect, x: f32) -> i64 {
        self.scale.frame_at(x - bounds.x()).round().max(0.0) as i64
    }

    fn marker_at(&self, bounds: Rect, point: Point) -> Option<(MarkerId, Handle)> {
        if point.y - bounds.y() > MARKER_ROW {
            return None;
        }
        let x_of = |frame: i64| bounds.x() + self.scale.x_of(frame as f64);
        let markers = self.project.markers();
        let ended = markers
            .iter()
            .filter(|(_, marker)| marker.length > 0)
            .find(|(_, marker)| (x_of(marker.end()) - point.x).abs() <= END_REACH)
            .map(|(id, _)| (id, Handle::End));
        let started = markers
            .iter()
            .map(|(id, marker)| (id, (x_of(marker.at) - point.x).abs()))
            .filter(|(_, distance)| *distance <= MARKER_REACH)
            .min_by(|left, right| left.1.total_cmp(&right.1))
            .map(|(id, _)| (id, Handle::Start));
        started.or(ended)
    }

    fn paint_ticks(&self, ctx: &mut PaintCtx<'_>, bounds: Rect) {
        let palette = palette();
        let rate = self.rate();
        let ticks = Ticks::new(self.scale.per_frame(), rate.nominal(), LABEL_ROOM);
        for Tick { frame, major } in ticks.across::<i64>(self.scale.first(), self.scale.last()) {
            let x = bounds.x() + self.scale.x_of(frame as f64);
            let length = if major { MAJOR } else { MINOR };
            ctx.draw_quad(
                Quad::new(Rect::new(x, bounds.max_y() - length, TICK, length))
                    .fill(palette.outline),
            );
            if major {
                let label = Timecode::from_frame(frame, rate).to_string();
                let run = TextRun::new(label, LABEL).no_wrap();
                ctx.draw_text(
                    Rect::new(
                        x + LABEL_PAD,
                        bounds.y() + MARKER_ROW + 2.0,
                        LABEL_ROOM,
                        LABEL_LINE,
                    ),
                    run,
                    palette.text_muted,
                );
            }
        }
    }

    fn paint_marks(&self, ctx: &mut PaintCtx<'_>, bounds: Rect) {
        let accent = palette().accent;
        let Marks { mark_in, mark_out } = self.marks();
        if mark_in.is_none() && mark_out.is_none() {
            return;
        }
        let left = bounds.x() + self.scale.x_of(mark_in.unwrap_or(0) as f64);
        let right = bounds.x()
            + self
                .scale
                .x_of(mark_out.map_or(self.end().max(1), |out| out + 1) as f64);
        let band = Rect::new(left, bounds.max_y() - MARK, (right - left).max(0.0), MARK);
        ctx.draw_quad(Quad::new(band).fill(accent.scale_alpha(MARK_FILL)));
        if mark_in.is_some() {
            ctx.draw_quad(
                Quad::new(Rect::new(left, bounds.y(), MARK, bounds.height())).fill(accent),
            );
        }
        if mark_out.is_some() {
            ctx.draw_quad(
                Quad::new(Rect::new(right - MARK, bounds.y(), MARK, bounds.height())).fill(accent),
            );
        }
    }

    fn paint_markers(&self, ctx: &mut PaintCtx<'_>, bounds: Rect, project: &Project) {
        let ink = palette().text;
        let placed: Vec<(f32, f32, &Marker)> = project
            .markers()
            .iter()
            .map(|(_, marker)| {
                (
                    bounds.x() + self.scale.x_of(marker.at as f64),
                    bounds.x() + self.scale.x_of(marker.end() as f64),
                    marker,
                )
            })
            .collect();
        for (index, (x, end, marker)) in placed.iter().enumerate() {
            let hue = markers::color(marker.color);
            let top = bounds.y() + 1.0;
            markers::paint_range(ctx, *x, *end, top + FLAG - 3.0, hue.scale_alpha(0.8));
            markers::paint_flag(ctx, *x, top, bounds.max_y(), hue);
            let room = placed
                .get(index + 1)
                .map_or(bounds.max_x(), |(next, _, _)| *next)
                - x
                - FLAG
                - NAME_GAP;
            if marker.name.is_empty() || room < MARKER_NAME * 2.0 {
                continue;
            }
            ctx.draw_text(
                Rect::new(x + FLAG / 2.0 + NAME_GAP, top - 2.0, room, MARKER_ROW),
                TextRun::new(marker.name.clone(), MARKER_NAME)
                    .no_wrap()
                    .ellipsized(),
                ink,
            );
        }
    }

    fn scrub(&self, ctx: &mut EventCtx<'_>, x: f32) {
        let frame = self.frame_at(ctx.bounds(), x);
        ctx.run(move |cut: &mut Cut| cut.go_to(frame));
    }

    fn press(&self, state: &mut PointerState, position: Point, ctx: &mut EventCtx<'_>) {
        let bounds = ctx.bounds();
        let is_double_click = ctx.presses() >= 2;
        let Some((id, handle)) = self.marker_at(bounds, position) else {
            state.scrubbing = true;
            self.scrub(ctx, position.x);
            return;
        };
        if is_double_click {
            ctx.run(move |cut: &mut Cut| cut.open_marker(MarkerScope::Timeline, id));
            return;
        }
        let Some(marker) = self.project.markers().get(id).cloned() else {
            return;
        };
        state.dragging = Some(Dragging {
            id,
            handle,
            from: position.x,
            original: marker,
            by: 0,
        });
    }

    fn drag(&self, dragging: &mut Dragging, x: f32, ctx: &mut EventCtx<'_>) {
        let by = frames(&self.scale, x - dragging.from);
        if by == dragging.by {
            return;
        }
        dragging.by = by;
        let edit = dragging.edit();
        ctx.run(move |cut: &mut Cut| cut.preview_edits(std::slice::from_ref(&edit)));
    }

    fn release(&self, dragging: Dragging, ctx: &mut EventCtx<'_>) {
        if dragging.by != 0 {
            let edit = dragging.edit();
            ctx.run(move |cut: &mut Cut| cut.commit_edits(vec![edit.clone()]));
            return;
        }
        let at = dragging.original.at;
        ctx.run(move |cut: &mut Cut| {
            cut.commit_preview();
            cut.go_to(at);
        });
    }
}

impl Widget for Ruler {
    type State = PointerState;

    fn paint(&self, _state: &mut PointerState, ctx: &mut PaintCtx<'_>) {
        let bounds = ctx.bounds();
        self.scale.meet(bounds.width(), self.end());
        let palette = palette();
        let project = self.live.project_or(&self.project);
        ctx.draw_quad(Quad::new(bounds).fill(palette.surface));
        let end = bounds.x() + self.scale.x_of(self.end() as f64);
        if end < bounds.max_x() {
            let from = end.max(bounds.x());
            ctx.draw_quad(
                Quad::new(Rect::new(
                    from,
                    bounds.y(),
                    bounds.max_x() - from,
                    bounds.height(),
                ))
                .fill(palette.text.scale_alpha(PAST_END)),
            );
        }
        self.paint_ticks(ctx, bounds);
        self.paint_marks(ctx, bounds);
        ctx.clipped(bounds, |ctx| self.paint_markers(ctx, bounds, &project));
        ctx.draw_quad(
            Quad::new(Rect::new(
                bounds.x(),
                bounds.max_y() - TICK,
                bounds.width(),
                TICK,
            ))
            .fill(palette.outline),
        );
    }

    fn event(&self, state: &mut PointerState, event: &Event, ctx: &mut EventCtx<'_>) {
        match *event {
            Event::PointerPressed {
                position,
                button: PointerButton::Primary,
                ..
            } => {
                ctx.capture_pointer();
                ctx.keep_focus();
                ctx.set_handled();
                self.press(state, position, ctx);
            }
            Event::PointerPressed {
                position,
                button: PointerButton::Secondary,
                ..
            } => {
                ctx.keep_focus();
                ctx.set_handled();
                if let Some((id, _)) = self.marker_at(ctx.bounds(), position) {
                    ctx.run(move |cut: &mut Cut| cut.open_marker(MarkerScope::Timeline, id));
                }
            }
            Event::PointerMoved(position) if state.dragging.is_some() => {
                ctx.set_handled();
                if let Some(dragging) = &mut state.dragging {
                    self.drag(dragging, position.x, ctx);
                }
            }
            Event::PointerMoved(position) if state.scrubbing => {
                ctx.set_handled();
                self.scrub(ctx, position.x);
            }
            Event::PointerReleased {
                button: PointerButton::Primary,
                ..
            } if ctx.captured() => {
                state.scrubbing = false;
                ctx.release_pointer();
                ctx.set_handled();
                if let Some(dragging) = state.dragging.take() {
                    self.release(dragging, ctx);
                }
            }
            Event::Scroll {
                position,
                delta,
                modifiers,
                ..
            } => {
                ctx.set_handled();
                if modifiers.control || modifiers.alt {
                    self.scale
                        .zoom_notches(delta, position.x - ctx.bounds().x());
                } else {
                    let offset = delta.offset();
                    self.scale.scroll_by(offset.y + offset.x, self.end());
                }
            }
            _ => {}
        }
    }

    fn cursor(&self, _state: &PointerState, bounds: Rect, point: Point) -> Option<CursorShape> {
        Some(match self.marker_at(bounds, point) {
            Some((_, Handle::End)) => CursorShape::ResizeColumn,
            Some((_, Handle::Start)) => CursorShape::Pointer,
            None => CursorShape::Default,
        })
    }

    fn behavior(&self, _state: &PointerState) -> Behavior {
        Behavior::CLIPS | Behavior::TRACKS_HOVER
    }

    fn same(&self, other: &Ruler) -> bool {
        Arc::ptr_eq(&self.project, &other.project)
            && self.live.same(&other.live)
            && self.scale == other.scale
    }
}

impl View for Ruler {
    fn build(self) -> Element {
        Element::new(self).height(HEIGHT)
    }
}

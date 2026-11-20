use animate_edit::KeyId;
use animate_fluid::Pour;
use animate_hold::HoldId;
use animate_sheet::HoldSpan;
use animate_track::Edge;
use animate_transport::Transport;
use rikka_app::prelude::*;
use rikka_timeline::{Band, KeyShift, Pick, ScrollTrack, paint_thumb, press};
use rustc_hash::FxHashSet;

use super::Names;
use super::hit::{ids_at, key_near, lane_at, reading, row_keys};
use super::hold::{self, HoldDrag};
use super::paint::{self, Moving};
use super::pour::{self, PourDrag};
use crate::layout::{Axis, Geometry, NAMES_WIDTH, Row};
use crate::state::{Board, Timeline};

const THUMB_INSET: f32 = 2.0;

#[derive(Widget)]
pub struct Dope {
    #[debug]
    timeline: Timeline,
    transport: Transport,
    names: Names,
    on_shift: Handler<(i64,)>,
    on_pour: Handler<(Pour,)>,
    on_hold: Handler<(HoldSpan,)>,
}

#[derive(Debug, Default)]
pub struct DopeState {
    grab: Option<Grab>,
    lit: Option<Axis>,
    over: Option<Edge>,
    over_hold: Option<(HoldId, Edge)>,
}

#[derive(Clone, Debug)]
enum Grab {
    Scrub,
    Pan(Point),
    Slide {
        axis: Axis,
        grip: f32,
    },
    Shift {
        shift: KeyShift<u32>,
        solo: Option<Vec<KeyId>>,
    },
    Box {
        band: Band,
        adding: bool,
    },
    Pour {
        drag: PourDrag,
        to: Pour,
    },
    Hold {
        drag: HoldDrag,
        to: HoldSpan,
    },
}

impl Dope {
    pub fn new<Shifted, Poured, Retimed>(
        timeline: &Timeline,
        transport: &Transport,
        names: Names,
        on_shift: impl IntoHandler<(i64,), (), Shifted>,
        on_pour: impl IntoHandler<(Pour,), (), Poured>,
        on_hold: impl IntoHandler<(HoldSpan,), (), Retimed>,
    ) -> Self {
        Self {
            timeline: timeline.clone(),
            transport: transport.clone(),
            names,
            on_shift: on_shift.into_handler(),
            on_pour: on_pour.into_handler(),
            on_hold: on_hold.into_handler(),
        }
    }

    fn grip_at(&self, geometry: &Geometry, point: Point) -> Option<Edge> {
        pour::grip_at(geometry, self.timeline.pour_row()?, point)
    }

    fn hold_at(&self, geometry: &Geometry, point: Point) -> Option<HoldDrag> {
        hold::grab(geometry, &self.timeline.hold_rows(), point)
    }

    fn grab_pour(&self, bounds: Rect, position: Point) -> Option<Grab> {
        let geometry = self.timeline.geometry(bounds);
        let drag = pour::grab(&geometry, self.timeline.pour_row()?, position)?;
        self.timeline.choose(Some(Row::Pour));
        self.timeline.pick(FxHashSet::default());
        Some(Grab::Pour {
            drag,
            to: drag.from(),
        })
    }

    fn grab_hold(&self, bounds: Rect, position: Point) -> Option<Grab> {
        let drag = self.hold_at(&self.timeline.geometry(bounds), position)?;
        self.timeline.choose_hold(drag.lane, Some(drag.from.id));
        self.timeline.pick(FxHashSet::default());
        Some(Grab::Hold {
            drag,
            to: drag.from,
        })
    }

    fn scrub(&self, x: f32, bounds: Rect) {
        let geometry = self.timeline.geometry(bounds);
        let frame = geometry.frame_at(x).round().max(0.0) as u32;
        self.seek(frame);
    }

    fn seek(&self, frame: u32) {
        if self.transport.is_playing() {
            self.transport.pause();
        }
        self.transport.seek_frame(frame);
    }

    fn pressed_name(&self, board: &Board, row: usize) {
        let row = board.rows[row];
        if let Row::Group(group) = row
            && board.sheet.groups()[group].opens()
            && !board.sheet.groups()[group].kind.stays_open()
        {
            self.timeline.toggle(group);
        }
        self.timeline.choose(Some(row));
    }

    fn pressed_keys(&self, bounds: Rect, position: Point, modifiers: Modifiers) -> Grab {
        let geometry = self.timeline.geometry(bounds);
        let board = self.timeline.board();
        let row = geometry
            .row_at(&board.rows, position.y)
            .map(|row| board.rows[row]);
        self.timeline.choose(row);
        if modifiers.shift {
            return Grab::Box {
                band: Band::at(position),
                adding: modifiers.control,
            };
        }
        let hit = row.and_then(|row| {
            let frame = key_near(&geometry, row_keys(&board, row), position.x)?;
            Some((frame, ids_at(&board, row, frame)))
        });
        let Some((frame, ids)) = hit.filter(|(_, ids)| !ids.is_empty()) else {
            if !modifiers.control {
                self.timeline.pick(FxHashSet::default());
            }
            self.scrub(position.x, bounds);
            return Grab::Scrub;
        };
        self.seek(frame);
        let picked: Vec<KeyId> = board.picked.into_iter().collect();
        match press(&picked, ids.into_iter().collect(), modifiers.control) {
            Pick::Toggled(left) => {
                self.timeline.pick(left.into_iter().collect());
                Grab::Scrub
            }
            Pick::Held { picked, solo } => {
                self.timeline.pick(picked.into_iter().collect());
                Grab::Shift {
                    shift: KeyShift::new(position.x, frame),
                    solo,
                }
            }
        }
    }

    fn boxed(&self, bounds: Rect, area: Rect, adding: bool) {
        let geometry = self.timeline.geometry(bounds);
        let board = self.timeline.board();
        let mut picked = match adding {
            true => board.picked.clone(),
            false => FxHashSet::default(),
        };
        let (from, to) = (
            geometry.frame_at(area.min_x()),
            geometry.frame_at(area.max_x()),
        );
        for (index, row) in board.rows.iter().enumerate() {
            let top = geometry.row_top(&board.rows, index);
            if top + row.height() < area.min_y() || top > area.max_y() {
                continue;
            }
            for frame in row_keys(&board, *row) {
                if (*frame as f32) >= from && (*frame as f32) <= to {
                    picked.extend(ids_at(&board, *row, *frame));
                }
            }
        }
        self.timeline.pick(picked);
    }

    fn scrollbar(&self, geometry: &Geometry, axis: Axis) -> ScrollTrack {
        let lane = geometry.scrollbar_track(axis);
        let (from, length) = match axis {
            Axis::Time => (lane.x(), lane.width()),
            Axis::Rows => (lane.y(), lane.height()),
        };
        ScrollTrack {
            extent: self.timeline.extent(axis),
            from: from + THUMB_INSET,
            length: (length - THUMB_INSET * 2.0).max(0.0),
        }
    }

    fn grab_thumb(&self, geometry: &Geometry, axis: Axis, position: Point) -> Option<Grab> {
        let scrollbar = self.scrollbar(geometry, axis);
        if !scrollbar.extent.scrolls() {
            return None;
        }
        let at = reading(axis, position);
        let grip = scrollbar.grip(at);
        self.timeline.slide(axis, scrollbar.at_for(at - grip));
        Some(Grab::Slide { axis, grip })
    }

    fn slide(&self, bounds: Rect, axis: Axis, grip: f32, position: Point) {
        let geometry = self.timeline.geometry(bounds);
        let scrollbar = self.scrollbar(&geometry, axis);
        self.timeline
            .slide(axis, scrollbar.at_for(reading(axis, position) - grip));
    }

    fn wheel(&self, bounds: Rect, position: Point, delta: ScrollDelta, modifiers: Modifiers) {
        let geometry = self.timeline.geometry(bounds);
        let on_time = position.y < geometry.keys().min_y()
            || geometry.scrollbar_track(Axis::Time).contains(position);
        let offset = delta.offset();
        if modifiers.control || modifiers.alt {
            let x = position.x.max(geometry.keys().min_x());
            self.timeline.zoom_notches(delta.zoom(), x, bounds);
        } else if modifiers.shift || on_time {
            self.timeline.scroll(offset.y + offset.x, 0.0);
        } else {
            self.timeline.scroll(offset.x, offset.y);
        }
    }

    fn light(
        &self,
        state: &mut DopeState,
        bounds: Rect,
        position: Option<Point>,
        ctx: &mut EventCtx<'_>,
    ) {
        let geometry = self.timeline.geometry(bounds);
        let highlighted = position.and_then(|position| lane_at(&geometry, position));
        let over = position.and_then(|position| self.grip_at(&geometry, position));
        let over_hold = position
            .and_then(|position| self.hold_at(&geometry, position))
            .map(|drag| (drag.from.id, drag.grip));
        if state.lit != highlighted || state.over != over || state.over_hold != over_hold {
            state.lit = highlighted;
            state.over = over;
            state.over_hold = over_hold;
            ctx.request_paint();
        }
    }
}

impl Widget for Dope {
    type State = DopeState;

    fn paint(&self, state: &mut DopeState, ctx: &mut PaintCtx<'_>) {
        let bounds = ctx.bounds();
        self.timeline.measured(bounds);
        let geometry = self.timeline.geometry(bounds);
        let board = self.timeline.board();
        let moving = match &state.grab {
            Some(Grab::Shift { shift, .. }) => Moving::Shift(shift.by()),
            Some(Grab::Pour { drag, to }) => Moving::Pour {
                grip: drag.grip(),
                to: *to,
            },
            Some(Grab::Hold { drag, to }) => Moving::Hold {
                grip: drag.grip,
                to: *to,
            },
            Some(_) => Moving::Still {
                over: None,
                hold: None,
            },
            None => Moving::Still {
                over: state.over,
                hold: state.over_hold,
            },
        };
        paint::sheet(ctx, &geometry, &board, &self.names, &moving);
        if let Some(Grab::Box { band, .. }) = &state.grab {
            paint::selection(ctx, &geometry, band.rect());
        }

        let sliding = match &state.grab {
            Some(Grab::Slide { axis, .. }) => Some(*axis),
            _ => None,
        };
        for axis in [Axis::Time, Axis::Rows] {
            let scrollbar = self.scrollbar(&geometry, axis);
            if scrollbar.extent.scrolls() {
                let highlighted = sliding.or(state.lit) == Some(axis);
                paint_thumb(
                    ctx,
                    geometry.scrollbar_track(axis),
                    axis.orientation(),
                    scrollbar.thumb(),
                    highlighted,
                );
            }
        }
    }

    fn event(&self, state: &mut DopeState, event: &Event, ctx: &mut EventCtx<'_>) {
        let bounds = ctx.bounds();
        match *event {
            Event::PointerPressed {
                position,
                button,
                modifiers,
            } => {
                let geometry = self.timeline.geometry(bounds);
                let lane = lane_at(&geometry, position);
                let in_keys = position.y >= geometry.keys().min_y();
                let grab = match (button, lane) {
                    (PointerButton::Middle, _) => Some(Grab::Pan(position)),
                    (PointerButton::Primary, Some(axis)) => {
                        self.grab_thumb(&geometry, axis, position)
                    }
                    (PointerButton::Primary, None) if position.x < bounds.x() + NAMES_WIDTH => {
                        let board = self.timeline.board();
                        if let Some(row) = geometry.row_at(&board.rows, position.y) {
                            self.pressed_name(&board, row);
                        }
                        None
                    }
                    (PointerButton::Primary, None)
                        if in_keys && position.y < geometry.scrollbar_track(Axis::Time).y() =>
                    {
                        self.grab_pour(bounds, position)
                            .or_else(|| self.grab_hold(bounds, position))
                            .or_else(|| Some(self.pressed_keys(bounds, position, modifiers)))
                    }
                    (PointerButton::Primary, None) if !in_keys => {
                        self.scrub(position.x, bounds);
                        Some(Grab::Scrub)
                    }
                    _ => None,
                };
                if grab.is_some() {
                    state.grab = grab;
                    ctx.capture_pointer();
                    ctx.request_paint();
                }
            }
            Event::PointerMoved(position) => match &mut state.grab {
                Some(Grab::Scrub) => self.scrub(position.x, bounds),
                Some(Grab::Pan(before)) => {
                    let (x, y) = (before.x - position.x, before.y - position.y);
                    *before = position;
                    self.timeline.scroll(x, y);
                }
                Some(Grab::Slide { axis, grip }) => self.slide(bounds, *axis, *grip, position),
                Some(Grab::Shift { shift, .. }) => {
                    let geometry = self.timeline.geometry(bounds);
                    if shift.drag(position.x, &geometry.scale, |_| None) {
                        ctx.request_paint();
                    }
                }
                Some(Grab::Box { band, .. }) => {
                    band.to = position;
                    ctx.request_paint();
                }
                Some(Grab::Pour { drag, to }) => {
                    let geometry = self.timeline.geometry(bounds);
                    let dragged = drag.to(&geometry, self.transport.frame(), position.x);
                    if dragged != *to {
                        *to = dragged;
                        ctx.request_paint();
                    }
                }
                Some(Grab::Hold { drag, to }) => {
                    let geometry = self.timeline.geometry(bounds);
                    let dragged = drag.to(&geometry, self.transport.frame(), position.x);
                    if dragged != *to {
                        *to = dragged;
                        ctx.request_paint();
                    }
                }
                None => self.light(state, bounds, Some(position), ctx),
            },
            Event::PointerReleased { position, .. } => {
                let Some(grab) = state.grab.take() else {
                    return;
                };
                match grab {
                    Grab::Shift { shift, .. } if shift.by() != 0 => {
                        ctx.call(&self.on_shift, (shift.by(),));
                    }
                    Grab::Shift {
                        solo: Some(solo), ..
                    } => self.timeline.pick(solo.into_iter().collect()),
                    Grab::Box { band, adding } => {
                        self.boxed(bounds, band.rect(), adding);
                    }
                    Grab::Pour { drag, to } if to != drag.from() => {
                        ctx.call(&self.on_pour, (to,));
                    }
                    Grab::Hold { drag, to } if to != drag.from => {
                        ctx.call(&self.on_hold, (to,));
                    }
                    _ => {}
                }
                ctx.release_pointer();
                ctx.request_paint();
                self.light(state, bounds, Some(position), ctx);
            }
            Event::PointerExited => self.light(state, bounds, None, ctx),
            Event::Scroll {
                position,
                delta,
                modifiers,
                ..
            } => {
                self.wheel(bounds, position, delta, modifiers);
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn same(&self, other: &Self) -> bool {
        self.timeline == other.timeline
            && self.transport == other.transport
            && self.names == other.names
    }

    fn cursor(&self, _state: &Self::State, bounds: Rect, point: Point) -> Option<CursorShape> {
        let geometry = self.timeline.geometry(bounds);
        let grip = self
            .grip_at(&geometry, point)
            .or_else(|| self.hold_at(&geometry, point).map(|drag| drag.grip))?;
        match grip {
            Edge::Start | Edge::End => Some(CursorShape::ResizeColumn),
            Edge::Body => Some(CursorShape::Grab),
        }
    }

    fn behavior(&self, _state: &DopeState) -> Behavior {
        Behavior::TRACKS_PLACE | Behavior::TRACKS_HOVER
    }
}

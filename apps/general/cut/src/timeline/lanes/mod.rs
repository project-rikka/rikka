pub(crate) mod grab;
mod hit;
mod leveling;
mod marking;
mod paint;
mod parts;
mod sound;
mod strip;
mod transition_spans;
mod trimming;
mod wave;
mod wedges;

use std::collections::BTreeSet;
use std::sync::Arc;

use cut_project::{
    ClipId, Edge, Edit, EditPoint, KeyChange, MarkerScope, MediaId, MixField, Project, Property,
    Snap,
};
use rikka_app::prelude::*;
use rikka_timeline::Band;
use rikka_types::ResizeEdge;
use rikka_ui::{Accept, DRAG_SLOP, DragOffer};

use crate::cut::Cut;
use crate::gallery;
use crate::live::LivePreview;
use crate::media_previews::{Display, MediaPreviews};
use crate::timeline::rows::{Row, Rows};
use crate::timeline::scale::Scale;
use crate::transport::Transport;
use crate::trimmer::Trimmer;
use crate::view::URI_LIST;

use grab::{Banding, Grab, Moving, PendingMove, PresetResize, TransitionResize};
use hit::{Hit, Part};
use leveling::{Fading, Leveling, Rekeying};
use marking::Marking;
use paint::Scene;
use transition_spans::{DropTarget, Grip};
use trimming::{Trim, Trimming};

pub(crate) const KEY: &str = "cut-lanes";

const VOLUME: Property = Property::Mix(MixField::Volume);

#[derive(Debug)]
pub(crate) struct Lanes {
    pub(crate) project: Arc<Project>,
    pub(crate) rows: Rows,
    pub(crate) selected: Arc<BTreeSet<ClipId>>,
    pub(crate) edit_point: Option<EditPoint>,
    pub(crate) offline: Arc<BTreeSet<MediaId>>,
    pub(crate) snapping: bool,
    pub(crate) trimmer: Trimmer,
    pub(crate) scale: Scale,
    pub(crate) transport: Transport,
    pub(crate) disclosure: Option<Disclosure>,
    pub(crate) media_previews: MediaPreviews,
    pub(crate) display: Display,
    pub(crate) live: LivePreview,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Disclosure {
    pub(crate) clip: ClipId,
    pub(crate) open: bool,
}

#[derive(Debug, Default)]
pub(crate) struct PointerState {
    grab: Option<Grab>,
    arrival: Option<(Row, i64)>,
    arriving: Option<Option<Snap>>,
    drop_target: Option<DropTarget>,
}

impl Lanes {
    fn local(bounds: Rect, point: Point) -> Point {
        Point::new(point.x - bounds.x(), point.y - bounds.y())
    }

    fn hit(&self, local: Point) -> Option<Hit> {
        hit::hit(&self.project, &self.rows, &self.scale, local)
    }

    fn discloses(&self, local: Point) -> bool {
        self.disclosure.is_some_and(|disclosure| {
            hit::discloses(
                &self.project,
                &self.rows,
                &self.scale,
                disclosure.clip,
                local,
            )
        })
    }

    fn snap(&self, moving: &[ClipId]) -> Option<Snap> {
        self.snapping
            .then(|| grab::snapping(&self.project, moving, self.transport.frame()))
    }

    fn drop_target(&self, local: Point, snap: Option<&Snap>) -> Option<(Row, i64)> {
        let row = self.rows.at(local.y)?;
        let mut frame = self.scale.frame_at(local.x).round().max(0.0) as i64;
        if let Some(snap) = snap {
            frame += snap.pull(&[frame], grab::reach(&self.scale)).unwrap_or(0);
        }
        Some((row, frame))
    }

    fn sound_of(&self, clip: ClipId) -> Option<Rect> {
        let (layer, clip_ref) = self.project.locate(clip)?;
        let row = self.rows.layer_row(layer);
        hit::parts_of(&self.project, &self.scale, &row, clip_ref).sound
    }

    fn trim_of(&self, hit: Hit, local: Point, modifiers: Modifiers) -> Option<Trim> {
        let clip = hit.clip;
        let nearest = || {
            let (_, clip_ref) = self.project.locate(clip)?;
            let (from, to) = hit::span(&self.scale, clip_ref);
            Some(match local.x - from < to - local.x {
                true => Edge::Head,
                false => Edge::Tail,
            })
        };
        let edge = match hit.part {
            Part::End(edge) => Some(edge),
            _ => None,
        };
        match (self.trimmer, edge) {
            (Trimmer::Select, Some(edge)) if modifiers.control => Some(Trim::Ripple { clip, edge }),
            (Trimmer::Select, Some(edge)) if modifiers.alt => Some(Trim::Roll {
                edit_point: EditPoint { clip, edge },
            }),
            (Trimmer::Select, Some(edge)) => Some(Trim::Plain { clip, edge }),
            (Trimmer::Select, None) if modifiers.alt && modifiers.control => {
                Some(Trim::Slide { clip })
            }
            (Trimmer::Select, None) if modifiers.alt => Some(Trim::Slip { clip }),
            (Trimmer::Select, None) => None,
            (Trimmer::Ripple, _) => Some(Trim::Ripple {
                clip,
                edge: edge.or_else(nearest)?,
            }),
            (Trimmer::Roll, _) => Some(Trim::Roll {
                edit_point: EditPoint {
                    clip,
                    edge: edge.or_else(nearest)?,
                },
            }),
            (Trimmer::Slip, _) => Some(Trim::Slip { clip }),
            (Trimmer::Slide, _) => Some(Trim::Slide { clip }),
        }
    }

    fn press(
        &self,
        state: &mut PointerState,
        local: Point,
        modifiers: Modifiers,
        ctx: &mut EventCtx<'_>,
    ) {
        let adding = modifiers.shift || modifiers.control;
        let is_double_click = ctx.presses() >= 2;
        let Some(mut hit) = self.hit(local).filter(|hit| !hit.locked) else {
            let base: Vec<ClipId> = match adding {
                true => self.selected.iter().copied().collect(),
                false => Vec::new(),
            };
            select(ctx, base.clone());
            state.grab = Some(Grab::Banding(Banding {
                band: Band::at(local),
                base,
            }));
            return;
        };
        if self.trimmer != Trimmer::Select
            && matches!(
                hit.part,
                Part::Fade(_)
                    | Part::Level
                    | Part::LevelKey(_)
                    | Part::Marker(_)
                    | Part::AnimationPreset(_)
            )
        {
            hit.part = Part::Body;
        }
        if let Part::TransitionSpan(edit_point, _) | Part::EditPoint(edit_point) = hit.part {
            ctx.run(move |cut: &mut Cut| cut.select_edit_point(edit_point));
            let grip = match hit.part {
                Part::TransitionSpan(_, grip) if grip != Grip::Body => Some(grip),
                _ => None,
            };
            state.grab = grip
                .and_then(|grip| TransitionResize::begin(local, edit_point, grip, &self.project))
                .map(Grab::TransitionResize);
            return;
        }
        let Some((_, clip)) = self.project.locate(hit.clip) else {
            return;
        };
        match hit.part {
            Part::AnimationPreset(side) => {
                select(ctx, vec![hit.clip]);
                state.grab = Some(Grab::PresetResize(PresetResize::begin(clip, side)));
                return;
            }
            Part::Fade(edge) => {
                select(ctx, vec![hit.clip]);
                state.grab = Fading::begin(local, clip, edge).map(Grab::Fading);
                return;
            }
            Part::LevelKey(frame) if modifiers.control => {
                edit(
                    ctx,
                    Edit::Key {
                        clip: hit.clip,
                        property: VOLUME,
                        change: KeyChange::Remove { at: frame },
                    },
                );
                return;
            }
            Part::LevelKey(frame) => {
                select(ctx, vec![hit.clip]);
                state.grab = self
                    .sound_of(hit.clip)
                    .and_then(|sound| Rekeying::begin(local, clip, sound, frame))
                    .map(Grab::Rekeying);
                return;
            }
            Part::Level if modifiers.control => {
                edit(
                    ctx,
                    Edit::Key {
                        clip: hit.clip,
                        property: VOLUME,
                        change: KeyChange::Add {
                            at: hit.frame.clamp(clip.start(), clip.end() - 1),
                        },
                    },
                );
                return;
            }
            Part::Level => {
                select(ctx, vec![hit.clip]);
                state.grab = self
                    .sound_of(hit.clip)
                    .and_then(|sound| Leveling::begin(local, clip, sound, hit.frame))
                    .map(Grab::Leveling);
                return;
            }
            Part::Marker(id) => {
                select(ctx, vec![hit.clip]);
                if is_double_click {
                    let scope = MarkerScope::Clip(hit.clip);
                    ctx.run(move |cut: &mut Cut| cut.open_marker(scope, id));
                    return;
                }
                state.grab = Marking::begin(local, clip, id).map(Grab::Marking);
                return;
            }
            Part::Body | Part::End(_) | Part::TransitionSpan(..) | Part::EditPoint(_) => {}
        }
        if let Some(trim) = self.trim_of(hit, local, modifiers) {
            select(ctx, vec![hit.clip]);
            let moving = trim.moving(&self.project);
            let snap = self.snap(&moving);
            state.grab = Trimming::begin(local, trim, &self.project, snap, self.transport.frame())
                .map(Grab::Trimming);
            return;
        }
        let already_selected = self.selected.contains(&hit.clip);
        let (clips, solo) = match (adding, already_selected) {
            (true, true) => {
                let left: Vec<ClipId> = self
                    .selected
                    .iter()
                    .copied()
                    .filter(|id| *id != hit.clip)
                    .collect();
                select(ctx, left);
                return;
            }
            (true, false) => {
                let mut more: BTreeSet<ClipId> = self.selected.iter().copied().collect();
                more.insert(hit.clip);
                let more: Vec<ClipId> = more.into_iter().collect();
                select(ctx, more.clone());
                (more, None)
            }
            (false, true) => (
                self.selected.iter().copied().collect(),
                Some(vec![hit.clip]),
            ),
            (false, false) => {
                select(ctx, vec![hit.clip]);
                (vec![hit.clip], None)
            }
        };
        let clips = self.project.editable(&clips);
        let snap = self.snap(&clips);
        state.grab = Some(Grab::PendingMove(PendingMove {
            at: local,
            clips,
            solo,
            snap,
        }));
    }

    fn drag(&self, state: &mut PointerState, local: Point, ctx: &mut EventCtx<'_>) -> bool {
        let Some(grab) = state.grab.take() else {
            return false;
        };
        let still = matches!(
            &grab,
            Grab::PendingMove(pressed) if (local.x - pressed.at.x).hypot(local.y - pressed.at.y) < DRAG_SLOP
        );
        state.grab = Some(match grab {
            Grab::PendingMove(pressed) if still => Grab::PendingMove(pressed),
            Grab::PendingMove(pressed) => {
                let mut moving = Moving::begin(pressed, &self.project);
                moving.drag(local, &self.scale, &self.rows);
                Grab::Moving(moving)
            }
            Grab::Moving(mut moving) => {
                moving.drag(local, &self.scale, &self.rows);
                Grab::Moving(moving)
            }
            Grab::PresetResize(mut animation_presets) => {
                let before = animation_presets.animation_presets;
                animation_presets.drag(local, &self.scale);
                if animation_presets.animation_presets != before {
                    let edit = Edit::AnimationPresets {
                        clip: animation_presets.clip,
                        animation_presets: animation_presets.animation_presets,
                    };
                    preview(ctx, vec![edit]);
                }
                Grab::PresetResize(animation_presets)
            }
            Grab::Trimming(mut trimming) => {
                trimming.drag(local, &self.scale);
                let edits = match trimming.has_moved() {
                    true => vec![trimming.edit()],
                    false => Vec::new(),
                };
                let peek = trimming.peek();
                ctx.run(move |cut: &mut Cut| {
                    cut.preview_edits(&edits);
                    cut.peek_at(peek);
                });
                Grab::Trimming(trimming)
            }
            Grab::TransitionResize(mut passing) => {
                passing.drag(local, &self.scale);
                preview(ctx, vec![passing.edit()]);
                Grab::TransitionResize(passing)
            }
            Grab::Leveling(mut leveling) => {
                leveling.drag(local);
                preview(ctx, leveling.edits());
                Grab::Leveling(leveling)
            }
            Grab::Rekeying(mut rekeying) => {
                rekeying.drag(local, &self.scale);
                preview(ctx, rekeying.edits());
                Grab::Rekeying(rekeying)
            }
            Grab::Fading(mut fading) => {
                fading.drag(local, &self.scale);
                preview(ctx, vec![fading.edit()]);
                Grab::Fading(fading)
            }
            Grab::Marking(mut marking) => {
                marking.drag(local, &self.scale);
                if let Some((_, clip)) = self.project.locate(marking.clip) {
                    preview(ctx, vec![marking.edit(clip)]);
                }
                Grab::Marking(marking)
            }
            Grab::Banding(Banding { band, base }) => Grab::Banding(Banding {
                band: Band { to: local, ..band },
                base,
            }),
        });
        !still
    }

    fn release(&self, state: &mut PointerState, ctx: &mut EventCtx<'_>) {
        match state.grab.take() {
            Some(Grab::PendingMove(PendingMove {
                solo: Some(solo), ..
            })) => select(ctx, solo),
            Some(Grab::Moving(moving)) if moving.has_moved() => {
                edit(
                    ctx,
                    Edit::Move {
                        clips: moving.clips,
                        by: moving.by,
                        across: moving.across,
                    },
                );
            }
            Some(Grab::Trimming(trimming)) => {
                settle(ctx, trimming.has_moved(), vec![trimming.edit()]);
            }
            Some(Grab::PresetResize(animation_presets)) => {
                let edit = Edit::AnimationPresets {
                    clip: animation_presets.clip,
                    animation_presets: animation_presets.animation_presets,
                };
                settle(ctx, animation_presets.has_changed(), vec![edit]);
            }
            Some(Grab::TransitionResize(passing)) => {
                settle(ctx, passing.has_changed(), vec![passing.edit()]);
            }
            Some(Grab::Leveling(leveling)) => {
                settle(ctx, leveling.has_moved(), leveling.edits());
            }
            Some(Grab::Rekeying(rekeying)) => {
                settle(ctx, rekeying.has_moved(), rekeying.edits());
            }
            Some(Grab::Fading(fading)) => {
                settle(ctx, fading.has_moved(), vec![fading.edit()]);
            }
            Some(Grab::Marking(marking)) if marking.has_moved() => {
                if let Some((_, clip)) = self.project.locate(marking.clip) {
                    settle(ctx, true, vec![marking.edit(clip)]);
                }
            }
            Some(Grab::Marking(marking)) => {
                let frame = marking.frame();
                ctx.run(move |cut: &mut Cut| {
                    cut.commit_preview();
                    cut.go_to(frame);
                });
            }
            Some(Grab::Banding(banding)) => {
                let caught =
                    hit::banded(&self.project, &self.rows, &self.scale, banding.band.rect());
                let mut selected: BTreeSet<ClipId> = banding.base.into_iter().collect();
                selected.extend(caught);
                select(ctx, selected.into_iter().collect());
            }
            _ => {}
        }
    }

    fn open_menu(&self, local: Point, ctx: &mut EventCtx<'_>) {
        let hit = self.hit(local).filter(|hit| !hit.locked);
        if let Some(Hit {
            clip,
            part: Part::Marker(id),
            ..
        }) = hit
        {
            ctx.run(move |cut: &mut Cut| cut.open_marker(MarkerScope::Clip(clip), id));
            return;
        }
        let layer = self.rows.at(local.y).map_or(0, |row| row.layer);
        let frame = self.scale.frame_at(local.x).round().max(0.0) as i64;
        let edit_point = hit.and_then(|hit| match hit.part {
            Part::TransitionSpan(edit_point, _) | Part::EditPoint(edit_point) => Some(edit_point),
            _ => None,
        });
        let clip = hit.filter(|_| edit_point.is_none()).map(|hit| hit.clip);
        ctx.run(move |cut: &mut Cut| cut.open_menu(local, clip, edit_point, frame, layer));
    }

    fn land(&self, state: &mut PointerState, local: Point, ctx: &mut EventCtx<'_>) {
        ctx.accept_drag(Accept::copy(gallery::MIME));
        ctx.set_handled();
        let open = self
            .rows
            .at(local.y)
            .and_then(|row| self.project.layer(row.layer))
            .is_some_and(|layer| !layer.is_locked());
        let drop_target =
            transition_spans::drop_target(&self.project, &self.rows, &self.scale, local)
                .filter(|_| open);
        if state.drop_target != drop_target {
            state.drop_target = drop_target;
            ctx.request_paint();
        }
    }

    fn landed(&self, state: &mut PointerState, offer: &DragOffer, ctx: &mut EventCtx<'_>) {
        ctx.set_handled();
        ctx.request_paint();
        let (Some(drop_target), Some(kind)) = (state.drop_target.take(), gallery::carried(offer))
        else {
            return;
        };
        ctx.run(move |cut: &mut Cut| {
            cut.transit_edit_points(vec![drop_target.edit_point], kind, drop_target.at)
        });
    }

    fn scroll(
        &self,
        local: Point,
        delta: ScrollDelta,
        modifiers: Modifiers,
        ctx: &mut EventCtx<'_>,
    ) {
        let end = self.project.end();
        let offset = delta.offset();
        if modifiers.control || modifiers.alt {
            self.scale.zoom_notches(delta, local.x);
        } else if modifiers.shift {
            self.scale.scroll_by(offset.y + offset.x, end);
        } else if offset.x != 0.0 {
            self.scale.scroll_by(offset.x, end);
        } else {
            return;
        }
        ctx.set_handled();
    }

    fn cursor_of(&self, hit: Hit) -> CursorShape {
        let edge_cursor = |edge: Edge| match edge {
            Edge::Head => CursorShape::Resize(ResizeEdge::Left),
            Edge::Tail => CursorShape::Resize(ResizeEdge::Right),
        };
        match (self.trimmer, hit.part) {
            (_, Part::TransitionSpan(_, Grip::Start | Grip::End)) => CursorShape::ResizeColumn,
            (_, Part::TransitionSpan(..) | Part::EditPoint(_)) => CursorShape::Default,
            (Trimmer::Select, Part::End(_) | Part::AnimationPreset(_)) => CursorShape::ResizeColumn,
            (Trimmer::Select, Part::Fade(_)) => CursorShape::ResizeEastWest,
            (Trimmer::Select, Part::Level) => CursorShape::ResizeNorthSouth,
            (Trimmer::Select, Part::LevelKey(_) | Part::Marker(_)) => CursorShape::Pointer,
            (Trimmer::Select, Part::Body) => CursorShape::Default,
            (Trimmer::Ripple, Part::End(edge)) => edge_cursor(edge),
            (Trimmer::Ripple | Trimmer::Roll, _) => CursorShape::ResizeColumn,
            (Trimmer::Slip, _) => CursorShape::ResizeEastWest,
            (Trimmer::Slide, _) => CursorShape::Move,
        }
    }
}

fn select(ctx: &mut EventCtx<'_>, clips: Vec<ClipId>) {
    ctx.run(move |cut: &mut Cut| cut.select(clips.clone()));
}

fn edit(ctx: &mut EventCtx<'_>, edit: Edit) {
    ctx.run(move |cut: &mut Cut| cut.edit(edit.clone()));
}

fn preview(ctx: &mut EventCtx<'_>, edits: Vec<Edit>) {
    ctx.run(move |cut: &mut Cut| cut.preview_edits(&edits));
}

fn settle(ctx: &mut EventCtx<'_>, changed: bool, edits: Vec<Edit>) {
    ctx.run(move |cut: &mut Cut| match changed {
        true => cut.commit_edits(edits.clone()),
        false => cut.commit_preview(),
    });
}

impl Widget for Lanes {
    type State = PointerState;

    fn paint(&self, state: &mut PointerState, ctx: &mut PaintCtx<'_>) {
        let project = self.live.project_or(&self.project);
        Scene {
            project: &project,
            rows: &self.rows,
            scale: &self.scale,
            selected: &self.selected,
            edit_point: self.edit_point,
            offline: &self.offline,
            disclosure: self.disclosure,
            media_previews: &self.media_previews,
            display: self.display,
            live: &self.live,
            bounds: ctx.bounds(),
        }
        .paint(ctx, state.grab.as_ref(), state.arrival, state.drop_target);
    }

    fn event(&self, state: &mut PointerState, event: &Event, ctx: &mut EventCtx<'_>) {
        let bounds = ctx.bounds();
        match event {
            Event::PointerPressed {
                position,
                button: PointerButton::Primary,
                ..
            } if self.discloses(Self::local(bounds, *position)) => {
                ctx.keep_focus();
                ctx.set_handled();
                ctx.run(Cut::toggle_key_lanes);
            }
            Event::PointerPressed {
                position,
                button: PointerButton::Primary,
                modifiers,
            } => {
                ctx.capture_pointer();
                ctx.keep_focus();
                ctx.set_handled();
                self.press(state, Self::local(bounds, *position), *modifiers, ctx);
                ctx.request_paint();
            }
            Event::PointerPressed {
                position,
                button: PointerButton::Secondary,
                ..
            } => {
                ctx.keep_focus();
                ctx.set_handled();
                self.open_menu(Self::local(bounds, *position), ctx);
            }
            Event::PointerMoved(position) => {
                if self.drag(state, Self::local(bounds, *position), ctx) {
                    ctx.set_handled();
                    ctx.request_paint();
                }
            }
            Event::PointerReleased {
                button: PointerButton::Primary,
                ..
            } if ctx.captured() => {
                ctx.release_pointer();
                ctx.set_handled();
                self.release(state, ctx);
                ctx.request_paint();
            }
            Event::Scroll {
                position,
                delta,
                modifiers,
                ..
            } => self.scroll(Self::local(bounds, *position), *delta, *modifiers, ctx),
            Event::DragMoved { position, offer } if offer.offers(gallery::MIME) => {
                self.land(state, Self::local(bounds, *position), ctx);
            }
            Event::DragDropped { offer, .. } if offer.offers(gallery::MIME) => {
                self.landed(state, offer, ctx);
            }
            Event::DragMoved { position, offer } if offer.offers(URI_LIST) => {
                ctx.accept_drag(Accept::copy(URI_LIST));
                ctx.set_handled();
                let snap = state.arriving.get_or_insert_with(|| self.snap(&[]));
                let arrival = self.drop_target(Self::local(bounds, *position), snap.as_ref());
                if state.arrival != arrival {
                    state.arrival = arrival;
                    ctx.request_paint();
                }
            }
            Event::DragLeft => {
                state.arriving = None;
                let dropped = state.drop_target.take().is_some();
                if state.arrival.take().is_some() || dropped {
                    ctx.request_paint();
                }
            }
            Event::DragDropped {
                position, offer, ..
            } if offer.offers(URI_LIST) => {
                ctx.set_handled();
                state.arrival = None;
                ctx.request_paint();
                let snap = state.arriving.take().unwrap_or_else(|| self.snap(&[]));
                if let Some((row, frame)) =
                    self.drop_target(Self::local(bounds, *position), snap.as_ref())
                {
                    let offer = offer.clone();
                    ctx.run(move |_: &mut Cut| {
                        Cut::dropped(offer.clone(), Some((frame, row.layer)));
                    });
                }
            }
            _ => {}
        }
    }

    fn cursor(&self, _state: &Self::State, bounds: Rect, point: Point) -> Option<CursorShape> {
        Some(
            self.hit(Self::local(bounds, point))
                .filter(|hit| !hit.locked)
                .map_or(CursorShape::Default, |hit| self.cursor_of(hit)),
        )
    }

    fn behavior(&self, _state: &PointerState) -> Behavior {
        Behavior::CLIPS | Behavior::TRACKS_HOVER
    }

    fn same(&self, other: &Lanes) -> bool {
        Arc::ptr_eq(&self.project, &other.project)
            && self.rows == other.rows
            && Arc::ptr_eq(&self.selected, &other.selected)
            && self.edit_point == other.edit_point
            && Arc::ptr_eq(&self.offline, &other.offline)
            && self.snapping == other.snapping
            && self.trimmer == other.trimmer
            && self.scale == other.scale
            && self.transport == other.transport
            && self.disclosure == other.disclosure
            && self.media_previews.same(&other.media_previews)
            && self.display == other.display
            && self.live.same(&other.live)
    }
}

impl View for Lanes {
    fn build(self) -> Element {
        let height = self.rows.height();
        Element::new(self).height(height)
    }
}

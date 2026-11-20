use std::collections::BTreeSet;

use cut_project::{Clip, Edit, KeyChange, KeyRef, Property, Side, Value};
use rikka_app::prelude::*;
use rikka_timeline::{Band, Pick, press};
use rikka_ui::DRAG_SLOP;

use super::graph::{self, HandleDrag, Plotting, Target};
use super::outline::Lane;
use super::shift::{self, Shift};
use super::{Keys, marks};
use crate::cut::{Cut, Plot};

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Grab {
    PendingKeyMove(PendingKeyMove),
    Shifting(Shift, bool),
    HandleDrag(HandleDrag),
    Banding(Banding),
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PendingKeyMove {
    at: Point,
    anchor: i64,
    keys: Vec<KeyRef>,
    solo: Option<Vec<KeyRef>>,
    duplicate: bool,
    valued: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Banding {
    pub(crate) band: Band,
    pub(crate) base: Vec<KeyRef>,
}

enum Hit {
    Keys { frame: i64, keys: Vec<KeyRef> },
    Handle(Property, i64, Side),
}

fn pick(ctx: &mut EventCtx<'_>, keys: Vec<KeyRef>) {
    ctx.run(move |cut: &mut Cut| cut.pick_keys(keys.clone()));
}

fn preview(ctx: &mut EventCtx<'_>, edits: Vec<Edit>) {
    ctx.run(move |cut: &mut Cut| cut.preview_edits(&edits));
}

impl Keys {
    fn hit(&self, clip: &Clip, plotting: &Plotting, local: Point) -> Option<Hit> {
        match self.outline.lane_at(local.y)? {
            Lane::Graph => match graph::hit(plotting, clip, &self.picked, &self.scale, local)? {
                Target::Key { property, frame } => Some(Hit::Keys {
                    frame,
                    keys: vec![KeyRef::new(property, frame)],
                }),
                Target::Handle {
                    property,
                    frame,
                    side,
                } => Some(Hit::Handle(property, frame, side)),
            },
            lane => {
                let mark = marks::nearest(
                    &marks::in_lane(clip, &self.outline, lane),
                    &self.scale,
                    local.x,
                )?;
                Some(Hit::Keys {
                    frame: mark.frame,
                    keys: mark.keys,
                })
            }
        }
    }

    pub(crate) fn caught(&self, clip: &Clip, plotting: &Plotting, band: Rect) -> Vec<KeyRef> {
        let mut caught = marks::banded(clip, &self.outline, &self.scale, band);
        if self.outline.graph {
            caught.extend(graph::banded(plotting, clip, &self.scale, band));
        }
        caught
    }

    pub(crate) fn press(
        &self,
        state: &mut super::PointerState,
        local: Point,
        modifiers: Modifiers,
        ctx: &mut EventCtx<'_>,
    ) {
        let Some(clip) = self.clip() else {
            return;
        };
        let plotting = self.plotting(clip, ctx.bounds().width(), None);
        let adding = modifiers.shift || modifiers.control;
        state.frozen = self.outline.graph.then(|| plotting.clone());
        let (anchor, keys) = match self.hit(clip, &plotting, local) {
            Some(Hit::Handle(property, frame, side)) => {
                state.grab = Some(Grab::HandleDrag(HandleDrag::begin(
                    clip,
                    (property, frame, side),
                    modifiers.alt,
                )));
                return;
            }
            None => {
                let base: Vec<KeyRef> = match adding {
                    true => self.picked.iter().cloned().collect(),
                    false => Vec::new(),
                };
                if base.len() != self.picked.len() {
                    pick(ctx, base.clone());
                }
                state.grab = Some(Grab::Banding(Banding {
                    band: Band::at(local),
                    base,
                }));
                return;
            }
            Some(Hit::Keys { frame, keys }) => (frame, keys),
        };
        if ctx.presses() >= 2 {
            ctx.run(move |cut: &mut Cut| cut.go_to(anchor));
        }
        let picked: Vec<KeyRef> = self.picked.iter().cloned().collect();
        let (moving, solo) = match press(&picked, keys, adding) {
            Pick::Toggled(left) => {
                pick(ctx, left);
                return;
            }
            Pick::Held {
                picked: moving,
                solo,
            } => {
                if moving != picked {
                    pick(ctx, moving.clone());
                }
                (moving, solo)
            }
        };
        let valued = self.plot == Plot::Value && self.outline.lane_at(local.y) == Some(Lane::Graph);
        state.grab = Some(Grab::PendingKeyMove(PendingKeyMove {
            at: local,
            anchor,
            keys: moving,
            solo,
            duplicate: modifiers.alt,
            valued,
        }));
    }

    pub(crate) fn drag(
        &self,
        state: &mut super::PointerState,
        local: Point,
        ctx: &mut EventCtx<'_>,
    ) -> bool {
        let (Some(grab), Some(clip)) = (state.grab.take(), self.clip()) else {
            return false;
        };
        let plotting = self.plotting(clip, ctx.bounds().width(), state.frozen.as_ref());
        let (grab, moved) = match grab {
            Grab::PendingKeyMove(pressed)
                if (local.x - pressed.at.x).hypot(local.y - pressed.at.y) < DRAG_SLOP =>
            {
                (Grab::PendingKeyMove(pressed), false)
            }
            Grab::PendingKeyMove(pressed) => {
                let snap = self.snapping.then(|| {
                    shift::snapping(&self.project, clip, &pressed.keys, self.transport.frame())
                });
                let mut shift = Shift::begin(
                    pressed.at,
                    pressed.anchor,
                    pressed.keys,
                    clip,
                    snap,
                    pressed.duplicate,
                );
                shift.drag(local, &self.scale);
                preview(ctx, self.shifted(clip, &plotting, &shift, pressed.valued));
                (Grab::Shifting(shift, pressed.valued), true)
            }
            Grab::Shifting(mut shift, valued) => {
                let before = (shift.by(), shift.rise);
                shift.drag(local, &self.scale);
                if (shift.by(), shift.rise) != before {
                    preview(ctx, self.shifted(clip, &plotting, &shift, valued));
                }
                (Grab::Shifting(shift, valued), true)
            }
            Grab::HandleDrag(mut handle_drag) => {
                let before = handle_drag.handle;
                handle_drag.drag(&plotting, clip, &self.scale, local);
                if handle_drag.handle != before {
                    preview(ctx, handle_drag.edits(clip.id()));
                }
                (Grab::HandleDrag(handle_drag), true)
            }
            Grab::Banding(Banding { band, base }) => (
                Grab::Banding(Banding {
                    band: Band { to: local, ..band },
                    base,
                }),
                true,
            ),
        };
        state.grab = Some(grab);
        moved
    }

    pub(crate) fn release(&self, state: &mut super::PointerState, ctx: &mut EventCtx<'_>) {
        let frozen = state.frozen.take();
        let (Some(grab), Some(clip)) = (state.grab.take(), self.clip()) else {
            return;
        };
        let plotting = self.plotting(clip, ctx.bounds().width(), frozen.as_ref());
        match grab {
            Grab::PendingKeyMove(PendingKeyMove {
                solo: Some(solo), ..
            }) => {
                pick(ctx, solo);
            }
            Grab::PendingKeyMove(_) => {}
            Grab::Shifting(shift, valued) if shift.has_moved() => {
                let edits = self.shifted(clip, &plotting, &shift, valued);
                let moved = shift.moved();
                ctx.run(move |cut: &mut Cut| cut.reshape_keys(edits.clone(), moved.clone()));
            }
            Grab::Shifting(..) => {
                ctx.run(Cut::commit_preview);
            }
            Grab::HandleDrag(handle_drag) => {
                let edits = handle_drag.edits(clip.id());
                let picked: Vec<KeyRef> = self.picked.iter().cloned().collect();
                ctx.run(move |cut: &mut Cut| match edits.is_empty() {
                    true => cut.commit_preview(),
                    false => cut.reshape_keys(edits.clone(), picked.clone()),
                });
            }
            Grab::Banding(banding) => {
                let mut selected: BTreeSet<KeyRef> = banding.base.iter().cloned().collect();
                selected.extend(self.caught(clip, &plotting, banding.band.rect()));
                if selected != *self.picked {
                    pick(ctx, selected.into_iter().collect());
                }
            }
        }
    }

    pub(crate) fn context(&self, local: Point, ctx: &mut EventCtx<'_>) {
        let Some(clip) = self.clip() else {
            return;
        };
        let plotting = self.plotting(clip, ctx.bounds().width(), None);
        let keys = match self.hit(clip, &plotting, local) {
            Some(Hit::Keys { keys, .. }) => keys,
            Some(Hit::Handle(..)) => Vec::new(),
            None if self.picked.is_empty() => return,
            None => Vec::new(),
        };
        let at = Point::new(local.x, local.y + self.top);
        ctx.run(move |cut: &mut Cut| cut.open_key_menu(at, keys.clone()));
    }

    fn shifted(&self, clip: &Clip, plotting: &Plotting, shift: &Shift, valued: bool) -> Vec<Edit> {
        let id = clip.id();
        let moving = (shift.by() != 0).then(|| Edit::Keys {
            clip: id,
            keys: shift.keys.clone(),
            change: shift.change(),
        });
        let landing = match shift.duplicate {
            true => shift.moved(),
            false => shift.keys.clone(),
        };
        let valuing = landing
            .iter()
            .zip(&shift.keys)
            .filter(|_| valued && shift.rise != 0.0)
            .filter_map(|(moved, from)| {
                let curve = plotting.curve(&from.property)?;
                let base = clip
                    .channel(&from.property)?
                    .value(from.at - clip.start())?
                    .scalar()?;
                let value = f64::from(base) + plotting.value_by(curve, shift.rise);
                Some(Edit::Key {
                    clip: id,
                    property: moved.property.clone(),
                    change: KeyChange::Value {
                        at: moved.at,
                        value: Value::Scalar(value as f32),
                    },
                })
            });
        match shift.duplicate {
            true => moving.into_iter().chain(valuing).collect(),
            false => valuing.chain(moving).collect(),
        }
    }
}

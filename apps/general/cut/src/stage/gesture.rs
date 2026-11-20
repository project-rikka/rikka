use std::collections::BTreeSet;

use cut_project::{Appearance, ClipId, Edit, Redraw, Viewpoint};
use rikka_app::prelude::*;
use rikka_figure::{Keys, constrained_span};
use rikka_ui::DRAG_SLOP;

use super::Stage;
use super::anchor::Anchoring;
use super::framing::Reframing;
use super::items::{HIT_REACH, Item, appearances, group, text_hit};
use super::manipulating::Manipulating;
use super::route::{Grab, HandleDrag};
use super::spinning::Spinning;
use super::viewing::Viewing;
use crate::cut::Cut;
use crate::tool::Tool;

#[derive(Clone, Debug)]
pub(crate) enum Gesture {
    Manipulating(Manipulating),
    Anchoring(Anchoring),
    Spinning(Spinning),
    HandleDrag(HandleDrag),
    Reframing(Reframing),
    Drawing {
        tool: Tool,
        anchor: Point,
        from: Point,
        to: Point,
        start: Point,
        end: Point,
    },
    Banding {
        from: Point,
        to: Point,
        base: Vec<ClipId>,
    },
    Selecting {
        clip: ClipId,
    },
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct HeldKeys {
    pub(crate) keys: Keys,
    pub(crate) control: bool,
}

impl Gesture {
    pub(crate) fn redrawn(&self) -> Option<(Vec<(ClipId, Appearance)>, Redraw)> {
        match self {
            Gesture::Manipulating(manipulating) if manipulating.has_moved() => Some((
                appearances(manipulating.reached().to_vec()),
                manipulating.redraw(),
            )),
            Gesture::Anchoring(anchoring) if anchoring.has_moved() => {
                Some((appearances(vec![anchoring.preview()]), Redraw::Move))
            }
            Gesture::Spinning(spinning) if spinning.has_moved() => {
                Some((appearances(vec![spinning.preview()]), spinning.redraw()))
            }
            Gesture::Reframing(reframing) if reframing.has_moved() => {
                Some((reframing.changes(), reframing.redraw()))
            }
            _ => None,
        }
    }

    pub(crate) fn viewpoint(&self) -> Option<Viewpoint> {
        match self {
            Gesture::Reframing(reframing) => Some(reframing.viewpoint()),
            _ => None,
        }
    }
}

fn select(ctx: &mut EventCtx<'_>, clips: Vec<ClipId>) {
    ctx.run(move |cut: &mut Cut| cut.select(clips.clone()));
}

impl Stage {
    pub(super) fn press(
        &self,
        gesture: &mut Option<Gesture>,
        position: Point,
        modifiers: Modifiers,
        ctx: &mut EventCtx<'_>,
    ) {
        let format = self.format();
        let viewing = Viewing::new(ctx.bounds(), format);
        let at = viewing.to_stage(position);
        let reach = HIT_REACH / viewing.scale.max(f32::EPSILON);
        let items = self.items();
        if let Some(draft) = self.typing() {
            let typed = items
                .iter()
                .find(|item| item.clip == draft.clip && item.hits(format, at, reach));
            if let Some(item) = typed {
                if let Some(hit) = text_hit(item, at) {
                    draft
                        .document
                        .place_cursor(hit.at, hit.affinity, modifiers.shift);
                }
                *gesture = Some(Gesture::Selecting { clip: item.clip });
                return;
            }
            ctx.run(Cut::stop_typing);
        }
        match self.tool {
            tool if tool.draws() => {
                *gesture = Some(Gesture::Drawing {
                    tool,
                    anchor: at,
                    from: at,
                    to: at,
                    start: position,
                    end: position,
                });
                return;
            }
            Tool::Anchor => {
                match self.anchor_target(&items, at, position, &viewing) {
                    Some(item) => {
                        if self.selected.len() != 1 || !self.selected.contains(&item.clip) {
                            select(ctx, vec![item.clip]);
                        }
                        *gesture = Anchoring::grab(item, at, position).map(Gesture::Anchoring);
                    }
                    None => self.press_empty(gesture, position, modifiers, ctx),
                }
                return;
            }
            Tool::Text => {
                let text = self
                    .hit(&items, at, reach)
                    .filter(|item| item.visual.is_text());
                match text {
                    Some(item) => {
                        let clip = item.clip;
                        let hit = text_hit(item, at);
                        ctx.run(move |cut: &mut Cut| cut.type_into(clip, hit));
                        *gesture = Some(Gesture::Selecting { clip });
                    }
                    None => {
                        ctx.run(move |cut: &mut Cut| cut.write_at(at));
                    }
                }
                return;
            }
            _ => {}
        }
        if let Some(framing) = self.framing(None) {
            if let Some(camera_gesture) = framing.hit(position, &viewing, modifiers.alt) {
                *gesture = Some(Gesture::Reframing(framing.reframe(
                    camera_gesture,
                    at,
                    position,
                )));
                return;
            }
            self.press_empty(gesture, position, modifiers, ctx);
            return;
        }
        if let Some(grabbed) = self.press_selected(&items, at, position, modifiers, &viewing) {
            *gesture = Some(grabbed);
            return;
        }
        if let Some(item) = self.hit(&items, at, reach) {
            self.press_item(gesture, item, &items, position, modifiers, ctx);
            return;
        }
        self.press_empty(gesture, position, modifiers, ctx);
    }

    fn press_selected(
        &self,
        items: &[Item],
        at: Point,
        position: Point,
        modifiers: Modifiers,
        viewing: &Viewing,
    ) -> Option<Gesture> {
        let format = self.format();
        let selected = self.selected_items(items);
        let single = match selected[..] {
            [item] => Some(item),
            _ => None,
        };
        let route = single.and_then(|item| self.route(item, &self.overrides()));
        let hidden = (!modifiers.alt).then(|| self.transport.frame());
        let spot = route
            .as_ref()
            .and_then(|route| route.spot_at(position, viewing, hidden));
        let toggles = modifiers.control || modifiers.alt;
        let steer = |spot| {
            route
                .as_ref()
                .and_then(|route| route.steer(spot, at, position, toggles))
                .map(Gesture::HandleDrag)
        };
        if let Some(spot @ (_, Grab::Handle(_))) = spot {
            return steer(spot);
        }
        if let Some((grips, projection)) = group(&selected, format)
            && let Some(grip) = projection.grip_at(&grips, viewing, position)
        {
            return Manipulating::shaping(
                grip,
                (grips, projection),
                &selected,
                items,
                at,
                position,
                format,
            )
            .map(Gesture::Manipulating);
        }
        if let Some(spot @ (_, Grab::Key)) = spot {
            return steer(spot);
        }
        let item = single?;
        self.gimbal(item, self.view(), &self.overrides(), viewing)?
            .grab(at, position)
            .map(Gesture::Spinning)
    }

    fn press_empty(
        &self,
        gesture: &mut Option<Gesture>,
        position: Point,
        modifiers: Modifiers,
        ctx: &mut EventCtx<'_>,
    ) {
        let base: Vec<ClipId> = match modifiers.shift || modifiers.control {
            true => self.selected.iter().copied().collect(),
            false => Vec::new(),
        };
        select(ctx, base.clone());
        *gesture = Some(Gesture::Banding {
            from: position,
            to: position,
            base,
        });
    }

    fn press_item(
        &self,
        gesture: &mut Option<Gesture>,
        item: &Item,
        items: &[Item],
        position: Point,
        modifiers: Modifiers,
        ctx: &mut EventCtx<'_>,
    ) {
        let format = self.format();
        let at = Viewing::new(ctx.bounds(), format).to_stage(position);
        let clip = item.clip;
        if ctx.presses() >= 2 && item.visual.is_text() {
            let hit = text_hit(item, at);
            ctx.run(move |cut: &mut Cut| cut.type_into(clip, hit));
            *gesture = Some(Gesture::Selecting { clip });
            return;
        }
        let adding = modifiers.shift || modifiers.control;
        let already_selected = self.selected.contains(&clip);
        let selected = self.selected_items(items);
        let (moving, solo) = match (adding, already_selected) {
            (true, true) => {
                let left: Vec<ClipId> = self
                    .selected
                    .iter()
                    .copied()
                    .filter(|id| *id != clip)
                    .collect();
                select(ctx, left);
                return;
            }
            (true, false) => {
                let mut more: BTreeSet<ClipId> = self.selected.iter().copied().collect();
                more.insert(clip);
                select(ctx, more.into_iter().collect());
                let mut moving = selected;
                moving.push(item);
                (moving, None)
            }
            (false, true) => (selected, Some(clip)),
            (false, false) => {
                select(ctx, vec![clip]);
                (vec![item], None)
            }
        };
        *gesture = Manipulating::moving(&moving, items, at, position, format, solo)
            .map(Gesture::Manipulating);
    }

    pub(super) fn drag(
        &self,
        gesture: &mut Option<Gesture>,
        held_keys: &HeldKeys,
        position: Point,
        ctx: &mut EventCtx<'_>,
    ) -> bool {
        let format = self.format();
        let viewing = Viewing::new(ctx.bounds(), format);
        let at = viewing.to_stage(position);
        let changed = match gesture {
            Some(Gesture::Manipulating(manipulating)) => manipulating.drag(
                at,
                position,
                held_keys.keys,
                self.snapping && !held_keys.control,
                &viewing,
                format,
            ),
            Some(Gesture::Anchoring(anchoring)) => anchoring.drag(at, position),
            Some(Gesture::Spinning(spinning)) => spinning.drag(at, position, held_keys.keys.shift),
            Some(Gesture::Reframing(reframing)) => reframing.drag(at, position),
            Some(Gesture::HandleDrag(handle_drag)) => {
                let Some(edit) = handle_drag.drag(at, position, held_keys.keys.alt) else {
                    return false;
                };
                ctx.run(move |cut: &mut Cut| cut.preview_edit(&edit));
                return true;
            }
            Some(Gesture::Drawing {
                tool,
                anchor,
                from,
                to,
                end,
                ..
            }) => {
                *end = position;
                if let Some(kind) = tool.form_kind() {
                    (*from, *to) = constrained_span(kind, *anchor, at, held_keys.keys);
                }
                true
            }
            Some(Gesture::Banding { to, .. }) => {
                *to = position;
                true
            }
            Some(Gesture::Selecting { clip }) => {
                if let (Some(draft), Some(item)) = (self.draft.as_ref(), self.item_of(*clip))
                    && let Some(hit) = text_hit(&item, at)
                {
                    draft.document.place_cursor(hit.at, hit.affinity, true);
                }
                true
            }
            None => false,
        };
        if changed && let Some((changes, by)) = gesture.as_ref().and_then(Gesture::redrawn) {
            let edit = Edit::Redraw {
                changes,
                by,
                at: self.transport.frame(),
            };
            ctx.run(move |cut: &mut Cut| cut.preview_edit(&edit));
        }
        changed
    }

    pub(super) fn release(&self, gesture: Option<Gesture>, ctx: &mut EventCtx<'_>) {
        let format = self.format();
        let viewing = Viewing::new(ctx.bounds(), format);
        if let Some((changes, by)) = gesture.as_ref().and_then(Gesture::redrawn) {
            ctx.run(move |cut: &mut Cut| cut.redraw(changes.clone(), by));
            return;
        }
        match gesture {
            Some(Gesture::Manipulating(manipulating)) => {
                if let Some(clip) = manipulating.solo() {
                    select(ctx, vec![clip]);
                }
            }
            Some(Gesture::HandleDrag(handle_drag)) => {
                let moved = handle_drag.has_moved();
                match handle_drag.finish() {
                    Some(edit) => {
                        ctx.run(move |cut: &mut Cut| cut.commit_edits(vec![edit.clone()]));
                    }
                    None if moved => self.live.settle(),
                    None => {}
                }
            }
            Some(Gesture::Drawing {
                tool,
                from,
                to,
                start,
                end,
                ..
            }) => {
                if start.distance(end) >= DRAG_SLOP {
                    ctx.run(move |cut: &mut Cut| cut.draw(tool, from, to));
                }
            }
            Some(Gesture::Banding { from, to, base }) => {
                let (low, high) = (
                    viewing.to_stage(from.min(to)),
                    viewing.to_stage(from.max(to)),
                );
                let band = Rect::new(low.x, low.y, high.x - low.x, high.y - low.y);
                let mut selected: BTreeSet<ClipId> = base.into_iter().collect();
                selected.extend(
                    self.items()
                        .iter()
                        .filter(|item| {
                            !item.locked
                                && item
                                    .bounds(format)
                                    .is_some_and(|bounds| bounds.intersects(&band))
                        })
                        .map(|item| item.clip),
                );
                select(ctx, selected.into_iter().collect());
            }
            Some(
                Gesture::Anchoring(_)
                | Gesture::Spinning(_)
                | Gesture::Reframing(_)
                | Gesture::Selecting { .. },
            )
            | None => {}
        }
    }
}

mod anchor;
mod framing;
mod gesture;
mod ghosts;
mod gimbal;
mod guides;
mod item_projection;
mod items;
mod manipulating;
mod paint;
mod route;
mod spinning;
mod typing;
mod viewing;

use std::collections::BTreeSet;
use std::sync::Arc;

use cut_project::{ClipId, Project, Redraw};
use rikka_app::prelude::*;
use rikka_figure::Keys;

use crate::cut::{Cut, Draft};
use crate::live::LivePreview;
use crate::playback::Preview;
use crate::quality::Quality;
use crate::tool::Tool;
use crate::transport::Transport;

use gesture::{Gesture, HeldKeys};
use items::{Item, ItemCache, appearances, group};
pub(crate) use viewing::Viewing;

const NUDGE: f32 = 1.0;
const LEAP: f32 = 10.0;

#[derive(Debug)]
pub(crate) struct Stage {
    pub(crate) preview: Preview,
    pub(crate) quality: Quality,
    pub(crate) project: Arc<Project>,
    pub(crate) transport: Transport,
    pub(crate) selected: Arc<BTreeSet<ClipId>>,
    pub(crate) tool: Tool,
    pub(crate) draft: Option<Draft>,
    pub(crate) live: LivePreview,
    pub(crate) snapping: bool,
    pub(crate) sighting: ItemCache,
}

#[derive(Default)]
pub(crate) struct PointerState {
    gesture: Option<Gesture>,
    held_keys: HeldKeys,
    caret: Option<Rect>,
    menu: ContextMenu,
}

impl std::fmt::Debug for PointerState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PointerState")
            .field("gesture", &self.gesture)
            .finish_non_exhaustive()
    }
}

impl Stage {
    fn typing(&self) -> Option<&Draft> {
        self.draft.as_ref().filter(|draft| draft.typing)
    }

    fn track_keys(state: &mut PointerState, key: Key, down: bool, modifiers: Modifiers) {
        let held_keys = &mut state.held_keys;
        held_keys.keys = Keys {
            shift: modifiers.shift,
            alt: modifiers.alt,
        };
        held_keys.control = modifiers.control;
        match key {
            Key::ShiftLeft | Key::ShiftRight => held_keys.keys.shift = down,
            Key::AltLeft | Key::AltRight => held_keys.keys.alt = down,
            Key::ControlLeft | Key::ControlRight => held_keys.control = down,
            _ => {}
        }
    }

    fn key(&self, key: Key, modifiers: Modifiers, ctx: &mut EventCtx<'_>) -> bool {
        let plain = !modifiers.control && !modifiers.alt && !modifiers.logo;
        let step = match modifiers.shift {
            true => LEAP,
            false => NUDGE,
        };
        let by = match key {
            Key::ArrowLeft => Point::new(-step, 0.0),
            Key::ArrowRight => Point::new(step, 0.0),
            Key::ArrowUp => Point::new(0.0, -step),
            Key::ArrowDown => Point::new(0.0, step),
            Key::Enter if plain => {
                let items = self.items();
                let [item] = self.selected_items(&items)[..] else {
                    return false;
                };
                if !item.visual.is_text() {
                    return false;
                }
                let clip = item.clip;
                ctx.run(move |cut: &mut Cut| cut.type_into(clip, None));
                return true;
            }
            _ => return false,
        };
        if modifiers.control || modifiers.alt {
            return false;
        }
        let items = self.items();
        let nudged: Vec<_> = self
            .selected_items(&items)
            .into_iter()
            .map(|item| (item.clip, item.visual.moved(by)))
            .collect();
        if nudged.is_empty() {
            return false;
        }
        let nudged = appearances(nudged);
        ctx.run(move |cut: &mut Cut| cut.redraw(nudged.clone(), Redraw::Move));
        true
    }

    fn selected_cursor(
        &self,
        items: &[Item],
        viewing: &Viewing,
        point: Point,
    ) -> Option<CursorShape> {
        let format = self.format();
        let at = viewing.to_stage(point);
        let selected = self.selected_items(items);
        let single = match selected[..] {
            [item] => Some(item),
            _ => None,
        };
        let playhead = Some(self.transport.frame());
        let steers = single
            .and_then(|item| self.route(item, &self.overrides()))
            .is_some_and(|route| route.spot_at(point, viewing, playhead).is_some());
        if steers {
            return Some(CursorShape::Crosshair);
        }
        if let Some((grips, projection)) = group(&selected, format)
            && let Some(grip) = projection.grip_at(&grips, viewing, point)
        {
            return Some(grips.cursor(grip));
        }
        let spins = single
            .and_then(|item| self.gimbal(item, self.view(), &self.overrides(), viewing))
            .is_some_and(|gimbal| gimbal.grab(at, point).is_some());
        spins.then_some(CursorShape::Grab)
    }

    fn scroll(&self, position: Point, delta: ScrollDelta, ctx: &mut EventCtx<'_>) {
        let viewing = Viewing::new(ctx.bounds(), self.format());
        let Some(framing) = self
            .framing(None)
            .filter(|framing| framing.contains(viewing.to_stage(position)))
        else {
            return;
        };
        let (camera, framed) = framing.dollied(delta.zoom());
        ctx.run(move |cut: &mut Cut| cut.dolly(camera, framed));
        ctx.set_handled();
        ctx.request_paint();
    }
}

impl Widget for Stage {
    type State = PointerState;

    fn paint(&self, state: &mut PointerState, ctx: &mut PaintCtx<'_>) {
        self.paint_stage(state, ctx);
        state.menu.paint(ctx);
    }

    fn event(&self, state: &mut PointerState, event: &Event, ctx: &mut EventCtx<'_>) {
        if state.menu.event(event, ctx) {
            return;
        }
        match event {
            Event::PointerPressed {
                position,
                button: PointerButton::Primary,
                modifiers,
            } => {
                ctx.capture_pointer();
                ctx.request_focus();
                ctx.set_handled();
                state.held_keys.keys = Keys {
                    shift: modifiers.shift,
                    alt: modifiers.alt,
                };
                state.held_keys.control = modifiers.control;
                self.press(&mut state.gesture, *position, *modifiers, ctx);
                ctx.request_paint();
            }
            Event::PointerPressed {
                position,
                button: PointerButton::Secondary,
                ..
            } if self.summon_text_menu(state, *position, ctx) => {}
            Event::PointerPressed {
                position,
                button: PointerButton::Secondary,
                ..
            } => {
                ctx.request_focus();
                ctx.set_handled();
                let format = self.format();
                let viewing = Viewing::new(ctx.bounds(), format);
                let at = viewing.to_stage(*position);
                let items = self.items();
                if self.framing(None).is_none()
                    && let Some(item) = self.hit(&items, at, 0.0)
                    && !self.selected.contains(&item.clip)
                {
                    let clip = item.clip;
                    ctx.run(move |cut: &mut Cut| cut.select(vec![clip]));
                }
            }
            Event::PointerMoved(position) => {
                let held_keys = state.held_keys;
                if self.drag(&mut state.gesture, &held_keys, *position, ctx) {
                    ctx.set_handled();
                    ctx.request_paint();
                }
            }
            Event::PointerReleased {
                button: PointerButton::Primary,
                ..
            } if state.gesture.is_some() => {
                ctx.release_pointer();
                ctx.set_handled();
                self.release(state.gesture.take(), ctx);
                ctx.request_paint();
            }
            Event::KeyPressed { key, modifiers, .. } => {
                Self::track_keys(state, *key, true, *modifiers);
                let handled = match self.typing() {
                    Some(_) => self.type_event(state, event, ctx),
                    None => self.key(*key, *modifiers, ctx),
                };
                if handled {
                    ctx.set_handled();
                    ctx.request_paint();
                }
            }
            Event::Scroll {
                position, delta, ..
            } => self.scroll(*position, *delta, ctx),
            Event::KeyReleased { key, modifiers, .. } => {
                Self::track_keys(state, *key, false, *modifiers);
            }
            Event::Command(_) | Event::Preedit { .. } | Event::Commit(_) => {
                if self.type_event(state, event, ctx) {
                    ctx.set_handled();
                    ctx.request_paint();
                }
            }
            Event::FocusLost if self.typing().is_some() => {
                self.type_event(state, event, ctx);
                ctx.run(Cut::stop_typing);
            }
            _ => {}
        }
    }

    fn cursor(&self, _state: &Self::State, bounds: Rect, point: Point) -> Option<CursorShape> {
        let format = self.format();
        let viewing = Viewing::new(bounds, format);
        let at = viewing.to_stage(point);
        let items = self.items();
        if let Some(draft) = self.typing()
            && items
                .iter()
                .any(|item| item.clip == draft.clip && item.hits(format, at, 0.0))
        {
            return Some(CursorShape::Text);
        }
        if self.tool.draws() {
            return Some(CursorShape::Crosshair);
        }
        if self.tool == Tool::Text {
            return Some(CursorShape::Text);
        }
        if self.tool == Tool::Anchor {
            return Some(match self.anchor_target(&items, at, point, &viewing) {
                Some(_) => CursorShape::Move,
                None => CursorShape::Default,
            });
        }
        if let Some(framing) = self.framing(None) {
            return Some(
                framing
                    .cursor(point, &viewing)
                    .unwrap_or(CursorShape::Default),
            );
        }
        if let Some(cursor) = self.selected_cursor(&items, &viewing, point) {
            return Some(cursor);
        }
        if self.hit(&items, at, 0.0).is_some() {
            return Some(CursorShape::Move);
        }
        Some(CursorShape::Default)
    }

    fn caret(&self, state: &PointerState) -> Option<Rect> {
        state.caret
    }

    fn behavior(&self, _held: &PointerState) -> Behavior {
        Behavior::CLIPS
            | Behavior::TRACKS_HOVER
            | Behavior::FOCUSABLE
            | Behavior::NO_TAB_STOP
            | Behavior::TAKES_FOCUS.when(self.typing().is_some())
    }

    fn same(&self, other: &Stage) -> bool {
        self.preview.same(&other.preview)
            && self.quality == other.quality
            && Arc::ptr_eq(&self.project, &other.project)
            && self.transport == other.transport
            && Arc::ptr_eq(&self.selected, &other.selected)
            && self.tool == other.tool
            && self.live.same(&other.live)
            && self.snapping == other.snapping
            && match (&self.draft, &other.draft) {
                (None, None) => true,
                (Some(one), Some(other)) => one.same(other),
                _ => false,
            }
    }
}

impl View for Stage {
    fn build(self) -> Element {
        Element::new(self)
    }
}

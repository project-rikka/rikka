use std::time::{Duration, Instant};

use desk_layout::{Content, Guide, REACH, Snapper, Widget, WidgetId, vacant};
use rikka_types::{Key, Modifiers, Point, PointerButton, Rect, ScrollDelta};
use rikka_ui::{Command, DRAG_SLOP, Event, EventCtx, ScrollSource};

use super::{Drag, EditorCanvas, EditorCanvasState, Press, Pressed};
use crate::session::{Draft, DragPreview, Hovered, Offer, Tool};
use crate::stage::Editor;

const NUDGE: f32 = 1.0;
const LEAP: f32 = 10.0;
const WHEEL: f32 = 120.0;
const STRIP_SLACK: f32 = 8.0;
const GUIDING: Duration = Duration::from_millis(900);

impl EditorCanvas {
    pub(super) fn respond(
        &self,
        state: &mut EditorCanvasState,
        event: &Event,
        ctx: &mut EventCtx<'_>,
    ) {
        match event {
            Event::PointerPressed {
                position,
                button: PointerButton::Primary,
                modifiers,
            } => self.pressed(state, *position, *modifiers, ctx),
            Event::PointerMoved(position) => self.moved(state, *position, ctx),
            Event::PointerReleased {
                position,
                button: PointerButton::Primary,
                ..
            } => self.released(state, *position, ctx),
            Event::PointerLeft | Event::PointerExited => {
                if state.drag.is_none() {
                    state.thumb = None;
                    self.sheet.session.mark(|marks| marks.hovered = None);
                    ctx.request_paint();
                }
            }
            Event::Scroll {
                position,
                delta,
                source,
                ..
            } => self.scrolled(state, *position, *delta, *source, ctx),
            Event::KeyPressed { key, modifiers, .. } => {
                if self.pressed_key(state, *key, *modifiers, ctx) {
                    ctx.set_handled();
                }
            }
            Event::KeyReleased {
                key: Key::AltLeft | Key::AltRight,
                ..
            } => {
                state.alt = false;
                self.follow(state);
            }
            Event::Command(Command::Undo) => {
                ctx.set_handled();
                self.sheet.editing.undo();
            }
            Event::Command(Command::Redo) => {
                ctx.set_handled();
                self.sheet.editing.redo();
            }
            _ => {}
        }
    }

    fn pressed(
        &self,
        state: &mut EditorCanvasState,
        at: Point,
        modifiers: Modifiers,
        ctx: &mut EventCtx<'_>,
    ) {
        state.alt = modifiers.alt;
        let on = match self.sheet.session.take_offer() {
            Some(offer) => Pressed::Offer(offer),
            None => self.spot(state, at),
        };
        if !state.settled || on == Pressed::Panel {
            return;
        }
        ctx.set_handled();
        ctx.request_focus();
        if matches!(on, Pressed::Widget { .. } | Pressed::Offer(_)) {
            ctx.capture_pointer();
        }
        state.press = Some(Press { at, on });
    }

    fn moved(&self, state: &mut EditorCanvasState, at: Point, ctx: &mut EventCtx<'_>) {
        if let Some(drag) = &mut state.drag {
            drag.pointer = at;
            self.follow(state);
            return;
        }
        if let Some(press) = state.press.clone()
            && self.sheet.editable
            && press.at.distance(at) >= DRAG_SLOP
        {
            match press.on {
                Pressed::Widget { id, rect } => {
                    return self.lift(state, id, rect, press.at, at, ctx);
                }
                Pressed::Offer(offer) => return self.carry(state, offer, at, ctx),
                _ => {}
            }
        }
        self.hover(state, at, ctx);
    }

    fn released(&self, state: &mut EditorCanvasState, at: Point, ctx: &mut EventCtx<'_>) {
        let press = state.press.take();
        if ctx.captured() {
            ctx.release_pointer();
        }
        if let Some(drag) = state.drag.take() {
            ctx.set_handled();
            return self.dropped(drag, state, ctx);
        }
        let Some(press) = press else {
            return;
        };
        ctx.set_handled();
        let here = self.spot(state, at);
        match press.on {
            Pressed::Widget { id, .. } => select(Some(id), ctx),
            Pressed::Offer(offer) => {
                let kind = offer.kind;
                ctx.run(move |editor: &mut Editor| editor.add(kind, kind.default_size()));
            }
            Pressed::Card if here == Pressed::Card => select(None, ctx),
            Pressed::Thumb(index) if here == Pressed::Thumb(index) => {
                ctx.run(move |editor: &mut Editor| editor.pick(index));
            }
            Pressed::Tool(tool) if here == Pressed::Tool(tool) => self.wield(tool, ctx),
            Pressed::Away if here == Pressed::Away => close(ctx),
            _ => {}
        }
    }

    fn lift(
        &self,
        state: &mut EditorCanvasState,
        id: WidgetId,
        rect: Rect,
        from: Point,
        at: Point,
        ctx: &mut EventCtx<'_>,
    ) {
        let (Some(projection), Some(widget)) = (state.hits.projection, self.sheet.layout.get(id))
        else {
            return;
        };
        let grabbed = projection.back(from);
        state.drag = Some(Drag {
            id,
            size: widget.size,
            grab: Point::new(grabbed.x - rect.min_x(), grabbed.y - rect.min_y()),
            adding: None,
            rect: None,
            pointer: at,
        });
        if self.sheet.selected != Some(id) {
            select(Some(id), ctx);
        }
        self.follow(state);
    }

    fn carry(
        &self,
        state: &mut EditorCanvasState,
        offer: Offer,
        at: Point,
        ctx: &mut EventCtx<'_>,
    ) {
        let size = offer.kind.default_size();
        let extent = size.extent();
        state.drag = Some(Drag {
            id: self.sheet.editing.fresh_id(),
            size,
            grab: Point::new(offer.grab.x * extent.width, offer.grab.y * extent.height),
            adding: Some(Content::new(offer.kind)),
            rect: None,
            pointer: at,
        });
        if self.sheet.selected.is_some() {
            select(None, ctx);
        }
        self.follow(state);
    }

    fn follow(&self, state: &mut EditorCanvasState) {
        let (Some(drag), Some(projection)) = (&mut state.drag, state.hits.projection) else {
            return;
        };
        let sheet = &self.sheet;
        let session = &sheet.session;
        let area = sheet.area;
        let extent = drag.size.extent();
        if let Some(content) = &drag.adding
            && !state.hits.card.contains(drag.pointer)
        {
            drag.rect = None;
            session.redraft(None);
            let drag_preview = DragPreview {
                kind: content.kind(),
                size: drag.size,
                at: drag.pointer,
                grab: Point::new(
                    drag.grab.x / extent.width.max(1.0),
                    drag.grab.y / extent.height.max(1.0),
                ),
            };
            session.mark(|marks| {
                marks.guides.clear();
                marks.hovered = None;
                marks.drag_preview = Some(drag_preview);
            });
            return;
        }
        let desk = projection.back(drag.pointer);
        let candidate = Rect::new(
            desk.x - drag.grab.x,
            desk.y - drag.grab.y,
            extent.width,
            extent.height,
        );
        let others = sheet.layout.rects_except(drag.id, &area);
        let snapper = Snapper::new(area.room(), &others).reach(projection.back_length(REACH));
        let contained = area.contain(candidate);
        let (rect, guides) = match state.alt {
            true => (contained, snapper.matches(contained)),
            false => {
                let snapped = snapper.snap(contained);
                let rect = area.contain(snapped.rect);
                match rect == snapped.rect {
                    true => (rect, snapped.guides),
                    false => (rect, snapper.matches(rect)),
                }
            }
        };
        drag.rect = Some(rect);
        let mut layout = sheet.layout.clone();
        let spot = area.spot_of(rect);
        match &drag.adding {
            Some(content) => layout.widgets.push(Widget {
                id: drag.id,
                size: drag.size,
                spot,
                content: content.clone(),
            }),
            None => {
                if let Some(moving) = layout.get_mut(drag.id) {
                    moving.spot = spot;
                }
            }
        }
        session.redraft(Some(Draft {
            layout,
            grabbed: Some(drag.id),
        }));
        session.mark(|marks| {
            marks.guides = guides;
            marks.fading = None;
            marks.hovered = None;
            marks.drag_preview = None;
        });
    }

    fn dropped(&self, drag: Drag, state: &mut EditorCanvasState, ctx: &mut EventCtx<'_>) {
        let sheet = &self.sheet;
        let area = sheet.area;
        sheet.session.mark(|marks| {
            marks.guides.clear();
            marks.drag_preview = None;
        });
        state.thumb = None;
        let Some(rect) = drag.rect else {
            sheet.session.redraft(None);
            return;
        };
        let others = sheet.layout.rects_except(drag.id, &area);
        let rect = vacant(rect, &others, &area);
        let id = match drag.adding {
            Some(content) => sheet
                .editing
                .insert(drag.id, content, drag.size, rect, &area),
            None => {
                sheet.editing.place(drag.id, rect, &area);
                drag.id
            }
        };
        sheet.session.redraft(None);
        select(Some(id), ctx);
    }

    fn cancel(&self, state: &mut EditorCanvasState, ctx: &mut EventCtx<'_>) {
        state.drag = None;
        state.press = None;
        if ctx.captured() {
            ctx.release_pointer();
        }
        self.sheet.session.redraft(None);
        self.sheet.session.mark(|marks| {
            marks.guides.clear();
            marks.drag_preview = None;
        });
    }

    fn hover(&self, state: &mut EditorCanvasState, at: Point, ctx: &mut EventCtx<'_>) {
        let thumb = state
            .hits
            .thumbs
            .iter()
            .find(|(_, rect)| rect.contains(at))
            .map(|(index, _)| *index);
        if thumb != state.thumb {
            state.thumb = thumb;
            ctx.request_paint();
        }
        let hovered = match self.spot(state, at) {
            Pressed::Tool(tool) => Some(Hovered::Tool(tool)),
            Pressed::Widget { id, .. } => Some(Hovered::Widget(id)),
            _ => None,
        };
        let hovered = hovered.filter(|_| state.settled);
        self.sheet.session.mark(|marks| marks.hovered = hovered);
    }

    fn scrolled(
        &self,
        state: &mut EditorCanvasState,
        at: Point,
        delta: ScrollDelta,
        source: ScrollSource,
        ctx: &mut EventCtx<'_>,
    ) {
        let strip = state.hits.strip;
        let within = at.y >= strip.min_y() - STRIP_SLACK && at.y <= strip.max_y() + STRIP_SLACK;
        if state.hits.overflow <= 0.0 || !within {
            return;
        }
        ctx.set_handled();
        let offset = delta.offset();
        let along = match offset.y.abs() >= offset.x.abs() {
            true => offset.y,
            false => offset.x,
        };
        let travel = match source {
            ScrollSource::Wheel => along.signum() * WHEEL,
            ScrollSource::Continuous(_) => along,
        };
        state.scroll = (state.scroll + travel).clamp(0.0, state.hits.overflow);
        ctx.request_paint();
    }

    fn pressed_key(
        &self,
        state: &mut EditorCanvasState,
        key: Key,
        modifiers: Modifiers,
        ctx: &mut EventCtx<'_>,
    ) -> bool {
        let selected = self.sheet.selected;
        match key.to_lowercase() {
            Key::AltLeft | Key::AltRight => {
                state.alt = true;
                self.follow(state);
                return false;
            }
            Key::Escape if state.drag.is_some() => self.cancel(state, ctx),
            Key::Escape if selected.is_some() => select(None, ctx),
            Key::Escape => close(ctx),
            Key::Delete | Key::Backspace => {
                let Some(id) = selected else {
                    return false;
                };
                ctx.run(move |editor: &mut Editor| editor.remove(id));
            }
            Key::Char('d') if modifiers.control => {
                let Some(id) = selected else {
                    return false;
                };
                ctx.run(move |editor: &mut Editor| editor.duplicate(id));
            }
            Key::Tab => self.cycle(if modifiers.shift { -1 } else { 1 }, ctx),
            Key::ArrowLeft if !modifiers.control => return self.nudge(-1.0, 0.0, modifiers),
            Key::ArrowRight if !modifiers.control => return self.nudge(1.0, 0.0, modifiers),
            Key::ArrowUp if !modifiers.control => return self.nudge(0.0, -1.0, modifiers),
            Key::ArrowDown if !modifiers.control => return self.nudge(0.0, 1.0, modifiers),
            _ => return false,
        }
        true
    }

    fn nudge(&self, x: f32, y: f32, modifiers: Modifiers) -> bool {
        let sheet = &self.sheet;
        let Some(id) = sheet.selected.filter(|_| sheet.editable) else {
            return false;
        };
        let step = match modifiers.shift {
            true => LEAP,
            false => NUDGE,
        };
        let area = sheet.area;
        let Some(rect) = sheet
            .editing
            .nudge(id, Point::new(x * step, y * step), &area)
        else {
            return false;
        };
        let others = sheet.layout.rects_except(id, &area);
        let guides: Vec<Guide> = Snapper::new(area.room(), &others).matches(rect);
        sheet.session.mark(|marks| {
            marks.guides = guides;
            marks.fading = Some(Instant::now() + GUIDING);
        });
        true
    }

    fn cycle(&self, by: i32, ctx: &mut EventCtx<'_>) {
        let area = self.sheet.area;
        let mut order = self.sheet.layout.rects(&area);
        if order.is_empty() {
            return;
        }
        order.sort_by(|(_, a), (_, b)| {
            a.min_y()
                .total_cmp(&b.min_y())
                .then(a.min_x().total_cmp(&b.min_x()))
        });
        let count = order.len() as i32;
        let next = match self
            .sheet
            .selected
            .and_then(|id| order.iter().position(|(widget, _)| *widget == id))
        {
            Some(at) => (at as i32 + by).rem_euclid(count),
            None if by < 0 => count - 1,
            None => 0,
        };
        select(Some(order[next as usize].0), ctx);
    }

    fn wield(&self, tool: Tool, ctx: &mut EventCtx<'_>) {
        let Some(id) = self.sheet.selected else {
            return;
        };
        ctx.run(move |editor: &mut Editor| match tool {
            Tool::Size(size) => editor.resize(id, size),
            Tool::Remove => editor.remove(id),
        });
    }

    fn spot(&self, state: &EditorCanvasState, at: Point) -> Pressed {
        if let Some(tool) = self.sheet.session.tool_at(at) {
            return Pressed::Tool(tool);
        }
        if let Some((index, _)) = state.hits.thumbs.iter().find(|(_, rect)| rect.contains(at)) {
            return Pressed::Thumb(*index);
        }
        if self.sheet.panel.contains(at) {
            return Pressed::Panel;
        }
        if !state.hits.card.contains(at) {
            return Pressed::Away;
        }
        match self.widget_at(state, at) {
            Some((id, rect)) => Pressed::Widget { id, rect },
            None => Pressed::Card,
        }
    }

    fn widget_at(&self, state: &EditorCanvasState, at: Point) -> Option<(WidgetId, Rect)> {
        let projection = state.hits.projection?;
        self.sheet
            .layout
            .widget_at(&self.sheet.area, projection.back(at))
    }
}

fn select(widget: Option<WidgetId>, ctx: &mut EventCtx<'_>) {
    ctx.run(move |editor: &mut Editor| editor.select(widget));
}

fn close(ctx: &mut EventCtx<'_>) {
    ctx.run(Editor::close);
}

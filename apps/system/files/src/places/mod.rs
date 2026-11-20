mod row;

use std::rc::Rc;
use std::time::Instant;

use files_core::Location;
use rikka_app::prelude::{Event as Input, *};
use rikka_ui::{Element, Handler, IntoHandler, View};

pub(crate) use row::{Intent, Row, rows};

const ROW: f32 = 30.0;
const HEADING: f32 = 28.0;
const PADDING: f32 = 4.0;
const INSET: f32 = 10.0;
const ICON: f32 = 18.0;
const GAP: f32 = 10.0;
const ACTION: f32 = 26.0;
const LABEL: f32 = 13.0;
const HEADING_LABEL: f32 = 11.0;
const DRAG: f32 = 4.0;

#[derive(Debug, Default)]
pub(crate) struct State {
    busy_since: Option<Instant>,
    highlights: Highlights,
    hovered: Option<usize>,
    on_action: bool,
    cursor: Option<usize>,
    pressed: Option<Press>,
    target: Option<usize>,
}

#[derive(Debug)]
struct Press {
    index: usize,
    at: Point,
    dragging: bool,
}

pub(crate) struct Places {
    rows: Rc<Vec<Row>>,
    selected: Option<usize>,
    managed: bool,
    on_open: Option<Handler<(usize,)>>,
    on_open_tab: Option<Handler<(usize,)>>,
    on_trailing: Option<Handler<(usize,)>>,
    on_context: Option<Handler<(usize, Point)>>,
    on_reorder: Option<Handler<(Location, Location)>>,
}

impl Places {
    pub(crate) fn new(rows: Rc<Vec<Row>>, here: Option<&Location>) -> Self {
        let selected = here.and_then(|here| {
            rows.iter()
                .position(|row| row.at.as_ref() == Some(here) && !row.is_heading())
        });
        Self {
            rows,
            selected,
            managed: true,
            on_open: None,
            on_open_tab: None,
            on_trailing: None,
            on_context: None,
            on_reorder: None,
        }
    }

    pub(crate) fn height(&self) -> f32 {
        self.rows.iter().map(entry_height).sum::<f32>() + PADDING * 2.0
    }

    #[must_use]
    pub(crate) fn managed(mut self, managed: bool) -> Self {
        self.managed = managed;
        self
    }

    #[must_use]
    pub(crate) fn on_open<M>(mut self, on_open: impl IntoHandler<(usize,), (), M>) -> Self {
        self.on_open = Some(on_open.into_handler());
        self
    }

    #[must_use]
    pub(crate) fn on_open_tab<M>(mut self, on_open_tab: impl IntoHandler<(usize,), (), M>) -> Self {
        self.on_open_tab = Some(on_open_tab.into_handler());
        self
    }

    #[must_use]
    pub(crate) fn on_trailing<M>(mut self, on_trailing: impl IntoHandler<(usize,), (), M>) -> Self {
        self.on_trailing = Some(on_trailing.into_handler());
        self
    }

    #[must_use]
    pub(crate) fn on_context<M>(
        mut self,
        on_context: impl IntoHandler<(usize, Point), (), M>,
    ) -> Self {
        self.on_context = Some(on_context.into_handler());
        self
    }

    #[must_use]
    pub(crate) fn on_reorder<M>(
        mut self,
        on_reorder: impl IntoHandler<(Location, Location), (), M>,
    ) -> Self {
        self.on_reorder = Some(on_reorder.into_handler());
        self
    }

    fn open(&self, index: usize, ctx: &mut EventCtx<'_>) {
        if let Some(on_open) = &self.on_open {
            ctx.call(on_open, (index,));
        }
    }

    fn rects(&self, bounds: Rect) -> Vec<Rect> {
        let width = (bounds.size.width - PADDING * 2.0).max(0.0);
        let mut top = bounds.origin.y + PADDING;
        self.rows
            .iter()
            .map(|row| {
                let rect = Rect::new(bounds.origin.x + PADDING, top, width, entry_height(row));
                top = rect.max_y();
                rect
            })
            .collect()
    }

    fn hit(&self, rects: &[Rect], at: Point) -> Option<usize> {
        rects
            .iter()
            .position(|rect| at.y >= rect.origin.y && at.y < rect.max_y())
    }

    fn action_rect(&self, rect: Rect, index: usize) -> Option<Rect> {
        if !self.managed {
            return None;
        }
        self.rows.get(index)?.trailing()?;
        Some(Rect::new(
            rect.max_x() - ACTION - 2.0,
            rect.origin.y + (rect.size.height - ACTION) / 2.0,
            ACTION,
            ACTION,
        ))
    }

    fn selectable(&self, index: usize) -> bool {
        self.rows.get(index).is_some_and(|row| !row.is_heading())
    }

    fn step(&self, from: Option<usize>, forward: bool) -> Option<usize> {
        let Some(from) = from else {
            return match forward {
                true => (0..self.rows.len()).find(|index| self.selectable(*index)),
                false => (0..self.rows.len()).rfind(|index| self.selectable(*index)),
            };
        };
        let next = match forward {
            true => (from + 1..self.rows.len()).find(|index| self.selectable(*index)),
            false => (0..from).rfind(|index| self.selectable(*index)),
        };
        next.or(Some(from))
    }

    fn drop_target(&self, rects: &[Rect], at: Point) -> Option<usize> {
        let first = self.rows.iter().position(Row::is_pin)?;
        let last = self.rows.iter().rposition(Row::is_pin)?;
        let index = self
            .hit(rects, at)
            .unwrap_or(if at.y < rects[first].origin.y {
                first
            } else {
                last
            });
        Some(index.clamp(first, last))
    }
}

impl Widget for Places {
    type State = State;

    fn paint(&self, state: &mut State, ctx: &mut PaintCtx<'_>) {
        let palette = palette();
        let radius = Corner::Item.radius();
        let busy = ctx.now() - *state.busy_since.get_or_insert(ctx.now());
        let metrics = TextRun::new("", LABEL).no_wrap();
        let rects = self.rects(ctx.bounds());
        let dragging = state
            .pressed
            .as_ref()
            .is_some_and(|pressed| pressed.dragging);

        let hovered = state
            .hovered
            .filter(|index| !dragging && *index < rects.len());
        let lighting = state.highlights.paint(
            ctx,
            Axis::Vertical,
            |index| rects[index],
            self.selected,
            hovered,
        );

        for (index, row) in self.rows.iter().enumerate() {
            let rect = rects[index];
            if row.is_heading() {
                let at = Rect::new(
                    rect.origin.x + INSET,
                    rect.max_y() - 16.0,
                    rect.size.width - INSET,
                    16.0,
                );
                ctx.draw_text(
                    at,
                    TextRun::new(row.label.as_str(), HEADING_LABEL),
                    palette.text_muted,
                );
                continue;
            }

            if state.cursor == Some(index) && self.selected != Some(index) && ctx.focused() {
                ctx.draw_quad(
                    Quad::new(rect)
                        .fill(Color::TRANSPARENT)
                        .stroke(1.0, palette.accent)
                        .radius(radius),
                );
            }

            let ink = if row.at.is_none() {
                palette.text_muted
            } else {
                palette
                    .text
                    .mix(palette.on_selection, lighting.emphasis(rect))
            };
            ctx.draw_icon(
                Rect::new(
                    rect.origin.x + INSET,
                    rect.origin.y + (rect.size.height - ICON) / 2.0,
                    ICON,
                    ICON,
                ),
                IconGlyph::from(row.icon).spec(ICON),
                ink,
            );

            let trailing = if self.managed { row.trailing() } else { None };
            let reserved = if trailing.is_some() || row.busy {
                ACTION + 4.0
            } else {
                INSET
            };
            let room = rect.size.width - INSET - ICON - GAP - reserved;
            let shown = fitted_line(&metrics.clone().content(row.label.as_str()), room);
            ctx.draw_text(
                Rect::new(
                    rect.origin.x + INSET + ICON + GAP,
                    rect.origin.y + (rect.size.height - 17.0) / 2.0,
                    shown.width + 2.0,
                    17.0,
                ),
                TextRun::new(shown.text.as_str(), LABEL),
                ink,
            );

            let Some(button) = self.action_rect(rect, index) else {
                continue;
            };
            if row.busy {
                let beat = (busy.as_secs_f32() * 3.0).sin().mul_add(0.35, 0.55);
                ctx.draw_icon(
                    button,
                    IconGlyph::from(MaterialSymbol::PROGRESS_ACTIVITY).spec(ICON),
                    palette.text_muted.scale_alpha(beat),
                );
                ctx.request_frame();
                continue;
            }
            let Some(symbol) = trailing else { continue };
            let hovered = state.hovered == Some(index);
            if hovered && state.on_action {
                ctx.draw_quad(Quad::new(button).fill(theme().press_layer()).radius(radius));
            }
            let lit = hovered || self.selected == Some(index);
            ctx.draw_icon(
                button,
                IconGlyph::from(symbol).spec(16.0),
                if lit { ink } else { palette.text_muted },
            );
        }

        if let (Some(pressed), Some(target)) = (state.pressed.as_ref(), state.target)
            && pressed.dragging
        {
            let rect = rects[target];
            let edge = if target > pressed.index {
                rect.max_y() - 1.0
            } else {
                rect.origin.y - 1.0
            };
            ctx.draw_quad(
                Quad::new(Rect::new(rect.origin.x, edge, rect.size.width, 2.0))
                    .fill(palette.accent)
                    .radius(1.0),
            );
        }
    }

    fn event(&self, state: &mut State, event: &Input, ctx: &mut EventCtx<'_>) {
        let bounds = ctx.bounds();
        match event {
            Input::PointerMoved(position) => self.moved(state, *position, bounds, ctx),
            Input::PointerExited | Input::PointerLeft => {
                if state.hovered.take().is_some() {
                    state.on_action = false;
                    ctx.request_paint();
                }
            }
            Input::PointerPressed {
                button, position, ..
            } => self.pressed(state, *button, *position, bounds, ctx),
            Input::PointerReleased { .. } => self.released(state, ctx),
            Input::KeyPressed { key, .. } if ctx.focused() => self.typed(state, *key, ctx),
            _ => {}
        }
    }

    fn behavior(&self, _state: &Self::State) -> Behavior {
        Behavior::TRACKS_HOVER | Behavior::FOCUSABLE
    }
}

impl Places {
    fn moved(&self, state: &mut State, at: Point, bounds: Rect, ctx: &mut EventCtx<'_>) {
        let rects = self.rects(bounds);
        let hovered = self.hit(&rects, at).filter(|index| self.selectable(*index));
        let on_action = hovered.is_some_and(|index| {
            self.action_rect(rects[index], index)
                .is_some_and(|button| button.contains(at))
        });
        if state.hovered != hovered || state.on_action != on_action {
            state.hovered = hovered;
            state.on_action = on_action;
            ctx.request_paint();
        }
        let Some(pressed) = state.pressed.as_mut() else {
            return;
        };
        if !pressed.dragging {
            if (at.y - pressed.at.y).abs() < DRAG || !self.managed {
                return;
            }
            if !self.rows[pressed.index].is_pin() {
                return;
            }
            pressed.dragging = true;
            ctx.capture_pointer();
        }
        state.target = self.drop_target(&rects, at);
        ctx.request_paint();
    }

    fn pressed(
        &self,
        state: &mut State,
        button: PointerButton,
        at: Point,
        bounds: Rect,
        ctx: &mut EventCtx<'_>,
    ) {
        let rects = self.rects(bounds);
        let Some(index) = self.hit(&rects, at).filter(|index| self.selectable(*index)) else {
            return;
        };
        ctx.set_handled();
        ctx.request_focus();
        state.cursor = Some(index);
        match button {
            PointerButton::Secondary if self.managed => {
                if let Some(on_context) = &self.on_context {
                    let at = Point::new(at.x - bounds.origin.x, at.y - bounds.origin.y);
                    ctx.call(on_context, (index, at));
                }
            }
            PointerButton::Middle => {
                if let Some(on_open_tab) = &self.on_open_tab {
                    ctx.call(on_open_tab, (index,));
                }
            }
            PointerButton::Primary => {
                if self
                    .action_rect(rects[index], index)
                    .is_some_and(|button| button.contains(at))
                {
                    if let Some(on_trailing) = &self.on_trailing {
                        ctx.call(on_trailing, (index,));
                    }
                    return;
                }
                state.pressed = Some(Press {
                    index,
                    at,
                    dragging: false,
                });
                self.open(index, ctx);
            }
            _ => {}
        }
    }

    fn released(&self, state: &mut State, ctx: &mut EventCtx<'_>) {
        let Some(pressed) = state.pressed.take() else {
            return;
        };
        let target = state.target.take();
        if !pressed.dragging {
            return;
        }
        ctx.release_pointer();
        ctx.request_paint();
        let (Some(moved), Some(before)) = (
            self.rows[pressed.index].at.clone(),
            target.and_then(|index| self.rows[index].at.clone()),
        ) else {
            return;
        };
        if moved != before
            && let Some(on_reorder) = &self.on_reorder
        {
            ctx.call(on_reorder, (moved, before));
        }
    }

    fn typed(&self, state: &mut State, key: Key, ctx: &mut EventCtx<'_>) {
        let moved = match key {
            Key::ArrowDown => self.step(state.cursor, true),
            Key::ArrowUp => self.step(state.cursor, false),
            Key::Home => self.step(None, true),
            Key::End => self.step(None, false),
            Key::Enter | Key::Space => {
                if let Some(index) = state.cursor {
                    ctx.set_handled();
                    self.open(index, ctx);
                }
                return;
            }
            _ => return,
        };
        let Some(index) = moved else { return };
        ctx.set_handled();
        state.cursor = Some(index);
        ctx.request_paint();
    }
}

impl View for Places {
    fn build(self) -> Element {
        Element::new(self)
    }
}

fn entry_height(row: &Row) -> f32 {
    if row.is_heading() { HEADING } else { ROW }
}

impl std::fmt::Debug for Places {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Places")
            .field("rows", &self.rows.len())
            .field("selected", &self.selected)
            .finish_non_exhaustive()
    }
}

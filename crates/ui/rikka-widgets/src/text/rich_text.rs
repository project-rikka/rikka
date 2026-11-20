use std::hash::{Hash, Hasher};
use std::ops::Range;
use std::sync::Arc;

use rikka_symbols::MaterialSymbol;
use rikka_text::{Affinity, TextLayout, TextSpan};
use rikka_theme::Corner;
use rikka_types::{Color, CursorShape, Key, Modifiers, Point, PointerButton, Rect};
use rikka_ui::search::{Found, Marking};
use rikka_ui::text::selection::{self, Part, Placement, Region};
use rikka_ui::text::{Granularity, TextRun, TextStyle, TextStyled};
use rikka_ui::{
    Action, Behavior, Command, DRAG_SLOP, DragActions, DragStart, Env, Event, EventCtx,
    IntoHandler, Layout, MeasureSpec, Offer, PaintCtx, Quad, Reveal, Style, Widget, clipboard,
    motion,
};

use crate::dragging::drag_chip;
use crate::navigation::menu::{ContextMenu, MenuItem, command_item, menu_separator};
use crate::text::caret;
use crate::text::unveil::RevealState;

const FILL_INSET: f32 = 2.0;
const FILL_HEIGHT: f32 = 1.25;

fn digest(text: &str) -> u64 {
    let mut hasher = rustc_hash::FxHasher::default();
    text.hash(&mut hasher);
    hasher.finish()
}

const BAND_ALPHA: f32 = 0.34;

#[derive(Clone, Debug, PartialEq)]
pub struct SpanFill {
    pub range: Range<usize>,
    pub color: Color,
    pub radius: f32,
}

impl SpanFill {
    pub fn new(range: Range<usize>, color: Color) -> Self {
        Self {
            range,
            color,
            radius: Corner::Detail.radius(),
        }
    }
}

#[must_use]
#[derive(Widget)]
pub struct RichText {
    #[debug]
    content: Arc<str>,
    spans: Arc<[TextSpan]>,
    fills: Arc<[SpanFill]>,
    links: Arc<[Range<usize>]>,
    on_link: Action<(usize,)>,
    part: Option<Part>,
    found: Option<Found>,
    unveiling: bool,
    text_style: TextStyle,
    #[style]
    style: Style,
}

pub fn rich_text(content: impl Into<Arc<str>>) -> RichText {
    RichText {
        content: content.into(),
        spans: Arc::from(Vec::new()),
        fills: Arc::from(Vec::new()),
        links: Arc::from(Vec::new()),
        on_link: Action::default(),
        part: None,
        found: None,
        unveiling: false,
        text_style: TextStyle::default(),
        style: Style::default(),
    }
}

impl RichText {
    pub fn spans(mut self, spans: impl Into<Arc<[TextSpan]>>) -> Self {
        self.spans = spans.into();
        self
    }

    pub fn fills(mut self, fills: impl Into<Arc<[SpanFill]>>) -> Self {
        self.fills = fills.into();
        self
    }

    pub fn links(mut self, links: impl Into<Arc<[Range<usize>]>>) -> Self {
        self.links = links.into();
        self
    }

    pub fn on_link<Marker>(mut self, on_link: impl IntoHandler<(usize,), (), Marker>) -> Self {
        self.on_link = Action::new(on_link);
        self
    }

    pub fn selectable(mut self, key: impl Hash) -> Self {
        self.part = Some(Part::whole(Region::new(key), &self.content));
        self
    }

    pub fn found(mut self, found: Found) -> Self {
        self.found = Some(found);
        self
    }

    pub fn unveiling(mut self) -> Self {
        self.unveiling = true;
        self
    }

    pub fn part(mut self, part: Part) -> Self {
        self.part = Some(part);
        self
    }

    fn run(&self, env: &Env) -> TextRun {
        let mut run = self
            .text_style
            .run_in(Arc::clone(&self.content), env)
            .ellipsized();
        run.spans = Arc::clone(&self.spans);
        run
    }
}

#[derive(Debug, Default)]
#[doc(hidden)]
pub struct RichTextState {
    laid: Option<(TextRun, f32, Arc<TextLayout>)>,
    plates: Vec<(usize, Rect)>,
    bounds: Rect,
    placement: Placement,
    selecting: bool,
    press_origin: Option<Point>,
    owner: Option<u64>,
    link: Option<usize>,
    revealed: Option<(u64, usize)>,
    unveiled: RevealState,
    menu: ContextMenu,
}

impl RichTextState {
    fn bands(&self, range: Range<usize>) -> Vec<Rect> {
        let Some((_, _, laid)) = &self.laid else {
            return Vec::new();
        };
        laid.selection_rects(range)
            .into_iter()
            .map(|(_, band)| band)
            .collect()
    }
}

impl Widget for RichText {
    type State = RichTextState;

    fn layout(&self) -> Layout {
        Layout::DEFAULT.with(&self.style)
    }

    fn measure(&self) -> Option<MeasureSpec> {
        let env = &rikka_ui::env();
        Some(MeasureSpec::text(self.run(env)))
    }

    fn paint(&self, state: &mut RichTextState, ctx: &mut PaintCtx<'_>) {
        let bounds = ctx.bounds();
        state.bounds = bounds;
        let run = self.run(ctx.env());
        let target = self.text_style.fill(ctx.theme(), ctx.env());
        let color = ctx.animate_curve("color", target, motion::COLOR);

        let width = bounds.width();
        let stale = state
            .laid
            .as_ref()
            .is_none_or(|(laid, was, _)| *laid != run || *was != width);
        if stale {
            let laid = rikka_text::layout_approx(run.as_shape(), Some(width));
            state.laid = Some((run.clone(), width, laid));
            state.plates = self.plates(state, run.size);
        }

        let veil = state.unveiled.veil(&self.content, self.unveiling, ctx);
        for (fill, plate) in &state.plates {
            let fill = &self.fills[*fill];
            let plate = plate.translate(bounds.origin);
            let tint = fill.color.scale_alpha(veil.alpha_at(fill.range.start));
            ctx.draw_quad(Quad::new(plate).fill(tint).radius(fill.radius));
        }
        self.draw_found(state, ctx);
        self.draw_selection(state, ctx);
        if let Some((_, _, laid)) = &state.laid {
            let drawn = match veil.is_clear() {
                true => run,
                false => TextRun {
                    spans: veil.inks(&run.spans, color, self.content.len()).into(),
                    ..run
                },
            };
            ctx.draw_laid_text(bounds, drawn, Arc::clone(laid), color);
        }
        state.menu.paint(ctx);
    }

    fn event(&self, state: &mut RichTextState, event: &Event, ctx: &mut EventCtx<'_>) {
        if state.menu.event(event, ctx) {
            return;
        }
        match event {
            Event::PointerPressed {
                button: PointerButton::Secondary,
                position,
                ..
            } if ctx.bounds().contains(*position) => self.summon(state, *position, ctx),
            Event::Command(Command::ContextMenu) if ctx.focused() => {
                self.open_from_keyboard(state, ctx);
            }
            Event::PointerPressed {
                button: PointerButton::Primary,
                position,
                modifiers,
            } if ctx.bounds().contains(*position) => {
                state.link = self.link_at(state, *position);
                let Some(part) = &self.part else {
                    if state.link.is_some() {
                        ctx.set_handled();
                    }
                    return;
                };
                let (local, affinity) = self.cursor_at(state, *position).unwrap_or_default();
                if ctx.presses() == 1
                    && !modifiers.shift
                    && selection::range_in(part)
                        .is_some_and(|press_origin| press_origin.contains(&local))
                {
                    state.press_origin = Some(*position);
                    ctx.capture_pointer();
                    ctx.set_handled();
                    return;
                }
                let at = part.absolute(local);
                state.owner = Some(match ctx.presses() == 1 && modifiers.shift {
                    true => selection::extend(part, at, affinity),
                    false => selection::press(
                        part,
                        at,
                        affinity,
                        Granularity::from_click_count(ctx.presses()),
                    ),
                });
                state.selecting = true;
                ctx.capture_pointer();
                ctx.request_paint();
                ctx.set_handled();
            }
            Event::PointerMoved(position) if state.press_origin.is_some() => {
                ctx.set_handled();
                if state
                    .press_origin
                    .is_some_and(|press_origin| position.distance(press_origin) > DRAG_SLOP)
                {
                    state.press_origin = None;
                    ctx.release_pointer();
                    self.lift(ctx);
                }
            }
            Event::PointerMoved(position) if state.selecting => {
                ctx.set_handled();
                if let Some(part) = &self.part
                    && selection::drag(part, *position)
                {
                    ctx.request_paint();
                }
            }
            Event::PointerCancelled => {
                state.link = None;
                state.press_origin = None;
                state.selecting = false;
            }
            Event::PointerReleased {
                button: PointerButton::Primary,
                position,
                ..
            } => {
                if state.press_origin.take().is_some() {
                    ctx.release_pointer();
                    if let Some(part) = &self.part {
                        let (local, affinity) =
                            self.cursor_at(state, *position).unwrap_or_default();
                        state.owner = Some(selection::press(
                            part,
                            part.absolute(local),
                            affinity,
                            Granularity::Character,
                        ));
                        ctx.request_paint();
                    }
                    ctx.set_handled();
                }
                if state.selecting {
                    state.selecting = false;
                    ctx.release_pointer();
                    ctx.set_handled();
                }
                self.follow_link(state, *position, ctx);
            }
            Event::Command(command) if ctx.focused() => {
                let Some(part) = &self.part else {
                    return;
                };
                match command {
                    Command::Copy => {
                        let Some(text) = selection::selected(part) else {
                            return;
                        };
                        clipboard::set_text(&text);
                        ctx.set_handled();
                    }
                    Command::SelectAll => {
                        let Some(owner) = selection::select_all(part) else {
                            return;
                        };
                        state.owner = Some(owner);
                        ctx.request_paint();
                        ctx.set_handled();
                    }
                    _ => {}
                }
            }
            Event::KeyPressed {
                key: Key::Escape,
                modifiers,
                ..
            } if ctx.focused() && *modifiers == Modifiers::NONE => {
                let Some(part) = &self.part else {
                    return;
                };
                if selection::holds(part) {
                    selection::forget(part);
                    ctx.request_paint();
                    ctx.set_handled();
                }
            }
            Event::FocusLost => {
                if state.selecting || state.press_origin.take().is_some() {
                    state.selecting = false;
                    ctx.release_pointer();
                }
                state.link = None;
                if let Some(part) = &self.part
                    && let Some(owner) = state.owner.take()
                    && selection::forget_owned(part, owner)
                {
                    ctx.request_paint();
                }
            }
            _ => {}
        }
    }

    fn relocated(&self, state: &mut RichTextState, by: Point) {
        state.bounds = state.bounds.translate(by);
        state.placement.shift(by);
    }

    fn cursor(&self, _state: &Self::State, bounds: Rect, point: Point) -> Option<CursorShape> {
        let selectable = self.part.is_some() && bounds.contains(point);
        selectable.then_some(CursorShape::Text)
    }

    fn behavior(&self, _state: &Self::State) -> Behavior {
        Behavior::FOCUSABLE.when(self.part.is_some()) | Behavior::NO_TAB_STOP
    }

    fn same(&self, other: &Self) -> bool {
        self.part.is_none()
            && other.part.is_none()
            && self.content == other.content
            && shared(&self.spans, &other.spans)
            && shared(&self.fills, &other.fills)
            && shared(&self.links, &other.links)
            && self.on_link == other.on_link
            && self.found == other.found
            && self.unveiling == other.unveiling
            && self.text_style == other.text_style
            && self.style == other.style
    }
}

fn menu_items() -> [MenuItem; 3] {
    [
        command_item(Command::Copy),
        menu_separator(),
        command_item(Command::SelectAll),
    ]
}

fn shared<T: PartialEq>(press_origin: &Arc<[T]>, candidate: &Arc<[T]>) -> bool {
    Arc::ptr_eq(press_origin, candidate) || press_origin == candidate
}

impl RichText {
    fn lift(&self, ctx: &mut EventCtx<'_>) {
        let Some(text) = self.part.as_ref().and_then(selection::selected) else {
            return;
        };
        let icon = drag_chip(ctx.theme().clone(), MaterialSymbol::TEXT_FIELDS, &text);
        ctx.start_drag(DragStart::new(Offer::text(&text), icon).actions(DragActions::COPY));
    }

    fn draw_selection(&self, state: &RichTextState, ctx: &mut PaintCtx<'_>) {
        let Some(part) = &self.part else {
            return;
        };
        let Some((_, _, laid)) = &state.laid else {
            return;
        };
        let bounds = state.bounds;
        state.placement.note(part, bounds, laid);
        let Some(range) = selection::range_in(part) else {
            return;
        };
        let fill = ctx.palette().accent.scale_alpha(BAND_ALPHA);
        for band in state.bands(range) {
            let band = band.translate(bounds.origin);
            ctx.draw_quad(Quad::new(band).fill(fill).radius(caret::SELECTION_RADIUS));
        }
    }

    fn draw_found(&self, state: &mut RichTextState, ctx: &mut PaintCtx<'_>) {
        let Some(found) = self.found.as_ref().filter(|found| !found.is_empty()) else {
            state.revealed = None;
            return;
        };
        let marks = match &self.part {
            Some(part) => part.read_text(|whole| found.marks(whole, part.stamp(), part.window())),
            None => found.marks(&self.content, digest(&self.content), 0..self.content.len()),
        };

        let marking = Marking::from_palette(ctx.palette());
        let bounds = state.bounds;
        let stamped = found.query().stamp();
        for mark in marks {
            let ink = marking.color(mark.current);
            for band in state.bands(mark.range.clone()) {
                let band = band.translate(bounds.origin);
                ctx.draw_quad(Quad::new(band).fill(ink).radius(caret::SELECTION_RADIUS));
                if mark.current && state.revealed != Some((stamped, mark.at)) {
                    state.revealed = Some((stamped, mark.at));
                    ctx.reveal(band, Reveal::Nearest);
                }
            }
        }
    }

    fn summon(&self, state: &mut RichTextState, at: Point, ctx: &mut EventCtx<'_>) {
        let Some(part) = &self.part else {
            return;
        };
        let Some(offset) = self.offset_at(state, at) else {
            return;
        };
        if !selection::range_in(part).is_some_and(|press_origin| press_origin.contains(&offset)) {
            return;
        }
        if let Some(owner) = selection::adopt(part) {
            state.owner = Some(owner);
        }
        state.menu.open(menu_items(), at, ctx);
    }

    fn open_from_keyboard(&self, state: &mut RichTextState, ctx: &mut EventCtx<'_>) {
        let Some(part) = self.part.as_ref().filter(|part| selection::holds(part)) else {
            return;
        };
        let band =
            selection::range_in(part).and_then(|range| state.bands(range).into_iter().next());
        state.menu.open_by_keys(menu_items(), band, ctx);
    }

    fn follow_link(&self, state: &mut RichTextState, at: Point, ctx: &mut EventCtx<'_>) {
        let Some(pressed) = state.link.take() else {
            return;
        };
        if !self.on_link.is_set() {
            return;
        }
        let in_selection = self.part.as_ref().is_some_and(selection::holds);
        if in_selection || !ctx.bounds().contains(at) || self.link_at(state, at) != Some(pressed) {
            return;
        }
        ctx.set_handled();
        ctx.act(&self.on_link, (pressed,));
    }

    fn link_at(&self, state: &RichTextState, at: Point) -> Option<usize> {
        if self.links.is_empty() {
            return None;
        }
        let offset = self.offset_at(state, at)?;
        self.links.iter().position(|link| link.contains(&offset))
    }

    fn plates(&self, state: &RichTextState, size: f32) -> Vec<(usize, Rect)> {
        let height = size * FILL_HEIGHT;
        self.fills
            .iter()
            .enumerate()
            .flat_map(|(index, fill)| {
                state
                    .bands(fill.range.clone())
                    .into_iter()
                    .map(move |band| {
                        let plate = Rect::new(
                            band.x() - FILL_INSET,
                            band.y() + (band.height() - height) / 2.0,
                            band.width() + FILL_INSET * 2.0,
                            height,
                        );
                        (index, plate)
                    })
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    fn offset_at(&self, state: &RichTextState, at: Point) -> Option<usize> {
        self.cursor_at(state, at).map(|(offset, _)| offset)
    }

    fn cursor_at(&self, state: &RichTextState, at: Point) -> Option<(usize, Affinity)> {
        let (_, _, laid) = state.laid.as_ref()?;
        let cursor = laid.hit(at.x - state.bounds.x(), at.y - state.bounds.y());
        Some((laid.offset_of(cursor), cursor.affinity))
    }
}

impl TextStyled for RichText {
    fn text_style_mut(&mut self) -> &mut TextStyle {
        &mut self.text_style
    }
}

mod chords;
mod cursor;
mod grid;
mod input;
mod layer;
mod lines;
mod row;

use std::rc::Rc;

use rikka_types::{Color, CursorShape as PointerShape, Key, Modifiers, Point, Rect, Size};
use rikka_ui::search::{Marking, Search, Step};
use rikka_ui::{
    Accept, Action, Arranged, Behavior, Element, Event as Input, EventCtx, IntoHandler, PaintCtx,
    Quad, Style, Tooltip, URI_LIST, Widget, palette,
};

use alacritty_terminal::Term;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Direction, Point as GridPoint, Side};
use alacritty_terminal::term::cell::{Cell, Flags};
use alacritty_terminal::term::color::Colors;
use alacritty_terminal::term::{RenderableContent, point_to_viewport};
use alacritty_terminal::vte::ansi::{Color as AnsiColor, CursorShape, NamedColor};

use crate::font::{Faces, Font};
use crate::layout::Layout;
use crate::link::Link;
use crate::palette::Palette;
use crate::screen::row::{CellStyle, Decoration, Row, Span};
use crate::session::{Proxy, Session};

use crate::composition::PreeditLayout;
use crate::screen::cursor::{Cursor, Mark};
use crate::screen::layer::{Layer, Sketch};
use crate::screen::lines::Lines;

pub use crate::screen::input::ScreenState;

const SELECTION: f32 = 0.30;
const RULE: f32 = 1.0;
const CARET: f32 = 2.0;
const LINES: &str = "lines";
const MARKS: &str = "marks";
const CURSOR: &str = "cursor";

#[must_use]
#[derive(Widget)]
pub struct Screen {
    #[debug]
    session: Rc<Session>,
    #[debug]
    font: Font,
    active: bool,
    on_key: Action<(Key, Modifiers), bool>,
    on_activate: Action,
    on_zoom: Action<(i32,)>,
    search: Option<Search>,
    #[style]
    layout: Style,
}

pub fn screen(session: Rc<Session>, font: Font) -> Screen {
    Screen {
        session,
        font,
        active: true,
        on_key: Action::default(),
        on_activate: Action::default(),
        on_zoom: Action::default(),
        search: None,
        layout: Style::default(),
    }
}

impl Screen {
    pub fn on_key<Marker>(
        mut self,
        on_key: impl IntoHandler<(Key, Modifiers), bool, Marker>,
    ) -> Self {
        self.on_key = Action::new(on_key);
        self
    }

    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    pub fn on_activate<Marker>(mut self, on_activate: impl IntoHandler<(), (), Marker>) -> Self {
        self.on_activate = Action::new(on_activate);
        self
    }

    pub fn on_zoom<Marker>(mut self, on_zoom: impl IntoHandler<(i32,), (), Marker>) -> Self {
        self.on_zoom = Action::new(on_zoom);
        self
    }

    pub fn search(mut self, search: &Search) -> Self {
        self.search = Some(search.clone());
        self
    }

    fn find(&self, state: &mut ScreenState, ctx: &mut PaintCtx<'_>, term: &mut Term<Proxy>) {
        let Some(search) = self.search.as_ref().filter(|search| search.is_open()) else {
            state.found.forget();
            return;
        };
        let query = search.query();
        state.found.read(term, &query, self.session.generation());
        match search.take_step() {
            Some(Step::Next) => state.found.step(term, Direction::Right),
            Some(Step::Previous) => state.found.step(term, Direction::Left),
            Some(Step::Replace | Step::ReplaceAll) | None => {}
        }

        let counting = state.found.is_counting();
        let news = match counting {
            true => search.reporting(state.found.tally()),
            false => search.report(state.found.tally()),
        };
        if news {
            ctx.request_view();
        }
        if counting {
            ctx.request_frame();
        }
    }

    fn draw_found(
        found: &crate::find::Found,
        sketch: &mut Sketch,
        frame: &Frame<'_>,
        display_offset: usize,
        layout: Layout,
    ) {
        let marking = Marking::from_palette(palette());
        for (current, hit) in found.on_screen() {
            let ink = marking.color(current);
            for (line, span) in spans(*hit.start(), *hit.end(), display_offset, layout) {
                sketch.fill(frame.cell_rect(line, span), ink);
            }
        }
    }

    fn draw_link(
        link: &Link,
        sketch: &mut Sketch,
        frame: &Frame<'_>,
        snapshot: &Snapshot,
        layout: Layout,
    ) {
        let ink = frame
            .palette
            .resolve(AnsiColor::Named(NamedColor::Foreground), &snapshot.colors);
        let (start, end) = (*link.cells.start(), *link.cells.end());
        for (line, span) in spans(start, end, snapshot.display_offset, layout) {
            draw_rules(
                sketch,
                frame.cell_rect(line, span),
                Decoration::Underline,
                ink,
            );
        }
    }

    fn fit(&self, state: &mut ScreenState) -> bool {
        let remeasured = state.measure(&self.font);
        if remeasured {
            state.grid.invalidate();
        }
        let Some(room) = state.room else {
            return remeasured;
        };
        let cell = state.measured().cell;
        self.session.resize(Layout::fit(
            room.width,
            room.height,
            cell.width,
            cell.height,
        ));
        let regrown = state.grid.regrow(self.session.layout().screen_lines());
        regrown || remeasured
    }

    fn focus(&self, focused: bool) {
        self.session.focused(focused);
        if !focused {
            self.session.compose("", None);
        }
    }
}

#[derive(Default)]
pub(crate) struct Snapshot {
    colors: Colors,
    display_offset: usize,
    cursor: GridPoint,
    shape: CursorShape,
    under_cursor: Cell,
    blinking: bool,
}

impl std::fmt::Debug for Snapshot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Snapshot")
            .field("display_offset", &self.display_offset)
            .field("cursor", &self.cursor)
            .finish_non_exhaustive()
    }
}

impl Snapshot {
    fn fill(&mut self, term: &Term<Proxy>) {
        self.blinking = term.cursor_style().blinking;
        let RenderableContent {
            cursor,
            display_offset,
            colors,
            ..
        } = term.renderable_content();
        self.colors = *colors;
        self.display_offset = display_offset;
        self.cursor = cursor.point;
        self.shape = cursor.shape;
        self.under_cursor = term.grid()[cursor.point].clone();
    }
}

pub(crate) fn paint_row(
    sketch: &mut Sketch,
    frame: &Frame<'_>,
    line: usize,
    row: &Row<'_>,
    text: &mut String,
) {
    row.backgrounds(|span, color| {
        sketch.fill(frame.cell_rect(line, span), color);
    });
    draw_runs(sketch, frame, line, 0, row, text);
    row.decorations(|span, color, decoration| {
        draw_rules(sketch, frame.cell_rect(line, span), decoration, color);
    });
    let tint = frame.palette.selection().with_alpha(SELECTION);
    row.selection(|span| {
        sketch.fill(frame.cell_rect(line, span), tint);
    });
}

fn paint_composition(
    sketch: &mut Sketch,
    frame: &Frame<'_>,
    placed: &PreeditLayout,
    colors: &Colors,
    text: &mut String,
) {
    let foreground = frame
        .palette
        .resolve(AnsiColor::Named(NamedColor::Foreground), colors);
    for segment in placed.segments() {
        let whole = Span::from(segment.column..segment.column + segment.cells.len());
        let rect = frame.cell_rect(segment.line, whole);
        sketch.fill(rect, frame.ground);

        let mut row = Row::default();
        for cell in segment.cells {
            row.push(CellStyle::new(cell, false, frame.palette, colors));
        }
        draw_runs(sketch, frame, segment.line, segment.column, &row, text);

        draw_rules(sketch, rect, Decoration::Underline, foreground);
        if !segment.converting.is_empty() {
            let converting = Span::from(
                segment.column + segment.converting.start..segment.column + segment.converting.end,
            );
            let rect = frame.cell_rect(segment.line, converting);
            draw_rules(sketch, rect, Decoration::Emphatic, frame.palette.cursor());
        }
    }

    let rect = frame.cell_rect(placed.caret().line, cell_span(placed.caret().column.0));
    let beam = Rect::new(rect.origin.x, rect.origin.y, CARET, rect.size.height);
    sketch.fill(beam, frame.palette.cursor());
}

fn mark_of(frame: &Frame<'_>, snapshot: &Snapshot, focused: bool) -> Option<Mark> {
    if snapshot.shape == CursorShape::Hidden {
        return None;
    }
    let point = point_to_viewport(snapshot.display_offset, snapshot.cursor)?;
    let cell = frame.cell_rect(point.line, cell_span(point.column.0));
    let under = &snapshot.under_cursor;
    let under = (!under.flags.contains(Flags::HIDDEN) && under.c != ' ').then(|| {
        let ink = CellStyle::new(under, false, frame.palette, &snapshot.colors).ink;
        let run = frame
            .faces
            .run(ink.bold, ink.italic, under.c.encode_utf8(&mut [0; 4]));
        (run, frame.ground)
    });
    Some(Mark {
        cell,
        shape: snapshot.shape,
        color: frame.palette.cursor(),
        focused,
        blinking: snapshot.blinking,
        under,
    })
}

impl Widget for Screen {
    type State = ScreenState;

    const ARRANGES: bool = true;

    fn layout(&self) -> rikka_ui::Layout {
        rikka_ui::Layout::DEFAULT.with(&self.layout)
    }

    fn behavior(&self, _state: &Self::State) -> Behavior {
        Behavior::FOCUSABLE.when(self.active)
            | Behavior::AUTOFOCUS.when(self.active)
            | Behavior::TAKES_FOCUS.when(self.active)
            | Behavior::TAKES_TAB
    }

    fn arranged(&self, state: &mut ScreenState, arranged: Arranged<'_>) -> bool {
        state.room = Some(arranged.room().size);
        self.fit(state)
    }

    fn paint(&self, state: &mut ScreenState, ctx: &mut PaintCtx<'_>) {
        self.session.watchers(ctx.handle());
        let bounds = ctx.bounds();
        let focused = ctx.focused();
        if self.session.recolor(ctx.theme()) {
            state.grid.invalidate();
        }
        let cell = state.measured().cell;

        let palette = self.session.palette();
        let layout = self.session.layout();
        let regrown = {
            let mut term = self.session.term().lock();
            self.find(state, ctx, &mut term);
            state.relink(&term, self.session.generation());
            state.snapshot.fill(&term);
            state.grid.copy(&mut term)
        };
        if regrown {
            ctx.handle().rebuild();
        }

        let ScreenState {
            measured,
            snapshot,
            grid,
            marks,
            sketch,
            found,
            cursor,
            link,
            ..
        } = state;
        let measured = measured.as_ref().expect("screen not measured yet");
        let ground_override = Palette::ground_override(&snapshot.colors);
        let frame = Frame {
            cell,
            faces: &measured.faces,
            ground: ground_override.unwrap_or_else(|| palette.background()),
            palette: &palette,
        };

        let mut text = String::new();
        grid.sketch(&frame, &snapshot.colors, &mut text);

        let at = point_to_viewport(snapshot.display_offset, snapshot.cursor).unwrap_or_else(|| {
            GridPoint::new(
                layout.screen_lines().saturating_sub(1),
                snapshot.cursor.column,
            )
        });
        let placed = {
            let composition = self.session.composition();
            (!composition.is_empty())
                .then(|| composition.place(at, layout))
                .filter(|placed| !placed.is_empty())
        };
        let caret = placed.as_ref().map_or(at, PreeditLayout::caret);
        let caret_rect = frame.cell_rect(caret.line, cell_span(caret.column.0));

        if let Some(ground) = ground_override {
            ctx.draw_quad(Quad::new(bounds).fill(ground.with_alpha(ctx.palette().background.a)));
        }

        Self::draw_found(found, sketch, &frame, snapshot.display_offset, layout);
        if let Some(link) = link {
            Self::draw_link(link, sketch, &frame, snapshot, layout);
        }
        if let Some(placed) = &placed {
            paint_composition(sketch, &frame, placed, &snapshot.colors, &mut text);
        }
        marks.show(sketch);

        let mark = match placed {
            Some(_) => None,
            None => mark_of(&frame, snapshot, focused),
        };
        cursor.show(mark);
        state.set_caret(caret_rect);
        state.menu.paint(ctx);
    }

    fn build(&self, state: &mut ScreenState) -> Vec<Element> {
        self.fit(state);
        let height = state.measured().cell.height;
        let lines = state
            .grid
            .lines()
            .iter()
            .map(|shown| Element::new(Layer::line(shown.layer(), height)).keyed(shown.key()))
            .collect();
        vec![
            Element::with_children(Lines::new(state.grid.strip(), height), lines).keyed(LINES),
            Element::new(Layer::over(&state.marks)).keyed(MARKS),
            Element::new(Cursor::new(&state.cursor)).keyed(CURSOR),
        ]
    }

    fn event(&self, state: &mut ScreenState, event: &Input, ctx: &mut EventCtx<'_>) {
        match event {
            Input::FocusGained => self.focus(true),
            Input::FocusLost => {
                self.focus(false);
                self.modifiers_changed(state, ctx, Modifiers::NONE);
            }
            _ => {}
        }
        if state.menu.event(event, ctx) {
            return;
        }
        self.activate(event, ctx);
        match *event {
            Input::KeyPressed {
                key,
                location,
                modifiers,
                ..
            } => self.key(state, ctx, key, location, modifiers),
            Input::KeyReleased { key, modifiers, .. } => {
                self.key_released(state, ctx, key, modifiers);
            }
            Input::Command(command) if ctx.focused() => self.handle_command(state, ctx, command),
            Input::Scroll {
                position,
                delta,
                modifiers,
                ..
            } => self.scroll(state, ctx, position, delta, modifiers),
            Input::PointerPressed {
                position,
                button,
                modifiers,
            } => self.pressed(state, ctx, position, button, modifiers),
            Input::PointerMoved(position) => self.moved(state, ctx, position),
            Input::PointerReleased {
                position,
                button,
                modifiers,
            } => self.released(state, ctx, position, button, modifiers),
            Input::PointerExited => self.left(state, ctx),
            Input::Preedit {
                ref text,
                ref cursor,
            } => self.compose(ctx, text, cursor.clone()),
            Input::Commit(ref text) => self.commit(ctx, text),
            Input::DragMoved { ref offer, .. } => {
                if let Some(mime) = offer.first_of(&[URI_LIST]).or_else(|| offer.text_mime()) {
                    ctx.accept_drag(Accept::copy(mime));
                    ctx.set_handled();
                }
            }
            Input::DragDropped { ref offer, .. } => {
                ctx.set_handled();
                ctx.request_focus();
                self.session.paste_dropped(offer.clone());
            }
            _ => {}
        }
    }

    fn caret(&self, state: &ScreenState) -> Option<Rect> {
        state.caret()
    }

    fn cursor(&self, state: &ScreenState, _bounds: Rect, _point: Point) -> Option<PointerShape> {
        match state.link {
            Some(_) if state.modifiers.control => Some(PointerShape::Pointer),
            _ => Some(PointerShape::Text),
        }
    }

    fn tooltip_at(
        &self,
        state: &ScreenState,
        bounds: Rect,
        point: Point,
    ) -> Option<(Tooltip, Rect)> {
        let link = state.link.as_ref().filter(|link| link.declared)?;
        let cell = state.cell()?;
        let at = cell_at(bounds, cell, self.session.layout(), point);
        let spot = Rect::new(
            bounds.origin.x + at.column.0 as f32 * cell.width,
            bounds.origin.y + at.line as f32 * cell.height,
            cell.width,
            cell.height,
        );
        Some((Tooltip::new(link.uri.clone()), spot))
    }
}

pub(crate) struct Frame<'a> {
    cell: Size,
    faces: &'a Faces,
    ground: Color,
    palette: &'a Palette,
}

impl Frame<'_> {
    fn cell_rect(&self, line: usize, span: Span) -> Rect {
        Rect::new(
            span.start as f32 * self.cell.width,
            line as f32 * self.cell.height,
            span.columns() as f32 * self.cell.width,
            self.cell.height,
        )
    }
}

fn cell_span(column: usize) -> Span {
    Span {
        start: column,
        end: column + 1,
    }
}

fn draw_runs(
    sketch: &mut Sketch,
    frame: &Frame<'_>,
    line: usize,
    column: usize,
    row: &Row<'_>,
    text: &mut String,
) {
    row.runs(text, |span, run, ink| {
        let span = Span::from(column + span.start..column + span.end);
        let rect = frame.cell_rect(line, span);
        sketch.text(rect, frame.faces.run(ink.bold, ink.italic, run), ink.color);
    });
}

fn draw_rules(sketch: &mut Sketch, rect: Rect, decoration: Decoration, color: Color) {
    for rule in rules(rect, decoration).into_iter().flatten() {
        sketch.fill(rule, color);
    }
}

fn rules(rect: Rect, decoration: Decoration) -> [Option<Rect>; 2] {
    let rule = |above: f32, thickness: f32| {
        Some(Rect::new(
            rect.origin.x,
            rect.max_y() - above - thickness,
            rect.size.width,
            thickness,
        ))
    };
    match decoration {
        Decoration::Underline => [rule(1.0, RULE), None],
        Decoration::Double => [rule(1.0, RULE), rule(1.0 + 2.0 * RULE, RULE)],
        Decoration::Emphatic => [rule(1.0, RULE * 2.0), None],
        Decoration::Strikeout => [
            Some(Rect::new(
                rect.origin.x,
                rect.origin.y + (rect.size.height / 2.0).round(),
                rect.size.width,
                RULE,
            )),
            None,
        ],
    }
}

fn spans(
    start: GridPoint,
    end: GridPoint,
    display_offset: usize,
    layout: Layout,
) -> impl Iterator<Item = (usize, Span)> {
    (0..=(end.line - start.line).0.max(0)).filter_map(move |step| {
        let line = start.line + step;
        let from = match line == start.line {
            true => start.column.0,
            false => 0,
        };
        let to = match line == end.line {
            true => end.column.0 + 1,
            false => layout.columns(),
        };
        let point = point_to_viewport(display_offset, GridPoint::new(line, Column(from)))?;
        Some((point.line, Span::from(point.column.0..to)))
    })
}

fn cell_at(bounds: Rect, cell: Size, layout: Layout, at: Point) -> GridPoint<usize> {
    let column = ((at.x - bounds.origin.x) / cell.width).floor().max(0.0) as usize;
    let line = ((at.y - bounds.origin.y) / cell.height).floor().max(0.0) as usize;
    GridPoint::new(
        line.min(layout.screen_lines().saturating_sub(1)),
        Column(column.min(layout.columns().saturating_sub(1))),
    )
}

fn side_at(bounds: Rect, cell: Size, at: Point) -> Side {
    let offset = (at.x - bounds.origin.x).rem_euclid(cell.width);
    if offset > cell.width / 2.0 {
        Side::Right
    } else {
        Side::Left
    }
}

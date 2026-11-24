use std::ops::Range;
use std::rc::Rc;

use rikka_types::{
    Key, KeyLocation, Modifiers, Point, PointerButton, Rect, SCROLL_LINE, ScrollDelta, Size,
};
use rikka_ui::{Command, Event as Input, EventCtx, Selection};
use rikka_widgets::{ContextMenu, MenuItem, command_item, menu_separator};

use alacritty_terminal::Term;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::Point as GridPoint;
use alacritty_terminal::term::{TermMode, viewport_to_point};

use crate::clipboard;
use crate::font::{Faces, Font};
use crate::link::Link;
use crate::mouse::{Button, Motion};
use crate::screen::chords::{self, Chord, Selected};
use crate::screen::cursor::CursorCell;
use crate::screen::grid::Grid;
use crate::screen::layer::{LayerCell, Sketch};
use crate::screen::{Screen, cell_at, side_at};
use crate::session::Granularity;

#[derive(Debug, Default)]
pub struct ScreenState {
    pub(crate) measured: Option<FontMetrics>,
    pub(crate) room: Option<Size>,
    pointer: Pointer,
    pub(crate) modifiers: Modifiers,
    caret: Option<Rect>,
    hover: Option<GridPoint<usize>>,
    pub(crate) link: Option<Link>,
    linked: Option<Linked>,
    zoom_travel: f32,
    pub(crate) cursor: Rc<CursorCell>,
    pub(crate) found: crate::find::Found,
    pub(crate) snapshot: crate::screen::Snapshot,
    pub(crate) grid: Grid,
    pub(crate) marks: Rc<LayerCell>,
    pub(crate) sketch: Sketch,
    pub(crate) menu: ContextMenu,
}

impl ScreenState {
    pub(crate) fn measure(&mut self, font: &Font) -> bool {
        let stale = self
            .measured
            .as_ref()
            .is_none_or(|measured| measured.font != *font);
        if stale {
            let faces = font.faces();
            self.measured = Some(FontMetrics {
                cell: font.cell(&faces),
                faces,
                font: font.clone(),
            });
        }
        stale
    }

    pub(crate) fn measured(&self) -> &FontMetrics {
        self.measured.as_ref().expect("screen not measured yet")
    }

    pub(crate) fn set_caret(&mut self, caret: Rect) {
        self.caret = Some(caret);
    }

    pub(crate) fn caret(&self) -> Option<Rect> {
        self.caret
    }

    pub(crate) fn hold(&mut self, modifiers: Modifiers) {
        self.modifiers = modifiers;
    }

    pub(crate) fn cell(&self) -> Option<Size> {
        Some(self.measured.as_ref()?.cell)
    }

    pub(crate) fn relink<T>(&mut self, term: &Term<T>, generation: u64) -> bool {
        let reports = reports_pointer(*term.mode(), self.modifiers);
        let hovered = self.hover.filter(|_| !reports);
        let stamp = Linked {
            generation,
            display_offset: term.grid().display_offset(),
            columns: term.columns(),
            hovered,
        };
        let previous = self.linked.replace(stamp);
        if previous == Some(stamp) {
            return false;
        }
        let display_offset = stamp.display_offset;
        let within = previous.is_some_and(|previous| {
            Linked {
                hovered,
                ..previous
            } == stamp
        }) && hovered
            .zip(self.link.as_ref())
            .is_some_and(|(at, link)| link.cells.contains(&viewport_to_point(display_offset, at)));
        if within {
            return false;
        }
        let link = hovered.and_then(|at| link_under(term, at));
        if self.link == link {
            return false;
        }
        self.link = link;
        true
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Linked {
    generation: u64,
    display_offset: usize,
    columns: usize,
    hovered: Option<GridPoint<usize>>,
}

#[derive(Debug)]
pub(crate) struct FontMetrics {
    font: Font,
    pub(crate) cell: Size,
    pub(crate) faces: Faces,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Pointer {
    #[default]
    Idle,
    Selecting,
    Reporting {
        button: Option<Button>,
        at: GridPoint<usize>,
    },
}

impl Screen {
    pub(super) fn activate(&self, event: &Input, ctx: &mut EventCtx<'_>) {
        let Input::PointerPressed { position, .. } = *event else {
            return;
        };
        if self.active || !ctx.bounds().contains(position) {
            return;
        }
        ctx.act(&self.on_activate, ());
    }

    pub(super) fn key(
        &self,
        state: &mut ScreenState,
        ctx: &mut EventCtx<'_>,
        key: Key,
        location: KeyLocation,
        modifiers: Modifiers,
    ) {
        if key.is_modifier() {
            self.modifiers_changed(state, ctx, held(key, true, modifiers));
            return;
        }
        state.hold(modifiers);
        if !self.session.composition().is_empty() {
            ctx.set_handled();
            return;
        }
        if ctx.answer(&self.on_key, (key, modifiers)).unwrap_or(false) {
            ctx.set_handled();
            return;
        }
        let selected = chords::selected(key, modifiers, self.session.is_marking());
        if selected.is_some_and(|selected| self.act_on_selection(selected)) {
            ctx.request_paint();
            ctx.set_handled();
            return;
        }
        if let Some(chord) = chords::chord(key, modifiers) {
            self.answer(chord);
            ctx.request_paint();
            ctx.set_handled();
            return;
        }
        if Command::for_key(key, modifiers) == Some(Command::ContextMenu) {
            return;
        }
        if self.session.key(key, location, modifiers) {
            self.session.clear_selection();
            ctx.request_paint();
            ctx.set_handled();
        }
    }

    pub(super) fn key_released(
        &self,
        state: &mut ScreenState,
        ctx: &mut EventCtx<'_>,
        key: Key,
        modifiers: Modifiers,
    ) {
        self.modifiers_changed(state, ctx, held(key, false, modifiers));
    }

    pub(super) fn modifiers_changed(
        &self,
        state: &mut ScreenState,
        ctx: &mut EventCtx<'_>,
        modifiers: Modifiers,
    ) {
        let shifted = state.modifiers.shift != modifiers.shift;
        state.hold(modifiers);
        if shifted {
            self.hover_link(state, ctx);
        }
    }

    pub(super) fn handle_command(
        &self,
        state: &mut ScreenState,
        ctx: &mut EventCtx<'_>,
        command: Command,
    ) {
        match command {
            Command::Copy => self.answer(Chord::Copy(Selection::Clipboard)),
            Command::Paste => self.answer(Chord::Paste(Selection::Clipboard)),
            Command::SelectAll => self.answer(Chord::SelectAll),
            Command::ContextMenu => state
                .menu
                .open_by_keys(self.menu_items(), state.caret(), ctx),
            _ => return,
        }
        ctx.request_paint();
        ctx.set_handled();
    }

    fn menu_items(&self) -> Vec<MenuItem> {
        vec![
            command_item(Command::Copy)
                .shortcut(chords::copying().to_string())
                .enabled(self.session.selected_text().is_some()),
            command_item(Command::Paste)
                .shortcut(chords::pasting().to_string())
                .enabled(rikka_ui::clipboard::has_text()),
            menu_separator(),
            command_item(Command::SelectAll).shortcut(chords::selecting_all().to_string()),
        ]
    }

    fn answer(&self, chord: Chord) {
        match chord {
            Chord::Copy(selection) => {
                self.copy(selection);
                self.session.clear_selection();
            }
            Chord::Paste(selection) => {
                self.session.clear_selection();
                self.session.paste_from(selection);
            }
            Chord::SelectAll => self.session.select_all(),
            Chord::Mark => self.session.toggle_marking(),
            Chord::Scroll(scroll) => self.session.scroll(scroll),
        }
    }

    fn act_on_selection(&self, selected: Selected) -> bool {
        match selected {
            Selected::Copy => {
                let Some(text) = self.session.selected_text() else {
                    return false;
                };
                clipboard::write(Selection::Clipboard, &text);
                self.session.clear_selection();
            }
            Selected::Dismiss | Selected::Move { .. } if !self.session.has_selection() => {
                return false;
            }
            Selected::Dismiss => self.session.clear_selection(),
            Selected::Move { movement, extend } => self.session.move_selection(movement, extend),
        }
        true
    }

    fn copy(&self, selection: Selection) {
        if let Some(text) = self.session.selected_text() {
            clipboard::write(selection, &text);
        }
    }

    pub(super) fn compose(&self, ctx: &mut EventCtx<'_>, text: &str, cursor: Option<Range<usize>>) {
        ctx.set_handled();
        if self.session.compose(text, cursor) {
            ctx.request_paint();
        }
    }

    pub(super) fn commit(&self, ctx: &mut EventCtx<'_>, text: &str) {
        ctx.set_handled();
        self.session.commit(text);
        self.session.clear_selection();
        ctx.request_paint();
    }

    pub(super) fn scroll(
        &self,
        state: &mut ScreenState,
        ctx: &mut EventCtx<'_>,
        position: Point,
        delta: ScrollDelta,
        modifiers: Modifiers,
    ) {
        let Some(cell) = state.cell() else {
            return;
        };
        state.hold(modifiers);
        ctx.set_handled();
        let content = delta.content();
        if modifiers.control && self.on_zoom.is_set() {
            self.zoom(state, ctx, content.y);
            return;
        }
        state.zoom_travel = 0.0;
        let across = detents(content.x, cell.width);
        let up = detents(content.y, cell.height);
        let at = cell_at(ctx.bounds(), cell, self.session.layout(), position);
        if self
            .session
            .wheel(across, up, at.column.0, at.line, modifiers)
        {
            self.hover_link(state, ctx);
            ctx.request_paint();
        }
    }

    fn zoom(&self, state: &mut ScreenState, ctx: &mut EventCtx<'_>, travel: f32) {
        state.zoom_travel += travel;
        let steps = (state.zoom_travel / SCROLL_LINE).trunc();
        if steps == 0.0 {
            return;
        }
        state.zoom_travel -= steps * SCROLL_LINE;
        ctx.act(&self.on_zoom, (steps as i32,));
    }

    pub(super) fn pressed(
        &self,
        state: &mut ScreenState,
        ctx: &mut EventCtx<'_>,
        position: Point,
        button: PointerButton,
        modifiers: Modifiers,
    ) {
        ctx.set_handled();
        state.hold(modifiers);
        let Some(cell) = state.cell() else {
            return;
        };
        let bounds = ctx.bounds();
        let at = cell_at(bounds, cell, self.session.layout(), position);

        let mode = self.session.mode();
        if reports_pointer(mode, modifiers)
            && let Some(button) = Button::from_pointer_button(button)
        {
            self.report(Motion::Pressed(button), at, modifiers, mode);
            state.pointer = Pointer::Reporting {
                button: Some(button),
                at,
            };
            ctx.capture_pointer();
            return;
        }

        if button == PointerButton::Primary
            && modifiers.control
            && let Some(link) = link_under(&self.session.term().lock(), at)
        {
            link.open();
            return;
        }

        match button {
            PointerButton::Middle => self.answer(Chord::Paste(Selection::Primary)),
            PointerButton::Secondary => state.menu.open(self.menu_items(), position, ctx),
            PointerButton::Primary => {
                let granularity = match ctx.presses() {
                    1 if modifiers.alt => Granularity::Block,
                    1 => Granularity::Cell,
                    2 => Granularity::Word,
                    _ => Granularity::Line,
                };
                let side = side_at(bounds, cell, position);
                let extended =
                    modifiers.shift && self.session.extend_selection(at.column.0, at.line, side);
                if !extended {
                    self.session.select(granularity, at.column.0, at.line, side);
                }
                state.pointer = Pointer::Selecting;
                state.hover = None;
                state.link = None;
                ctx.capture_pointer();
                ctx.request_paint();
            }
            _ => {
                self.session.clear_selection();
                ctx.request_paint();
            }
        }
    }

    pub(super) fn moved(&self, state: &mut ScreenState, ctx: &mut EventCtx<'_>, position: Point) {
        let Some(cell) = state.cell() else {
            return;
        };
        let bounds = ctx.bounds();
        let at = cell_at(bounds, cell, self.session.layout(), position);
        match state.pointer {
            Pointer::Selecting => {
                let side = side_at(bounds, cell, position);
                self.session.extend_selection(at.column.0, at.line, side);
                ctx.request_paint();
                ctx.set_handled();
            }
            Pointer::Reporting { button, at: was } => {
                state.pointer = Pointer::Reporting { button, at };
                if button.is_none() {
                    self.hover(state, ctx, Some(at));
                }
                if was != at {
                    let mode = self.session.mode();
                    self.report(Motion::Moved(button), at, state.modifiers, mode);
                }
                ctx.set_handled();
            }
            Pointer::Idle if ctx.hovered() => {
                self.hover(state, ctx, Some(at));
                let mode = self.session.mode();
                if !mode.contains(TermMode::MOUSE_MOTION) {
                    return;
                }
                state.pointer = Pointer::Reporting { button: None, at };
                self.report(Motion::Moved(None), at, state.modifiers, mode);
                ctx.set_handled();
            }
            Pointer::Idle => self.hover(state, ctx, None),
        }
    }

    pub(super) fn released(
        &self,
        state: &mut ScreenState,
        ctx: &mut EventCtx<'_>,
        position: Point,
        button: PointerButton,
        modifiers: Modifiers,
    ) {
        state.hold(modifiers);
        let Some(cell) = state.cell() else {
            return;
        };
        let at = cell_at(ctx.bounds(), cell, self.session.layout(), position);
        match state.pointer {
            Pointer::Selecting => {
                state.pointer = Pointer::Idle;
                self.copy(Selection::Primary);
                ctx.release_pointer();
                ctx.set_handled();
            }
            Pointer::Reporting {
                button: Some(_), ..
            } => {
                state.pointer = Pointer::Idle;
                if let Some(button) = Button::from_pointer_button(button) {
                    let mode = self.session.mode();
                    self.report(Motion::Released(button), at, modifiers, mode);
                }
                ctx.release_pointer();
                ctx.set_handled();
            }
            Pointer::Reporting { button: None, .. } | Pointer::Idle => {}
        }
    }

    pub(super) fn left(&self, state: &mut ScreenState, ctx: &mut EventCtx<'_>) {
        if matches!(state.pointer, Pointer::Reporting { button: None, .. }) {
            state.pointer = Pointer::Idle;
        }
        self.hover(state, ctx, None);
    }

    fn hover(&self, state: &mut ScreenState, ctx: &mut EventCtx<'_>, at: Option<GridPoint<usize>>) {
        if state.hover == at {
            return;
        }
        state.hover = at;
        self.hover_link(state, ctx);
    }

    fn hover_link(&self, state: &mut ScreenState, ctx: &mut EventCtx<'_>) {
        let relinked = state.relink(&self.session.term().lock(), self.session.generation());
        if relinked {
            ctx.request_paint();
        }
    }

    fn report(&self, motion: Motion, at: GridPoint<usize>, modifiers: Modifiers, mode: TermMode) {
        self.session
            .report(motion, at.column.0, at.line, modifiers, mode);
    }
}

fn link_under<T>(term: &Term<T>, at: GridPoint<usize>) -> Option<Link> {
    Link::at(term, viewport_to_point(term.grid().display_offset(), at))
}

fn held(key: Key, pressed: bool, mut modifiers: Modifiers) -> Modifiers {
    match key {
        Key::ShiftLeft | Key::ShiftRight => modifiers.shift = pressed,
        Key::ControlLeft | Key::ControlRight => modifiers.control = pressed,
        Key::AltLeft | Key::AltRight => modifiers.alt = pressed,
        Key::MetaLeft | Key::MetaRight => modifiers.logo = pressed,
        _ => {}
    }
    modifiers
}

fn reports_pointer(mode: TermMode, modifiers: Modifiers) -> bool {
    mode.intersects(TermMode::MOUSE_MODE) && !modifiers.shift
}

fn detents(travel: f32, cell: f32) -> i32 {
    match (travel / cell).round() as i32 {
        0 if travel != 0.0 => travel.signum() as i32,
        steps => steps,
    }
}

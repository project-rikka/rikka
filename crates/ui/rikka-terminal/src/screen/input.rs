use std::ops::Range;
use std::rc::Rc;

use rikka_input::Chord as Binding;
use rikka_types::{Key, KeyLocation, Modifiers, Point, PointerButton, Rect, ScrollDelta, Size};
use rikka_ui::{Command, Event as Input, EventCtx, Selection};
use rikka_widgets::{ContextMenu, MenuItem, command_item, menu_separator};

use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::Point as GridPoint;
use alacritty_terminal::term::TermMode;

use crate::clipboard;
use crate::font::{Faces, Font};
use crate::mouse::{Button, Motion};
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
    modifiers: Modifiers,
    caret: Option<Rect>,
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

    fn cell(&self) -> Option<Size> {
        Some(self.measured.as_ref()?.cell)
    }
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
        state.hold(modifiers);
        if !self.session.composition().is_empty() {
            ctx.set_handled();
            return;
        }
        if ctx.answer(&self.on_key, (key, modifiers)).unwrap_or(false) {
            ctx.set_handled();
            return;
        }
        if let Some(chord) = chord(key, modifiers) {
            self.answer(chord);
            ctx.set_handled();
            return;
        }
        if Command::for_key(key, modifiers) == Some(Command::ContextMenu) {
            return;
        }
        if let Some(pages) = paged(key, modifiers) {
            let screen = self.session.layout().screen_lines() as i32;
            self.session.scroll(pages * screen.max(1));
            ctx.request_paint();
            ctx.set_handled();
            return;
        }
        if self.session.key(key, location, modifiers) {
            self.session.clear_selection();
            ctx.request_paint();
            ctx.set_handled();
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
            Command::SelectAll => {
                self.session.select_all();
                ctx.request_paint();
            }
            Command::ContextMenu => state
                .menu
                .open_by_keys(self.menu_items(), state.caret(), ctx),
            _ => return,
        }
        ctx.set_handled();
    }

    fn menu_items(&self) -> Vec<MenuItem> {
        vec![
            command_item(Command::Copy)
                .shortcut(copying().to_string())
                .enabled(self.session.selected_text().is_some()),
            command_item(Command::Paste)
                .shortcut(pasting().to_string())
                .enabled(rikka_ui::clipboard::has_text()),
            menu_separator(),
            command_item(Command::SelectAll).no_shortcut(),
        ]
    }

    fn answer(&self, chord: Chord) {
        match chord {
            Chord::Copy(selection) => self.copy(selection),
            Chord::Paste(selection) => self.session.paste_from(selection),
        }
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
        let across = detents(content.x, cell.width);
        let up = detents(content.y, cell.height);
        let at = cell_at(ctx.bounds(), cell, self.session.layout(), position);
        if self
            .session
            .wheel(across, up, at.column.0, at.line, modifiers)
        {
            ctx.request_paint();
        }
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

        match button {
            PointerButton::Middle => self.answer(Chord::Paste(Selection::Primary)),
            PointerButton::Secondary => state.menu.open(self.menu_items(), position, ctx),
            PointerButton::Primary => {
                let granularity = match ctx.presses() {
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
                if was != at {
                    let mode = self.session.mode();
                    self.report(Motion::Moved(button), at, state.modifiers, mode);
                }
                ctx.set_handled();
            }
            Pointer::Idle if ctx.hovered() => {
                let mode = self.session.mode();
                if !mode.contains(TermMode::MOUSE_MOTION) {
                    return;
                }
                state.pointer = Pointer::Reporting { button: None, at };
                self.report(Motion::Moved(None), at, state.modifiers, mode);
                ctx.set_handled();
            }
            Pointer::Idle => {}
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

    pub(super) fn left(&self, state: &mut ScreenState) {
        if matches!(state.pointer, Pointer::Reporting { button: None, .. }) {
            state.pointer = Pointer::Idle;
        }
    }

    fn report(&self, motion: Motion, at: GridPoint<usize>, modifiers: Modifiers, mode: TermMode) {
        self.session
            .report(motion, at.column.0, at.line, modifiers, mode);
    }
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Chord {
    Copy(Selection),
    Paste(Selection),
}

fn chord(key: Key, modifiers: Modifiers) -> Option<Chord> {
    if modifiers.alt || modifiers.logo {
        return None;
    }
    if key == Key::Insert {
        return match (modifiers.control, modifiers.shift) {
            (true, false) => Some(Chord::Copy(Selection::Clipboard)),
            (false, true) => Some(Chord::Paste(Selection::Primary)),
            _ => None,
        };
    }
    if copying().matches(modifiers, key) {
        return Some(Chord::Copy(Selection::Clipboard));
    }
    pasting()
        .matches(modifiers, key)
        .then_some(Chord::Paste(Selection::Clipboard))
}

fn copying() -> Binding {
    Binding::new(Modifiers::CONTROL_SHIFT, Key::Char('c'))
}

fn pasting() -> Binding {
    Binding::new(Modifiers::CONTROL_SHIFT, Key::Char('v'))
}

fn paged(key: Key, modifiers: Modifiers) -> Option<i32> {
    if !modifiers.shift || modifiers.control || modifiers.alt {
        return None;
    }
    match key {
        Key::PageUp => Some(1),
        Key::PageDown => Some(-1),
        _ => None,
    }
}

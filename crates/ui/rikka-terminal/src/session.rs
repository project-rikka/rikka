use std::borrow::Cow;
use std::cell::{Cell, Ref, RefCell};
use std::ops::{Range, RangeInclusive};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use alacritty_terminal::event::{Event, EventListener};
use alacritty_terminal::grid::{Dimensions, Scroll};
use alacritty_terminal::index::{Column, Point as GridPoint, Side};
use alacritty_terminal::selection::{Selection as Highlight, SelectionType};
use alacritty_terminal::sync::FairMutex;
use alacritty_terminal::term::{Config, Osc52, Term, TermMode, viewport_to_point};
use alacritty_terminal::tty;
use rikka_theme::{Ansi, Theme};
use rikka_types::{Color, Key, KeyLocation, Modifiers, Size};
use rikka_ui::{DragOffer, Handler, NodeHandle, Selection, Watchers};

use crate::composition::Composition;
use crate::dropped;
use crate::error::Result;
use crate::keys;
use crate::layout::Layout;
use crate::mouse::{self, Button, Motion};
use crate::movement::{self, Movement};
use crate::options::Options;
use crate::palette::Palette;
use crate::pty::{self, PtyWriter};
use crate::report::{Question, Report};
use crate::words;

const WINDOW_ID: u64 = 1;

const WHEEL_LINES: i32 = 3;

const FOCUS_IN: &[u8] = b"\x1b[I";
const FOCUS_OUT: &[u8] = b"\x1b[O";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Granularity {
    Cell,
    Block,
    Word,
    Line,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SelectionMode {
    Free,
    Words,
    Marking,
}

struct Ends {
    anchor: GridPoint,
    focus: GridPoint,
    block: bool,
}

pub struct Session {
    term: Arc<FairMutex<Term<Proxy>>>,
    writer: PtyWriter,
    child: u32,
    layout: Cell<Layout>,
    composition: RefCell<Composition>,
    palette: RefCell<Palette>,
    colored: Cell<Option<(Ansi, Color)>>,
    generation: Arc<AtomicU64>,
    watchers: Watchers,
    selecting: Cell<SelectionMode>,
}

impl Session {
    pub fn open(
        options: &Options,
        report: impl Fn(Report) + Send + Sync + 'static,
    ) -> Result<Self> {
        let layout = Layout::DEFAULT;
        let generation = Arc::new(AtomicU64::new(0));
        let watchers = Watchers::new();
        let proxy = Proxy {
            report: Arc::new(report),
            generation: Arc::clone(&generation),
            watchers: watchers.clone(),
        };
        let config = Config {
            scrolling_history: options.scrollback_lines(),
            kitty_keyboard: false,
            osc52: Osc52::Disabled,
            ..Config::default()
        };
        let term = Term::new(config, &layout, proxy.clone());
        let term = Arc::new(FairMutex::new(term));

        let pty = tty::new(&options.to_pty(), layout.window_size(), WINDOW_ID)?;
        let child = pty.child().id();
        let writer = pty::start(pty, Arc::clone(&term), proxy, options.access())?;

        Ok(Self {
            term,
            writer,
            child,
            layout: Cell::new(layout),
            composition: RefCell::new(Composition::default()),
            palette: RefCell::new(Palette::from_theme(&Theme::default())),
            colored: Cell::new(None),
            generation,
            watchers,
            selecting: Cell::new(SelectionMode::Free),
        })
    }

    pub fn working_directory(&self) -> Option<PathBuf> {
        std::fs::read_link(format!("/proc/{}/cwd", self.child)).ok()
    }

    pub fn pixel_size(&self) -> Size {
        self.layout.get().pixel_size()
    }

    pub(crate) fn recolor(&self, theme: &Theme) -> bool {
        let colors = (theme.ansi(), theme.palette().selection);
        if self.colored.replace(Some(colors)) == Some(colors) {
            return false;
        }
        *self.palette.borrow_mut() = Palette::from_theme(theme);
        true
    }

    pub(crate) fn palette(&self) -> Ref<'_, Palette> {
        self.palette.borrow()
    }

    pub(crate) fn term(&self) -> &Arc<FairMutex<Term<Proxy>>> {
        &self.term
    }

    pub(crate) fn watchers(&self, handle: NodeHandle) {
        self.watchers.watch(handle);
    }

    pub(crate) fn generation(&self) -> u64 {
        self.generation.load(Ordering::Relaxed)
    }

    pub(crate) fn mode(&self) -> TermMode {
        *self.term.lock().mode()
    }

    pub(crate) fn layout(&self) -> Layout {
        self.layout.get()
    }

    pub(crate) fn resize(&self, layout: Layout) {
        if layout == self.layout.get() {
            return;
        }
        self.layout.set(layout);
        self.term.lock().resize(layout);
        self.writer.resize(layout.window_size());
    }

    pub(crate) fn send(&self, bytes: impl Into<Cow<'static, [u8]>>) {
        self.writer.write(bytes);
    }

    pub(crate) fn key(&self, key: Key, location: KeyLocation, modifiers: Modifiers) -> bool {
        let mut term = self.term.lock();
        let Some(bytes) = keys::encode(key, location, modifiers, *term.mode()) else {
            return false;
        };
        term.scroll_display(Scroll::Bottom);
        drop(term);
        self.send(bytes);
        true
    }

    pub(crate) fn paste_dropped(&self, offer: DragOffer) {
        let typing = self.typing();
        let spawned = std::thread::Builder::new()
            .name("rikka-terminal-drop".to_owned())
            .spawn(move || {
                if let Some(text) = dropped::typed(&offer) {
                    typing.paste(&text);
                }
            });
        if let Err(error) = spawned {
            tracing::warn!("failed to paste dropped data: {error}");
        }
    }

    pub(crate) fn paste_from(&self, selection: Selection) {
        let typing = self.typing();
        let on_text = Handler::relaying(move |_, (text,): (Option<String>,)| {
            typing.paste(&text.unwrap_or_default());
            Some(())
        });
        rikka_ui::clipboard::read_text_of(selection, on_text);
    }

    fn typing(&self) -> Typing {
        Typing {
            term: Arc::clone(&self.term),
            writer: self.writer.clone(),
        }
    }

    pub(crate) fn composition(&self) -> Ref<'_, Composition> {
        self.composition.borrow()
    }

    pub fn compose(&self, text: &str, cursor: Option<Range<usize>>) -> bool {
        if !self.composition.borrow_mut().set(text, cursor) {
            return false;
        }
        if !text.is_empty() {
            self.term.lock().scroll_display(Scroll::Bottom);
        }
        true
    }

    pub(crate) fn commit(&self, text: &str) {
        self.composition.borrow_mut().clear();
        let typed = typed(text);
        if typed.is_empty() {
            return;
        }
        self.term.lock().scroll_display(Scroll::Bottom);
        self.send(typed.into_bytes());
    }

    pub(crate) fn scroll(&self, scroll: Scroll) {
        self.term.lock().scroll_display(scroll);
    }

    pub(crate) fn wheel(
        &self,
        across: i32,
        up: i32,
        column: usize,
        line: usize,
        modifiers: Modifiers,
    ) -> bool {
        let mode = self.mode();

        if mode.intersects(TermMode::MOUSE_MODE) && !modifiers.shift {
            for (detents, forward, backward) in [
                (across, Button::WheelLeft, Button::WheelRight),
                (up, Button::WheelUp, Button::WheelDown),
            ] {
                let button = match detents.signum() {
                    1 => forward,
                    -1 => backward,
                    _ => continue,
                };
                for _ in 0..detents.abs() {
                    self.report(Motion::Pressed(button), column, line, modifiers, mode);
                }
            }
            return false;
        }

        if mode.contains(TermMode::ALT_SCREEN) && mode.contains(TermMode::ALTERNATE_SCROLL) {
            let key = match up > 0 {
                true => Key::ArrowUp,
                false => Key::ArrowDown,
            };
            if let Some(bytes) = keys::encode(key, KeyLocation::Standard, Modifiers::NONE, mode) {
                for _ in 0..up.abs() * WHEEL_LINES {
                    self.send(bytes.clone());
                }
            }
            return false;
        }

        if up == 0 {
            return false;
        }
        self.scroll(Scroll::Delta(up * WHEEL_LINES));
        true
    }

    pub(crate) fn report(
        &self,
        motion: Motion,
        column: usize,
        line: usize,
        modifiers: Modifiers,
        mode: TermMode,
    ) {
        if let Some(bytes) = mouse::report(motion, column, line, modifiers, mode) {
            self.send(bytes);
        }
    }

    pub(crate) fn focused(&self, focused: bool) {
        let mut term = self.term.lock();
        term.is_focused = focused;
        let reports_focus = term.mode().contains(TermMode::FOCUS_IN_OUT);
        drop(term);
        if reports_focus {
            self.send(if focused { FOCUS_IN } else { FOCUS_OUT }.to_vec());
        }
    }

    pub(crate) fn select(&self, granularity: Granularity, column: usize, line: usize, side: Side) {
        let mut term = self.term.lock();
        let point = viewport_to_point(term.grid().display_offset(), point_of(column, line));
        let selection = match granularity {
            Granularity::Cell => Highlight::new(SelectionType::Simple, point, side),
            Granularity::Block => Highlight::new(SelectionType::Block, point, side),
            Granularity::Line => Highlight::new(SelectionType::Lines, point, side),
            Granularity::Word => spanning(words::word_cells(&term, point)),
        };
        term.selection = Some(selection);
        self.selecting.set(match granularity {
            Granularity::Word => SelectionMode::Words,
            Granularity::Cell | Granularity::Block | Granularity::Line => SelectionMode::Free,
        });
    }

    pub(crate) fn extend_selection(&self, column: usize, line: usize, side: Side) -> bool {
        let mut term = self.term.lock();
        let point = viewport_to_point(term.grid().display_offset(), point_of(column, line));
        if self.selecting.get() != SelectionMode::Words {
            let Some(selection) = term.selection.as_mut() else {
                return false;
            };
            selection.update(point, side);
            return true;
        }
        let Some(ends) = ends(&term) else {
            return false;
        };
        let anchor = words::word_cells(&term, ends.anchor);
        let target = words::word_cells(&term, point);
        let selection = match target.start() >= anchor.start() {
            true => anchored(
                SelectionType::Simple,
                *anchor.start(),
                *anchor.end().max(target.end()),
            ),
            false => anchored(SelectionType::Simple, *anchor.end(), *target.start()),
        };
        term.selection = Some(selection);
        true
    }

    pub(crate) fn select_all(&self) {
        let mut term = self.term.lock();
        let all = movement::first_cell(&term)..=movement::last_cell(&term);
        term.selection = Some(spanning(all));
        self.selecting.set(SelectionMode::Free);
    }

    pub(crate) fn toggle_marking(&self) {
        if self.is_marking() && self.has_selection() {
            self.clear_selection();
            return;
        }
        let mut term = self.term.lock();
        term.scroll_display(Scroll::Bottom);
        let cursor = term.grid().cursor.point;
        term.selection = Some(spanning(cursor..=cursor));
        self.selecting.set(SelectionMode::Marking);
    }

    pub(crate) fn is_marking(&self) -> bool {
        self.selecting.get() == SelectionMode::Marking
    }

    pub(crate) fn move_selection(&self, movement: Movement, extend: bool) {
        let mut term = self.term.lock();
        let Some(ends) = ends(&term) else {
            return;
        };
        let moved = movement.apply(&term, ends.focus);
        let selection = match (extend, ends.block) {
            (true, true) => anchored(SelectionType::Block, ends.anchor, moved),
            (true, false) => anchored(SelectionType::Simple, ends.anchor, moved),
            (false, _) => spanning(moved..=moved),
        };
        term.selection = Some(selection);
        term.scroll_to_point(moved);
        if self.selecting.get() == SelectionMode::Words {
            self.selecting.set(SelectionMode::Free);
        }
    }

    pub(crate) fn has_selection(&self) -> bool {
        self.term
            .lock()
            .selection
            .as_ref()
            .is_some_and(|selection| !selection.is_empty())
    }

    pub(crate) fn clear_selection(&self) {
        self.term.lock().selection = None;
        self.selecting.set(SelectionMode::Free);
    }

    pub fn selected_text(&self) -> Option<String> {
        self.term
            .lock()
            .selection_to_string()
            .filter(|text| !text.is_empty())
    }

    pub fn alternate_screen(&self) -> bool {
        self.mode().contains(TermMode::ALT_SCREEN)
    }

    pub fn answer(&self, question: Question) {
        match question.0 {
            Event::PtyWrite(text) => self.send(text.into_bytes()),
            Event::ColorRequest(index, format) => {
                let term = self.term.lock();
                let color = self.palette.borrow().report(index, term.colors());
                drop(term);
                self.send(format(color).into_bytes());
            }
            Event::TextAreaSizeRequest(format) => {
                self.send(format(self.layout.get().window_size()).into_bytes());
            }
            _ => {}
        }
    }
}

fn spanning(cells: RangeInclusive<GridPoint>) -> Highlight {
    let (start, end) = cells.into_inner();
    let mut selection = Highlight::new(SelectionType::Simple, start, Side::Left);
    selection.update(end, Side::Right);
    selection
}

fn anchored(ty: SelectionType, anchor: GridPoint, focus: GridPoint) -> Highlight {
    let forward = match ty {
        SelectionType::Block => anchor.column <= focus.column,
        SelectionType::Simple | SelectionType::Semantic | SelectionType::Lines => anchor <= focus,
    };
    let (anchor_side, focus_side) = match forward {
        true => (Side::Left, Side::Right),
        false => (Side::Right, Side::Left),
    };
    let mut selection = Highlight::new(ty, anchor, anchor_side);
    selection.update(focus, focus_side);
    selection
}

fn ends<T>(term: &Term<T>) -> Option<Ends> {
    let selection = term.selection.as_ref()?;
    let range = selection.to_range(term)?;
    let mut probe = selection.clone();
    probe.update(movement::last_cell(term), Side::Right);
    let held = probe
        .to_range(term)
        .map_or(range.start, |probed| probed.start);
    let (start, end) = (range.start, range.end);
    let (anchor, focus) = match range.is_block {
        true => {
            let (anchor_line, focus_line) = match held.line == start.line {
                true => (start.line, end.line),
                false => (end.line, start.line),
            };
            let column = held.column.0;
            let (anchor_column, focus_column) =
                match column.abs_diff(start.column.0) <= column.abs_diff(end.column.0) {
                    true => (start.column, end.column),
                    false => (end.column, start.column),
                };
            (
                GridPoint::new(anchor_line, anchor_column),
                GridPoint::new(focus_line, focus_column),
            )
        }
        false => {
            let position = |point: GridPoint| {
                i64::from(point.line.0) * term.columns() as i64 + point.column.0 as i64
            };
            let from_start = position(held).abs_diff(position(start));
            match from_start <= position(held).abs_diff(position(end)) {
                true => (start, end),
                false => (end, start),
            }
        }
    };
    Some(Ends {
        anchor,
        focus,
        block: range.is_block,
    })
}

pub(crate) struct Typing {
    term: Arc<FairMutex<Term<Proxy>>>,
    writer: PtyWriter,
}

impl Typing {
    fn send(&self, bytes: Vec<u8>) {
        self.writer.write(bytes);
    }

    fn paste(&self, text: &str) {
        if text.is_empty() {
            return;
        }
        let mut term = self.term.lock();
        let bracketed = term.mode().contains(TermMode::BRACKETED_PASTE);
        term.scroll_display(Scroll::Bottom);
        drop(term);
        if bracketed {
            self.send(b"\x1b[200~".to_vec());
            self.send(text.replace(['\x1b', '\x03'], "").into_bytes());
            self.send(b"\x1b[201~".to_vec());
            return;
        }
        self.send(typed(text).into_bytes());
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.writer.shut_down();
    }
}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session")
            .field("layout", &self.layout.get())
            .finish_non_exhaustive()
    }
}

fn typed(text: &str) -> String {
    let mut typed = String::with_capacity(text.len());
    let mut after_return = false;
    for character in text.chars() {
        let ends_pair = after_return && character == '\n';
        after_return = character == '\r';
        match character {
            '\n' if ends_pair => {}
            '\n' | '\r' => typed.push('\r'),
            '\t' => typed.push('\t'),
            control if control.is_control() => {}
            other => typed.push(other),
        }
    }
    typed
}

fn point_of(column: usize, line: usize) -> GridPoint<usize> {
    GridPoint::new(line, Column(column))
}

#[derive(Clone)]
pub(crate) struct Proxy {
    report: Arc<dyn Fn(Report) + Send + Sync>,
    generation: Arc<AtomicU64>,
    watchers: Watchers,
}

impl EventListener for Proxy {
    fn send_event(&self, event: Event) {
        self.generation.fetch_add(1, Ordering::Relaxed);
        match Report::from_event(event) {
            Some(report) => (self.report)(report),
            None => self.watchers.repaint(),
        }
    }
}

impl std::fmt::Debug for Proxy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Proxy").finish_non_exhaustive()
    }
}

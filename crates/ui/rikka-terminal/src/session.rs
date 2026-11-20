use std::borrow::Cow;
use std::cell::{Cell, Ref, RefCell};
use std::ops::{Range, RangeInclusive};
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
use rikka_types::{Color, Key, KeyLocation, Modifiers};
use rikka_ui::{DragOffer, Handler, NodeHandle, Selection, Watchers};

use crate::composition::Composition;
use crate::dropped;
use crate::error::Result;
use crate::keys;
use crate::layout::Layout;
use crate::mouse::{self, Button, Motion};
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
    Word,
    Line,
}

pub struct Session {
    term: Arc<FairMutex<Term<Proxy>>>,
    writer: PtyWriter,
    layout: Cell<Layout>,
    composition: RefCell<Composition>,
    palette: RefCell<Palette>,
    colored: Cell<Option<(Ansi, Color)>>,
    generation: Arc<AtomicU64>,
    watchers: Watchers,
    word_anchor: Cell<Option<Side>>,
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
        let writer = pty::start(pty, Arc::clone(&term), proxy, options.access())?;

        Ok(Self {
            term,
            writer,
            layout: Cell::new(layout),
            composition: RefCell::new(Composition::default()),
            palette: RefCell::new(Palette::from_theme(&Theme::default())),
            colored: Cell::new(None),
            generation,
            watchers,
            word_anchor: Cell::new(None),
        })
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

    pub(crate) fn scroll(&self, lines: i32) {
        self.term.lock().scroll_display(Scroll::Delta(lines));
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
        self.scroll(up * WHEEL_LINES);
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
        let (selection, word_anchor) = match granularity {
            Granularity::Cell => (Highlight::new(SelectionType::Simple, point, side), None),
            Granularity::Line => (Highlight::new(SelectionType::Lines, point, side), None),
            Granularity::Word => (spanning(words::word_cells(&term, point)), Some(Side::Left)),
        };
        term.selection = Some(selection);
        self.word_anchor.set(word_anchor);
    }

    pub(crate) fn extend_selection(&self, column: usize, line: usize, side: Side) -> bool {
        let mut term = self.term.lock();
        let point = viewport_to_point(term.grid().display_offset(), point_of(column, line));
        let Some(anchored) = self.word_anchor.get() else {
            let Some(selection) = term.selection.as_mut() else {
                return false;
            };
            selection.update(point, side);
            return true;
        };
        let Some(range) = term
            .selection
            .as_ref()
            .and_then(|selection| selection.to_range(&term))
        else {
            return false;
        };
        let anchor = words::word_cells(
            &term,
            match anchored {
                Side::Left => range.start,
                Side::Right => range.end,
            },
        );
        let target = words::word_cells(&term, point);
        let (span, anchored) = match target.start() >= anchor.start() {
            true => (
                *anchor.start()..=*anchor.end().max(target.end()),
                Side::Left,
            ),
            false => (*target.start()..=*anchor.end(), Side::Right),
        };
        term.selection = Some(spanning(span));
        self.word_anchor.set(Some(anchored));
        true
    }

    pub(crate) fn select_all(&self) {
        let mut term = self.term.lock();
        let start = GridPoint::new(term.topmost_line(), Column(0));
        let end = GridPoint::new(term.bottommost_line(), term.last_column());
        term.selection = Some(spanning(start..=end));
        self.word_anchor.set(None);
    }

    pub(crate) fn clear_selection(&self) {
        self.term.lock().selection = None;
        self.word_anchor.set(None);
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

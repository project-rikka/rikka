mod assist;
pub(crate) mod block;
mod carry;
mod find;
mod gesture;
mod keys;
mod menu;
mod paint;
mod pointer;
pub(crate) mod rows;
mod state;
mod walk;

use std::rc::Rc;

use rikka_types::{Color, Key, Modifiers, PointerButton, Rect};

use rikka_ui::scroll::Place;
use rikka_ui::search::Search;
use rikka_ui::text::{Content, DisplayText, Label, LineEnding, TextDocument, WordRule};
use rikka_ui::{Action, Command, Event, EventCtx, Handler};

use crate::navigation::menu::MenuItem;

pub(crate) use state::EditingState;

pub(crate) const TAB_WIDTH: u16 = 4;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Options {
    pub(crate) single_line: bool,
    pub(crate) editable: bool,
    pub(crate) secret: bool,
    pub(crate) wrap: bool,
    pub(crate) leaves_arrows: bool,
    pub(crate) max_len: Option<usize>,
    pub(crate) bars: bool,
    pub(crate) code: bool,
}

impl Options {
    pub(crate) fn field() -> Self {
        Self {
            single_line: true,
            editable: true,
            secret: false,
            wrap: false,
            leaves_arrows: false,
            max_len: None,
            bars: false,
            code: false,
        }
    }

    pub(crate) fn editor() -> Self {
        Self {
            single_line: false,
            editable: true,
            secret: false,
            wrap: true,
            leaves_arrows: false,
            max_len: None,
            bars: true,
            code: false,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Room {
    pub(crate) bounds: Rect,
    pub(crate) left: f32,
    pub(crate) right: f32,
    pub(crate) top: f32,
    pub(crate) ink: Color,
    pub(crate) hint: Color,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Glance {
    revision: u64,
    preedit: u64,
    caret: usize,
    anchor: usize,
}

impl Glance {
    fn from_document(document: &TextDocument) -> Self {
        let content = document.content();
        Self {
            revision: content.revision(),
            preedit: content.preedit_revision(),
            caret: content.caret(),
            anchor: content.anchor(),
        }
    }
}

pub(crate) struct Editing {
    pub(crate) document: TextDocument,
    glance: Glance,
    pub(crate) options: Options,
    pub(crate) placeholder: Option<Label>,
    pub(crate) on_change: Action,
    pub(crate) on_submit: Action,
    pub(crate) on_blur: Action,
    pub(crate) on_key: Action<(Key, Modifiers), bool>,
    pub(crate) on_paste: Action<(Vec<String>,), bool>,
    pub(crate) on_command: Action<(Command,), bool>,
    pub(crate) context_items: Action<(), Vec<MenuItem>>,
    pub(crate) search: Option<Search>,
    pub(crate) place: Option<Place>,
    pub(crate) language: Option<rikka_syntax::Language>,
}

impl Editing {
    pub(crate) fn new(document: &TextDocument, options: Options) -> Self {
        Self {
            document: document.clone(),
            glance: Glance::from_document(document),
            options,
            placeholder: None,
            on_change: Action::default(),
            on_submit: Action::default(),
            on_blur: Action::default(),
            on_key: Action::default(),
            on_paste: Action::default(),
            on_command: Action::default(),
            context_items: Action::default(),
            search: None,
            place: None,
            language: None,
        }
    }

    pub(crate) fn looks_like(&self, other: &Editing) -> bool {
        !self.reads_shared()
            && !other.reads_shared()
            && self.document.same(&other.document)
            && self.glance == other.glance
            && self.options == other.options
            && self.placeholder == other.placeholder
    }

    fn reads_shared(&self) -> bool {
        self.search.is_some() || self.place.is_some() || self.language.is_some()
    }

    pub(crate) fn editable(&self) -> bool {
        self.options.editable
    }

    pub(crate) fn is_secret(&self) -> bool {
        self.options.secret
    }

    pub(crate) fn composes(&self) -> bool {
        self.options.editable && !self.options.secret
    }

    pub(crate) fn word_rule(&self) -> WordRule {
        match self.options.code || self.language.is_some() {
            true => WordRule::Code,
            false => WordRule::Prose,
        }
    }

    pub(crate) fn wraps(&self) -> bool {
        self.options.wrap && !self.options.single_line
    }

    pub(crate) fn shown<'a>(&self, content: &'a Content) -> DisplayText<'a> {
        match self.options.secret {
            true => DisplayText::secret(content),
            false => DisplayText::new(content),
        }
    }

    fn acceptable(&self, text: &str) -> String {
        acceptable(&self.document, self.options, text)
    }

    fn insert(&self, text: &str) -> bool {
        insert(&self.document, self.options, text)
    }

    fn landing(&self, state: &EditingState) -> Handler<(Option<String>,)> {
        let document = self.document.clone();
        let options = self.options;
        let on_change = self.on_change.clone();
        let pasted = Rc::clone(&state.pasted);
        Handler::relaying(move |reach, (text,): (Option<String>,)| {
            let text = text?;
            pasted.set(true);
            if insert(&document, options, &text) {
                on_change.call(reach, ());
            }
            Some(())
        })
    }
}

fn room(document: &TextDocument, options: Options) -> usize {
    let Some(max) = options.max_len else {
        return usize::MAX;
    };
    let content = document.content();
    max.saturating_sub(content.char_count() - content.selected_char_count())
}

fn acceptable(document: &TextDocument, options: Options, text: &str) -> String {
    let breaks = !options.single_line;
    LineEnding::normalize(text)
        .chars()
        .filter(|letter| !letter.is_control() || (breaks && (*letter == '\n' || *letter == '\t')))
        .take(room(document, options))
        .collect()
}

fn insert(document: &TextDocument, options: Options, text: &str) -> bool {
    let typed = acceptable(document, options, text);
    if typed.is_empty() && !text.is_empty() {
        return false;
    }
    document.insert(&typed)
}

impl Editing {
    pub(crate) fn changed(&self, ctx: &mut EventCtx<'_>) {
        ctx.request_paint();
        ctx.act(&self.on_change, ());
    }

    pub(crate) fn event(&self, state: &mut EditingState, event: &Event, ctx: &mut EventCtx<'_>) {
        if state.menu.event(event, ctx) {
            return;
        }
        if !self.assists() {
            self.handle(state, event, ctx);
            return;
        }
        if self.completion_pointer(state, event, ctx) {
            return;
        }
        self.handle(state, event, ctx);
        match event {
            Event::FocusLost => self.dismiss(state, ctx),
            _ => {
                let cause = state.cause.take();
                self.follow_completion(state, cause, ctx);
            }
        }
    }

    fn handle(&self, state: &mut EditingState, event: &Event, ctx: &mut EventCtx<'_>) {
        state.down.adopt(self.place.as_ref(), None);
        if self.grabbed(state, event, ctx) || self.handle_drop_event(state, event, ctx) {
            return;
        }
        match event {
            Event::PointerPressed {
                button: PointerButton::Primary,
                position,
                modifiers,
            } if ctx.bounds().contains(*position) => {
                ctx.set_handled();
                self.press(state, *position, modifiers.shift, ctx);
            }
            Event::PointerPressed {
                button: PointerButton::Secondary,
                position,
                ..
            } if ctx.bounds().contains(*position) => {
                self.summon(state, *position, ctx);
            }
            Event::PointerMoved(position) if state.dragging => {
                ctx.set_handled();
                self.drag(state, *position, ctx);
            }
            Event::PointerReleased {
                button: PointerButton::Primary,
                ..
            } if state.dragging => {
                state.dragging = false;
                ctx.release_pointer();
                ctx.set_handled();
            }
            Event::Scroll {
                delta, modifiers, ..
            } if self.options.bars => {
                if self.scrolled(state, *delta, modifiers.shift) {
                    ctx.request_paint();
                    ctx.set_handled();
                }
            }
            Event::KeyPressed { key, modifiers, .. } if ctx.focused() => {
                if self.typed(state, *key, *modifiers, ctx) {
                    ctx.set_handled();
                }
            }
            Event::Command(command) if ctx.focused() => {
                if self.commanded(state, *command, ctx) {
                    ctx.set_handled();
                }
            }
            Event::Preedit { text, cursor } if ctx.focused() && self.composes() => {
                ctx.set_handled();
                if self.document.edit(|content| {
                    content.interrupt();
                    content.set_preedit(text, cursor.clone())
                }) {
                    state.follow = true;
                    self.changed(ctx);
                }
            }
            Event::Commit(text) if ctx.focused() && self.composes() => {
                ctx.set_handled();
                let composed = self.document.edit(|content| content.clear_preedit());
                state.cause = text.chars().last().map(assist::Cause::Typed);
                if self.insert(text) || composed {
                    state.follow = true;
                    self.changed(ctx);
                }
            }
            Event::FocusLost => {
                state.dragging = false;
                state.drag_origin = None;
                if self.document.edit(|content| content.clear_preedit()) {
                    self.changed(ctx);
                }
                ctx.act(&self.on_blur, ());
            }
            _ => {}
        }
    }
}

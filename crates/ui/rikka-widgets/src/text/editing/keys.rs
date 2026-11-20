use std::sync::Arc;

use rikka_types::{Key, Modifiers};
use rikka_ui::search::{self, Action};
use rikka_ui::text::{KeyEdit, TextLayout};
use rikka_ui::{Command, EventCtx, clipboard};

use crate::text::editing::state::EditingState;
use crate::text::editing::walk::{self, Step};
use crate::text::editing::{Editing, TAB_WIDTH};

impl Editing {
    pub(crate) fn typed(
        &self,
        state: &mut EditingState,
        key: Key,
        modifiers: Modifiers,
        ctx: &mut EventCtx<'_>,
    ) -> bool {
        if ctx.answer(&self.on_key, (key, modifiers)).unwrap_or(false) {
            return true;
        }
        if self.assists() {
            if self.assisted(state, key, modifiers, ctx) {
                return true;
            }
            state.cause = Self::cause_of(key, modifiers);
        }

        if self.options.leaves_arrows && matches!(key, Key::ArrowLeft | Key::ArrowRight) {
            return false;
        }

        if let Some(rows) = self.step_of(state, key) {
            let step = Step {
                rows,
                extend: modifiers.shift,
            };
            return self.walk(state, step, ctx);
        }
        if key == Key::Enter && self.submitted(modifiers, ctx) {
            return true;
        }

        let Some(edit) = KeyEdit::from_key(key, modifiers, !self.options.single_line) else {
            return false;
        };
        let document = &self.document;
        let editable = self.options.editable;
        let changed = match edit {
            KeyEdit::Travel { to, extend } => {
                state.goal = None;
                let laid = self.caret_line(state);
                document.travel(
                    to,
                    extend,
                    laid.as_deref().and_then(|laid| laid.paragraph(0)),
                    self.word_rule(),
                )
            }
            KeyEdit::Erase(_) if !editable => false,
            KeyEdit::Erase(erase) => document.erase(erase, self.word_rule()),
            KeyEdit::Newline if editable => document.insert_newline(true),
            KeyEdit::Tab { back: true } if self.indents() => document.outdent(&self.indent_unit()),
            KeyEdit::Tab { .. } if self.indents() && self.spans_lines() => {
                document.indent(&self.indent_unit())
            }
            KeyEdit::Tab { .. } if self.indents() => self.insert(&self.indent_unit()),
            KeyEdit::Type(typed) if editable => self.insert(typed.encode_utf8(&mut [0u8; 4])),
            KeyEdit::Newline | KeyEdit::Tab { .. } | KeyEdit::Type(_) => return false,
        };
        state.follow = true;
        if changed {
            self.changed(ctx);
        } else {
            ctx.request_paint();
        }
        true
    }

    pub(crate) fn commanded(
        &self,
        state: &mut EditingState,
        command: Command,
        ctx: &mut EventCtx<'_>,
    ) -> bool {
        if ctx.answer(&self.on_command, (command,)).unwrap_or(false) {
            return true;
        }
        match command {
            Command::ContextMenu => {
                self.open_from_keyboard(state, ctx);
                true
            }
            Command::Delete if self.options.editable => {
                state.follow = true;
                if self.document.delete_selection() {
                    self.changed(ctx);
                }
                true
            }
            Command::Copy | Command::Cut | Command::Paste => self.clipboard(state, command, ctx),
            Command::Undo | Command::Redo => self.history(state, command, ctx),
            Command::FindNext | Command::FindPrevious => {
                let Some(search) = self.search.as_ref().filter(|search| search.is_open()) else {
                    return false;
                };
                search.act(Action::Step(match command {
                    Command::FindNext => search::Step::Next,
                    _ => search::Step::Previous,
                }));
                true
            }
            Command::SelectNone => {
                let document = &self.document;
                if document.edit(|content| {
                    let caret = content.caret();
                    content.place_caret(caret, false)
                }) {
                    ctx.request_paint();
                }
                true
            }
            Command::SelectAll => {
                state.follow = true;
                if self.document.select_all() {
                    self.changed(ctx);
                } else {
                    ctx.request_paint();
                }
                true
            }
            _ => false,
        }
    }

    fn clipboard(
        &self,
        state: &mut EditingState,
        command: Command,
        ctx: &mut EventCtx<'_>,
    ) -> bool {
        let document = &self.document;
        match command {
            Command::Copy | Command::Cut if self.options.secret => true,
            Command::Copy | Command::Cut => {
                if document.has_selection() {
                    clipboard::set_text(&document.selected_text());
                }
                if command == Command::Cut && self.options.editable && document.delete_selection() {
                    state.follow = true;
                    self.changed(ctx);
                }
                true
            }
            Command::Paste => {
                if ctx
                    .answer(&self.on_paste, (clipboard::mime_types(),))
                    .unwrap_or(false)
                {
                    return true;
                }
                if !self.options.editable {
                    return true;
                }
                clipboard::read_text(self.landing(state));
                true
            }
            _ => false,
        }
    }

    fn history(&self, state: &mut EditingState, command: Command, ctx: &mut EventCtx<'_>) -> bool {
        let took_back = match command {
            Command::Undo => self.options.editable && self.document.undo(),
            _ => self.options.editable && self.document.redo(),
        };
        state.follow = true;
        state.goal = None;
        if took_back {
            self.changed(ctx);
        }
        true
    }

    fn submitted(&self, modifiers: Modifiers, ctx: &mut EventCtx<'_>) -> bool {
        let plain = !modifiers.control && !modifiers.alt && !modifiers.logo;
        if self.options.single_line {
            if !plain {
                return false;
            }
            ctx.act(&self.on_submit, ());
            return true;
        }
        if modifiers != Modifiers::NONE {
            return false;
        }
        if !self.on_submit.is_set() {
            return false;
        }
        ctx.act(&self.on_submit, ());
        true
    }

    fn step_of(&self, state: &EditingState, key: Key) -> Option<i32> {
        if self.options.single_line {
            return None;
        }
        let page = (state.frame.room.height() / state.frame.line_height.max(1.0)).floor() as i32;
        Some(match key {
            Key::ArrowUp => -1,
            Key::ArrowDown => 1,
            Key::PageUp => -(page - 1).max(1),
            Key::PageDown => (page - 1).max(1),
            _ => return None,
        })
    }

    fn walk(&self, state: &mut EditingState, step: Step, ctx: &mut EventCtx<'_>) -> bool {
        let EditingState {
            template: Some(template),
            rows,
            down,
            frame,
            visible,
            goal,
            follow,
            ..
        } = state
        else {
            return false;
        };
        let width = self.wraps().then_some(frame.area.width());
        let target = {
            let content = self.document.content();
            let shown = self.shown(&content);
            let risen = rows.sync(&shown, template, width, visible.clone());
            down.shift(risen as f32 * template.line_height);
            walk::take(&shown, rows, template, width, step, *goal)
        };
        *goal = Some(target.aim);
        *follow = true;
        self.document
            .place_cursor(target.at, target.affinity, step.extend);
        ctx.request_paint();
        true
    }

    fn caret_line(&self, state: &EditingState) -> Option<Arc<TextLayout>> {
        let template = state.template.as_ref()?;
        let content = self.document.content();
        let shown = self.shown(&content);
        let line = shown.line_of(shown.caret());
        if shown.is_secret() || shown.composing_line() == Some(line) {
            return None;
        }
        let width = self.wraps().then_some(state.frame.area.width());
        Some(walk::line_layout(&shown, template, width, line))
    }

    fn indents(&self) -> bool {
        self.options.editable && !self.options.single_line
    }

    fn spans_lines(&self) -> bool {
        let content = self.document.content();
        let selection = content.selection();
        !selection.is_empty() && content.line_of(selection.start) != content.line_of(selection.end)
    }
}

impl Editing {
    fn indent_unit(&self) -> String {
        match self.assists() {
            true => self.unit(),
            false => " ".repeat(usize::from(TAB_WIDTH)),
        }
    }
}

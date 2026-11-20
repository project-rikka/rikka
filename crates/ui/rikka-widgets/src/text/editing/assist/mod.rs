mod comment;
mod complete;
mod indent;
mod pairs;
mod popup;

use rikka_syntax::{Lexicon, QUOTES, is_word};
use rikka_theme::Token;
use rikka_types::{Key, Modifiers};
use rikka_ui::EventCtx;

use crate::text::editing::state::EditingState;
use crate::text::editing::{Editing, TAB_WIDTH};

pub(crate) use complete::{Cause, Completing};

use complete::{SHOWN, reaches_into, word_start};
use pairs::Typing;

impl Editing {
    pub(crate) fn assists(&self) -> bool {
        self.options.code && self.options.editable && !self.options.single_line
    }

    pub(crate) fn unit(&self) -> String {
        indent::unit_of(&self.document.content(), usize::from(TAB_WIDTH))
    }

    fn quotes(&self) -> &'static str {
        self.language.map_or(QUOTES, rikka_syntax::Language::quotes)
    }

    pub(crate) fn assisted(
        &self,
        state: &mut EditingState,
        key: Key,
        modifiers: Modifiers,
        ctx: &mut EventCtx<'_>,
    ) -> bool {
        if self.choosing(state, key, modifiers, ctx) {
            return true;
        }
        let command = modifiers.control && !modifiers.alt && !modifiers.logo;
        let plain = modifiers == Modifiers::NONE;
        let changed = match key {
            Key::Space if command && !modifiers.shift => {
                state.cause = Some(Cause::Asked);
                return true;
            }
            Key::Char('/') if command => {
                let Some(comment) = self.language.and_then(rikka_syntax::Language::comment) else {
                    return false;
                };
                self.document
                    .edit(|content| comment::toggle(content, comment))
            }
            Key::Enter if plain => {
                let unit = self.unit();
                let openers = self.language.map_or("", rikka_syntax::Language::openers);
                self.document
                    .edit(|content| indent::newline(content, &unit, openers))
            }
            Key::Backspace if plain => {
                let quotes = self.quotes();
                match self
                    .document
                    .edit(|content| pairs::backspace(content, quotes).unwrap_or(false))
                {
                    true => {
                        state.cause = Some(Cause::Deleted);
                        true
                    }
                    false => return false,
                }
            }
            Key::Char(letter) if !command && !modifiers.alt && !modifiers.logo => {
                let quotes = self.quotes();
                let typing = Typing {
                    quotes,
                    prose: quotes.contains(letter) && self.in_prose(state),
                    fallback: usize::from(TAB_WIDTH),
                };
                let mut handled = false;
                let changed = self
                    .document
                    .edit(|content| match typing.typed(content, letter) {
                        Some(changed) => {
                            handled = true;
                            changed
                        }
                        None => false,
                    });
                if !handled {
                    return false;
                }
                state.cause = Some(Cause::Typed(letter));
                changed
            }
            _ => return false,
        };
        state.follow = true;
        match changed {
            true => self.changed(ctx),
            false => ctx.request_paint(),
        }
        true
    }

    pub(crate) fn cause_of(key: Key, modifiers: Modifiers) -> Option<Cause> {
        let typing = !modifiers.control && !modifiers.alt && !modifiers.logo;
        match key {
            Key::Char(letter) if typing => Some(Cause::Typed(letter)),
            Key::Space if typing => Some(Cause::Typed(' ')),
            Key::Backspace | Key::Delete => Some(Cause::Deleted),
            _ => None,
        }
    }

    fn choosing(
        &self,
        state: &mut EditingState,
        key: Key,
        modifiers: Modifiers,
        ctx: &mut EventCtx<'_>,
    ) -> bool {
        let Some(completing) = state.completing.as_mut() else {
            return false;
        };
        if modifiers != Modifiers::NONE {
            return false;
        }
        match key {
            Key::ArrowUp => completing.step(-1),
            Key::ArrowDown => completing.step(1),
            Key::PageUp => completing.step(-(SHOWN as isize)),
            Key::PageDown => completing.step(SHOWN as isize),
            Key::Tab | Key::Enter => {
                self.accept(state, ctx);
                return true;
            }
            Key::Escape => {
                self.dismiss(state, ctx);
                return true;
            }
            _ => return false,
        }
        self.preview(state);
        ctx.request_paint();
        true
    }

    pub(crate) fn accept(&self, state: &mut EditingState, ctx: &mut EventCtx<'_>) {
        let Some(completing) = state.completing.take() else {
            return;
        };
        let Some(chosen) = completing.chosen() else {
            return;
        };
        let word = chosen.word().to_owned();
        let start = completing.start;
        self.document.edit(|content| {
            content.clear_ghost();
            let caret = content.caret().max(start);
            content.replace_each(&[(start..caret, word)])
        });
        state.follow = true;
        self.changed(ctx);
    }

    pub(crate) fn dismiss(&self, state: &mut EditingState, ctx: &mut EventCtx<'_>) {
        if state.completing.take().is_none() {
            return;
        }
        self.document.edit(|content| content.clear_ghost());
        ctx.request_paint();
    }

    fn in_prose(&self, state: &mut EditingState) -> bool {
        let content = self.document.content();
        let caret = content.caret();
        state
            .syntax
            .as_mut()
            .and_then(|parsed| parsed.token_at(&content, caret))
            .is_some_and(|token| matches!(token, Token::Comment | Token::String))
    }

    pub(crate) fn follow_completion(
        &self,
        state: &mut EditingState,
        cause: Option<Cause>,
        ctx: &mut EventCtx<'_>,
    ) {
        if !self.assists() || (cause.is_none() && state.completing.is_none()) {
            return;
        }
        let trigger = {
            let content = self.document.content();
            (content.revision(), content.caret())
        };
        let unmoved = state
            .completing
            .as_ref()
            .is_some_and(|completing| completing.trigger == trigger);
        if cause.is_none() && unmoved {
            return;
        }
        let (caret, start, typed, selecting) = {
            let content = self.document.content();
            let caret = content.caret();
            let start = word_start(&content, caret, self.language);
            (
                caret,
                start,
                content.slice(start..caret),
                content.has_selection(),
            )
        };
        if selecting {
            self.dismiss(state, ctx);
            return;
        }
        let keep_open = state
            .completing
            .as_ref()
            .map(|completing| (completing.start, completing.trigger, completing.lingers));
        let opening = match cause {
            Some(Cause::Asked) => Some((start, true)),
            Some(Cause::Typed(letter)) if is_word(self.language, letter) => match keep_open {
                Some((was, ..)) if was == start => None,
                _ if self.in_prose(state) => {
                    self.dismiss(state, ctx);
                    return;
                }
                _ => Some((start, false)),
            },
            Some(Cause::Typed(letter)) => {
                let reaching = reaches_into(&self.document.content(), caret, letter);
                match reaching && !self.in_prose(state) {
                    true => Some((caret, true)),
                    false => {
                        self.dismiss(state, ctx);
                        return;
                    }
                }
            }
            Some(Cause::Deleted) | None => {
                let Some((was, before, lingers)) = keep_open else {
                    return;
                };
                if before == trigger {
                    return;
                }
                if was != start || caret < was || (typed.is_empty() && !lingers) {
                    self.dismiss(state, ctx);
                    return;
                }
                None
            }
        };
        if let Some((start, lingers)) = opening {
            let lexicon = self.lexicon(state, caret);
            state.completing = Some(Completing::new(start, lexicon, lingers));
        }
        let Some(completing) = state.completing.as_mut() else {
            return;
        };
        if !completing.narrow(&typed, trigger) {
            self.dismiss(state, ctx);
            return;
        }
        self.preview(state);
        ctx.request_paint();
    }

    fn lexicon(&self, state: &mut EditingState, caret: usize) -> Lexicon {
        let content = self.document.content();
        match state.syntax.as_mut() {
            Some(parsed) => parsed.lexicon(&content, caret),
            None => Lexicon::from_text(content.rope(), caret, self.language),
        }
    }

    fn preview(&self, state: &EditingState) {
        let Some(completing) = state.completing.as_ref() else {
            return;
        };
        let typed = {
            let content = self.document.content();
            content.slice(completing.start..content.caret().max(completing.start))
        };
        let ghost = completing.ghost(&typed);
        self.document.edit(|content| content.set_ghost(&ghost));
    }
}

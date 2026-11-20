use rikka_types::{Key, KeyLocation, Modifiers, Point, Rect};
use rikka_ui::text::{KeyEdit, WordRule};
use rikka_ui::{Command, Event, EventCtx, Handler, LineEnding, TextDocument, clipboard};
use rikka_widgets::{ContextMenu, text_menu};

use rikka_figure::Typeset;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TypingOutcome {
    Edited,
    Unchanged,
    Committed,
    Dropped,
    Ignored,
}

impl TypingOutcome {
    fn from_changed(changed: bool) -> TypingOutcome {
        match changed {
            true => TypingOutcome::Edited,
            false => TypingOutcome::Unchanged,
        }
    }
}

pub struct Typing<'a> {
    document: &'a TextDocument,
    menu: &'a mut ContextMenu,
    caret: Option<Rect>,
}

impl<'a> Typing<'a> {
    pub fn new(document: &'a TextDocument, menu: &'a mut ContextMenu) -> Typing<'a> {
        Typing {
            document,
            menu,
            caret: None,
        }
    }

    #[must_use]
    pub fn caret(mut self, caret: Option<Rect>) -> Typing<'a> {
        self.caret = caret;
        self
    }

    pub fn handle(
        self,
        event: &Event,
        typeset: impl FnOnce() -> Typeset,
        ctx: &mut EventCtx<'_>,
    ) -> TypingOutcome {
        match event {
            Event::KeyPressed {
                key,
                location,
                modifiers,
                ..
            } => type_key(self.document, typeset, *key, *location, *modifiers),
            Event::Command(Command::ContextMenu) => {
                self.menu
                    .open_by_keys(text_menu(self.document), self.caret, ctx);
                TypingOutcome::Unchanged
            }
            Event::Command(command) => type_command(self.document, *command),
            _ => compose(self.document, event),
        }
    }
}

impl std::fmt::Debug for Typing<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Typing")
            .field("caret", &self.caret)
            .finish_non_exhaustive()
    }
}

fn type_key(
    document: &TextDocument,
    typeset: impl FnOnce() -> Typeset,
    key: Key,
    location: KeyLocation,
    modifiers: Modifiers,
) -> TypingOutcome {
    let command = modifiers.control && !modifiers.alt && !modifiers.logo;
    match key {
        Key::Escape => return TypingOutcome::Dropped,
        Key::Enter if location == KeyLocation::Keypad || command => {
            return TypingOutcome::Committed;
        }
        Key::Tab if command => return TypingOutcome::Ignored,
        Key::ArrowUp | Key::ArrowDown => {
            document.interrupt();
            let typeset = typeset();
            let caret = typeset.caret(document.caret(), document.affinity());
            let step = match key {
                Key::ArrowUp => -typeset.line_height(),
                _ => typeset.line_height(),
            };
            let aim = Point::new(caret.x(), caret.y() + caret.height() / 2.0 + step);
            let hit = typeset.hit(aim);
            return TypingOutcome::from_changed(document.place_cursor(
                hit.at,
                hit.affinity,
                modifiers.shift,
            ));
        }
        _ => {}
    }
    let Some(edit) = KeyEdit::from_key(key, modifiers, true) else {
        return TypingOutcome::Ignored;
    };
    TypingOutcome::from_changed(match edit {
        KeyEdit::Travel { to, extend } => {
            let typeset = typeset();
            let (line, _) = document.caret_line_column();
            document.travel(to, extend, typeset.paragraph(line), WordRule::Prose)
        }
        KeyEdit::Erase(erase) => document.erase(erase, WordRule::Prose),
        KeyEdit::Newline => document.insert_newline(false),
        KeyEdit::Tab { .. } => document.insert("\t"),
        KeyEdit::Type(typed) => document.insert(typed.encode_utf8(&mut [0u8; 4])),
    })
}

fn type_command(document: &TextDocument, command: Command) -> TypingOutcome {
    match command {
        Command::SelectAll => TypingOutcome::from_changed(document.select_all()),
        Command::Copy | Command::Cut => {
            if !document.has_selection() {
                return TypingOutcome::Unchanged;
            }
            clipboard::set_text(&document.selected_text());
            TypingOutcome::from_changed(command == Command::Cut && document.delete_selection())
        }
        Command::Paste => {
            let document = document.clone();
            clipboard::read_text(Handler::relaying(move |_, (pasted,): (Option<String>,)| {
                if let Some(pasted) = pasted {
                    document.insert(typable(&pasted));
                }
                Some(())
            }));
            TypingOutcome::Unchanged
        }
        Command::Delete => TypingOutcome::from_changed(document.delete_selection()),
        Command::Undo => TypingOutcome::from_changed(document.undo()),
        Command::Redo => TypingOutcome::from_changed(document.redo()),
        _ => TypingOutcome::Ignored,
    }
}

fn compose(document: &TextDocument, event: &Event) -> TypingOutcome {
    match event {
        Event::Preedit { text, cursor } => {
            document.edit(|content| {
                content.interrupt();
                content.set_preedit(text, cursor.clone())
            });
        }
        Event::Commit(text) => {
            document.edit(|content| content.clear_preedit());
            document.insert(typable(text));
        }
        Event::FocusLost => {
            document.edit(|content| content.clear_preedit());
            return TypingOutcome::Ignored;
        }
        _ => return TypingOutcome::Ignored,
    }
    TypingOutcome::Edited
}

#[must_use]
pub fn text_with_preedit(document: &TextDocument) -> String {
    let content = document.content();
    let mut text = content.text();
    text.insert_str(content.caret(), content.preedit());
    text
}

#[must_use]
fn typable(text: &str) -> String {
    LineEnding::normalize(text)
        .chars()
        .filter(|letter| !letter.is_control() || *letter == '\n' || *letter == '\t')
        .collect()
}

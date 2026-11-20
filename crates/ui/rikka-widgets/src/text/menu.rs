use rikka_ui::text::TextDocument;
use rikka_ui::{Command, clipboard};

use crate::navigation::menu::{MenuItem, command_item, menu_separator};

pub fn text_menu(document: &TextDocument) -> Vec<MenuItem> {
    MenuOptions::editable(clipboard::has_text()).items(document)
}

pub(crate) struct MenuOptions {
    pub(crate) editable: bool,
    pub(crate) secret: bool,
    pub(crate) pasteable: bool,
    pub(crate) extras: Vec<MenuItem>,
}

impl MenuOptions {
    pub(crate) fn editable(pasteable: bool) -> Self {
        Self {
            editable: true,
            secret: false,
            pasteable,
            extras: Vec::new(),
        }
    }

    pub(crate) fn items(self, document: &TextDocument) -> Vec<MenuItem> {
        let (selected, empty, undoable, redoable) = {
            let content = document.content();
            (
                content.has_selection(),
                content.char_count() == 0,
                content.can_undo(),
                content.can_redo(),
            )
        };
        let copyable = selected && !self.secret;
        let mut items = match self.editable {
            true => vec![
                command_item(Command::Undo).enabled(undoable),
                command_item(Command::Redo).enabled(redoable),
                menu_separator(),
                command_item(Command::Cut).enabled(copyable),
                command_item(Command::Copy).enabled(copyable),
                command_item(Command::Paste).enabled(self.pasteable),
                command_item(Command::Delete).enabled(selected),
            ],
            false => vec![command_item(Command::Copy).enabled(copyable)],
        };
        items.push(menu_separator());
        items.extend(self.extras);
        items.push(menu_separator());
        items.push(command_item(Command::SelectAll).enabled(!empty));
        items
    }
}

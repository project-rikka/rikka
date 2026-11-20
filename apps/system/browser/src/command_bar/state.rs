use rikka_app::prelude::*;

use crate::omnibox::Picker;

#[derive(State)]
pub(crate) struct CommandBar {
    #[untracked]
    field: TextDocument,
    open: Tracked<bool>,
    #[state]
    pub(crate) picker: Picker,
}

impl CommandBar {
    pub(crate) fn new() -> CommandBar {
        CommandBar {
            field: TextDocument::new(""),
            open: Tracked::new(false),
            picker: Picker::new(),
        }
    }

    pub(crate) fn open(&mut self) {
        self.field.reset("");
        self.picker.reset();
        self.open.set(true);
    }

    pub(crate) fn close(&mut self) {
        self.picker.reset();
        if *self.open {
            self.open.set(false);
        }
    }

    pub(crate) fn is_open(&self) -> bool {
        *self.open
    }

    pub(crate) fn field(&self) -> &TextDocument {
        &self.field
    }

    pub(crate) fn typed(&self) -> String {
        self.field.text()
    }
}

impl std::fmt::Debug for CommandBar {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CommandBar")
            .field("open", &*self.open)
            .finish_non_exhaustive()
    }
}

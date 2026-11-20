mod view;

use rikka_app::prelude::*;

pub(crate) use view::view;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) enum Tab {
    #[default]
    Online,
    All,
    Pending,
    Blocked,
    Add,
}

impl Tab {
    pub(crate) const LISTED: [Tab; 4] = [Tab::Online, Tab::All, Tab::Pending, Tab::Blocked];

    pub(crate) fn label(self) -> Label {
        match self {
            Tab::Online => tr!("discord.friends.online"),
            Tab::All => tr!("discord.friends.all"),
            Tab::Pending => tr!("discord.friends.pending"),
            Tab::Blocked => tr!("discord.friends.blocked"),
            Tab::Add => tr!("discord.friends.add"),
        }
    }
}

#[derive(State)]
pub(crate) struct Friends {
    tab: Tracked<Tab>,
    #[untracked]
    handle: TextDocument,
    outcome: Tracked<Option<Result<(), String>>>,
}

impl Friends {
    pub(crate) fn new() -> Friends {
        Friends {
            tab: Tracked::new(Tab::default()),
            handle: TextDocument::new(""),
            outcome: Tracked::new(None),
        }
    }

    pub(crate) fn tab(&self) -> Tab {
        *self.tab
    }

    pub(crate) fn outcome(&self) -> Option<&Result<(), String>> {
        self.outcome.as_ref()
    }

    pub(crate) fn field(&self) -> &TextDocument {
        &self.handle
    }

    pub(crate) fn handle(&self) -> String {
        self.handle.text().trim().to_owned()
    }

    pub(crate) fn show(&mut self, tab: Tab) {
        self.outcome.set(None);
        self.tab.set(tab);
    }

    pub(crate) fn answered(&mut self, answer: Result<(), String>) {
        if answer.is_ok() && *self.tab == Tab::Add {
            self.handle.clear();
        }
        self.outcome.set(Some(answer));
    }
}

impl std::fmt::Debug for Friends {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Friends")
            .field("tab", &*self.tab)
            .finish_non_exhaustive()
    }
}

use rikka_app::prelude::*;

use browser_model::{FolderId, ProfileId, SpaceId};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Renaming {
    Space(SpaceId),
    Folder(FolderId),
    Profile(ProfileId),
}

#[derive(State)]
pub(crate) struct Sidebar {
    renaming: Tracked<Option<Renaming>>,
    #[untracked]
    draft: TextDocument,
}

impl Sidebar {
    pub(crate) fn new() -> Sidebar {
        Sidebar {
            renaming: Tracked::new(None),
            draft: TextDocument::new(""),
        }
    }

    pub(crate) fn renaming(&self) -> Option<Renaming> {
        *self.renaming
    }

    pub(crate) fn draft(&self) -> &TextDocument {
        &self.draft
    }

    pub(crate) fn rename(&mut self, target: Renaming, current: &str) {
        self.draft.reset(current);
        self.draft.select_all();
        self.renaming.set(Some(target));
    }

    pub(crate) fn abandon_renaming(&mut self) {
        if self.renaming.is_some() {
            self.renaming.set(None);
        }
    }

    pub(crate) fn finish_renaming(&mut self) -> Option<(Renaming, String)> {
        let target = *self.renaming;
        if target.is_some() {
            self.renaming.set(None);
        }
        target.map(|target| (target, self.draft.text()))
    }
}

impl std::fmt::Debug for Sidebar {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Sidebar")
            .field("renaming", &*self.renaming)
            .finish_non_exhaustive()
    }
}

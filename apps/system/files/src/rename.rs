use files_core::Location;
use files_vfs::{Error, Vfs};
use rikka_file_operations::Store;
use rikka_ui::TextDocument;

use crate::browser::{Rename, Row};
use crate::tab::Tab;
use crate::wording;

pub(crate) enum RenameOutcome {
    Nothing,
    Renamed { from: Location, to: Location },
    Refused(Error),
}

#[derive(Debug, Default)]
pub(crate) struct Renaming {
    open: Option<Open>,
    sessions: u64,
}

impl Renaming {
    pub(crate) fn make(
        &mut self,
        vfs: &Vfs,
        here: &Location,
        folder: bool,
    ) -> Result<Location, Error> {
        let name = rikka_format::unique_name(&wording::new_name(folder), |candidate| {
            vfs.stat(&here.child(candidate)).is_ok()
        });
        let at = here.child(&name);
        if folder {
            vfs.create_directory(&at)?;
        } else {
            vfs.create_file(&at)?;
        }
        self.begin(at.clone(), &name, folder);
        Ok(at)
    }

    pub(crate) fn commit(&mut self, vfs: &Vfs, typed: &str) -> RenameOutcome {
        let Some(open) = self.open.take() else {
            return RenameOutcome::Nothing;
        };
        let name = typed.trim();
        if name == open.name {
            return RenameOutcome::Nothing;
        }
        match vfs.rename(&open.at, name) {
            Ok(to) => RenameOutcome::Renamed { from: open.at, to },
            Err(error) => {
                self.open = Some(open);
                RenameOutcome::Refused(error)
            }
        }
    }

    pub(crate) fn end(&mut self) {
        self.open = None;
    }

    pub(crate) fn showing(&self, tab: &Tab) -> Option<Rename> {
        let open = self.open.as_ref()?;
        Some(Rename {
            index: tab.index_of(&open.at)?,
            document: open.document.clone(),
            session: open.session,
        })
    }

    pub(crate) fn follow(&mut self, rows: &[Row]) {
        if self
            .open
            .as_ref()
            .is_some_and(|open| !rows.iter().any(|row| row.at == open.at))
        {
            self.open = None;
        }
    }

    pub(crate) fn begin(&mut self, at: Location, name: &str, folder: bool) {
        let selected = if folder {
            name.len()
        } else {
            rikka_format::name_stem(name).len()
        };
        let document = TextDocument::new(name);
        document.select(0, selected);
        self.sessions = self.sessions.wrapping_add(1);
        self.open = Some(Open {
            at,
            name: name.to_owned(),
            document,
            session: self.sessions,
        });
    }
}

#[derive(Debug)]
struct Open {
    at: Location,
    name: String,
    document: TextDocument,
    session: u64,
}

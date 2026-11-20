use files_core::Entry;
use rikka_mime::Database;
use rikka_mime::MimeType;
use rikka_xdg::{Associations, DesktopEntry, MimeApps};

#[derive(Debug)]
pub struct Applications {
    associations: Option<Associations>,
}

impl Applications {
    #[must_use]
    pub fn load() -> Self {
        Self {
            associations: MimeApps::session().map(|apps| apps.read()),
        }
    }

    #[must_use]
    pub fn get(&self, id: &str) -> Option<&DesktopEntry> {
        self.associations.as_ref()?.application(id)
    }

    #[must_use]
    pub fn for_type<'a>(&'a self, mime: &MimeType, database: &Database) -> Handlers<'a> {
        let Some(associations) = &self.associations else {
            return Handlers::default();
        };
        let ancestry = database.ancestry(mime);
        let default = associations.default_for_any(&ancestry);
        let mut all = associations.handlers_for_any(&ancestry);
        if let Some(default) = default {
            all.retain(|entry| entry.id != default.id);
            all.insert(0, default);
        }
        Handlers { default, all }
    }

    pub fn open(&self, entry: &Entry, database: &Database) -> rikka_open::Result<()> {
        let targets = std::slice::from_ref(&entry.location);
        match self.for_type(&entry.mime, database).default {
            Some(application) => crate::launch::launch(application, targets),
            None => crate::launch::open_with_desktop(targets),
        }
    }
}

#[derive(Debug, Default)]
pub struct Handlers<'a> {
    pub default: Option<&'a DesktopEntry>,
    pub all: Vec<&'a DesktopEntry>,
}

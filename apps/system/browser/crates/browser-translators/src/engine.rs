use std::fmt;

use browser_translate::Adapter;
use rikka_languages::Language;

use crate::adapters;

#[derive(Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Engine(usize);

impl Engine {
    pub fn all() -> impl ExactSizeIterator<Item = Engine> + Clone {
        (0..adapters::ALL.len()).map(Engine)
    }

    #[must_use]
    pub fn from_id(id: &str) -> Option<Engine> {
        Engine::all().find(|engine| engine.id() == id)
    }

    #[must_use]
    pub fn id(self) -> &'static str {
        self.adapter().service().id
    }

    #[must_use]
    pub fn label(self) -> &'static str {
        self.adapter().service().label
    }

    #[must_use]
    pub fn serves(self, language: &Language) -> bool {
        self.adapter().service().serves(language)
    }

    pub(crate) fn adapter(self) -> &'static dyn Adapter {
        adapters::ALL[self.0]
    }
}

impl fmt::Debug for Engine {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.id())
    }
}

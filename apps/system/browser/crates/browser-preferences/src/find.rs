use rikka_settings::{Key, Schema};

use crate::Preferences;

const MATCH_CASE: &str = "find-match-case";
const WHOLE_WORD: &str = "find-whole-word";
const MATCH_DIACRITICS: &str = "find-match-diacritics";
const LINKS_ONLY: &str = "find-links-only";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FindOption {
    MatchCase,
    WholeWord,
    MatchDiacritics,
    LinksOnly,
}

impl FindOption {
    pub const ALL: [FindOption; 4] = [
        FindOption::MatchCase,
        FindOption::WholeWord,
        FindOption::MatchDiacritics,
        FindOption::LinksOnly,
    ];

    fn key(self) -> &'static str {
        match self {
            FindOption::MatchCase => MATCH_CASE,
            FindOption::WholeWord => WHOLE_WORD,
            FindOption::MatchDiacritics => MATCH_DIACRITICS,
            FindOption::LinksOnly => LINKS_ONLY,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct FindOptions {
    pub match_case: bool,
    pub whole_word: bool,
    pub match_diacritics: bool,
    pub links_only: bool,
}

impl FindOptions {
    pub fn get(self, option: FindOption) -> bool {
        match option {
            FindOption::MatchCase => self.match_case,
            FindOption::WholeWord => self.whole_word,
            FindOption::MatchDiacritics => self.match_diacritics,
            FindOption::LinksOnly => self.links_only,
        }
    }

    pub fn apply(self, mut find: rikka_web::Find) -> rikka_web::Find {
        for option in FindOption::ALL
            .into_iter()
            .filter(|option| self.get(*option))
        {
            find = match option {
                FindOption::MatchCase => find.match_case(),
                FindOption::WholeWord => find.whole_word(),
                FindOption::MatchDiacritics => find.match_diacritics(),
                FindOption::LinksOnly => find.links_only(),
            };
        }
        find
    }
}

impl Preferences {
    pub fn find_options(&self) -> FindOptions {
        self.observers.read();
        let settings = self.settings.borrow();
        FindOptions {
            match_case: settings.get(MATCH_CASE),
            whole_word: settings.get(WHOLE_WORD),
            match_diacritics: settings.get(MATCH_DIACRITICS),
            links_only: settings.get(LINKS_ONLY),
        }
    }

    pub fn set_find_option(&self, option: FindOption, on: bool) {
        self.put::<bool>(option.key(), on);
    }
}

pub(super) fn keys(schema: Schema) -> Schema {
    schema
        .key(Key::bool(MATCH_CASE, false).summary("Find in page matches letter case"))
        .key(Key::bool(WHOLE_WORD, false).summary("Find in page matches whole words only"))
        .key(
            Key::bool(MATCH_DIACRITICS, false).summary("Find in page tells accented letters apart"),
        )
        .key(Key::bool(LINKS_ONLY, false).summary("Find in page searches link text only"))
}

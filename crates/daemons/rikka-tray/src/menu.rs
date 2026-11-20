use crate::icon::Icon;
use crate::id::ItemId;

#[derive(Clone, Debug, PartialEq)]
pub struct Menu {
    pub item: ItemId,
    pub entries: Vec<Entry>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Entry {
    Separator,
    Row(Row),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub id: i32,
    pub label: String,
    pub enabled: bool,
    pub icon: Option<Icon>,
    pub toggle: Option<Toggle>,
    pub entries: Vec<Entry>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Toggle {
    Check(bool),
    Radio(bool),
}

impl Menu {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Row {
    #[must_use]
    pub fn opens_submenu(&self) -> bool {
        !self.entries.is_empty()
    }
}

impl Toggle {
    #[must_use]
    pub fn is_on(self) -> bool {
        match self {
            Toggle::Check(on) | Toggle::Radio(on) => on,
        }
    }
}

#[must_use]
pub(crate) fn plain(label: &str) -> String {
    let mut plain = String::with_capacity(label.len());
    let mut rest = label.chars();
    while let Some(character) = rest.next() {
        match character {
            '_' => {
                if rest.as_str().starts_with('_') {
                    rest.next();
                    plain.push('_');
                }
            }
            character => plain.push(character),
        }
    }
    plain
}

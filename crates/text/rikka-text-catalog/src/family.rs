use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Family {
    Named(Box<str>),
    SansSerif,
    Serif,
    Monospace,
    Cursive,
    Fantasy,
    Emoji,
    Math,
    SystemUi,
}

const GENERICS: [(&str, Family); 12] = [
    ("sans-serif", Family::SansSerif),
    ("sansserif", Family::SansSerif),
    ("sans", Family::SansSerif),
    ("serif", Family::Serif),
    ("monospace", Family::Monospace),
    ("mono", Family::Monospace),
    ("cursive", Family::Cursive),
    ("fantasy", Family::Fantasy),
    ("emoji", Family::Emoji),
    ("math", Family::Math),
    ("system-ui", Family::SystemUi),
    ("systemui", Family::SystemUi),
];

impl Family {
    pub fn parse(name: &str) -> Self {
        let folded = fold(name);
        GENERICS
            .iter()
            .find(|(keyword, _)| *keyword == folded)
            .map_or_else(
                || Self::Named(name.trim().into()),
                |(_, family)| family.clone(),
            )
    }

    pub fn name(&self) -> &str {
        match self {
            Self::Named(name) => name,
            Self::SansSerif => "sans-serif",
            Self::Serif => "serif",
            Self::Monospace => "monospace",
            Self::Cursive => "cursive",
            Self::Fantasy => "fantasy",
            Self::Emoji => "emoji",
            Self::Math => "math",
            Self::SystemUi => "system-ui",
        }
    }
}

impl From<&str> for Family {
    fn from(name: &str) -> Self {
        Self::parse(name)
    }
}

impl From<String> for Family {
    fn from(name: String) -> Self {
        Self::parse(&name)
    }
}

impl fmt::Display for Family {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

pub(crate) fn fold(name: &str) -> String {
    folded(name).collect()
}

pub(crate) fn folds_to(name: &str, folded_name: &str) -> bool {
    folded(name).eq(folded_name.chars())
}

fn folded(name: &str) -> impl Iterator<Item = char> {
    name.chars()
        .filter(|c| !c.is_whitespace())
        .flat_map(char::to_lowercase)
}

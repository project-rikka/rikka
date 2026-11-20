use rikka_ui::{Label, MaterialSymbol};

use crate::chat::Labels;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Builtin {
    Compact,
}

impl Builtin {
    const ALL: [Builtin; 1] = [Builtin::Compact];

    pub fn named(name: &str) -> Option<Builtin> {
        Builtin::ALL
            .into_iter()
            .find(|builtin| builtin.name().eq_ignore_ascii_case(name))
    }

    pub fn matching(typed: &str) -> impl Iterator<Item = Builtin> {
        let typed = typed.to_lowercase();
        Builtin::ALL
            .into_iter()
            .filter(move |builtin| builtin.name().starts_with(&typed))
    }

    pub fn name(self) -> &'static str {
        match self {
            Builtin::Compact => "compact",
        }
    }

    pub fn symbol(self) -> MaterialSymbol {
        match self {
            Builtin::Compact => MaterialSymbol::COMPRESS,
        }
    }

    pub fn hint<C: Labels>(self) -> Label {
        match self {
            Builtin::Compact => C::compact_hint(),
        }
    }

    pub fn description<C: Labels>(self) -> Label {
        match self {
            Builtin::Compact => C::compact_about(),
        }
    }
}

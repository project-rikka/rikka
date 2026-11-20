#[derive(Clone, Debug)]
pub(crate) enum Pattern {
    Symbols(u64),
    Sequence(Vec<Pattern>),
    Choice(Vec<Pattern>),
    Repeat(Box<Pattern>),
    Optional(Box<Pattern>),
}

pub(crate) fn one(category: u8) -> Pattern {
    Pattern::Symbols(1 << category)
}

pub(crate) fn any_of(categories: &[u8]) -> Pattern {
    Pattern::Symbols(
        categories
            .iter()
            .fold(0, |set, &category| set | 1 << category),
    )
}

pub(crate) fn any() -> Pattern {
    Pattern::Symbols(u64::MAX)
}

pub(crate) fn seq(parts: impl IntoIterator<Item = Pattern>) -> Pattern {
    Pattern::Sequence(parts.into_iter().collect())
}

pub(crate) fn either(parts: impl IntoIterator<Item = Pattern>) -> Pattern {
    Pattern::Choice(parts.into_iter().collect())
}

pub(crate) fn star(part: Pattern) -> Pattern {
    Pattern::Repeat(Box::new(part))
}

pub(crate) fn plus(part: Pattern) -> Pattern {
    seq([part.clone(), star(part)])
}

pub(crate) fn opt(part: Pattern) -> Pattern {
    Pattern::Optional(Box::new(part))
}

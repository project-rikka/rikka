use rikka_ui::{Label, tr};

use crate::language::Language;
use crate::side::Side;

const OTHER: &str = "shell.translate.other";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Choice {
    Speaking(Language),
    Other,
}

impl Choice {
    #[must_use]
    fn label(&self) -> Label {
        match self {
            Choice::Speaking(language) => language.label(),
            Choice::Other => tr!(OTHER),
        }
    }
}

#[must_use]
pub(crate) fn offered(side: Side, current: &Language) -> Vec<Choice> {
    let mut offered: Vec<Choice> = match side {
        Side::Source => vec![Choice::Speaking(Language::Auto)],
        Side::Target => Vec::new(),
    };
    let listed: Vec<Language> = Language::offered().collect();
    if !current.is_auto() && !listed.contains(current) {
        offered.push(Choice::Speaking(current.clone()));
    }
    offered.extend(listed.into_iter().map(Choice::Speaking));
    offered.push(Choice::Other);
    offered
}

#[must_use]
pub(crate) fn labels(offered: &[Choice]) -> Vec<Label> {
    offered.iter().map(Choice::label).collect()
}

#[must_use]
pub(crate) fn at(offered: &[Choice], current: &Language) -> Option<usize> {
    offered
        .iter()
        .position(|shown| *shown == Choice::Speaking(current.clone()))
}

#[must_use]
pub(super) fn swapped(
    source: &Language,
    target: &Language,
    detected: Option<&Language>,
) -> Option<(Language, Language)> {
    let unfolded = match source {
        Language::Auto => detected.filter(|language| !language.is_auto())?.clone(),
        chosen => chosen.clone(),
    };
    Some((target.clone(), unfolded))
}

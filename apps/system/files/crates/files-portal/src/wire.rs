use files_dialog::{Choice, ChoiceOption, Filter, Rule};
use serde::{Deserialize, Serialize};
use zbus::zvariant::Type;

const GLOB: u32 = 0;
const MIME: u32 = 1;

#[derive(Clone, Debug, Deserialize, Serialize, Type)]
pub(crate) struct WireFilter(pub(crate) String, pub(crate) Vec<WireRule>);

#[derive(Clone, Debug, Deserialize, Serialize, Type)]
pub(crate) struct WireRule(pub(crate) u32, pub(crate) String);

#[derive(Clone, Debug, Deserialize, Serialize, Type)]
pub(crate) struct WireChoice(
    pub(crate) String,
    pub(crate) String,
    pub(crate) Vec<WireOption>,
    pub(crate) String,
);

#[derive(Clone, Debug, Deserialize, Serialize, Type)]
pub(crate) struct WireOption(pub(crate) String, pub(crate) String);

impl From<&WireFilter> for Filter {
    fn from(wire: &WireFilter) -> Self {
        Self {
            label: wire.0.clone(),
            rules: wire
                .1
                .iter()
                .filter_map(|rule| match rule.0 {
                    GLOB => Some(Rule::Glob(rule.1.clone())),
                    MIME => Some(Rule::Mime(rule.1.clone())),
                    _ => None,
                })
                .collect(),
        }
    }
}

impl From<&Filter> for WireFilter {
    fn from(filter: &Filter) -> Self {
        Self(
            filter.label.clone(),
            filter
                .rules
                .iter()
                .map(|rule| match rule {
                    Rule::Glob(pattern) => WireRule(GLOB, pattern.clone()),
                    Rule::Mime(pattern) => WireRule(MIME, pattern.clone()),
                })
                .collect(),
        )
    }
}

impl From<&WireChoice> for Choice {
    fn from(wire: &WireChoice) -> Self {
        Self {
            id: wire.0.clone(),
            label: wire.1.clone(),
            options: wire
                .2
                .iter()
                .map(|option| ChoiceOption {
                    id: option.0.clone(),
                    label: option.1.clone(),
                })
                .collect(),
            initial: wire.3.clone(),
        }
    }
}

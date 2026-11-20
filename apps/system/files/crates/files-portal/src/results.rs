use files_core::Location;
use files_dialog::{Answer, Ask, Kind};
use zbus::zvariant::{SerializeDict, Type};

use crate::wire::WireFilter;

#[derive(Debug, Default, SerializeDict, Type)]
#[zvariant(signature = "a{sv}")]
pub(crate) struct Results {
    uris: Vec<String>,
    choices: Option<Vec<(String, String)>>,
    current_filter: Option<WireFilter>,
    writable: Option<bool>,
}

impl Results {
    pub(crate) fn new(ask: &Ask, answer: &Answer) -> Self {
        Self {
            uris: answer
                .chosen
                .iter()
                .map(|path| Location::from_path(path).as_str().to_owned())
                .collect(),
            choices: (!answer.choices.is_empty()).then(|| answer.choices.clone()),
            current_filter: answer
                .filter
                .and_then(|index| ask.filters.get(index))
                .map(WireFilter::from),
            writable: (ask.kind == Kind::Open).then_some(answer.writable),
        }
    }
}

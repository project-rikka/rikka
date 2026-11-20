mod answer;
mod request;
mod source;

pub(crate) use answer::Answer;
pub(crate) use request::{Persist, PickerRequest};
pub(crate) use source::{CastSource, SourceKind};

pub(crate) type Outcome = rikka_portal::Outcome<Answer>;

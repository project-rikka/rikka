use serde::{Deserialize, Serialize};

use crate::protocol::source::CastSource;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct Answer {
    pub(crate) sources: Vec<CastSource>,
    #[serde(default)]
    pub(crate) remember: bool,
}

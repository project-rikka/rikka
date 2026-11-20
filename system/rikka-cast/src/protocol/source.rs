use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum SourceKind {
    Monitor,
    Window,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct CastSource {
    pub(crate) kind: SourceKind,
    pub(crate) id: String,
}

impl CastSource {
    pub(crate) fn monitor(id: impl Into<String>) -> CastSource {
        CastSource {
            kind: SourceKind::Monitor,
            id: id.into(),
        }
    }

    pub(crate) fn window(id: impl Into<String>) -> CastSource {
        CastSource {
            kind: SourceKind::Window,
            id: id.into(),
        }
    }
}

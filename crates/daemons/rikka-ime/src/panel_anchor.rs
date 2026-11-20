use serde::{Deserialize, Serialize};

use crate::Caret;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "at", rename_all = "kebab-case")]
pub enum PanelAnchor {
    Caret { output: String, caret: Caret },
    Pointer { output: String, x: i32, y: i32 },
}

impl PanelAnchor {
    #[must_use]
    pub fn output(&self) -> &str {
        match self {
            Self::Caret { output, .. } | Self::Pointer { output, .. } => output,
        }
    }
}

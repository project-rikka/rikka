use serde::{Deserialize, Serialize};
use zeroize::Zeroize;
use zvariant::{OwnedObjectPath, Type};

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct WireSecret {
    pub session: OwnedObjectPath,
    pub parameters: Vec<u8>,
    pub value: Vec<u8>,
    pub content_type: String,
}

impl Drop for WireSecret {
    fn drop(&mut self) {
        self.value.zeroize();
    }
}

impl std::fmt::Debug for WireSecret {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("WireSecret")
            .field("session", &self.session.as_str())
            .field("content_type", &self.content_type)
            .finish_non_exhaustive()
    }
}

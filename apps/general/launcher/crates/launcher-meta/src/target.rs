use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GameTarget {
    pub minecraft: String,
    pub fabric_loader: String,
}

impl GameTarget {
    #[must_use]
    pub fn loader_profile(&self) -> String {
        format!("fabric-loader-{}-{}", self.fabric_loader, self.minecraft)
    }
}

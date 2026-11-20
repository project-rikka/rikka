use rikka_proton_pgp::Modulus;
use serde::{Deserialize, Serialize};

use crate::error::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedModulus {
    #[serde(rename = "ModulusID")]
    id: String,
    #[serde(rename = "Modulus")]
    value: String,
}

impl SignedModulus {
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub fn value(&self) -> &str {
        &self.value
    }

    pub fn verify(&self) -> Result<Modulus> {
        Ok(Modulus::from_signed(&self.value)?.with_id(&self.id))
    }
}

use sha_crypt::{Algorithm, Params, PasswordHasher, ShaCrypt};

use crate::{
    error::{Error, Result},
    secret::Secret,
};

const SALT_BYTES: usize = 12;

const ROUNDS: u32 = 200_000;

pub fn hash_password(password: &Secret) -> Result<String> {
    let mut salt = [0u8; SALT_BYTES];
    getrandom::fill(&mut salt).map_err(|error| Error::Hash(error.to_string()))?;

    let params = Params::new(ROUNDS).map_err(|error| Error::Hash(error.to_string()))?;
    let hash = ShaCrypt::new(Algorithm::Sha512Crypt, params)
        .hash_password_with_salt(password.as_str().as_bytes(), &salt)
        .map_err(|error| Error::Hash(error.to_string()))?;

    Ok(AsRef::<str>::as_ref(&hash).to_owned())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Strength {
    Weak,
    Fair,
    Good,
    Strong,
}

impl Strength {
    #[must_use]
    pub fn measure(password: &str) -> Self {
        let length = password.chars().count();
        let classes = [
            password.chars().any(char::is_lowercase),
            password.chars().any(char::is_uppercase),
            password.chars().any(|ch| ch.is_ascii_digit()),
            password.chars().any(|ch| !ch.is_alphanumeric()),
        ]
        .into_iter()
        .filter(|present| *present)
        .count();

        match (length, classes) {
            (20.., _) | (16.., 3..) => Self::Strong,
            (16.., _) | (12.., 3..) => Self::Good,
            (12.., _) | (8.., 3..) => Self::Fair,
            _ => Self::Weak,
        }
    }

    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Weak => "weak",
            Self::Fair => "fair",
            Self::Good => "good",
            Self::Strong => "strong",
        }
    }
}

use serde_json::Value;

use crate::backend::Backend;

const KEY: &str = "key";

#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "StoredCredential", into = "StoredCredential")]
pub struct Credential(Stored);

#[derive(Clone)]
enum Stored {
    Key(String),
    Tokens { backend: Backend, tokens: Value },
}

impl Credential {
    #[must_use]
    pub fn keyed(key: impl Into<String>) -> Credential {
        Credential(Stored::Key(key.into()))
    }

    pub(crate) fn signed(backend: Backend, tokens: Value) -> Credential {
        Credential(Stored::Tokens { backend, tokens })
    }

    #[must_use]
    pub fn key(&self) -> &str {
        match &self.0 {
            Stored::Key(key) => key,
            Stored::Tokens { .. } => "",
        }
    }

    #[must_use]
    pub fn who(&self) -> String {
        match &self.0 {
            Stored::Key(_) => String::new(),
            Stored::Tokens { backend, tokens } => backend.provider().who(tokens),
        }
    }

    pub(crate) fn backend(&self) -> Option<Backend> {
        match &self.0 {
            Stored::Key(_) => None,
            Stored::Tokens { backend, .. } => Some(*backend),
        }
    }

    pub(crate) fn tokens(&self) -> Option<&Value> {
        match &self.0 {
            Stored::Key(_) => None,
            Stored::Tokens { tokens, .. } => Some(tokens),
        }
    }
}

impl std::fmt::Debug for Credential {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let kind = self.backend().map_or(KEY, Backend::id);
        formatter
            .debug_struct("Credential")
            .field("kind", &kind)
            .finish_non_exhaustive()
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
struct StoredCredential {
    kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    tokens: Option<Value>,
}

impl TryFrom<StoredCredential> for Credential {
    type Error = String;

    fn try_from(stored: StoredCredential) -> Result<Credential, String> {
        let StoredCredential { kind, key, tokens } = stored;
        if kind == KEY {
            return key
                .map(Credential::keyed)
                .ok_or_else(|| "a key credential carries no key".to_owned());
        }

        let backend =
            Backend::from_id(&kind).ok_or_else(|| format!("no backend is called \"{kind}\""))?;
        let tokens = tokens.ok_or_else(|| format!("a {kind} credential carries no tokens"))?;
        if !backend.provider().accepts(&tokens) {
            return Err(format!(
                "a {kind} credential carries tokens {kind} cannot read"
            ));
        }
        Ok(Credential(Stored::Tokens { backend, tokens }))
    }
}

impl From<Credential> for StoredCredential {
    fn from(credential: Credential) -> StoredCredential {
        match credential.0 {
            Stored::Key(key) => StoredCredential {
                kind: KEY.to_owned(),
                key: Some(key),
                tokens: None,
            },
            Stored::Tokens { backend, tokens } => StoredCredential {
                kind: backend.id().to_owned(),
                key: None,
                tokens: Some(tokens),
            },
        }
    }
}

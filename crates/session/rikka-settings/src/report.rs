use std::path::PathBuf;

#[derive(Debug, Default)]
pub struct LoadReport {
    pub rejected: Vec<Rejection>,
}

impl LoadReport {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rejected.is_empty()
    }

    pub fn log_warnings(&self, domain: &crate::domain::Domain) {
        for rejection in &self.rejected {
            tracing::warn!(domain = %domain, "{rejection}");
        }
    }

    pub(crate) fn reject(&mut self, path: &std::path::Path, key: Option<&str>, reason: String) {
        self.rejected.push(Rejection {
            path: path.to_path_buf(),
            key: key.map(ToOwned::to_owned),
            reason,
        });
    }
}

#[derive(Debug)]
pub struct Rejection {
    pub path: PathBuf,
    pub key: Option<String>,
    pub reason: String,
}

impl std::fmt::Display for Rejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.key {
            Some(key) => write!(f, "{}: `{key}` {}", self.path.display(), self.reason),
            None => write!(f, "{}: {}", self.path.display(), self.reason),
        }
    }
}

use std::io;
use std::path::{Path, PathBuf};

const MAX_RECORDED: usize = 32;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MetadataReport {
    pub failed: usize,
    pub rejected: Vec<MetadataRejection>,
}

impl MetadataReport {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.failed == 0
    }

    pub fn warn(&self) {
        for rejection in &self.rejected {
            tracing::warn!("{rejection}");
        }
        let elided = self.failed - self.rejected.len();
        if elided > 0 {
            tracing::warn!("failed to restore metadata of {elided} more entries");
        }
    }

    pub(crate) fn record(&mut self, path: &Path, kind: MetadataKind, applied: io::Result<()>) {
        let Err(error) = applied else {
            return;
        };
        self.failed += 1;
        if self.rejected.len() < MAX_RECORDED {
            self.rejected.push(MetadataRejection {
                path: path.to_path_buf(),
                kind,
                reason: error.to_string(),
            });
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetadataRejection {
    pub path: PathBuf,
    pub kind: MetadataKind,
    pub reason: String,
}

impl std::fmt::Display for MetadataRejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "failed to restore {} of {}: {}",
            self.kind,
            self.path.display(),
            self.reason
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetadataKind {
    Permissions,
    Timestamp,
}

impl std::fmt::Display for MetadataKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            MetadataKind::Permissions => "permissions",
            MetadataKind::Timestamp => "timestamp",
        })
    }
}

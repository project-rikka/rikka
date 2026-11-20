use std::fmt::Debug;
use std::path::{Path, PathBuf};

pub trait Address: Clone + Debug + PartialEq + Send + Sync + 'static {
    #[must_use]
    fn child(&self, name: &str) -> Self;

    fn parent(&self) -> Option<Self>;

    fn contains(&self, other: &Self) -> bool;

    fn name(&self) -> String;
}

impl Address for PathBuf {
    fn child(&self, name: &str) -> Self {
        self.join(name)
    }

    fn parent(&self) -> Option<Self> {
        Path::parent(self).map(Path::to_path_buf)
    }

    fn contains(&self, other: &Self) -> bool {
        other.starts_with(self)
    }

    fn name(&self) -> String {
        self.file_name().map_or_else(
            || self.display().to_string(),
            |name| name.to_string_lossy().into_owned(),
        )
    }
}

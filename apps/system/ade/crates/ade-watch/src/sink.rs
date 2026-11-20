use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;

pub type Sink = Arc<dyn Fn(BTreeSet<PathBuf>) + Send + Sync>;

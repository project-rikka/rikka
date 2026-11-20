mod backend;
mod error;
mod local;
mod operation;
mod stat;
mod trash;
mod vfs;
mod watch;

pub use backend::{Backend, Capabilities, Detail, ListOptions};
pub use error::{Error, Result};
pub use local::LocalBackend;
pub use operation::{
    Conflict, Failure, Operation, Outcome, Placement, ReadSink, Reporter, Snapshot, Task,
};
pub use trash::TrashBackend;
pub use vfs::Vfs;
pub use watch::{Change, Watch, WatchSink};

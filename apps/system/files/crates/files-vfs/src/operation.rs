use files_core::{Entry, Location};

pub type Task = rikka_file_operations::Task<Location>;

pub type Placement = rikka_file_operations::Placement<Location>;

pub type Outcome = rikka_file_operations::Outcome<Location>;

pub type Failure = rikka_file_operations::Failure<Location>;

pub type Conflict = rikka_file_operations::Conflict<Entry>;

pub type Snapshot = rikka_file_operations::Snapshot<Entry>;

pub type Operation = rikka_file_operations::Operation<Entry>;

pub type Reporter = rikka_file_operations::Reporter<Entry>;

pub type ReadSink<'a> = rikka_file_operations::ReadSink<'a, Location>;

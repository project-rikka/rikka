use files_core::Location;

pub type Error = rikka_file_operations::Error<Location>;

pub type Result<T> = rikka_file_operations::Result<T, Location>;

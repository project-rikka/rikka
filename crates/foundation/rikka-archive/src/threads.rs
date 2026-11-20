pub(crate) fn available() -> u32 {
    std::thread::available_parallelism()
        .ok()
        .and_then(|count| u32::try_from(count.get()).ok())
        .unwrap_or(1)
        .max(1)
}

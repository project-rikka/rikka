use discord_core::MessageId;

#[must_use]
pub fn value(id: &str) -> u64 {
    id.parse().unwrap_or(0)
}

#[must_use]
pub fn id<T: From<u64>>(text: &str) -> T {
    T::from(value(text))
}

#[must_use]
pub fn millis(id: &str) -> Option<i64> {
    id.parse().ok().map(MessageId::created_at)
}

#[must_use]
pub fn newer(left: &str, right: &str) -> bool {
    value(left) > value(right)
}

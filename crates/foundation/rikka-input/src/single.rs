pub(crate) fn exactly_one<T>(iter: impl IntoIterator<Item = T>) -> Option<T> {
    let mut items = iter.into_iter();
    match (items.next(), items.next()) {
        (Some(item), None) => Some(item),
        _ => None,
    }
}

const LABEL_LIMIT: usize = 63;
const NAME_LIMIT: usize = 253;

pub(crate) fn is_label(label: &str) -> bool {
    !label.is_empty()
        && label.len() <= LABEL_LIMIT
        && !label.starts_with('-')
        && !label.ends_with('-')
        && label
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
}

pub(crate) fn is_name(name: &str) -> bool {
    name.len() <= NAME_LIMIT && name.split('.').all(is_label)
}

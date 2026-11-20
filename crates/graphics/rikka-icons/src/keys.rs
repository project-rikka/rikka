use rikka_xdg::Group;

pub(crate) fn number(group: &Group, key: &str) -> Option<u32> {
    group.raw(key)?.trim().parse().ok()
}

pub(crate) fn list<'a>(group: &'a Group, key: &str) -> Vec<&'a str> {
    group
        .raw(key)
        .into_iter()
        .flat_map(|value| value.split(','))
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect()
}

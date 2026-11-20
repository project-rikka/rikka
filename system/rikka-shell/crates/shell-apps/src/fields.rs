use rikka_xdg::DesktopEntry;

pub fn searchable(entry: &DesktopEntry) -> impl Iterator<Item = (&str, f32)> {
    std::iter::once((entry.name.as_str(), 1.0))
        .chain(entry.generic_name.as_deref().map(|generic| (generic, 0.8)))
        .chain(entry.keywords.iter().map(|word| (word.as_str(), 0.7)))
        .chain(entry.comment.as_deref().map(|comment| (comment, 0.45)))
        .chain(std::iter::once((entry.exec.as_str(), 0.4)))
}

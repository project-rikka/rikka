use crate::blend::BlendId;

const PREFIX: &str = "rikka: ";
const MARK: &str = "rikka:";

#[must_use]
pub(crate) fn mirror_name(name: &str) -> String {
    format!("{PREFIX}{name}")
}

#[must_use]
pub(crate) fn blend_name(mirrored: &str) -> &str {
    mirrored.strip_prefix(PREFIX).unwrap_or(mirrored).trim()
}

#[must_use]
pub(crate) fn looks_mirrored(name: &str) -> bool {
    name.starts_with(PREFIX)
}

#[must_use]
pub(crate) fn description(id: &BlendId) -> String {
    format!("{MARK}{id}")
}

#[must_use]
pub(crate) fn marked(description: &str) -> Option<BlendId> {
    description
        .match_indices(MARK)
        .find_map(|(at, _)| marked_at(&description[at + MARK.len()..]))
}

fn marked_at(rest: &str) -> Option<BlendId> {
    let end = rest
        .find(|letter: char| !(letter.is_ascii_hexdigit() || letter == '-'))
        .unwrap_or(rest.len());
    BlendId::parse(&rest[..end])
}

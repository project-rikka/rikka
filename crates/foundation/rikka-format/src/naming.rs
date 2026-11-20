#[must_use]
pub fn numbered_name(name: &str, index: usize) -> String {
    with_stem(name, |stem| format!("{stem} ({index})"))
}

#[must_use]
pub fn name_stem(name: &str) -> &str {
    split(name).0
}

#[must_use]
pub fn unique_name(name: &str, taken: impl Fn(&str) -> bool) -> String {
    free(name, &taken)
}

fn free(name: &str, taken: &impl Fn(&str) -> bool) -> String {
    if !taken(name) {
        return name.to_owned();
    }
    (2..)
        .map(|index| numbered_name(name, index))
        .find(|candidate| !taken(candidate))
        .unwrap_or_else(|| name.to_owned())
}

#[must_use]
pub fn safe_file_name(suggested: &str) -> Option<String> {
    let last = suggested.rsplit(['/', '\\']).next().unwrap_or_default();
    let name: String = last
        .chars()
        .filter(|character| !character.is_control())
        .collect();
    let name = name.trim();
    (!name.trim_start_matches('.').is_empty()).then(|| name.to_owned())
}

#[must_use]
pub fn with_stem(name: &str, make: impl Fn(&str) -> String) -> String {
    let (stem, extension) = split(name);
    match extension {
        Some(extension) => format!("{}.{extension}", make(stem)),
        None => make(stem),
    }
}

fn split(name: &str) -> (&str, Option<&str>) {
    const COMPOUND: &[&str] = &["gz", "bz2", "xz", "zst", "lz", "lzma", "z"];

    let Some(dot) = name.rfind('.').filter(|&dot| dot > 0) else {
        return (name, None);
    };
    let (stem, extension) = (&name[..dot], &name[dot + 1..]);
    if COMPOUND.contains(&extension.to_ascii_lowercase().as_str())
        && let Some(inner) = stem.rfind('.').filter(|&inner| inner > 0)
    {
        return (&name[..inner], Some(&name[inner + 1..]));
    }
    (stem, Some(extension))
}

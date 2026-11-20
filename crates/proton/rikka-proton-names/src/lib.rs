pub const NAME_LIMIT: usize = 255;

const MAX_KEPT_EXTENSION_LEN: usize = 32;

#[must_use]
pub fn split_extension(name: &str) -> (&str, &str) {
    match name.rfind('.') {
        Some(dot) if dot > 0 && dot < name.len() - 1 => (&name[..dot], &name[dot + 1..]),
        _ => (name, ""),
    }
}

#[must_use]
pub fn numbered(stem: &str, number: u32, extension: &str) -> String {
    match (stem.is_empty(), extension.is_empty()) {
        (true, true) => format!("({number})"),
        (true, false) => format!("({number}).{extension}"),
        (false, true) => format!("{stem} ({number})"),
        (false, false) => format!("{stem} ({number}).{extension}"),
    }
}

#[must_use]
pub fn numbered_name(name: &str, number: u32) -> String {
    let (stem, extension) = split_extension(name);
    numbered(stem, number, extension)
}

#[must_use]
pub fn numbered_local_name(name: &str, number: u32) -> String {
    let (stem, extension) = kept_extension(name);
    let suffix = format!(" ({number})");
    let stem = cut(stem, stem_room(extension).saturating_sub(suffix.len()));
    numbered(stem, number, extension)
}

#[must_use]
pub fn fitted_name(name: &str) -> String {
    if name.len() <= NAME_LIMIT {
        return name.to_owned();
    }
    let (stem, extension) = kept_extension(name);
    let stem = cut(stem, stem_room(extension));
    match extension {
        "" => stem.to_owned(),
        extension => format!("{stem}.{extension}"),
    }
}

fn stem_room(extension: &str) -> usize {
    match extension {
        "" => NAME_LIMIT,
        extension => NAME_LIMIT - extension.len() - 1,
    }
}

fn kept_extension(name: &str) -> (&str, &str) {
    match split_extension(name) {
        (stem, extension) if !extension.is_empty() && extension.len() <= MAX_KEPT_EXTENSION_LEN => {
            (stem, extension)
        }
        _ => (name, ""),
    }
}

fn cut(text: &str, limit: usize) -> &str {
    let mut end = limit.min(text.len());
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

const MAX_URL: usize = 4096;
const MAX_DATA_URL: usize = 512 * 1024;
const LINK_SCHEMES: [&str; 3] = ["http://", "https://", "mailto:"];
const IMAGE_SCHEMES: [&str; 2] = ["http://", "https://"];
const DATA_IMAGE_TYPES: [&str; 8] = [
    "png", "jpeg", "jpg", "gif", "webp", "avif", "bmp", "svg+xml",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Link(String);

impl Link {
    pub(crate) fn parse(text: &str) -> Option<Link> {
        let text = text.trim();
        (text.len() <= MAX_URL && plain(text) && has_scheme(text, &LINK_SCHEMES))
            .then(|| Link(text.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageSource(String);

impl ImageSource {
    pub(crate) fn parse(text: &str) -> Option<ImageSource> {
        let text = text.trim();
        let accepted = if has_scheme(text, &["data:"]) {
            text.len() <= MAX_DATA_URL && data_image(text)
        } else {
            text.len() <= MAX_URL && plain(text) && has_scheme(text, &IMAGE_SCHEMES)
        };
        accepted.then(|| ImageSource(text.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn is_data(&self) -> bool {
        has_scheme(&self.0, &["data:"])
    }
}

fn has_scheme(text: &str, schemes: &[&str]) -> bool {
    schemes.iter().any(|scheme| {
        text.get(..scheme.len())
            .is_some_and(|head| head.eq_ignore_ascii_case(scheme))
            && text.len() > scheme.len()
    })
}

fn plain(text: &str) -> bool {
    !text
        .chars()
        .any(|character| character.is_whitespace() || character.is_control())
}

fn data_image(text: &str) -> bool {
    let Some((header, _)) = text["data:".len()..].split_once(',') else {
        return false;
    };
    let mut parts = header.split(';');
    let Some(subtype) = parts.next().and_then(|media| {
        media
            .trim()
            .to_ascii_lowercase()
            .strip_prefix("image/")
            .map(str::to_owned)
    }) else {
        return false;
    };
    DATA_IMAGE_TYPES.contains(&subtype.as_str())
        && !text.chars().any(|character| character.is_control())
}

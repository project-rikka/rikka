pub use rikka_compositor_types::MimeList;

const UTF8_TEXT: &str = "text/plain;charset=utf-8";
const PLAIN_TEXT: &str = "text/plain";
const LATIN1_TEXT: &str = "text/plain;charset=iso-8859-1";

const UTF8_STRING: &str = "UTF8_STRING";
const TEXT: &str = "TEXT";
const STRING: &str = "STRING";

const RESERVED: [&str; 7] = [
    "TARGETS",
    "TIMESTAMP",
    "MULTIPLE",
    "DELETE",
    "INCR",
    "SAVE_TARGETS",
    "_WL_SELECTION",
];

pub(crate) fn offered_mime_types<T: AsRef<str>>(targets: &[T]) -> MimeList {
    let mut offered: Vec<&str> = Vec::new();
    for target in targets {
        let mime_type = match target.as_ref() {
            UTF8_STRING => UTF8_TEXT,
            TEXT => PLAIN_TEXT,
            STRING => LATIN1_TEXT,
            other if other.contains('/') => other,
            _ => continue,
        };
        push_unique(&mut offered, mime_type);
    }
    offered.into_iter().map(Box::from).collect()
}

pub(crate) fn advertised_targets<T: AsRef<str>>(mime_types: &[T]) -> Vec<&str> {
    let mut targets = Vec::new();
    for mime_type in mime_types {
        match mime_type.as_ref() {
            UTF8_TEXT => {
                push_unique(&mut targets, UTF8_STRING);
                push_unique(&mut targets, TEXT);
            }
            PLAIN_TEXT => push_unique(&mut targets, TEXT),
            LATIN1_TEXT => push_unique(&mut targets, STRING),
            reserved if RESERVED.contains(&reserved) => {}
            other => push_unique(&mut targets, other),
        }
    }
    targets
}

pub(crate) fn mime_type_for_target<T: AsRef<str>>(
    target: &str,
    mime_types: &[T],
) -> Option<String> {
    let offers = |candidate: &str| {
        mime_types
            .iter()
            .any(|offered| offered.as_ref() == candidate)
    };
    let candidates: &[&str] = match target {
        UTF8_STRING => &[UTF8_TEXT, UTF8_STRING],
        TEXT => &[UTF8_TEXT, TEXT, PLAIN_TEXT],
        STRING => &[LATIN1_TEXT, STRING],
        reserved if RESERVED.contains(&reserved) => &[],
        other => return offers(other).then(|| other.to_owned()),
    };
    candidates
        .iter()
        .find(|candidate| offers(candidate))
        .map(|candidate| (*candidate).to_owned())
}

pub(crate) fn target_for_mime_type<'a, T: AsRef<str>>(
    mime_type: &str,
    targets: &'a [T],
) -> Option<&'a T> {
    let target = match mime_type {
        UTF8_TEXT => UTF8_STRING,
        PLAIN_TEXT => TEXT,
        LATIN1_TEXT => STRING,
        other => other,
    };
    targets.iter().find(|offered| offered.as_ref() == target)
}

fn push_unique<'a>(list: &mut Vec<&'a str>, item: &'a str) {
    if !list.contains(&item) {
        list.push(item);
    }
}

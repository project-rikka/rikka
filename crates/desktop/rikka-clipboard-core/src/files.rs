use crate::offer::{Offer, URI_LIST, uri_list};

pub const COPIED_FILES: &str = "x-special/gnome-copied-files";
const KDE_CUT: &str = "application/x-kde-cutselection";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Files {
    pub uris: Vec<String>,
    pub cut: bool,
}

impl Offer {
    #[must_use]
    pub fn files<S: AsRef<str>>(uris: &[S], cut: bool) -> Self {
        let verb = match cut {
            true => "cut",
            false => "copy",
        };
        let gnome = std::iter::once(verb)
            .chain(uris.iter().map(AsRef::as_ref))
            .collect::<Vec<_>>()
            .join("\n");
        Self::uris(uris)
            .with(COPIED_FILES, gnome.into_bytes())
            .with(KDE_CUT, if cut { &b"1"[..] } else { &b"0"[..] })
    }
}

#[must_use]
pub fn offers_files(offered: &[String]) -> bool {
    offered
        .iter()
        .any(|mime| mime == COPIED_FILES || mime == URI_LIST)
}

pub fn files_in(offered: &[String], read: impl Fn(&str) -> Option<Vec<u8>>) -> Option<Files> {
    let has = |mime_type: &str| offered.iter().any(|mime| mime == mime_type);
    let text = |mime: &str| read(mime).and_then(|bytes| String::from_utf8(bytes).ok());
    if has(COPIED_FILES)
        && let Some(files) = text(COPIED_FILES).and_then(|written| copied_files(&written))
    {
        return Some(files);
    }
    if !has(URI_LIST) {
        return None;
    }
    let uris = uri_list(&text(URI_LIST)?);
    let cut = has(KDE_CUT) && text(KDE_CUT).is_some_and(|flag| flag.trim() == "1");
    (!uris.is_empty()).then_some(Files { uris, cut })
}

fn copied_files(written: &str) -> Option<Files> {
    let mut lines = written.lines();
    let cut = match lines.next()?.trim() {
        "cut" => true,
        "copy" => false,
        _ => return None,
    };
    let uris = uri_list(&lines.collect::<Vec<_>>().join("\n"));
    (!uris.is_empty()).then_some(Files { uris, cut })
}

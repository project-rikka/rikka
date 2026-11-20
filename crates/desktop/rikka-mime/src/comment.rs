use std::borrow::Cow;
use std::path::PathBuf;

use crate::mime_type::MimeType;

const MIME: &str = "mime";
const COMMENT: &str = "comment";
const XML: &str = "http://www.w3.org/XML/1998/namespace";
const LANG: &str = "lang";

pub(crate) fn read(dirs: &[PathBuf], mime: &MimeType, locales: &[String]) -> Option<String> {
    dirs.iter().find_map(|dir| {
        let path = dir
            .join(MIME)
            .join(mime.kind())
            .join(format!("{}.xml", mime.subtype()));
        let text = std::fs::read_to_string(path).ok()?;
        pick(&text, locales)
    })
}

pub(crate) fn locales_of(tag: &str) -> Vec<String> {
    let tag = tag.replace('_', "-");
    if tag.is_empty() {
        return Vec::new();
    }
    match tag.split_once('-') {
        Some((language, _)) => vec![tag.clone(), language.to_owned()],
        None => vec![tag],
    }
}

fn pick(xml: &str, locales: &[String]) -> Option<String> {
    let root = rikka_xml::parse(xml).ok()?;
    let comments: Vec<(Option<&str>, Cow<'_, str>)> = root
        .children_named(COMMENT)
        .map(|node| (node.attribute_in(XML, LANG), node.text()))
        .filter(|(_, text)| !text.is_empty())
        .collect();
    let spelled =
        |locale: &str, written: &str| locale.replace('-', "_") == written.replace('-', "_");
    locales
        .iter()
        .find_map(|locale| {
            comments
                .iter()
                .find(|(lang, _)| lang.is_some_and(|lang| spelled(locale, lang)))
        })
        .or_else(|| comments.iter().find(|(lang, _)| lang.is_none()))
        .map(|(_, text)| text.to_string())
}

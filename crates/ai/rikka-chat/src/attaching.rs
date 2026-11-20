use std::path::{Path, PathBuf};

use rikka_clipboard::{URI_LIST, uri_list};
use rikka_dialog::{Chooser, ChooserResponse};
use rikka_llm::Media;
use rikka_open::uri_to_path;
use rikka_thread::Attachment;
use rikka_ui::clipboard;
use rikka_ui::{Accept, DragOffer, WindowIdentifier};

use crate::chat::Labels;

const NAMED: usize = 2;

const LISTED: usize = 1000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Pasted {
    Picture { offered: String, media: Media },
    Files,
}

#[derive(Clone, Debug, Default)]
pub struct Attached {
    pub attachments: Vec<Attachment>,
    pub typed: Option<String>,
    pub refused: Option<String>,
}

impl Attached {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.attachments.is_empty() && self.typed.is_none() && self.refused.is_none()
    }

    #[must_use]
    pub fn join(mut self, other: Attached) -> Attached {
        self.attachments.extend(other.attachments);
        self.typed = self.typed.or(other.typed);
        self.refused = match (self.refused, other.refused) {
            (Some(one), Some(two)) => Some(format!("{one}\n{two}")),
            (one, two) => one.or(two),
        };
        self
    }
}

pub(crate) fn pasted_kind(mime_types: &[String]) -> Option<Pasted> {
    let picture = mime_types
        .iter()
        .filter_map(|mime| Some((mime, Media::from_mime(mime)?)))
        .find(|(_, media)| media.is_picture());
    if let Some((mime, media)) = picture {
        return Some(Pasted::Picture {
            offered: mime.clone(),
            media,
        });
    }
    mime_types
        .iter()
        .any(|mime| mime == URI_LIST)
        .then_some(Pasted::Files)
}

pub(crate) fn accepts(offer: &DragOffer) -> Option<Accept> {
    match pasted_kind(offer.mimes())? {
        Pasted::Picture { offered, .. } => Some(Accept::copy(offered)),
        Pasted::Files => Some(Accept::copy(URI_LIST)),
    }
}

pub(crate) fn pasted<C: Labels>(content: Pasted) -> Attached {
    match content {
        Pasted::Picture { offered, media } => match clipboard::read(&offered) {
            Ok(data) => read::<C>([Attachment::new(named(&C::pasted(), media), media, data)]),
            Err(error) => refused::<C>(&error.to_string()),
        },
        Pasted::Files => match clipboard::read(URI_LIST) {
            Ok(list) => files::<C>(&uri_list(&String::from_utf8_lossy(&list))),
            Err(error) => refused::<C>(&error.to_string()),
        },
    }
}

pub(crate) fn dropped<C: Labels>(offer: &DragOffer) -> Attached {
    match pasted_kind(offer.mimes()) {
        Some(Pasted::Picture { offered, media }) => match offer.read(&offered) {
            Some(data) => read::<C>([Attachment::new(named(&C::dropped(), media), media, data)]),
            None => refused::<C>(&C::unreadable()),
        },
        Some(Pasted::Files) => {
            let paths: Vec<PathBuf> = offer
                .uris()
                .iter()
                .filter_map(|uri| uri_to_path(uri))
                .collect();
            match paths.is_empty() {
                true => Attached {
                    typed: offer.text(),
                    ..Attached::default()
                },
                false => read::<C>(paths.iter().map(|path| Attachment::read(path))),
            }
        }
        None => Attached::default(),
    }
}

pub(crate) fn chosen<C: Labels>(parent: Option<WindowIdentifier>) -> Attached {
    let picked_files = Chooser::open(C::choose_files().to_string())
        .parent(parent)
        .accept_label(C::attach().to_string())
        .multiple(true)
        .run();
    match picked_files {
        Ok(ChooserResponse::Files(picked)) => {
            read::<C>(picked.iter().map(|path| Attachment::read(path)))
        }
        Ok(ChooserResponse::Cancelled) => Attached::default(),
        Err(error) => refused::<C>(&error.to_string()),
    }
}

pub fn read_mentioned<C: Labels>(
    root: &Path,
    paths: &[String],
    listed: impl Fn(&Path) -> Vec<String>,
) -> Attached {
    read::<C>(paths.iter().map(|path| {
        let full = root.join(path);
        match full.is_dir() {
            true => listing(path, &listed(&full)),
            false => Attachment::read_as(&full, path),
        }
    }))
}

fn listing(called: &str, entries: &[String]) -> rikka_thread::Result<Attachment> {
    if entries.is_empty() {
        return Attachment::new(
            called,
            Media::PlainText,
            b"(nothing in this folder)".to_vec(),
        );
    }

    let hidden = entries.len().saturating_sub(LISTED);
    let mut text = entries[..entries.len().min(LISTED)].join("\n");
    if hidden > 0 {
        text = format!("{text}\n(and {hidden} more)");
    }
    Attachment::new(called, Media::PlainText, text.into_bytes())
}

fn files<C: Labels>(uris: &[String]) -> Attached {
    let paths: Vec<PathBuf> = uris.iter().filter_map(|uri| uri_to_path(uri)).collect();
    if !paths.is_empty() {
        return read::<C>(paths.iter().map(|path| Attachment::read(path)));
    }
    Attached {
        typed: clipboard::text(),
        ..Attached::default()
    }
}

pub(crate) fn read<C: Labels>(
    attachments: impl IntoIterator<Item = Result<Attachment, impl std::fmt::Display>>,
) -> Attached {
    let mut accepted = Vec::new();
    let mut refusals = Vec::new();
    for one in attachments {
        match one {
            Ok(one) => accepted.push(one),
            Err(error) => refusals.push(error.to_string()),
        }
    }
    Attached {
        attachments: accepted,
        typed: None,
        refused: (!refusals.is_empty()).then(|| C::refused(&gathered::<C>(&refusals)).to_string()),
    }
}

fn gathered<C: Labels>(refusals: &[String]) -> String {
    let named = refusals[..refusals.len().min(NAMED)].join("; ");
    match refusals.len().saturating_sub(NAMED) {
        0 => named,
        rest => format!("{named}; {}", C::refused_more(rest)),
    }
}

fn refused<C: Labels>(reason: &str) -> Attached {
    Attached {
        refused: Some(C::refused(reason).to_string()),
        ..Attached::default()
    }
}

fn named(called: &str, media: Media) -> String {
    format!("{called}.{}", media.extension())
}

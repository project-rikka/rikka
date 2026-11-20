use std::rc::Rc;

use rikka_ui::{Accept, DragAction, DragOffer, Element, View as _};
use rikka_widgets::drop_zone;

use crate::ffi;
use crate::handlers::Callback;
use crate::window::Model;

const URI_LIST: &str = "text/uri-list";

#[derive(Default)]
pub(crate) struct Drops {
    pub(crate) files: Option<Rc<Callback<ffi::PathsFn>>>,
    pub(crate) text: Option<Rc<Callback<ffi::StringFn>>>,
}

#[derive(Clone, Copy, Debug)]
struct Wants {
    files: bool,
    text: bool,
}

#[derive(Debug)]
enum Dropped {
    Files(Vec<String>),
    Text(String),
    Nothing,
}

impl Drops {
    pub(crate) fn is_empty(&self) -> bool {
        self.files.is_none() && self.text.is_none()
    }

    pub(crate) fn wrap(self, content: Element) -> Element {
        let Self { files, text } = self;
        let wants = Wants {
            files: files.is_some(),
            text: text.is_some(),
        };
        drop_zone(content)
            .accepts(move |offer: &DragOffer| accept(offer, wants))
            .on_drop(move |offer: DragOffer, _: DragAction| {
                let files = files.clone();
                let text = text.clone();
                rikka_ui::spawn_blocking(
                    move || read(&offer, wants),
                    move |model: &mut Model, dropped: Dropped| match dropped {
                        Dropped::Files(paths) => {
                            let borrowed: Vec<&str> = paths.iter().map(String::as_str).collect();
                            if let Some(files) = &files {
                                files.call(&mut model.context(), &borrowed);
                            }
                        }
                        Dropped::Text(dropped) => {
                            if let Some(text) = &text {
                                text.call(&mut model.context(), &dropped);
                            }
                        }
                        Dropped::Nothing => rikka_ui::skip_view(),
                    },
                );
            })
            .build()
    }
}

fn accept(offer: &DragOffer, wants: Wants) -> Option<Accept> {
    if wants.files && offer.has_uris() {
        return Some(Accept::copy(URI_LIST));
    }
    if !wants.text {
        return None;
    }
    match offer.text_mime() {
        Some(mime) => Some(Accept::copy(mime)),
        None => offer.has_uris().then(|| Accept::copy(URI_LIST)),
    }
}

fn read(offer: &DragOffer, wants: Wants) -> Dropped {
    let mut uris = None;
    if wants.files {
        let paths: Vec<String> = uris
            .get_or_insert_with(|| offer.uris())
            .iter()
            .filter_map(|uri| rikka_open::uri_to_path(uri))
            .filter_map(|path| path.into_os_string().into_string().ok())
            .collect();
        if !paths.is_empty() {
            return Dropped::Files(paths);
        }
    }
    if !wants.text {
        return Dropped::Nothing;
    }
    if let Some(text) = offer.text() {
        return Dropped::Text(text);
    }
    let uris = uris.unwrap_or_else(|| offer.uris());
    match uris.is_empty() {
        true => Dropped::Nothing,
        false => Dropped::Text(uris.join("\n")),
    }
}

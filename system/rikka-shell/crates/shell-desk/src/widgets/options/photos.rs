use std::path::Path;
use std::rc::Rc;

use desk_layout::{Content, PhotoInterval, Photos};
use rikka_ui::{Align, Element, Label, Space, Styled, TextStyled, View, spawn_blocking, text, tr};
use rikka_widgets::{ControlSize, button, column, field, row};

use crate::widgets::options::folder::choose;
use crate::widgets::options::{Change, chooser, edited, toggle};
use crate::widgets::photos::folder_name;

#[must_use]
pub(crate) fn options(photos: &Photos, change: &Change) -> Element {
    let folder = folder_name(photos.folder.as_deref().map(Path::new));
    let picking = {
        let (change, photos) = (Rc::clone(change), photos.clone());
        move || {
            let (change, photos) = (Rc::clone(&change), photos.clone());
            let title = tr!("shell.desk.widget.options.choose-folder").to_string();
            let from = photos.folder.clone();
            spawn_blocking(
                move || choose(title, from),
                move |chosen: Option<String>| {
                    if let Some(folder) = chosen {
                        change(Content::Photos(Photos {
                            folder: Some(folder),
                            ..photos.clone()
                        }));
                    }
                },
            );
        }
    };
    let pictures = {
        let (change, photos) = (Rc::clone(change), photos.clone());
        move || {
            change(Content::Photos(Photos {
                folder: None,
                ..photos.clone()
            }));
        }
    };
    let every = edited(change, photos, Content::Photos, |photos, index: usize| {
        photos.interval = PhotoInterval::ALL[index.min(PhotoInterval::ALL.len() - 1)];
    });
    let shuffled = edited(change, photos, Content::Photos, |photos, shuffle: bool| {
        photos.shuffle = shuffle;
    });
    let filled = edited(change, photos, Content::Photos, |photos, fill: bool| {
        photos.fill = fill;
    });
    column! {
        field(
            tr!("shell.desk.widget.options.folder"),
            row! {
                text(folder).muted().no_wrap().shrink(1.0);
                button(text(tr!("shell.desk.widget.options.choose")))
                    .secondary()
                    .control_size(ControlSize::Small)
                    .on_press(picking);
            }
            .gap(Space::SNUG)
            .align_items(Align::Center),
        );
        if photos.folder.is_some() {
            field(
                tr!("shell.desk.widget.options.folder-reset"),
                button(text(tr!("shell.desk.widget.photos.pictures")))
                    .ghost()
                    .control_size(ControlSize::Small)
                    .on_press(pictures),
            );
        }
        chooser(
            tr!("shell.desk.widget.options.interval"),
            PhotoInterval::ALL.iter().map(|every| interval(*every)),
            photos.interval as usize,
            every,
        );
        toggle(tr!("shell.desk.widget.options.shuffle"), photos.shuffle, shuffled);
        toggle(tr!("shell.desk.widget.options.fill"), photos.fill, filled);
    }
    .gap(Space::NORMAL)
    .fill_width()
    .build()
}

fn interval(every: PhotoInterval) -> Label {
    tr!(&format!("shell.desk.widget.options.every-{}", every.secs()))
}

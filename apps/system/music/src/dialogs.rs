use std::sync::Arc;

use music_blend::BlendId;
use music_core::{Playlist, Track};
use rikka_app::prelude::*;
use rikka_ui::Element;
use rikka_widgets::{column, row};

use crate::music::Music;

pub(crate) const STAGE: &str = "music-stage";
const WIDTH: f32 = 400.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Target {
    Playlist,
    Blend,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Named {
    Playlist(Playlist),
    Blend { id: BlendId, name: String },
}

impl Named {
    pub(crate) fn name(&self) -> &str {
        match self {
            Named::Playlist(playlist) => &playlist.name,
            Named::Blend { name, .. } => name,
        }
    }

    fn target(&self) -> Target {
        match self {
            Named::Playlist(_) => Target::Playlist,
            Named::Blend { .. } => Target::Blend,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Dialog {
    Create {
        target: Target,
        adding: Arc<[Track]>,
        suggested: String,
    },
    Rename(Named),
    Delete(Named),
}

#[derive(Clone, Debug, PartialEq)]
struct Shown {
    dialog: Dialog,
    key: u64,
}

#[derive(State)]
pub(crate) struct Dialogs {
    shown: Tracked<Option<Shown>>,
    #[untracked]
    name: TextDocument,
    #[untracked]
    generation: u64,
}

impl Dialogs {
    pub(crate) fn new() -> Dialogs {
        Dialogs {
            shown: Tracked::new(None),
            name: TextDocument::new(""),
            generation: 0,
        }
    }

    pub(crate) fn shown(&self) -> Option<&Dialog> {
        self.shown.as_ref().map(|shown| &shown.dialog)
    }

    pub(crate) fn name(&self) -> &TextDocument {
        &self.name
    }

    pub(crate) fn ask(&mut self, dialog: Dialog) {
        let name = match &dialog {
            Dialog::Create { suggested, .. } => suggested.as_str(),
            Dialog::Rename(named) => named.name(),
            Dialog::Delete(_) => "",
        };
        self.name.set_text(name);
        self.name.select_all();
        self.generation += 1;
        self.shown.set(Some(Shown {
            dialog,
            key: self.generation,
        }));
    }

    pub(crate) fn close(&mut self) {
        if self.shown.is_some() {
            self.shown.set(None);
        }
    }

    pub(crate) fn accept(&mut self) -> Option<(Dialog, String)> {
        let dialog = self.shown()?.clone();
        let name = self.name.text().trim().to_owned();
        if name.is_empty() && !matches!(dialog, Dialog::Delete(_)) {
            return None;
        }
        self.shown.set(None);
        Some((dialog, name))
    }
}

impl std::fmt::Debug for Dialogs {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Dialogs")
            .field("shown", &self.shown())
            .finish_non_exhaustive()
    }
}

pub(crate) fn view(dialogs: &Dialogs) -> Option<Element> {
    let Shown { dialog, key } = dialogs.shown.as_ref()?;
    let (heading, go) = match dialog {
        Dialog::Create {
            target: Target::Playlist,
            ..
        } => (tr!("music.dialog.create"), tr!("music.dialog.make")),
        Dialog::Create {
            target: Target::Blend,
            ..
        } => (tr!("music.dialog.create-blend"), tr!("music.dialog.make")),
        Dialog::Rename(_) => (tr!("music.dialog.rename"), tr!("music.dialog.save")),
        Dialog::Delete(_) => (tr!("music.dialog.delete"), tr!("music.dialog.erase")),
    };
    let body = match dialog {
        Dialog::Delete(named) => text(match named.target() {
            Target::Playlist => tr!("music.dialog.delete-message", name = named.name()),
            Target::Blend => tr!("music.dialog.delete-blend-message", name = named.name()),
        })
        .label()
        .muted()
        .fill_width()
        .any(),
        Dialog::Create { .. } | Dialog::Rename(_) => text_field(dialogs.name())
            .placeholder(tr!("music.dialog.name"))
            .autofocus(true)
            .on_submit(|music: &mut Music| music.accept_dialog())
            .fill_width()
            .any(),
    };
    let confirm = button(text(go));
    let confirm = match dialog {
        Dialog::Delete(_) => confirm.danger(),
        Dialog::Create { .. } | Dialog::Rename(_) => confirm.primary(),
    };
    Some(
        sheet(
            column! {
                text(heading).label().semibold();
                body;
                row! {
                    spacer().grow(1.0);
                    button(text(tr!("music.dialog.cancel")))
                        .ghost()
                        .on_press(|dialogs: &mut Dialogs| dialogs.close());
                    confirm.on_press(|music: &mut Music| music.accept_dialog());
                }
                .gap(Space::SNUG)
                .align_items(Align::Center)
                .fill_width();
            }
            .gap(Space::ROOMY)
            .padding(20.0)
            .fill_width(),
        )
        .width(WIDTH)
        .on_dismiss(|dialogs: &mut Dialogs| dialogs.close())
        .keyed(("music-dialog", *key))
        .centered_on(STAGE)
        .build(),
    )
}

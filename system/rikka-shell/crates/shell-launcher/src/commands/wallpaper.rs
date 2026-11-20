use std::sync::Arc;

use futures_util::StreamExt;
use rikka_seam::{Delivery, EventQueue, SeamClient};
use rikka_session::wallpaper::{
    self, CAPABILITY, Preview, Thumbnail, WallpaperClient, WallpaperEntry,
};
use rikka_ui::{ImageSource, MaterialSymbol, tr};
use shell_kit::appearance;

use crate::commands::chosen::Chosen;
use crate::commands::command::{Command, CommandInfo};
use crate::provider::{Activation, ChangeStream, Hit, Preview as Shown};

const ID: &str = "wallpaper";

#[derive(Debug)]
pub(crate) struct WallpaperCommand {
    current: Chosen,
    seam: SeamClient,
    wallpapers: WallpaperClient,
    announcements: EventQueue,
    listed: Vec<WallpaperEntry>,
}

impl WallpaperCommand {
    #[must_use]
    pub(crate) fn new(current: Chosen, seam: SeamClient) -> WallpaperCommand {
        WallpaperCommand {
            current,
            wallpapers: WallpaperClient::new(seam.clone()),
            announcements: seam.poll_events(Some(CAPABILITY)),
            seam,
            listed: Vec::new(),
        }
    }

    fn choices(&self) -> Vec<Hit> {
        self.listed
            .iter()
            .map(|listed| tile(listed, &self.current))
            .collect()
    }

    fn drain_announcements(&mut self) -> bool {
        let mut changed = false;
        while let Some(incoming) = self.announcements.try_next() {
            match incoming {
                Delivery::Event(event) => {
                    if let Some(listed) = wallpaper::listed(&event) {
                        self.listed = listed;
                        changed = true;
                    }
                }
                Delivery::Lagged(_) => self.wallpapers.refresh(),
            }
        }
        changed
    }
}

fn tile(listed: &WallpaperEntry, current: &Chosen) -> Hit {
    let preview = match &listed.preview {
        Preview::Still { thumbnail } => Shown::Still(match thumbnail {
            Thumbnail::Made { file } => Some(ImageSource::file(file.as_path())),
            Thumbnail::Making | Thumbnail::None => None,
        }),
        Preview::Motion { file } => Shown::Motion(Arc::from(file.as_path())),
    };
    let hit = Hit::new(listed.id.as_str(), listed.name.as_str()).preview(preview);
    match current.is(&listed.id) {
        true => hit.in_use(),
        false => hit,
    }
}

impl Command for WallpaperCommand {
    fn info(&self) -> CommandInfo {
        CommandInfo::new(ID, tr!("shell.launcher.command.wallpaper"))
            .summary(tr!("shell.launcher.wallpaper.summary"))
            .icon(MaterialSymbol::WALLPAPER)
            .gallery()
    }

    fn open(&mut self) -> Vec<Hit> {
        self.wallpapers.refresh();
        self.choices()
    }

    fn reopened(&mut self) -> Option<Vec<Hit>> {
        self.drain_announcements().then(|| self.choices())
    }

    fn apply(&mut self, id: &str) -> Activation {
        Activation::ask(appearance::write(appearance::WALLPAPER, id))
    }

    fn changes(&self) -> Option<ChangeStream> {
        Some(self.seam.subscribe_to(CAPABILITY).map(|_| ()).boxed())
    }
}

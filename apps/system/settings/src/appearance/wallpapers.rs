use std::path::{Path, PathBuf};
use std::sync::Arc;

use rikka_app::prelude::*;
use rikka_dirs::UserDirs;
use rikka_seam::net::SocketClient;
use rikka_seam::{Payload, SeamClient, SeamError};
use rikka_session::wallpaper::{
    self, CAPABILITY, Preview, Thumbnail, WallpaperClient, WallpaperEntry,
};
use rikka_settings::session::appearance as session_appearance;
use rikka_widgets::{column, row};

use crate::appearance::component::Appearance;
use crate::domain::DomainPage;
use crate::notice::error_notice;
use crate::spot_reveal::SpotReveal;

const TILE_WIDTH: f32 = 176.0;
const TILE_HEIGHT: f32 = 99.0;
const TILE_RADIUS: f32 = Corner::Item.radius();
const TILE_FRAME: f32 = 3.0;
const TILE_BORDER: f32 = 2.0;

const FOLLOWING: &str = "wallpapers";
const REACHING: &str = "wallpapers-reaching";

pub(crate) struct Wallpapers {
    session: Option<Arc<SocketClient>>,
    folder: Option<PathBuf>,
    listed: Vec<WallpaperEntry>,
    answered: bool,
    unavailable: Option<String>,
    failure: Option<String>,
    reveal: SpotReveal<()>,
}

impl Wallpapers {
    #[must_use]
    pub(crate) fn new() -> Wallpapers {
        Wallpapers {
            session: None,
            folder: UserDirs::load().wallpapers(),
            listed: Vec::new(),
            answered: false,
            unavailable: None,
            failure: None,
            reveal: SpotReveal::new(),
        }
    }

    pub(crate) fn reveal(&mut self) {
        self.reveal.request(());
        if self.answered {
            self.reveal.settle();
        }
    }

    pub(crate) fn conceal(&mut self) {
        self.reveal.clear();
    }

    #[must_use]
    pub(crate) fn section(&self, section: Container) -> Element {
        self.reveal.section((), section)
    }

    fn arrived(&mut self) {
        self.answered = true;
        self.reveal.settle();
    }

    pub(crate) fn watch(&mut self) {
        match self.connected_seam() {
            Some(seam) => following(&seam),
            None => spawn_blocking_keyed(
                REACHING,
                || {
                    SocketClient::for_capability(CAPABILITY)
                        .map(Arc::new)
                        .map_err(|error| error.to_string())
                },
                |wallpapers: &mut Wallpapers, session: Result<Arc<SocketClient>, String>| {
                    wallpapers.connected(session);
                },
            ),
        }
    }

    fn connected(&mut self, session: Result<Arc<SocketClient>, String>) {
        self.failure = None;
        match session {
            Ok(session) => {
                let seam = session.client();
                self.session = Some(session);
                self.unavailable = None;
                following(&seam);
            }
            Err(reason) => {
                self.session = None;
                self.listed.clear();
                self.unavailable = Some(reason);
                self.arrived();
            }
        }
    }

    fn connected_seam(&self) -> Option<SeamClient> {
        self.session
            .as_ref()
            .filter(|session| session.is_connected())
            .map(|session| session.client())
    }

    fn read(&mut self, reply: Result<Payload, SeamError>) {
        match reply.and_then(|payload| payload.decode::<Vec<WallpaperEntry>>()) {
            Ok(listed) => self.listed = listed,
            Err(error) => self.failure = Some(error.to_string()),
        }
        self.arrived();
    }

    fn open_library(&mut self) {
        let Some(folder) = self.folder.clone() else {
            self.failure = Some(tr!("settings.appearance.wallpapers.nowhere").to_string());
            return;
        };
        spawn_blocking(
            move || open_folder(&folder),
            |wallpapers: &mut Wallpapers, outcome: Result<(), String>| {
                wallpapers.failure = outcome.err();
            },
        );
    }

    #[must_use]
    pub(crate) fn count(&self) -> usize {
        self.listed.len()
    }
}

fn following(seam: &SeamClient) {
    spawn(
        WallpaperClient::new(seam.clone()).list(),
        |wallpapers: &mut Wallpapers, reply: Result<Payload, SeamError>| wallpapers.read(reply),
    );
    stream_keyed(
        FOLLOWING,
        wallpaper::listings(seam),
        |wallpapers: &mut Wallpapers, listed: Vec<WallpaperEntry>| wallpapers.listed = listed,
    );
}

pub(crate) fn view(wallpapers: &Wallpapers, showing: Option<&str>) -> impl View {
    column! {
        error_notice(
            wallpapers.unavailable.as_deref(),
            wallpapers.failure.as_deref(),
            (
                "settings.appearance.wallpapers.unavailable",
                "settings.appearance.wallpapers.refused",
            ),
        );
        tiles(wallpapers, showing);
        actions();
    }
    .gap(Space::NORMAL)
    .fill_width()
}

fn tiles(wallpapers: &Wallpapers, showing: Option<&str>) -> Element {
    if wallpapers.listed.is_empty() {
        return text(empty(wallpapers.folder.as_deref()))
            .label()
            .muted()
            .build();
    }
    row_of(wallpapers.listed.iter().map(|listed| tile(listed, showing)))
        .wrap_items()
        .gap(Space::NORMAL)
        .fill_width()
        .build()
}

fn empty(folder: Option<&Path>) -> Label {
    match folder {
        Some(_) => tr!("settings.appearance.wallpapers.empty"),
        None => tr!("settings.appearance.wallpapers.nowhere"),
    }
}

fn tile(listed: &WallpaperEntry, showing: Option<&str>) -> impl View {
    let palette = palette();
    let is_selected = showing == Some(listed.id.as_str());
    let frame = container(picture(&listed.preview))
        .radius(TILE_RADIUS + TILE_FRAME)
        .padding(Space::HAIR)
        .border(
            TILE_BORDER,
            match is_selected {
                true => palette.accent,
                false => Color::TRANSPARENT,
            },
        );
    let id = listed.id.clone();
    container(
        column! {
            frame;
            text(listed.name.as_str())
                .caption()
                .muted()
                .no_wrap()
                .align(TextAlign::Center)
                .width(TILE_WIDTH);
        }
        .gap(Space::TIGHT)
        .align_items(Align::Center),
    )
    .on_press(move |appearance: &mut Appearance| {
        appearance
            .domain_mut()
            .set(session_appearance::WALLPAPER, &id);
    })
    .radius(TILE_RADIUS + TILE_FRAME)
    .tooltip(listed.name.as_str())
}

fn picture(preview: &Preview) -> Element {
    let palette = palette();
    let symbol = match preview {
        Preview::Still {
            thumbnail: Thumbnail::Made { file },
        } => {
            return image(ImageSource::file(file.as_path()))
                .fit(ImageFit::Cover)
                .radius(TILE_RADIUS)
                .width(TILE_WIDTH)
                .height(TILE_HEIGHT)
                .build();
        }
        Preview::Still { .. } => MaterialSymbol::IMAGE,
        Preview::Motion { .. } => MaterialSymbol::MOVIE,
    };
    container(
        icon(symbol)
            .extent(Length::px(24.0))
            .color(palette.text_muted),
    )
    .background(palette.surface_variant)
    .radius(TILE_RADIUS)
    .width(TILE_WIDTH)
    .height(TILE_HEIGHT)
    .center()
    .build()
}

fn actions() -> impl View {
    row! {
        button(tr!("settings.appearance.wallpapers.open"))
            .secondary()
            .on_press(|wallpapers: &mut Wallpapers| wallpapers.open_library());
        button(tr!("settings.appearance.wallpapers.none"))
            .ghost()
            .on_press(|appearance: &mut Appearance| {
                appearance.domain_mut().reset(session_appearance::WALLPAPER);
            });
        spacer().grow(1.0);
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
}

fn open_folder(folder: &Path) -> Result<(), String> {
    rikka_dirs::ensure(folder).map_err(|error| error.to_string())?;
    rikka_open::open(&folder.to_string_lossy()).map_err(|error| error.to_string())
}

impl std::fmt::Debug for Wallpapers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Wallpapers")
            .field("listed", &self.listed.len())
            .field("unavailable", &self.unavailable)
            .finish_non_exhaustive()
    }
}

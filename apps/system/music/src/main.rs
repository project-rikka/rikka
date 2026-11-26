mod accounts;
mod artwork;
mod blends;
mod blocks;
mod desk;
mod dialogs;
mod gallery;
mod keys;
mod library;
mod local;
mod marks;
mod menus;
mod mpris;
mod music;
mod nav;
mod pages;
mod player;
mod player_bar;
mod portal;
mod queue;
mod services;
mod settings;
mod sidebar;
mod signal;
mod signing;
mod sound;
mod widgets;
mod wording;

use std::path::Path;

use music_blend::{Places, Store};
use music_core::{AudioCache, Providers, Roots, SavedAccount};
use music_library::Library;
use music_local::Local;
use music_preferences::Preferences;
use rikka_account_book::Accounts;
use rikka_app::prelude::*;
use rikka_dirs::{Paths, UserDir, UserDirs};
use rikka_hifi::{Hifi, Identity};

use crate::music::Music;

fn window(
    providers: Providers,
    accounts: Accounts<SavedAccount>,
    preferences: Preferences,
    audio: AudioCache,
    store: Store,
    library: Option<Library>,
) -> Window<Music> {
    keys::bind(Window::new(
        move |sender| {
            Music::new(
                sender,
                providers,
                accounts,
                preferences,
                audio,
                store,
                library,
            )
        },
        desk::view,
    ))
    .title(tr!("music.title"))
    .size(1360.0, 860.0)
    .min_size(960.0, 620.0)
    .on_start(|music: &mut Music| music.start())
    .caption(desk::caption_bar)
}

fn local_library(roots: &Roots, playlists: &Path) -> Option<Library> {
    Local::library(roots)
        .exclude(playlists)
        .open()
        .inspect_err(|error| tracing::warn!("local music unavailable: {error}"))
        .ok()
}

rikka_app::mimalloc!();

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let paths = Paths::resolve()?;
    let roots = Roots {
        cache: paths.cache_dir().join("music"),
        state: paths.state_dir().join("music"),
    };
    let accounts = Accounts::<SavedAccount>::open(&roots.state)?;
    let preferences = Preferences::open();
    let audio = AudioCache::open(roots.cache.join("audio"), preferences.cache().bytes());
    let blends = roots.state.join("blends");
    let places = Places {
        playlists: UserDirs::load()
            .path(UserDir::Music)
            .map_or_else(|| blends.clone(), |music| music.join("Playlists")),
        state: blends,
    };
    let library = local_library(&roots, &places.playlists);
    let output = Hifi::start(Identity::new("Music", "rikka-music").icon("rikka-music-symbolic"))?;
    let providers = services::services(roots, &audio, &output, library.clone())?;
    let store = Store::open(places, providers.services())?;
    App::new("rikka-music")
        .instance(Instance::single())
        .i18n(rikka_i18n::embedded!())
        .window(window(
            providers,
            accounts,
            preferences,
            audio,
            store,
            library,
        ))
        .run()?;
    rikka_web::wait_for_closed_runtimes();
    Ok(())
}

mod notice;
mod pictures;

use std::collections::HashSet;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;

use futures_channel::mpsc;
use rikka_app::prelude::*;
use rikka_audio::{PlayParams, PlaybackEvent, Track};
use rikka_dialog::{Chooser, ChooserResponse};
use rikka_dirs::UserDirs;
use rikka_format::{safe_file_name, unique_name};
use rikka_movie::{Movie, MovieEvent};
use rikka_net::DiskCache;

use crate::chat::Chat;

pub(crate) use notice::notice_bar;
pub(crate) use pictures::{blurred, decoded_frames, frames};

const LISTENING: &str = "discord-media-playback";
const UNNAMED: &str = "download";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Playing {
    pub(crate) id: String,
    pub(crate) video: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Notice {
    pub(crate) caption: String,
    pub(crate) folder: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Heard {
    Moved,
    Changed,
    Over,
}

#[derive(State)]
pub(crate) struct Media {
    #[untracked]
    movie: Option<Movie>,
    #[untracked]
    sound: Option<Track>,
    playing: Tracked<Option<Playing>>,
    paused: Tracked<bool>,
    fetching: Tracked<Option<String>>,
    revealed: Tracked<HashSet<String>>,
    notice: Tracked<Option<Notice>>,
    clock: Revision,
}

impl Media {
    pub(crate) fn new() -> Media {
        Media {
            movie: None,
            sound: None,
            playing: Tracked::new(None),
            paused: Tracked::new(false),
            fetching: Tracked::new(None),
            revealed: Tracked::new(HashSet::new()),
            notice: Tracked::new(None),
            clock: Revision::new(),
        }
    }

    pub(crate) fn playing(&self) -> Option<&Playing> {
        self.playing.as_ref()
    }

    pub(crate) fn is_paused(&self) -> bool {
        *self.paused
    }

    pub(crate) fn fetching(&self) -> Option<&str> {
        self.fetching.as_deref()
    }

    pub(crate) fn video(&self) -> Option<&VideoController> {
        self.movie.as_ref().map(Movie::video)
    }

    pub(crate) fn progress(&self) -> (Duration, Option<Duration>) {
        self.clock.read();
        match &self.sound {
            Some(sound) => (sound.position(), sound.duration()),
            None => (Duration::ZERO, None),
        }
    }

    pub(crate) fn is_revealed(&self, attachment: &str) -> bool {
        self.revealed.contains(attachment)
    }

    pub(crate) fn reveal(&mut self, attachment: &str) {
        if !self.revealed.contains(attachment) {
            self.revealed.insert(attachment.to_owned());
        }
    }

    pub(crate) fn notice(&self) -> Option<&Notice> {
        self.notice.as_ref()
    }

    pub(crate) fn dismiss(&mut self) {
        self.notice.set(None);
    }

    pub(crate) fn show_notice(&mut self, caption: String) {
        self.notice.set(Some(Notice {
            caption,
            folder: None,
        }));
    }

    pub(crate) fn toggle_pause(&mut self) {
        let paused = !*self.paused;
        if let Some(movie) = &self.movie {
            match paused {
                true => movie.pause(),
                false => movie.play(),
            }
        }
        if let Some(sound) = &mut self.sound {
            match paused {
                true => sound.pause(),
                false => sound.play(),
            }
        }
        self.paused.set(paused);
    }

    pub(crate) fn stop(&mut self) {
        if let Some(sound) = self.sound.take() {
            sound.pause();
        }
        if let Some(movie) = self.movie.take() {
            movie.pause();
        }
        if self.playing.is_some() {
            self.playing.set(None);
            cancel(LISTENING);
        }
        self.paused.set(false);
        self.fetching.set(None);
    }

    fn heard(&mut self, heard: Heard) {
        let failed = self
            .movie
            .as_ref()
            .is_some_and(|movie| movie.picture() == VideoState::Failed);
        match heard {
            Heard::Over => self.stop(),
            Heard::Changed if failed => self.stop(),
            Heard::Changed | Heard::Moved => self.clock.changed(),
        }
    }

    fn start(&mut self, playing: Playing, path: &Path) {
        self.stop();
        let (told, heard) = mpsc::unbounded();
        let tell = move |heard: Heard| {
            if told.unbounded_send(heard).is_err() {
                tracing::debug!("the conversation stopped listening to its media");
            }
        };
        if playing.video {
            let movie = Movie::open(path);
            movie.on_change(move |event| match event {
                MovieEvent::Changed => tell(Heard::Changed),
                MovieEvent::Ended => tell(Heard::Over),
                MovieEvent::Progressed => {}
            });
            movie.play();
            self.movie = Some(movie);
        } else {
            match Track::stream(path, PlayParams::new()) {
                Ok(mut sound) => {
                    sound.on_change(move |event| match event {
                        PlaybackEvent::Progressed => tell(Heard::Moved),
                        PlaybackEvent::Finished => tell(Heard::Over),
                    });
                    sound.play();
                    self.sound = Some(sound);
                }
                Err(error) => {
                    self.notice.set(Some(Notice {
                        caption: error.to_string(),
                        folder: None,
                    }));
                    return;
                }
            }
        }
        self.playing.set(Some(playing));
        stream_keyed(LISTENING, heard, |media: &mut Media, heard: Heard| {
            media.heard(heard);
        });
    }
}

impl std::fmt::Debug for Media {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Media")
            .field("playing", &*self.playing)
            .finish_non_exhaustive()
    }
}

pub(crate) fn open_link(url: &str) {
    if let Err(error) = rikka_open::open(url) {
        tracing::warn!("failed to open link: {error}");
    }
}

fn cached(url: &str, name: &str) -> Option<PathBuf> {
    static DISK_CACHE: OnceLock<Option<DiskCache>> = OnceLock::new();
    let extension = Path::new(name)
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("bin");
    DISK_CACHE
        .get_or_init(|| crate::pictures::disk_cache_in("media"))
        .as_ref()
        .map(|disk_cache| disk_cache.path_for(url, extension))
}

fn claimed(folder: &Path, name: &str) -> Result<PathBuf, String> {
    let clean = safe_file_name(name).unwrap_or_else(|| UNNAMED.to_owned());
    loop {
        let free = unique_name(&clean, |candidate| {
            folder.join(candidate).symlink_metadata().is_ok()
        });
        let path = folder.join(free);
        match std::fs::File::create_new(&path) {
            Ok(_) => return Ok(path),
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error.to_string()),
        }
    }
}

fn fetch_download(url: String, folder: PathBuf, path: PathBuf) {
    let shown = path
        .file_name()
        .map(|file| file.to_string_lossy().into_owned())
        .unwrap_or_default();
    let placeholder = path.clone();
    spawn(
        async move {
            let fetched = fetch_to(url, path).await;
            if fetched.is_err() {
                let _ = std::fs::remove_file(&placeholder);
            }
            fetched
        },
        move |media: &mut Media, fetched: Result<PathBuf, String>| {
            let notice = match fetched {
                Ok(_) => Notice {
                    caption: tr!("discord.conversation.downloaded", name = shown.as_str())
                        .to_string(),
                    folder: Some(folder.clone()),
                },
                Err(caption) => Notice {
                    caption,
                    folder: None,
                },
            };
            media.notice.set(Some(notice));
        },
    );
}

async fn fetch_to(url: String, path: PathBuf) -> Result<PathBuf, String> {
    rikka_net::download_with(url, &path)
        .start()
        .await
        .map(|_| path)
        .map_err(|error| error.to_string())
}

impl Chat {
    pub(crate) fn play(&mut self, id: &str, url: &str, name: &str, video: bool) {
        let media = &mut self.conversation_mut().media;
        if media.playing().is_some_and(|playing| playing.id == id) {
            media.toggle_pause();
            return;
        }
        let playing = Playing {
            id: id.to_owned(),
            video,
        };
        let Some(path) = cached(url, name) else {
            media.notice.set(Some(Notice {
                caption: tr!("discord.conversation.no-cache").to_string(),
                folder: None,
            }));
            return;
        };
        if path.exists() {
            media.start(playing, &path);
            return;
        }
        media.stop();
        media.fetching.set(Some(id.to_owned()));
        spawn_keyed(
            "discord-media-fetch",
            fetch_to(url.to_owned(), path),
            move |media: &mut Media, fetched: Result<PathBuf, String>| {
                if media.fetching() != Some(playing.id.as_str()) {
                    return;
                }
                media.fetching.set(None);
                match fetched {
                    Ok(path) => media.start(playing.clone(), &path),
                    Err(caption) => media.notice.set(Some(Notice {
                        caption,
                        folder: None,
                    })),
                }
            },
        );
    }

    pub(crate) fn download(&mut self, url: &str, name: &str) {
        let url = url.to_owned();
        let name = name.to_owned();
        spawn_blocking(
            move || {
                let folder = UserDirs::load().downloads();
                claimed(&folder, &name).map(|path| (folder, path))
            },
            move |media: &mut Media, claimed: Result<(PathBuf, PathBuf), String>| match claimed {
                Ok((folder, path)) => fetch_download(url.clone(), folder, path),
                Err(caption) => media.notice.set(Some(Notice {
                    caption,
                    folder: None,
                })),
            },
        );
    }

    pub(crate) fn save_as(&mut self, url: &str, name: &str) {
        let url = url.to_owned();
        let name = name.to_owned();
        let parent = window_identifier();
        spawn_blocking(
            move || {
                Chooser::save(tr!("discord.conversation.save").to_string())
                    .parent(parent)
                    .current_name(name)
                    .run()
                    .map_err(|error| error.to_string())
            },
            move |chat: &mut Chat, chosen: Result<ChooserResponse, String>| match chosen {
                Ok(ChooserResponse::Files(paths)) => {
                    if let Some(path) = paths.into_iter().next() {
                        chat.save_to(&url, path);
                    }
                }
                Ok(ChooserResponse::Cancelled) => skip_view(),
                Err(caption) => chat.conversation_mut().media.notice.set(Some(Notice {
                    caption,
                    folder: None,
                })),
            },
        );
    }

    fn save_to(&mut self, url: &str, path: PathBuf) {
        let folder = path.parent().map(Path::to_path_buf);
        let shown = path
            .file_name()
            .map(|file| file.to_string_lossy().into_owned())
            .unwrap_or_default();
        spawn(
            fetch_to(url.to_owned(), path),
            move |media: &mut Media, fetched: Result<PathBuf, String>| {
                let notice = match fetched {
                    Ok(_) => Notice {
                        caption: tr!("discord.conversation.downloaded", name = shown.as_str())
                            .to_string(),
                        folder: folder.clone(),
                    },
                    Err(caption) => Notice {
                        caption,
                        folder: None,
                    },
                };
                media.notice.set(Some(notice));
            },
        );
    }
}

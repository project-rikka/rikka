use std::future::Future;
use std::path::PathBuf;
use std::time::Duration;

use lofty::config::WriteOptions;
use lofty::picture::{MimeType, Picture, PictureType};
use lofty::tag::{ItemKey, ItemValue, Tag, TagExt, TagItem, TagType};
use music_core::{
    AlbumRef, Artwork, AudioCache, Connection, Credit, Id, Kind, Mark, Providers, Roots, Service,
    Track,
};
use music_library::Library;
use music_local::Local;
use rikka_hifi::{Hifi, Identity};
use rikka_image::{Image, Png};

pub(crate) const STREAMING: Service = Service {
    slug: "spotify",
    name: "Spotify",
    mark: Mark::Symbol('\u{e030}'),
    kind: Kind::Streaming,
};

pub(crate) struct Fixture {
    dir: tempfile::TempDir,
    music: PathBuf,
}

pub(crate) struct Opened {
    pub(crate) library: Library,
    pub(crate) local: Connection,
}

impl Fixture {
    pub(crate) fn new() -> Fixture {
        let dir = tempfile::tempdir().expect("a temporary folder");
        let music = dir.path().join("Music");
        std::fs::create_dir_all(music.join("Playlists")).unwrap();
        Fixture { dir, music }
    }

    pub(crate) fn song(&self, relative: &str, sound: Sound, items: &[(ItemKey, &str)]) {
        self.song_with(relative, sound, items, None);
    }

    pub(crate) fn song_with(
        &self,
        relative: &str,
        sound: Sound,
        items: &[(ItemKey, &str)],
        cover: Option<&[u8]>,
    ) {
        let path = self.music.join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, flac(sound)).unwrap();
        let mut tag = Tag::new(TagType::VorbisComments);
        for (key, value) in items {
            tag.push_unchecked(TagItem::new(*key, ItemValue::Text((*value).to_owned())));
        }
        if let Some(cover) = cover {
            tag.push_picture(
                Picture::unchecked(cover.to_vec())
                    .pic_type(PictureType::CoverFront)
                    .mime_type(MimeType::Png)
                    .build(),
            );
        }
        tag.save_to_path(&path, WriteOptions::default())
            .expect("tags saved");
    }

    pub(crate) fn open(&self) -> Opened {
        let roots = Roots {
            cache: self.dir.path().join("cache"),
            state: self.dir.path().join("state"),
        };
        let library = Local::library(&roots)
            .music_folder(&self.music)
            .exclude(self.music.join("Playlists"))
            .open()
            .expect("the library opens");
        library.scan().wait().expect("the scan completes");
        let audio = AudioCache::open(self.dir.path().join("audio"), 0);
        let hifi = Hifi::start(Identity::new("Rikka Music Test", "rikka-music-local-test"))
            .expect("the hifi player starts");
        let shared = library.clone();
        let providers = Providers::new(roots, &audio, &hifi)
            .try_provider(Local::SERVICE, move |setup| Local::new(setup, shared))
            .expect("local music registers");
        let registered = providers.find(Local::SERVICE.slug).expect("local music");
        let local = block(registered.resume(&self.dir.path().join("account")))
            .expect("local music links")
            .expect("local music is always there");
        Opened { library, local }
    }
}

pub(crate) fn block<T>(future: impl Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("a runtime")
        .block_on(future)
}

#[derive(Clone, Copy)]
pub(crate) struct Sound {
    pub(crate) rate: u32,
    pub(crate) bits: u8,
}

pub(crate) const CD: Sound = Sound {
    rate: 44_100,
    bits: 16,
};

pub(crate) const HI_RES: Sound = Sound {
    rate: 96_000,
    bits: 24,
};

fn flac(sound: Sound) -> Vec<u8> {
    let samples = u64::from(sound.rate);
    let mut info = Vec::new();
    info.extend_from_slice(&4096u16.to_be_bytes());
    info.extend_from_slice(&4096u16.to_be_bytes());
    info.extend_from_slice(&[0; 6]);
    let packed =
        (u64::from(sound.rate) << 44) | (1 << 41) | (u64::from(sound.bits - 1) << 36) | samples;
    info.extend_from_slice(&packed.to_be_bytes());
    info.extend_from_slice(&[0; 16]);
    let mut bytes = b"fLaC".to_vec();
    bytes.push(0x80);
    bytes.extend_from_slice(&u32::try_from(info.len()).unwrap().to_be_bytes()[1..]);
    bytes.extend_from_slice(&info);
    bytes
}

pub(crate) fn png(side: u32) -> Vec<u8> {
    Image::from_rgba8(
        side,
        side,
        [200, 40, 90, 255].repeat((side * side) as usize),
    )
    .expect("an image")
    .write(Png)
    .to_bytes()
    .expect("a PNG")
}

pub(crate) fn tagged<'a>(
    title: &'a str,
    artist: &'a str,
    album: &'a str,
) -> Vec<(ItemKey, &'a str)> {
    vec![
        (ItemKey::TrackTitle, title),
        (ItemKey::TrackArtist, artist),
        (ItemKey::AlbumTitle, album),
    ]
}

pub(crate) fn streaming(title: &str, artist: &str, seconds: u64) -> Track {
    Track {
        service: STREAMING,
        id: Id::new(format!("spotify-{title}")),
        name: title.to_owned(),
        artists: vec![Credit {
            id: None,
            name: artist.to_owned(),
        }],
        album: AlbumRef::default(),
        artwork: Artwork::default(),
        duration: Duration::from_secs(seconds),
        explicit: false,
        playable: true,
        number: 0,
        fidelity: None,
        isrc: None,
    }
}

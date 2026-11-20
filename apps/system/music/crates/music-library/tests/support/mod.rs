use std::path::{Path, PathBuf};

use lofty::config::WriteOptions;
use lofty::picture::{MimeType, Picture, PictureType};
use lofty::tag::{ItemKey, ItemValue, Tag, TagExt, TagItem, TagType};
use music_library::{Library, Setup, Summary};
use rikka_image::{Image, Png};

pub(crate) struct Fixture {
    pub(crate) dir: tempfile::TempDir,
    pub(crate) music: PathBuf,
}

impl Fixture {
    pub(crate) fn new() -> Fixture {
        let dir = tempfile::tempdir().expect("a temporary folder");
        let music = dir.path().join("Music");
        std::fs::create_dir_all(music.join("Playlists")).unwrap();
        Fixture { dir, music }
    }

    pub(crate) fn database(&self) -> PathBuf {
        self.dir.path().join("state").join("library.sqlite")
    }

    pub(crate) fn artwork(&self) -> PathBuf {
        self.dir.path().join("cache").join("artwork")
    }

    pub(crate) fn open(&self) -> Library {
        Setup::new(self.database(), self.artwork())
            .music_folder(&self.music)
            .exclude(self.music.join("Playlists"))
            .open()
            .expect("the library opens")
    }

    pub(crate) fn path(&self, relative: &str) -> PathBuf {
        self.music.join(relative)
    }
}

pub(crate) fn scan(library: &Library) -> Summary {
    library.scan().wait().expect("the scan completes")
}

#[derive(Clone, Copy)]
pub(crate) struct Sound {
    pub(crate) rate: u32,
    pub(crate) bits: u8,
    pub(crate) seconds: u64,
}

pub(crate) const CD: Sound = Sound {
    rate: 44_100,
    bits: 16,
    seconds: 1,
};

pub(crate) fn song(path: &Path, sound: Sound, items: &[(ItemKey, &str)]) -> PathBuf {
    song_with(path, sound, items, None)
}

pub(crate) fn song_with(
    path: &Path,
    sound: Sound,
    items: &[(ItemKey, &str)],
    cover: Option<&[u8]>,
) -> PathBuf {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, flac(sound)).unwrap();
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
    tag.save_to_path(path, WriteOptions::default())
        .expect("tags saved");
    path.to_path_buf()
}

fn flac(sound: Sound) -> Vec<u8> {
    let samples = u64::from(sound.rate) * sound.seconds;
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

pub(crate) fn png(width: u32, height: u32, shade: u8) -> Vec<u8> {
    let pixels = [shade, 255 - shade, shade / 2, 255].repeat((width * height) as usize);
    Image::from_rgba8(width, height, pixels)
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

use std::io::{Cursor, Read, Seek, SeekFrom};

use music_spotify_audio::{AudioKey, Decrypted, Label};
use rikka_audio_decode::{Codec, StreamReader};

const ENCRYPTED: &[u8] = include_bytes!("fixtures/tone.encrypted");
const VORBIS: &[u8] = include_bytes!("fixtures/tone.ogg");
const KEY: AudioKey = [
    0x3a, 0x91, 0xc0, 0x5e, 0x77, 0x42, 0xb8, 0xd6, 0xf0, 0x1b, 0x2c, 0x9d, 0x4e, 0x65, 0x83, 0xa7,
];
const LABEL: Label = Label {
    track: [0x11; 16],
    file: [0x22; 20],
};

struct Kept {
    bytes: Cursor<Vec<u8>>,
    arriving: bool,
}

impl Read for Kept {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.bytes.read(buf)
    }
}

impl Seek for Kept {
    fn seek(&mut self, to: SeekFrom) -> std::io::Result<u64> {
        self.bytes.seek(to)
    }
}

impl StreamReader for Kept {
    fn complete_len(&self) -> Option<u64> {
        (!self.arriving).then_some(self.bytes.get_ref().len() as u64)
    }
}

fn kept() -> Kept {
    Kept {
        bytes: Cursor::new([LABEL.encode().as_slice(), ENCRYPTED].concat()),
        arriving: false,
    }
}

#[test]
fn kept_stream_decrypts_to_its_vorbis() {
    let mut kept = kept();
    assert_eq!(Label::read(&mut kept).unwrap(), LABEL);

    let mut decrypted = Decrypted::new(Box::new(kept), &KEY);
    assert_eq!(decrypted.complete_len(), Some(VORBIS.len() as u64));
    let mut audio = Vec::new();
    decrypted.read_to_end(&mut audio).unwrap();
    assert_eq!(audio, VORBIS);
}

#[test]
fn seeks_decrypt_from_the_sought_byte() {
    let length = VORBIS.len() as u64;
    let mut decrypted = Decrypted::new(Box::new(kept()), &KEY);
    let mut window = [0; 61];
    for (to, at) in [
        (SeekFrom::Start(1003), 1003),
        (SeekFrom::Current(-(window.len() as i64) - 500), 503),
        (SeekFrom::End(-77), length - 77),
        (SeekFrom::Start(0), 0),
        (SeekFrom::Current(4096), 4096 + window.len() as u64),
    ] {
        assert_eq!(decrypted.seek(to).unwrap(), at, "{to:?}");
        decrypted.read_exact(&mut window).unwrap();
        let from = at as usize;
        assert_eq!(window, VORBIS[from..from + window.len()], "{to:?}");
    }
    assert!(decrypted.seek(SeekFrom::End(-(length as i64) - 1)).is_err());
}

#[test]
fn decrypted_stream_decodes_as_vorbis() {
    for arriving in [false, true] {
        let kept = Kept { arriving, ..kept() };
        let info = *rikka_audio_decode::reader_decoder(Decrypted::new(Box::new(kept), &KEY))
            .unwrap()
            .info();
        assert_eq!(info.encoding.codec, Codec::Vorbis, "arriving: {arriving}");
    }
}

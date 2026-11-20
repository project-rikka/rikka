use std::io::{self, Read, Seek, SeekFrom};

use aes::Aes128;
use aes::cipher::{KeyIvInit, StreamCipher, StreamCipherSeek};
use rikka_audio_decode::StreamReader;

use crate::label::Label;

const IV: [u8; 16] = [
    0x72, 0xe0, 0x67, 0xfb, 0xdd, 0xcb, 0xcf, 0x77, 0xeb, 0xe8, 0xbc, 0x64, 0x3f, 0x63, 0x0d, 0x93,
];
const HEADER: u64 = 0xa7;
const AUDIO: u64 = Label::LEN as u64 + HEADER;

pub type AudioKey = [u8; 16];

pub struct Decrypted {
    source: Box<dyn StreamReader>,
    cipher: ctr::Ctr128BE<Aes128>,
    position: u64,
}

impl Decrypted {
    #[must_use]
    pub fn new(source: Box<dyn StreamReader>, key: &AudioKey) -> Decrypted {
        let mut cipher = ctr::Ctr128BE::<Aes128>::new(&(*key).into(), &IV.into());
        cipher.seek(HEADER);
        Decrypted {
            source,
            cipher,
            position: 0,
        }
    }
}

impl Read for Decrypted {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.source.seek(SeekFrom::Start(AUDIO + self.position))?;
        let read = self.source.read(buf)?;
        self.cipher.apply_keystream(&mut buf[..read]);
        self.position += read as u64;
        Ok(read)
    }
}

impl Seek for Decrypted {
    fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
        let target = match to {
            SeekFrom::Start(offset) => Some(offset),
            SeekFrom::Current(delta) => self.position.checked_add_signed(delta),
            SeekFrom::End(delta) => self.source.seek(SeekFrom::End(delta))?.checked_sub(AUDIO),
        };
        let target = target
            .filter(|target| target.checked_add(AUDIO).is_some())
            .ok_or_else(|| io::Error::other("a seek outside the stream"))?;
        self.cipher.seek(HEADER + target);
        self.position = target;
        Ok(target)
    }
}

impl StreamReader for Decrypted {
    fn complete_len(&self) -> Option<u64> {
        self.source
            .complete_len()
            .map(|length| length.saturating_sub(AUDIO))
    }
}

impl std::fmt::Debug for Decrypted {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Decrypted")
            .field("position", &self.position)
            .finish_non_exhaustive()
    }
}

use crate::decoding::backend::Opened;
use crate::decoding::input::Input;
use crate::decoding::source::Shared;
use crate::decoding::{ffmpeg, sniff};
use crate::encoding::{Codec, Encoding};
use crate::error::{Error, Result};

pub(crate) fn open(input: Input) -> Result<Opened> {
    let Input { media, name } = input;
    let mut source = Shared::new(media);
    let sniffed = sniff::foreign(&mut source)?;
    if rikka_ffmpeg::load().is_err() {
        let codec = sniffed.map_or(Codec::Other("sound"), |encoding: Encoding| encoding.codec);
        return Err(Error::NeedsFfmpeg(codec));
    }
    ffmpeg::open(source, name, sniffed)
}

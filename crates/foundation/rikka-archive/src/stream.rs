use std::io::{Read, Seek, SeekFrom, Write};
use std::time::SystemTime;

use crate::encoding;
use crate::error::{Error, Result};
use crate::level::Level;
use crate::timestamp;

const MIN_GZIP_LEN: u64 = 18;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StreamCompression {
    Gzip,
    Bzip2,
    Xz,
    Zstd,
}

#[derive(Debug, Default)]
pub(crate) struct StreamMetadata {
    pub(crate) name: Option<String>,
    pub(crate) modified: Option<SystemTime>,
    pub(crate) size: Option<u64>,
}

pub(crate) trait EncoderStream: Write {
    fn finish_stream(self: Box<Self>) -> Result<()>;
}

impl StreamCompression {
    pub(crate) const fn magic(self) -> &'static [u8] {
        match self {
            StreamCompression::Gzip => &[0x1f, 0x8b],
            StreamCompression::Bzip2 => b"BZh",
            StreamCompression::Xz => &[0xfd, b'7', b'z', b'X', b'Z', 0x00],
            StreamCompression::Zstd => &[0x28, 0xb5, 0x2f, 0xfd],
        }
    }

    pub(crate) fn decoder<'a, R: Read + 'a>(self, source: R) -> Result<Box<dyn Read + 'a>> {
        Ok(match self {
            StreamCompression::Gzip => Box::new(flate2::read::MultiGzDecoder::new(source)),
            StreamCompression::Bzip2 => Box::new(bzip2::read::MultiBzDecoder::new(source)),
            StreamCompression::Xz => Box::new(lzma_rust2::XzReader::new(source, true)),
            StreamCompression::Zstd => Box::new(zstd::stream::read::Decoder::new(source)?),
        })
    }

    pub(crate) fn encoder<'a, W: Write + 'a>(
        self,
        sink: W,
        level: Level,
    ) -> Result<Box<dyn EncoderStream + 'a>> {
        self.encoder_with(sink, level, &StreamMetadata::default())
    }

    pub(crate) fn encoder_with<'a, W: Write + 'a>(
        self,
        sink: W,
        level: Level,
        metadata: &StreamMetadata,
    ) -> Result<Box<dyn EncoderStream + 'a>> {
        Ok(match self {
            StreamCompression::Gzip => {
                Box::new(gzip_builder(metadata).write(sink, level.deflate()))
            }
            StreamCompression::Bzip2 => Box::new(bzip2::write::BzEncoder::new(sink, level.bzip2())),
            StreamCompression::Xz => Box::new(lzma_rust2::XzWriter::new(
                sink,
                lzma_rust2::XzOptions::with_preset(level.preset()),
            )?),
            StreamCompression::Zstd => {
                Box::new(zstd::stream::write::Encoder::new(sink, level.zstd())?)
            }
        })
    }

    pub(crate) fn read_metadata<R: Read + Seek>(
        self,
        source: &mut R,
        length: u64,
    ) -> Result<StreamMetadata> {
        match self {
            StreamCompression::Gzip => gzip_metadata(source, length),
            _ => Ok(StreamMetadata::default()),
        }
    }
}

fn gzip_builder(metadata: &StreamMetadata) -> flate2::GzBuilder {
    let mut builder = flate2::GzBuilder::new();

    if let Some(name) = metadata.name.as_deref()
        && !name.is_empty()
        && !name.contains('\0')
    {
        builder = builder.filename(name);
    }
    if let Some(seconds) = metadata
        .modified
        .map(timestamp::to_unix_seconds)
        .and_then(|seconds| u32::try_from(seconds).ok())
    {
        builder = builder.mtime(seconds);
    }
    builder
}

fn gzip_metadata<R: Read + Seek>(source: &mut R, length: u64) -> Result<StreamMetadata> {
    source.rewind()?;
    let mut decoder = flate2::read::GzDecoder::new(&mut *source);

    let mut probe = [0u8; 1];
    decoder
        .read(&mut probe)
        .map_err(|error| Error::Damaged(format!("invalid gzip header: {error}")))?;
    let Some(header) = decoder.header() else {
        return Err(Error::Damaged("missing gzip header".to_owned()));
    };

    let name = header
        .filename()
        .map(encoding::decode_name)
        .filter(|name| !name.is_empty());
    let modified = match header.mtime() {
        0 => None,
        seconds => Some(timestamp::from_unix_seconds(i64::from(seconds))),
    };

    let size = match length >= MIN_GZIP_LEN {
        true => {
            source.seek(SeekFrom::End(-4))?;
            let mut trailer = [0u8; 4];
            source.read_exact(&mut trailer)?;
            Some(u64::from(u32::from_le_bytes(trailer)))
        }
        false => None,
    };

    Ok(StreamMetadata {
        name,
        modified,
        size,
    })
}

pub(crate) struct Passthrough<W: Write>(W);

impl<W: Write> Passthrough<W> {
    pub(crate) fn new(sink: W) -> Passthrough<W> {
        Passthrough(sink)
    }
}

impl<W: Write> Write for Passthrough<W> {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        self.0.write(buffer)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.0.flush()
    }
}

impl<W: Write> EncoderStream for Passthrough<W> {
    fn finish_stream(mut self: Box<Self>) -> Result<()> {
        self.0.flush()?;
        Ok(())
    }
}

impl<W: Write> EncoderStream for flate2::write::GzEncoder<W> {
    fn finish_stream(self: Box<Self>) -> Result<()> {
        (*self).finish()?;
        Ok(())
    }
}

impl<W: Write> EncoderStream for bzip2::write::BzEncoder<W> {
    fn finish_stream(self: Box<Self>) -> Result<()> {
        (*self).finish()?;
        Ok(())
    }
}

impl<W: Write> EncoderStream for lzma_rust2::XzWriter<W> {
    fn finish_stream(self: Box<Self>) -> Result<()> {
        (*self).finish()?;
        Ok(())
    }
}

impl<W: Write> EncoderStream for zstd::stream::write::Encoder<'static, W> {
    fn finish_stream(self: Box<Self>) -> Result<()> {
        (*self).finish()?;
        Ok(())
    }
}

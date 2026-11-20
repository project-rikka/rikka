use std::fs::File;
use std::io::{BufWriter, Cursor, Seek, Write};
use std::path::Path;

use crate::animated::Dispose;
use crate::codecs::foreign;
use crate::codecs::gif::GifStream;
use crate::codecs::{bmp, farbfeld, ffmpeg, hdr, ico, png, pnm, qoi, tga, webp};
use crate::error::{Error, Result};
use crate::format::Format;
use crate::loops::Loops;
use crate::raster::Image;
use sealed::Options;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Compression {
    Fast,
    #[default]
    Balanced,
    Best,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Png;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Jpeg;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct WebP;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Avif;

pub trait Encoding: Copy + sealed::Encode {}

impl<E: Copy + sealed::Encode> Encoding for E {}

mod sealed {
    use std::io::{Seek, Write};

    use super::Compression;
    use crate::error::Result;
    use crate::format::Format;
    use crate::raster::Image;

    #[derive(Clone, Copy, Debug)]
    pub struct Options<'a> {
        pub(crate) quality: u8,
        pub(crate) speed: u8,
        pub(crate) compression: Compression,
        pub(crate) profile: Option<&'a [u8]>,
        pub(crate) lossless: bool,
        pub(crate) text: &'a [(String, String)],
    }

    pub trait Encode {
        fn format(self) -> Format;

        fn encode<W: Write + Seek>(
            self,
            image: &Image,
            options: Options<'_>,
            writer: W,
        ) -> Result<()>;
    }

    pub trait Lossy {}

    pub trait Paced {}

    pub trait Packed {}
}

impl sealed::Lossy for Jpeg {}
impl sealed::Lossy for WebP {}
impl sealed::Lossy for Avif {}
impl sealed::Lossy for Format {}

impl sealed::Paced for Avif {}
impl sealed::Paced for Format {}

impl sealed::Packed for Png {}
impl sealed::Packed for WebP {}
impl sealed::Packed for Format {}

impl sealed::Encode for Png {
    fn format(self) -> Format {
        Format::Png
    }

    fn encode<W: Write + Seek>(
        self,
        image: &Image,
        options: Options<'_>,
        mut writer: W,
    ) -> Result<()> {
        let mut encoded = Vec::new();
        png::encode(
            image,
            options.compression,
            options.profile,
            options.text,
            &mut encoded,
        )?;
        Ok(writer.write_all(&encoded)?)
    }
}

impl sealed::Encode for Jpeg {
    fn format(self) -> Format {
        Format::Jpeg
    }

    fn encode<W: Write + Seek>(self, image: &Image, options: Options<'_>, writer: W) -> Result<()> {
        foreign::encode_jpeg(image, options.quality, options.profile, writer)
    }
}

impl sealed::Encode for WebP {
    fn format(self) -> Format {
        Format::WebP
    }

    fn encode<W: Write + Seek>(
        self,
        image: &Image,
        options: Options<'_>,
        mut writer: W,
    ) -> Result<()> {
        let settings = webp::WebPOptions {
            lossless: options.lossless,
            quality: options.quality,
            compression: options.compression,
        };
        Ok(writer.write_all(&webp::encode(image, settings, options.profile)?)?)
    }
}

impl sealed::Encode for Avif {
    fn format(self) -> Format {
        Format::Avif
    }

    fn encode<W: Write + Seek>(
        self,
        image: &Image,
        options: Options<'_>,
        mut writer: W,
    ) -> Result<()> {
        let encoded = ffmpeg::encode(
            Format::Avif,
            image,
            options.quality,
            options.speed,
            options.profile,
        )?;
        Ok(writer.write_all(&encoded)?)
    }
}

fn gif(image: &Image, out: &mut Vec<u8>) -> Result<()> {
    let (width, height) = image.dimensions();
    let (Ok(narrow_width), Ok(narrow_height)) = (u16::try_from(width), u16::try_from(height))
    else {
        return Err(Error::Encoding(format!(
            "GIF: {width}x{height} exceeds 65535 pixels a side"
        )));
    };
    let mut stream = GifStream::start(out, narrow_width, narrow_height, Loops::Finite(1))?;
    stream.frame(&image.rgba8(), 0, Dispose::Keep)?;
    stream.finish()?;
    Ok(())
}

impl sealed::Encode for Format {
    fn format(self) -> Format {
        self
    }

    fn encode<W: Write + Seek>(
        self,
        image: &Image,
        options: Options<'_>,
        mut writer: W,
    ) -> Result<()> {
        let mut encoded = Vec::new();
        match self {
            Format::Png => return Png.encode(image, options, writer),
            Format::Jpeg => return Jpeg.encode(image, options, writer),
            Format::WebP => return WebP.encode(image, options, writer),
            Format::Avif => return Avif.encode(image, options, writer),
            Format::Tiff | Format::OpenExr | Format::JpegXl => {
                encoded =
                    ffmpeg::encode(self, image, options.quality, options.speed, options.profile)?;
            }
            Format::Gif => gif(image, &mut encoded)?,
            Format::Bmp => bmp::encode(image, &mut encoded)?,
            Format::Ico => ico::encode(image, &mut encoded)?,
            Format::Tga => tga::encode(image, &mut encoded)?,
            Format::Qoi => qoi::encode(image, &mut encoded),
            Format::Hdr => hdr::encode(image, &mut encoded),
            Format::Farbfeld => farbfeld::encode(image, &mut encoded),
            Format::Pnm => pnm::encode(image, &mut encoded),
            Format::Dds | Format::Heif | Format::Jpeg2000 => {
                return Err(Error::Unsupported(format!(
                    "encoding {self} is not supported"
                )));
            }
        }
        Ok(writer.write_all(&encoded)?)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct WriteRequest<'a, E> {
    image: &'a Image,
    encoding: E,
    options: Options<'a>,
}

impl<'a, E: Encoding + sealed::Lossy> WriteRequest<'a, E> {
    #[must_use]
    pub fn quality(mut self, quality: u8) -> WriteRequest<'a, E> {
        self.options.quality = quality.clamp(1, 100);
        self
    }
}

impl<'a, E: Encoding + sealed::Paced> WriteRequest<'a, E> {
    #[must_use]
    pub fn speed(mut self, speed: u8) -> WriteRequest<'a, E> {
        self.options.speed = speed.clamp(1, 10);
        self
    }
}

impl<'a, E: Encoding + sealed::Packed> WriteRequest<'a, E> {
    #[must_use]
    pub fn compression(mut self, compression: Compression) -> WriteRequest<'a, E> {
        self.options.compression = compression;
        self
    }
}

impl<'a, E: Encoding> WriteRequest<'a, E> {
    pub fn to(self, path: impl AsRef<Path>) -> Result<()> {
        let mut file = BufWriter::new(File::create(path.as_ref())?);
        self.encode_into(&mut file)?;
        file.into_inner()
            .map_err(|unwritten| unwritten.into_error())?;
        Ok(())
    }

    pub fn to_bytes(self) -> Result<Vec<u8>> {
        let mut bytes = Vec::new();
        self.encode_into(Cursor::new(&mut bytes))?;
        Ok(bytes)
    }

    fn encode_into<W: Write + Seek>(self, writer: W) -> Result<()> {
        let format = self.encoding.format();
        if !format.can_encode() {
            return Err(Error::Unsupported(format!(
                "encoding {format} is not supported"
            )));
        }
        let image = self.image.in_color(format.encode_color(self.image.color()));
        self.encoding.encode(&image, self.options, writer)
    }
}

impl<'a> WriteRequest<'a, WebP> {
    #[must_use]
    pub fn lossless(mut self) -> WriteRequest<'a, WebP> {
        self.options.lossless = true;
        self
    }
}

impl<'a> WriteRequest<'a, Png> {
    #[must_use]
    pub fn text(mut self, entries: &'a [(String, String)]) -> WriteRequest<'a, Png> {
        self.options.text = entries;
        self
    }
}

impl Image {
    #[must_use]
    pub fn write<E: Encoding>(&self, encoding: E) -> WriteRequest<'_, E> {
        WriteRequest {
            image: self,
            encoding,
            options: Options {
                quality: 90,
                speed: 6,
                compression: Compression::Balanced,
                profile: self.icc_profile(),
                lossless: false,
                text: &[],
            },
        }
    }

    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        let format =
            Format::from_path(path).ok_or_else(|| Error::UnknownExtension(path.to_path_buf()))?;
        self.write(format).to(path)
    }

    pub fn encode(&self, encoding: impl Encoding) -> Result<Vec<u8>> {
        self.write(encoding).to_bytes()
    }
}

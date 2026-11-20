use std::borrow::Cow;
use std::fs::File;
use std::io::{BufRead, BufReader, Cursor, Read, Seek, SeekFrom};
use std::path::Path;

use rikka_svg::ColorScheme;
use rikka_types::{Bytes, Source};

use crate::animated::{AnimatedImage, Blend, Compositor, Patch};
use crate::codecs::foreign::{self, Foreign};
use crate::codecs::gif::GifReader;
use crate::codecs::{
    Limits, Probe, bmp, dds, farbfeld, ffmpeg, hdr, ico, plausible, png, pnm, qoi, tga,
};
use crate::error::{Error, Result};
use crate::format::Format;
use crate::info::ImageInfo;
use crate::orientation::Orientation;
use crate::raster::Image;
use crate::vector;

pub const DEFAULT_MAX_ALLOC: u64 = 512 * 1024 * 1024;

const MIN_PIXEL_BYTES: usize = 4;
const SIGNATURE_BYTES: usize = 64;
const HEADER_BYTES: u64 = 64 * 1024;
const GIF_MOST_EXPANSION: usize = 16384;

#[must_use]
pub fn read<'a>(source: impl Into<Source<'a>>) -> ReadRequest<'a> {
    ReadRequest {
        source: source.into(),
        format: None,
        limits: Limits {
            max_alloc: Some(DEFAULT_MAX_ALLOC),
            max_dimensions: None,
        },
        color_scheme: ColorScheme::default(),
    }
}

#[derive(Clone, Debug)]
pub struct ReadRequest<'a> {
    source: Source<'a>,
    format: Option<Format>,
    limits: Limits,
    color_scheme: ColorScheme,
}

impl<'a> ReadRequest<'a> {
    #[must_use]
    pub fn format(mut self, format: Format) -> ReadRequest<'a> {
        self.format = Some(format);
        self
    }

    #[must_use]
    pub fn color_scheme(mut self, scheme: ColorScheme) -> ReadRequest<'a> {
        self.color_scheme = scheme;
        self
    }

    #[must_use]
    pub fn max_alloc(mut self, bytes: u64) -> ReadRequest<'a> {
        self.limits.max_alloc = Some(bytes);
        self
    }

    #[must_use]
    pub fn no_alloc_limit(mut self) -> ReadRequest<'a> {
        self.limits.max_alloc = None;
        self
    }

    #[must_use]
    pub fn max_dimensions(mut self, width: u32, height: u32) -> ReadRequest<'a> {
        self.limits.max_dimensions = Some((width, height));
        self
    }

    pub fn decode(self) -> Result<Image> {
        let limits = self.limits;
        self.open()?.decode(&limits)
    }

    pub fn animation(self) -> Result<AnimatedImage> {
        let limits = self.limits;
        self.open()?.animation(&limits)
    }

    pub fn rasterize_svg(self) -> Result<Image> {
        vector::rasterize_to_fit(&self.source, None, self.color_scheme)
    }

    pub fn rasterize_svg_to_fit(self, width: u32, height: u32) -> Result<Image> {
        vector::rasterize_to_fit(&self.source, Some((width, height)), self.color_scheme)
    }

    pub(crate) fn decode_admitted(
        self,
        admit: impl FnOnce(usize) -> bool,
    ) -> Result<Option<Image>> {
        let limits = self.limits;
        self.open()?.still(&limits, admit)
    }

    pub fn probe(self) -> Result<ImageInfo> {
        self.open()?.describe()
    }

    fn open(self) -> Result<Opened<'a>> {
        let (mut stored, hint) = match self.source {
            Source::File(path) => (Stored::File(File::open(&path)?), Format::from_path(&path)),
            Source::Bytes(bytes) => (Stored::Bytes(bytes), None),
        };
        let format = match self.format {
            Some(format) => format,
            None => stored.sniff()?.or(hint).ok_or(Error::UnknownFormat)?,
        };
        Ok(Opened { stored, format })
    }
}

impl Image {
    pub fn open(path: impl AsRef<Path>) -> Result<Image> {
        read(path.as_ref()).decode()
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Image> {
        read(bytes).decode()
    }
}

enum Stored<'a> {
    File(File),
    Bytes(Bytes<'a>),
}

impl<'a> Stored<'a> {
    fn sniff(&mut self) -> Result<Option<Format>> {
        Ok(Format::detect(&self.head(SIGNATURE_BYTES as u64)?))
    }

    fn head(&mut self, most: u64) -> Result<Cow<'_, [u8]>> {
        match self {
            Stored::Bytes(bytes) => Ok(Cow::Borrowed(bytes)),
            Stored::File(file) => {
                let mut head = Vec::new();
                file.seek(SeekFrom::Start(0))?;
                file.by_ref().take(most).read_to_end(&mut head)?;
                file.seek(SeekFrom::Start(0))?;
                Ok(Cow::Owned(head))
            }
        }
    }

    fn whole(&mut self, limits: &Limits) -> Result<Cow<'_, [u8]>> {
        match self {
            Stored::Bytes(bytes) => Ok(Cow::Borrowed(bytes)),
            Stored::File(file) => {
                let length = file.metadata()?.len();
                limits.admit_bytes(length)?;
                let mut content = Vec::with_capacity(length as usize);
                file.read_to_end(&mut content)?;
                Ok(Cow::Owned(content))
            }
        }
    }

    fn into_stream(self) -> Stream<'a> {
        match self {
            Stored::File(file) => Stream::File(BufReader::new(file)),
            Stored::Bytes(bytes) => Stream::Bytes(Cursor::new(bytes)),
        }
    }
}

struct Opened<'a> {
    stored: Stored<'a>,
    format: Format,
}

fn is_foreign(format: Format) -> bool {
    matches!(format, Format::Jpeg | Format::WebP)
}

fn probe_in_memory(format: Format, data: &[u8]) -> Result<(Probe, Orientation)> {
    let probe = match format {
        Format::Png => {
            let described = png::probe(Cursor::new(data))?;
            return Ok((described.probe, described.orientation));
        }
        Format::Gif => GifReader::new(data)?.probe(),
        Format::Bmp => bmp::probe(data)?,
        Format::Ico => ico::probe(data)?,
        Format::Tga => tga::probe(data)?,
        Format::Qoi => qoi::probe(data)?,
        Format::Dds => dds::probe(data)?,
        Format::Hdr => hdr::probe(data)?,
        Format::Farbfeld => farbfeld::probe(data)?,
        Format::Pnm => pnm::probe(data)?,
        other => return Err(crate::codecs::unsupported(other, "decoding")),
    };
    Ok((probe, Orientation::Upright))
}

fn decode_in_memory(format: Format, data: &[u8], limits: &Limits) -> Result<(Image, Orientation)> {
    let image = match format {
        Format::Png => return png::decode(data, limits),
        Format::Gif => gif_frames(data, limits, 1)?
            .into_frames()
            .swap_remove(0)
            .into_image(),
        Format::Bmp => bmp::decode(data, limits)?,
        Format::Ico => ico::decode(data, limits)?,
        Format::Tga => tga::decode(data, limits)?,
        Format::Qoi => qoi::decode(data, limits)?,
        Format::Dds => dds::decode(data, limits)?,
        Format::Hdr => hdr::decode(data, limits)?,
        Format::Farbfeld => farbfeld::decode(data, limits)?,
        Format::Pnm => pnm::decode(data, limits)?,
        other => return Err(crate::codecs::unsupported(other, "decoding")),
    };
    Ok((image, Orientation::Upright))
}

fn gif_frames(data: &[u8], limits: &Limits, most: usize) -> Result<AnimatedImage> {
    let mut reader = GifReader::new(data)?;
    let canvas = reader.probe();
    let bytes = limits.admit(canvas.width, canvas.height, canvas.color)?;
    plausible(Format::Gif, bytes, data.len(), GIF_MOST_EXPANSION)?;
    let mut compositor = Compositor::new(canvas.width, canvas.height, limits)?;
    let mut rgba = Vec::new();
    for _ in 0..most {
        let Some(frame) = reader.next_frame(limits, &mut rgba)? else {
            break;
        };
        let patch = Patch {
            left: frame.left,
            top: frame.top,
            width: frame.width,
            height: frame.height,
            rgba: &rgba,
        };
        compositor.push(patch, Blend::Over, frame.dispose, frame.delay)?;
    }
    compositor.finish(reader.loops())
}

impl Opened<'_> {
    fn still(
        mut self,
        limits: &Limits,
        admit: impl FnOnce(usize) -> bool,
    ) -> Result<Option<Image>> {
        let cost = |probe: Probe| {
            (probe.width as usize)
                .saturating_mul(probe.height as usize)
                .saturating_mul(usize::from(probe.color.bytes_per_pixel()).max(MIN_PIXEL_BYTES))
        };
        let (mut image, orientation) = if is_foreign(self.format) {
            let mut foreign = Foreign::open(self.format, self.stored.into_stream(), limits)?;
            if !admit(cost(foreign.probe()?.0)) {
                return Ok(None);
            }
            foreign.decode()?
        } else if self.format.needs_ffmpeg() {
            let data = self.stored.whole(limits)?;
            if !admit(cost(ffmpeg::probe(self.format, Cursor::new(&data[..]))?.0)) {
                return Ok(None);
            }
            ffmpeg::decode(self.format, &data, limits)?
        } else {
            let data = self.stored.whole(limits)?;
            if !admit(cost(probe_in_memory(self.format, &data)?.0)) {
                return Ok(None);
            }
            decode_in_memory(self.format, &data, limits)?
        };
        image.apply_orientation(orientation);
        Ok(Some(image))
    }

    fn decode(self, limits: &Limits) -> Result<Image> {
        let image = self.still(limits, |_| true)?;
        Ok(image.unwrap_or_else(|| unreachable!("every size is admitted")))
    }

    fn animation(mut self, limits: &Limits) -> Result<AnimatedImage> {
        match self.format {
            Format::Gif => gif_frames(&self.stored.whole(limits)?, limits, usize::MAX),
            Format::WebP => foreign::webp_animation(self.stored.into_stream(), limits),
            Format::Png => {
                let data = self.stored.whole(limits)?;
                match png::animation(&data, limits)? {
                    Some((compositor, loops)) => compositor.finish(loops),
                    None => {
                        let (mut image, orientation) = png::decode(&data, limits)?;
                        image.apply_orientation(orientation);
                        Ok(AnimatedImage::from_still(image))
                    }
                }
            }
            Format::Avif | Format::Heif | Format::JpegXl => {
                let data = self.stored.whole(limits)?;
                match ffmpeg::animation(self.format, &data, limits)? {
                    Some(animation) => Ok(animation),
                    None => {
                        let (mut image, orientation) = ffmpeg::decode(self.format, &data, limits)?;
                        image.apply_orientation(orientation);
                        Ok(AnimatedImage::from_still(image))
                    }
                }
            }
            _ => self.decode(limits).map(AnimatedImage::from_still),
        }
    }

    fn describe(mut self) -> Result<ImageInfo> {
        let format = self.format;
        let unlimited = Limits::default();
        let mut text = Vec::new();
        let (probe, orientation) = if is_foreign(format) {
            Foreign::open(format, self.stored.into_stream(), &unlimited)?.probe()?
        } else if format.needs_ffmpeg() {
            ffmpeg::probe(format, self.stored.into_stream())?
        } else if format == Format::Png {
            let described = png::probe(self.stored.into_stream())?;
            text = described.text;
            (described.probe, described.orientation)
        } else if format == Format::Ico {
            probe_in_memory(format, &self.stored.whole(&unlimited)?)?
        } else {
            probe_in_memory(format, &self.stored.head(HEADER_BYTES)?)?
        };
        let (width, height) = match orientation.swaps_axes() {
            true => (probe.height, probe.width),
            false => (probe.width, probe.height),
        };
        Ok(ImageInfo {
            format,
            width,
            height,
            color: probe.color,
            orientation,
            text,
        })
    }
}

enum Stream<'a> {
    File(BufReader<File>),
    Bytes(Cursor<Bytes<'a>>),
}

impl BufRead for Stream<'_> {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        match self {
            Stream::File(reader) => reader.fill_buf(),
            Stream::Bytes(reader) => reader.fill_buf(),
        }
    }

    fn consume(&mut self, amount: usize) {
        match self {
            Stream::File(reader) => reader.consume(amount),
            Stream::Bytes(reader) => reader.consume(amount),
        }
    }
}

impl Read for Stream<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Stream::File(reader) => reader.read(buffer),
            Stream::Bytes(reader) => reader.read(buffer),
        }
    }
}

impl Seek for Stream<'_> {
    fn seek(&mut self, to: SeekFrom) -> std::io::Result<u64> {
        match self {
            Stream::File(reader) => reader.seek(to),
            Stream::Bytes(reader) => reader.seek(to),
        }
    }
}

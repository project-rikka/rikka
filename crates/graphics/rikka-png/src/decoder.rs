use rikka_deflate::Inflater;

use crate::animation::{Animation, AnimationFrame, FrameControl};
use crate::chunk::{ACTL, Chunk, Chunks, EXIF, FCTL, FDAT, ICCP, IDAT, IEND, IHDR, PLTE, TRNS};
use crate::error::{Error, Result};
use crate::expand::{Palette, Transparency, is_verbatim, native_row, read_transparency, rgba8_row};
use crate::header::{Color, Header};
use crate::layout::{Layout, Limits, Picture, Samples};
use crate::unfilter::unfilter;

const MAX_PROFILE: usize = 64 << 20;
const SEQUENCE_NUMBER: usize = 4;
const MOST_EXPANSION: usize = 1032;

#[derive(Debug, Default)]
pub struct Decoder {
    inflater: Inflater,
    compressed: Vec<u8>,
    raw: Vec<u8>,
    native: Vec<u8>,
    converted: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Decoded {
    pub picture: Picture,
    pub icc_profile: Option<Vec<u8>>,
    pub exif: Option<Vec<u8>>,
}

pub(crate) struct Stream<'d> {
    pub(crate) control: Option<FrameControl>,
    pub(crate) pieces: Vec<&'d [u8]>,
}

pub(crate) struct Parsed<'d> {
    pub(crate) header: Header,
    pub(crate) palette: Palette,
    pub(crate) transparency: Transparency,
    pub(crate) profile: Option<&'d [u8]>,
    pub(crate) exif: Option<&'d [u8]>,
    pub(crate) plays: Option<u32>,
    pub(crate) image: Stream<'d>,
    pub(crate) frames: Vec<Stream<'d>>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Progress {
    Before,
    Within,
    After,
}

impl<'d> Parsed<'d> {
    pub(crate) fn read(data: &'d [u8]) -> Result<Self> {
        let mut header: Option<Header> = None;
        let mut palette = Palette::new();
        let mut transparency = Transparency::None;
        let mut profile = None;
        let mut exif = None;
        let mut plays = None;
        let mut image = Stream {
            control: None,
            pieces: Vec::new(),
        };
        let mut frames: Vec<Stream<'d>> = Vec::new();
        let mut progress = Progress::Before;
        for chunk in Chunks::new(data)? {
            let chunk = match chunk {
                Ok(chunk) => chunk,
                Err(Error::Checksum(kind)) if !kind.is_critical() => continue,
                Err(Error::Truncated) if progress != Progress::Before => break,
                Err(error) => return Err(error),
            };
            let Chunk { kind, body } = chunk;
            let known = match header {
                Some(header) => header,
                None if kind == IHDR => {
                    header = Some(Header::parse(body)?);
                    continue;
                }
                None => return Err(Error::Header),
            };
            if progress == Progress::Within && kind != IDAT {
                progress = Progress::After;
            }
            match kind {
                IHDR => return Err(Error::Chunk(IHDR)),
                PLTE if progress == Progress::Before => palette.read(body)?,
                TRNS if progress == Progress::Before => {
                    transparency = read_transparency(&known, body, &mut palette)?;
                }
                PLTE | TRNS => return Err(Error::Chunk(kind)),
                IDAT if progress == Progress::After => return Err(Error::Chunk(IDAT)),
                IDAT => {
                    progress = Progress::Within;
                    image.pieces.push(body);
                }
                IEND => break,
                ICCP => profile = profile.or(Some(body)),
                EXIF => exif = exif.or(Some(body)),
                ACTL => {
                    let counts: &[u8; 8] = body.try_into().map_err(|_| Error::Chunk(ACTL))?;
                    plays = Some(u32::from_be_bytes([
                        counts[4], counts[5], counts[6], counts[7],
                    ]));
                }
                FCTL => {
                    let control = FrameControl::parse(body, &known)?;
                    if progress == Progress::Before {
                        image.control = Some(control);
                    } else {
                        frames.push(Stream {
                            control: Some(control),
                            pieces: Vec::new(),
                        });
                    }
                }
                FDAT => {
                    let piece = body.get(SEQUENCE_NUMBER..).ok_or(Error::Chunk(FDAT))?;
                    frames
                        .last_mut()
                        .ok_or(Error::Chunk(FDAT))?
                        .pieces
                        .push(piece);
                }
                unknown if unknown.is_critical() => {
                    return Err(Error::UnknownCriticalChunk(unknown));
                }
                _ => {}
            }
        }
        let header = header.ok_or(Error::Truncated)?;
        if image.pieces.is_empty() {
            return Err(Error::NoImageData);
        }
        if header.color == Color::Indexed && palette.len == 0 {
            return Err(Error::Chunk(PLTE));
        }
        Ok(Self {
            header,
            palette,
            transparency,
            profile,
            exif,
            plays,
            image,
            frames,
        })
    }

    pub(crate) fn layout(&self) -> Layout {
        self.header.layout(self.transparency.adds_alpha())
    }
}

impl Decoder {
    #[must_use]
    pub fn new() -> Decoder {
        Decoder::default()
    }

    pub fn rgba8(&mut self, data: &[u8], limits: &Limits, out: &mut Vec<u8>) -> Result<(u32, u32)> {
        let parsed = Parsed::read(data)?;
        let header = parsed.header;
        header.admit(Layout::Rgba8, limits)?;
        self.rgba8_stream(&parsed, &header, &parsed.image, out)?;
        Ok((header.width as u32, header.height as u32))
    }

    pub fn decode(&mut self, data: &[u8], limits: &Limits) -> Result<Decoded> {
        let parsed = Parsed::read(data)?;
        let header = parsed.header;
        let layout = parsed.layout();
        let bytes = header.admit(layout, limits)?;
        plausible(&header, &parsed.image)?;
        let samples = if is_verbatim(&header, parsed.transparency) {
            let mut raw = Vec::new();
            self.inflate(&header, &parsed.image, &mut raw)?;
            let row = header.row_bytes(header.width);
            for y in 0..header.height {
                let start = y * (row + 1) + 1;
                raw.copy_within(start..start + row, y * row);
            }
            raw.truncate(bytes);
            Samples::U8(raw)
        } else if layout.is_deep() {
            let mut samples = vec![0u16; bytes / 2];
            self.native_stream(&parsed, &header, bytemuck::cast_slice_mut(&mut samples))?;
            Samples::U16(samples)
        } else {
            let mut samples = vec![0u8; bytes];
            self.native_stream(&parsed, &header, &mut samples)?;
            Samples::U8(samples)
        };
        let icc_profile = parsed.profile.and_then(|body| self.profile(body, limits));
        Ok(Decoded {
            picture: Picture {
                width: header.width as u32,
                height: header.height as u32,
                layout,
                samples,
            },
            icc_profile,
            exif: parsed.exif.map(<[u8]>::to_vec),
        })
    }

    pub fn animation(&mut self, data: &[u8], limits: &Limits) -> Result<Option<Animation>> {
        let parsed = Parsed::read(data)?;
        let Some(plays) = parsed.plays else {
            return Ok(None);
        };
        let canvas = parsed.header;
        canvas.admit(Layout::Rgba8, limits)?;
        let streams = parsed
            .image
            .control
            .is_some()
            .then_some(&parsed.image)
            .into_iter()
            .chain(&parsed.frames);
        let mut frames = Vec::new();
        let mut budget = limits.max_bytes;
        for stream in streams {
            let Some(control) = stream.control else {
                continue;
            };
            if stream.pieces.is_empty() {
                return Err(Error::Chunk(FDAT));
            }
            let header = canvas.with_size(control.width as usize, control.height as usize);
            let bytes = header.admit(Layout::Rgba8, limits)? as u64;
            budget = budget.checked_sub(bytes).ok_or(Error::LimitsExceeded {
                width: canvas.width as u32,
                height: canvas.height as u32,
            })?;
            let mut rgba = Vec::new();
            self.rgba8_stream(&parsed, &header, stream, &mut rgba)?;
            frames.push(AnimationFrame { control, rgba });
        }
        if frames.is_empty() {
            return Ok(None);
        }
        Ok(Some(Animation {
            width: canvas.width as u32,
            height: canvas.height as u32,
            plays,
            frames,
        }))
    }

    fn profile(&mut self, body: &[u8], limits: &Limits) -> Option<Vec<u8>> {
        let name_end = body.iter().take(80).position(|&byte| byte == 0)?;
        let (&method, compressed) = body.get(name_end + 1..)?.split_first()?;
        if method != 0 {
            return None;
        }
        let limit = MAX_PROFILE.min(usize::try_from(limits.max_bytes).unwrap_or(usize::MAX));
        let mut profile = Vec::new();
        self.inflater.zlib(compressed, &mut profile, limit).ok()?;
        (!profile.is_empty()).then_some(profile)
    }

    fn inflate(&mut self, header: &Header, stream: &Stream<'_>, raw: &mut Vec<u8>) -> Result<()> {
        let expected = header.raw_len();
        plausible(header, stream)?;
        let compressed = match stream.pieces.as_slice() {
            [single] => *single,
            pieces => {
                self.compressed.clear();
                for piece in pieces {
                    self.compressed.extend_from_slice(piece);
                }
                self.compressed.as_slice()
            }
        };
        raw.clear();
        raw.reserve(expected);
        self.inflater.zlib(compressed, raw, expected)?;
        if raw.len() != expected {
            return Err(Error::Data(rikka_deflate::Error::Truncated));
        }
        let mut offset = 0;
        for pass in header.passes() {
            let row = header.row_bytes(pass.width);
            let len = pass.height * (1 + row);
            unfilter(&mut raw[offset..offset + len], row, header.filter_stride())?;
            offset += len;
        }
        Ok(())
    }

    fn native_stream(
        &mut self,
        parsed: &Parsed<'_>,
        header: &Header,
        out: &mut [u8],
    ) -> Result<()> {
        let mut raw = std::mem::take(&mut self.raw);
        let result = self
            .inflate(header, &parsed.image, &mut raw)
            .and_then(|()| self.expand(parsed, header, &raw, Target::Native, out));
        self.raw = raw;
        result
    }

    fn rgba8_stream(
        &mut self,
        parsed: &Parsed<'_>,
        header: &Header,
        stream: &Stream<'_>,
        out: &mut Vec<u8>,
    ) -> Result<()> {
        let mut raw = std::mem::take(&mut self.raw);
        let result = self.inflate(header, stream, &mut raw).and_then(|()| {
            out.clear();
            out.resize(header.width * header.height * 4, 0);
            self.expand(parsed, header, &raw, Target::Rgba8, out)
        });
        self.raw = raw;
        result
    }

    fn expand(
        &mut self,
        parsed: &Parsed<'_>,
        header: &Header,
        raw: &[u8],
        target: Target,
        out: &mut [u8],
    ) -> Result<()> {
        let layout = parsed.layout();
        let native_pixel = layout.bytes_per_pixel();
        let converts = target == Target::Rgba8 && layout != Layout::Rgba8;
        let pixel = if converts { 4 } else { native_pixel };
        let pitch = header.width * pixel;
        self.native.clear();
        self.native.resize(header.width * native_pixel, 0);
        self.converted.clear();
        self.converted.resize(header.width * 4, 0);
        let mut offset = 0;
        for pass in header.passes() {
            let row = header.row_bytes(pass.width);
            let rows = &raw[offset..offset + pass.height * (1 + row)];
            offset += rows.len();
            for (index, line) in rows.chunks_exact(row + 1).enumerate() {
                let line = &line[1..];
                let y = pass.y0 + index * pass.dy;
                let target_row = &mut out[y * pitch..(y + 1) * pitch];
                let native = &mut self.native[..pass.width * native_pixel];
                let direct = pass.dx == 1;
                if direct && !converts {
                    native_row(
                        header,
                        &parsed.palette,
                        parsed.transparency,
                        line,
                        target_row,
                    )?;
                    continue;
                }
                native_row(header, &parsed.palette, parsed.transparency, line, native)?;
                if direct {
                    rgba8_row(layout, native, target_row);
                    continue;
                }
                let pixels: &[u8] = if converts {
                    let converted = &mut self.converted[..pass.width * 4];
                    rgba8_row(layout, native, converted);
                    converted
                } else {
                    native
                };
                for (column, source) in pixels.chunks_exact(pixel).enumerate() {
                    let at = (pass.x0 + column * pass.dx) * pixel;
                    target_row[at..at + pixel].copy_from_slice(source);
                }
            }
        }
        Ok(())
    }
}

fn plausible(header: &Header, stream: &Stream<'_>) -> Result<()> {
    let available: usize = stream.pieces.iter().map(|piece| piece.len()).sum();
    if header.raw_len() / MOST_EXPANSION > available {
        return Err(Error::Data(rikka_deflate::Error::Truncated));
    }
    Ok(())
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Target {
    Native,
    Rgba8,
}

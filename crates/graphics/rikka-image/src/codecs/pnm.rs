use crate::codecs::{Limits, Probe, damaged};
use crate::color_type::ColorType;
use crate::error::{Error, Result};
use crate::format::Format;
use crate::pixels::Pixels;
use crate::raster::Image;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Encoding {
    PlainBits,
    PlainSamples,
    PackedBits,
    Binary,
}

#[derive(Clone, Copy, Debug)]
struct Header {
    width: u32,
    height: u32,
    channels: u8,
    most: u32,
    encoding: Encoding,
}

impl Header {
    fn color(&self) -> ColorType {
        let deep = self.most > 255;
        match (self.channels, deep) {
            (1, false) => ColorType::L8,
            (2, false) => ColorType::La8,
            (3, false) => ColorType::Rgb8,
            (4, false) => ColorType::Rgba8,
            (1, true) => ColorType::L16,
            (2, true) => ColorType::La16,
            (3, true) => ColorType::Rgb16,
            _ => ColorType::Rgba16,
        }
    }
}

struct Text<'d> {
    data: &'d [u8],
    at: usize,
}

impl<'d> Text<'d> {
    fn malformed() -> Error {
        damaged(Format::Pnm, "malformed header")
    }

    fn skip_blank(&mut self) {
        while let Some(&byte) = self.data.get(self.at) {
            if byte == b'#' {
                while self.data.get(self.at).is_some_and(|&byte| byte != b'\n') {
                    self.at += 1;
                }
            } else if byte.is_ascii_whitespace() {
                self.at += 1;
            } else {
                break;
            }
        }
    }

    fn token(&mut self) -> &'d [u8] {
        self.skip_blank();
        let start = self.at;
        while self
            .data
            .get(self.at)
            .is_some_and(|byte| !byte.is_ascii_whitespace())
        {
            self.at += 1;
        }
        &self.data[start..self.at]
    }

    fn number(&mut self) -> Result<u32> {
        let token = self.token();
        if token.is_empty() || token.len() > 10 || !token.iter().all(u8::is_ascii_digit) {
            return Err(Self::malformed());
        }
        token
            .iter()
            .try_fold(0u32, |value, &digit| {
                value.checked_mul(10)?.checked_add(u32::from(digit - b'0'))
            })
            .ok_or_else(Self::malformed)
    }

    fn bit(&mut self) -> Result<bool> {
        self.skip_blank();
        let bit = match self.data.get(self.at) {
            Some(b'0') => false,
            Some(b'1') => true,
            _ => return Err(damaged(Format::Pnm, "file ends early")),
        };
        self.at += 1;
        Ok(bit)
    }

    fn data(&mut self) -> &'d [u8] {
        self.at = (self.at + 1).min(self.data.len());
        &self.data[self.at..]
    }
}

fn header(text: &mut Text<'_>) -> Result<Header> {
    let magic = text.token();
    let (channels, encoding) = match magic {
        b"P1" => (1, Encoding::PlainBits),
        b"P2" => (1, Encoding::PlainSamples),
        b"P3" => (3, Encoding::PlainSamples),
        b"P4" => (1, Encoding::PackedBits),
        b"P5" => (1, Encoding::Binary),
        b"P6" => (3, Encoding::Binary),
        b"P7" => return arbitrary_header(text),
        _ => return Err(damaged(Format::Pnm, "missing signature")),
    };
    let width = text.number()?;
    let height = text.number()?;
    let most = match encoding {
        Encoding::PlainBits | Encoding::PackedBits => 1,
        _ => text.number()?,
    };
    checked(Header {
        width,
        height,
        channels,
        most,
        encoding,
    })
}

fn arbitrary_header(text: &mut Text<'_>) -> Result<Header> {
    let (mut width, mut height, mut depth, mut most) = (None, None, None, None);
    loop {
        match text.token() {
            b"WIDTH" => width = Some(text.number()?),
            b"HEIGHT" => height = Some(text.number()?),
            b"DEPTH" => depth = Some(text.number()?),
            b"MAXVAL" => most = Some(text.number()?),
            b"TUPLTYPE" => {
                text.token();
            }
            b"ENDHDR" => break,
            _ => return Err(Text::malformed()),
        }
    }
    let (Some(width), Some(height), Some(depth @ 1..=4), Some(most)) = (width, height, depth, most)
    else {
        return Err(Text::malformed());
    };
    checked(Header {
        width,
        height,
        channels: depth as u8,
        most,
        encoding: Encoding::Binary,
    })
}

fn checked(header: Header) -> Result<Header> {
    if header.most == 0 || header.most > 65535 {
        return Err(damaged(Format::Pnm, "invalid maximum value"));
    }
    Ok(header)
}

pub(crate) fn probe(data: &[u8]) -> Result<Probe> {
    let header = header(&mut Text { data, at: 0 })?;
    Ok(Probe {
        width: header.width,
        height: header.height,
        color: header.color(),
    })
}

pub(crate) fn decode(data: &[u8], limits: &Limits) -> Result<Image> {
    let mut text = Text { data, at: 0 };
    let header = header(&mut text)?;
    let color = header.color();
    limits.admit(header.width, header.height, color)?;
    let most = header.most;
    let pixels = if most > 255 {
        Pixels::U16(samples(&mut text, &header, |value| {
            (value * 65535 / most) as u16
        })?)
    } else {
        Pixels::U8(samples(&mut text, &header, |value| {
            (value * 255 / most) as u8
        })?)
    };
    Ok(Image::from_pixels(
        header.width,
        header.height,
        color,
        pixels,
    ))
}

fn samples<T>(text: &mut Text<'_>, header: &Header, scale: impl Fn(u32) -> T) -> Result<Vec<T>> {
    let (width, height) = (header.width as usize, header.height as usize);
    let count = width * height * usize::from(header.channels);
    let short = || damaged(Format::Pnm, "file ends early");
    let most = header.most;
    let remaining = text.data.len() - text.at;
    let mut out = Vec::new();
    match header.encoding {
        Encoding::PlainBits => {
            if remaining < count {
                return Err(short());
            }
            out.reserve_exact(count);
            for _ in 0..count {
                out.push(scale(u32::from(!text.bit()?)));
            }
        }
        Encoding::PlainSamples => {
            if remaining.div_ceil(2) < count {
                return Err(short());
            }
            out.reserve_exact(count);
            for _ in 0..count {
                let value = text.number().map_err(|_| short())?;
                if value > most {
                    return Err(damaged(Format::Pnm, "sample above the maximum value"));
                }
                out.push(scale(value));
            }
        }
        Encoding::PackedBits => {
            let pitch = width.div_ceil(8);
            let rows = text.data().get(..pitch * height).ok_or_else(short)?;
            out.reserve_exact(count);
            for row in rows.chunks_exact(pitch) {
                for column in 0..width {
                    let ink = row[column / 8] >> (7 - column % 8) & 1;
                    out.push(scale(u32::from(ink == 0)));
                }
            }
        }
        Encoding::Binary if most > 255 => {
            let bytes = text.data().get(..count * 2).ok_or_else(short)?;
            out.reserve_exact(count);
            out.extend(
                bytes
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|pair| scale(u32::from(u16::from_be_bytes(*pair)).min(most))),
            );
        }
        Encoding::Binary => {
            let bytes = text.data().get(..count).ok_or_else(short)?;
            out.reserve_exact(count);
            out.extend(bytes.iter().map(|&byte| scale(u32::from(byte).min(most))));
        }
    }
    Ok(out)
}

pub(crate) fn encode(image: &Image, out: &mut Vec<u8>) {
    let (width, height) = image.dimensions();
    let color = image.color();
    let most = if color.bits_per_channel() == 16 {
        65535
    } else {
        255
    };
    let heading = match (color.has_color(), color.has_alpha()) {
        (false, false) => format!("P5\n{width} {height}\n{most}\n"),
        (true, false) => format!("P6\n{width} {height}\n{most}\n"),
        (colored, _) => {
            let (depth, kind) = match colored {
                false => (2, "GRAYSCALE_ALPHA"),
                true => (4, "RGB_ALPHA"),
            };
            format!(
                "P7\nWIDTH {width}\nHEIGHT {height}\nDEPTH {depth}\nMAXVAL {most}\nTUPLTYPE {kind}\nENDHDR\n"
            )
        }
    };
    out.extend_from_slice(heading.as_bytes());
    match &image.pixels {
        Pixels::U16(samples) => {
            out.reserve(samples.len() * 2);
            for sample in samples {
                out.extend_from_slice(&sample.to_be_bytes());
            }
        }
        other => out.extend_from_slice(other.as_bytes()),
    }
}

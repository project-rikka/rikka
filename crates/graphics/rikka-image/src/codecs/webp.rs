use webp::{Encoder, PixelLayout, WebPConfig};

use crate::error::{Error, Result};
use crate::raster::Image;
use crate::write::Compression;

const ICC_FLAG: u8 = 0x20;
const ALPHA_FLAG: u8 = 0x10;
const HEADER: usize = 12;
const VP8L_SIGNATURE: u8 = 0x2F;
const VP8L_ALPHA_BIT: u32 = 28;

#[derive(Clone, Copy, Debug)]
pub(crate) struct WebPOptions {
    pub(crate) lossless: bool,
    pub(crate) quality: u8,
    pub(crate) compression: Compression,
}

pub(crate) fn encode(
    image: &Image,
    options: WebPOptions,
    profile: Option<&[u8]>,
) -> Result<Vec<u8>> {
    let layout = if image.has_alpha() {
        PixelLayout::Rgba
    } else {
        PixelLayout::Rgb
    };
    let encoder = Encoder::new(image.as_bytes(), layout, image.width(), image.height());
    let encoded = encoder
        .encode_advanced(&config(options)?)
        .map_err(|refused| Error::Encoding(format!("WebP: {refused:?}")))?;
    Ok(match profile {
        Some(profile) if !profile.is_empty() => {
            with_profile(&encoded, image.width(), image.height(), profile)?
        }
        _ => encoded.to_vec(),
    })
}

fn config(options: WebPOptions) -> Result<WebPConfig> {
    let mut config = WebPConfig::new()
        .map_err(|()| Error::Encoding("WebP: invalid encoder configuration".into()))?;
    config.method = match options.compression {
        Compression::Fast => 2,
        Compression::Balanced => 4,
        Compression::Best => 6,
    };
    if options.lossless {
        config.lossless = 1;
        config.exact = 1;
        config.quality = match options.compression {
            Compression::Fast => 25.0,
            Compression::Balanced => 75.0,
            Compression::Best => 100.0,
        };
    } else {
        config.lossless = 0;
        config.quality = f32::from(options.quality);
        config.use_sharp_yuv = 1;
    }
    Ok(config)
}

fn with_profile(encoded: &[u8], width: u32, height: u32, profile: &[u8]) -> Result<Vec<u8>> {
    if encoded.len() < HEADER + 8 || &encoded[..4] != b"RIFF" || &encoded[8..12] != b"WEBP" {
        return Err(Error::Encoding(
            "WebP: encoder output lacks RIFF container".into(),
        ));
    }
    let chunks = &encoded[HEADER..];
    let mut out = Vec::with_capacity(encoded.len() + profile.len() + 32);
    out.extend_from_slice(b"RIFF\0\0\0\0WEBP");
    let rest = if &chunks[..4] == b"VP8X" {
        let extended = HEADER + 8 + 10;
        if encoded.len() < extended {
            return Err(Error::Encoding("WebP: truncated extended header".into()));
        }
        let mut header = encoded[HEADER..extended].to_vec();
        header[8] |= ICC_FLAG;
        out.extend_from_slice(&header);
        &encoded[extended..]
    } else {
        out.extend_from_slice(&extended_header(width, height, lossless_alpha(chunks)));
        chunks
    };
    push_chunk(&mut out, b"ICCP", profile)?;
    out.extend_from_slice(rest);
    let size = riff_size(out.len() - 8)?;
    out[4..8].copy_from_slice(&size.to_le_bytes());
    Ok(out)
}

fn lossless_alpha(chunks: &[u8]) -> bool {
    chunks.get(..4) == Some(b"VP8L")
        && chunks.get(8) == Some(&VP8L_SIGNATURE)
        && chunks
            .get(9..13)
            .and_then(|bits| bits.try_into().ok())
            .is_some_and(|bits: [u8; 4]| u32::from_le_bytes(bits) >> VP8L_ALPHA_BIT & 1 == 1)
}

fn extended_header(width: u32, height: u32, alpha: bool) -> [u8; 18] {
    let mut header = [0; 18];
    header[..4].copy_from_slice(b"VP8X");
    header[4..8].copy_from_slice(&10u32.to_le_bytes());
    header[8] = ICC_FLAG | if alpha { ALPHA_FLAG } else { 0 };
    header[12..15].copy_from_slice(&(width - 1).to_le_bytes()[..3]);
    header[15..18].copy_from_slice(&(height - 1).to_le_bytes()[..3]);
    header
}

fn push_chunk(out: &mut Vec<u8>, fourcc: &[u8; 4], data: &[u8]) -> Result<()> {
    out.extend_from_slice(fourcc);
    out.extend_from_slice(&riff_size(data.len())?.to_le_bytes());
    out.extend_from_slice(data);
    if data.len() % 2 == 1 {
        out.push(0);
    }
    Ok(())
}

fn riff_size(len: usize) -> Result<u32> {
    u32::try_from(len).map_err(|_| Error::Encoding("WebP: file exceeds RIFF size limit".into()))
}

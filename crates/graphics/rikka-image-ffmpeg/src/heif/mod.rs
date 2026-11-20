mod meta;
mod playback;
mod sequence;
mod still;

use std::borrow::Cow;

use core::ffi::c_int;

use crate::atoms::top_level;
use crate::decode::Decoded;
use crate::error::{Error, Result};
use crate::fetch::Fetch;
use crate::ffmpeg::decoder::{Decoder, Decoding};
use crate::ffmpeg::store::{Converter, Region, Signal};
use crate::format::Format;
use crate::layout::{Layout, Limits, Picture, Samples};
use crate::orientation::Orientation;
use crate::probe::Header;

pub(crate) use meta::Nclx;
pub(crate) use playback::Playback;
pub(crate) use sequence::{Sequence, Track};
pub(crate) use still::Codec;

use meta::Meta;
use still::{Crop, Plane, Still, Tiling};

enum Container {
    Image(Meta),
    Sequence,
}

fn container(fetch: &mut Fetch<'_>) -> Result<Container> {
    let spans = top_level(fetch)?;
    if spans.iter().any(|span| span.kind == *b"meta") {
        return Meta::read(fetch).map(Container::Image);
    }
    if spans.iter().any(|span| span.kind == *b"moov") {
        return Ok(Container::Sequence);
    }
    Err(Error::malformed(fetch.format(), "file without images"))
}

pub(crate) fn header(fetch: &mut Fetch<'_>) -> Result<Header> {
    match container(fetch)? {
        Container::Image(meta) => {
            let still = Still::read(&meta, fetch)?;
            let (width, height) = still.dimensions();
            Ok(Header {
                width,
                height,
                layout: still.layout(),
                orientation: still.orientation,
                premultiplied: still.premultiplied,
                profile: still.profile,
            })
        }
        Container::Sequence => {
            let sequence = Sequence::read(fetch)?
                .ok_or_else(|| Error::malformed(fetch.format(), "file without images"))?;
            Ok(Header {
                width: sequence.color.width,
                height: sequence.color.height,
                layout: Layout::Rgba8,
                orientation: Orientation::Upright,
                profile: None,
                premultiplied: false,
            })
        }
    }
}

pub(crate) fn decode(format: Format, data: &[u8], limits: &Limits) -> Result<Decoded> {
    let mut fetch = Fetch::slice(format, data);
    let meta = match container(&mut fetch)? {
        Container::Image(meta) => meta,
        Container::Sequence => return first_frame(format, data, limits),
    };
    let still = Still::read(&meta, &mut fetch)?;
    let layout = still.layout();
    let (width, height) = (still.color.width, still.color.height);
    let picture_bytes = limits.admit(width, height, layout)?;
    let alpha_layout = Layout::new(true, false, layout.depth());
    if still.alpha.is_some() {
        let alpha_bytes = limits.admit(width, height, alpha_layout)?;
        limits.admit_total(width, height, picture_bytes + alpha_bytes)?;
    }
    let mut converter = Converter::new(format);
    let mut picture = Picture::zeroed(width, height, layout);
    decode_plane(
        &meta,
        &still.color,
        data,
        &mut converter,
        &mut picture,
        false,
    )?;
    if let Some(alpha) = &still.alpha {
        let mut plane = Picture::zeroed(width, height, alpha_layout);
        decode_plane(&meta, alpha, data, &mut converter, &mut plane, true)?;
        merge_alpha(&mut picture, &plane);
        if still.premultiplied {
            picture.unpremultiply();
        }
    }
    if let Some(crop) = still.crop {
        picture = cropped(&picture, crop);
    }
    Ok(Decoded {
        picture,
        icc_profile: still.profile,
        orientation: still.orientation,
    })
}

fn first_frame(format: Format, data: &[u8], limits: &Limits) -> Result<Decoded> {
    let mut animation = Playback::open(format, data, limits)?
        .ok_or_else(|| Error::malformed(format, "file without images"))?;
    let mut rgba = Vec::new();
    animation
        .next_frame(&mut rgba)?
        .ok_or_else(|| Error::Decoding {
            format,
            message: "no picture".to_owned(),
        })?;
    Ok(Decoded {
        picture: Picture {
            width: animation.width(),
            height: animation.height(),
            layout: Layout::Rgba8,
            samples: Samples::U8(rgba),
        },
        icc_profile: None,
        orientation: Orientation::Upright,
    })
}

pub(crate) const fn decoding(codec: Codec) -> Decoding {
    match codec {
        Codec::Av1 => Decoding::Av1,
        Codec::Hevc => Decoding::Hevc,
    }
}

pub(crate) fn signal(
    frame: &rikka_ffmpeg::Frame,
    nclx: Option<Nclx>,
    unmarked_full: bool,
) -> Signal {
    match nclx {
        Some(nclx) => Signal {
            matrix: c_int::from(nclx.matrix),
            full_range: nclx.full_range,
        },
        None => Signal::of(frame, unmarked_full),
    }
}

fn decode_plane(
    meta: &Meta,
    plane: &Plane,
    data: &[u8],
    converter: &mut Converter,
    target: &mut Picture,
    auxiliary: bool,
) -> Result<()> {
    let format = meta.format;
    let decoding = decoding(plane.coding.codec);
    let missing = || Error::Decoding {
        format,
        message: "missing picture".to_owned(),
    };
    match &plane.tiling {
        Tiling::Single(item) => {
            let bytes = meta.item_data(*item, data)?;
            let mut decoder = Decoder::open(
                format,
                decoding,
                &plane.coding.config,
                (plane.width, plane.height),
            )?;
            let frame = decoder
                .next(&mut std::iter::once((0, bytes.as_ref())))?
                .ok_or_else(missing)?;
            let region = Region::whole(target);
            converter.store(frame, signal(frame, plane.nclx, auxiliary), target, region)
        }
        Tiling::Grid {
            columns,
            tile_width,
            tile_height,
            tiles,
        } => {
            let pieces = tiles
                .iter()
                .map(|&tile| meta.item_data(tile, data))
                .collect::<Result<Vec<Cow<'_, [u8]>>>>()?;
            let mut decoder = Decoder::open(
                format,
                decoding,
                &plane.coding.config,
                (*tile_width, *tile_height),
            )?;
            let mut packets = pieces
                .iter()
                .enumerate()
                .map(|(index, piece)| (index as i64, piece.as_ref()));
            let mut placed = vec![false; tiles.len()];
            while let Some(frame) = decoder.next(&mut packets)? {
                let index = frame
                    .timestamp()
                    .and_then(|stamp| usize::try_from(stamp).ok())
                    .filter(|&index| index < tiles.len() && !placed[index])
                    .ok_or_else(|| Error::malformed(format, "image grid tiles"))?;
                placed[index] = true;
                let left = (index as u32 % columns) * tile_width;
                let top = (index as u32 / columns) * tile_height;
                let region = Region {
                    left,
                    top,
                    width: (*tile_width).min(plane.width - left),
                    height: (*tile_height).min(plane.height - top),
                };
                converter.store(frame, signal(frame, plane.nclx, auxiliary), target, region)?;
            }
            match placed.iter().all(|&done| done) {
                true => Ok(()),
                false => Err(missing()),
            }
        }
    }
}

fn merge_alpha(picture: &mut Picture, alpha: &Picture) {
    let channels = picture.layout.channels();
    match (&mut picture.samples, &alpha.samples) {
        (Samples::U8(color), Samples::U8(alpha)) => {
            for (pixel, &opacity) in color.chunks_exact_mut(channels).zip(alpha) {
                pixel[channels - 1] = opacity;
            }
        }
        (Samples::U16(color), Samples::U16(alpha)) => {
            for (pixel, &opacity) in color.chunks_exact_mut(channels).zip(alpha) {
                pixel[channels - 1] = opacity;
            }
        }
        _ => unreachable!("alpha is decoded at the picture's depth"),
    }
}

fn cropped(picture: &Picture, crop: Crop) -> Picture {
    let mut out = Picture::zeroed(crop.width, crop.height, picture.layout);
    let bytes_per_pixel = picture.layout.bytes_per_pixel();
    let (from_stride, to_stride) = (picture.stride(), out.stride());
    let source = picture.samples.as_bytes();
    let target = out.samples.as_bytes_mut();
    for row in 0..crop.height as usize {
        let from = (crop.top as usize + row) * from_stride + crop.left as usize * bytes_per_pixel;
        target[row * to_stride..(row + 1) * to_stride]
            .copy_from_slice(&source[from..from + to_stride]);
    }
    out
}

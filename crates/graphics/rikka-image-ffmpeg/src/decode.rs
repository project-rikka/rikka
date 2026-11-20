use crate::error::{Error, Result};
use crate::fetch::Fetch;
use crate::ffmpeg::decoder::{Decoder, Decoding};
use crate::ffmpeg::store::{Converter, Region, Signal};
use crate::format::Format;
use crate::layout::{Limits, Picture};
use crate::orientation::Orientation;
use crate::probe::header;
use crate::{exr, heif, jxl};

#[derive(Clone, Debug, PartialEq)]
pub struct Decoded {
    pub picture: Picture,
    pub icc_profile: Option<Vec<u8>>,
    pub orientation: Orientation,
}

pub fn decode(format: Format, data: &[u8], limits: &Limits) -> Result<Decoded> {
    match format {
        Format::Avif | Format::Heif => heif::decode(format, data, limits),
        Format::Tiff => whole(format, Decoding::Tiff, data, limits),
        Format::OpenExr => {
            exr::verify_chunks(data)?;
            whole(format, Decoding::Exr, data, limits)
        }
        Format::Jpeg2000 => whole(format, Decoding::Jpeg2000, data, limits),
        Format::JpegXl => jxl::decode(data, limits),
    }
}

fn whole(format: Format, decoding: Decoding, data: &[u8], limits: &Limits) -> Result<Decoded> {
    let header = header(&mut Fetch::slice(format, data))?;
    limits.admit(header.width, header.height, header.layout)?;
    let mut decoder = Decoder::open(format, decoding, &[], (header.width, header.height))?;
    let frame = decoder
        .next(&mut std::iter::once((0, data)))?
        .ok_or_else(|| Error::Decoding {
            format,
            message: "no picture".to_owned(),
        })?;
    let mut picture = Picture::zeroed(header.width, header.height, header.layout);
    let region = Region::whole(&picture);
    Converter::new(format).store(frame, Signal::of(frame, true), &mut picture, region)?;
    if header.premultiplied {
        picture.unpremultiply();
    }
    Ok(Decoded {
        picture,
        icc_profile: header.profile,
        orientation: header.orientation,
    })
}

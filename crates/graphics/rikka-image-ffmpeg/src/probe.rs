use std::io::{Read, Seek};

use crate::error::Result;
use crate::fetch::Fetch;
use crate::format::Format;
use crate::layout::Layout;
use crate::orientation::Orientation;
use crate::{exr, heif, jpeg2000, jxl, tiff};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Info {
    pub width: u32,
    pub height: u32,
    pub layout: Layout,
    pub orientation: Orientation,
}

#[derive(Clone, Debug)]
pub(crate) struct Header {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) layout: Layout,
    pub(crate) orientation: Orientation,
    pub(crate) profile: Option<Vec<u8>>,
    pub(crate) premultiplied: bool,
}

pub fn probe<S: Read + Seek>(format: Format, mut stream: S) -> Result<Info> {
    let mut fetch = Fetch::stream(format, &mut stream)?;
    let header = header(&mut fetch)?;
    Ok(Info {
        width: header.width,
        height: header.height,
        layout: header.layout,
        orientation: header.orientation,
    })
}

pub(crate) fn header(fetch: &mut Fetch<'_>) -> Result<Header> {
    match fetch.format() {
        Format::Avif | Format::Heif => heif::header(fetch),
        Format::Tiff => tiff::header(fetch),
        Format::OpenExr => exr::header(fetch),
        Format::Jpeg2000 => jpeg2000::header(fetch),
        Format::JpegXl => jxl::header(fetch),
    }
}

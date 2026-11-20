use std::ffi::c_int;

use rikka_types::Point;

use crate::barcode::Barcode;
use crate::error::{Error, Result};
use crate::ffi;
use crate::format::Format;
use crate::message::{self, owned};
use crate::picture::{Luma, Picture};

const MIN_FRAMED_SIDE: usize = 480;
const MAX_UPSCALE: usize = 4;
const MAX_UPSCALED_SIDE: usize = 4096;
const QUIET_ZONE_DIVISOR: usize = 8;
const MIN_QUIET_ZONE: usize = 8;
const DARK: u8 = 0;
const LIGHT: u8 = 255;
const MID_GRAY: u8 = 128;

pub fn read(picture: &Picture<'_>) -> Result<Vec<Barcode>> {
    decode(&picture.luma())
}

pub fn read_framed(picture: &Picture<'_>) -> Result<Vec<Barcode>> {
    let luma = picture.luma();
    let factor = upscale_factor(&luma);
    let upscaled = luma.upscaled(factor);
    let frame = Frame { factor, margin: 0 };

    let detections = decode(&upscaled)?;
    if !detections.is_empty() {
        return Ok(detections
            .into_iter()
            .map(|detection| frame.to_source(detection, picture))
            .collect());
    }

    let margin = (upscaled.width.max(upscaled.height) / QUIET_ZONE_DIVISOR).max(MIN_QUIET_ZONE);
    let quiet = match upscaled.mean_corner_luma() < MID_GRAY {
        true => LIGHT,
        false => DARK,
    };
    let frame = Frame { factor, margin };
    let detections = decode(&upscaled.padded(margin, quiet))?;
    Ok(detections
        .into_iter()
        .map(|detection| frame.to_source(detection, picture))
        .collect())
}

fn upscale_factor(luma: &Luma) -> usize {
    let shortest = luma.width.min(luma.height).max(1);
    let longest = luma.width.max(luma.height).max(1);
    let needed = MIN_FRAMED_SIDE.div_ceil(shortest);
    let allowed = (MAX_UPSCALED_SIDE / longest).max(1);
    needed.clamp(1, MAX_UPSCALE).min(allowed)
}

struct Frame {
    factor: usize,
    margin: usize,
}

impl Frame {
    fn to_source(&self, mut barcode: Barcode, picture: &Picture<'_>) -> Barcode {
        let factor = self.factor as f32;
        let margin = self.margin as f32;
        let (width, height) = (picture.width() as f32, picture.height() as f32);
        barcode.corners = barcode.corners.map(|corner| {
            Point::new(
                ((corner.x - margin) / factor).clamp(0.0, width),
                ((corner.y - margin) / factor).clamp(0.0, height),
            )
        });
        barcode
    }
}

struct Options(*mut ffi::ReaderOptions);

impl Options {
    fn new() -> Result<Options> {
        let options = unsafe { ffi::ZXing_ReaderOptions_new() };
        if options.is_null() {
            return Err(last_error());
        }
        unsafe {
            ffi::ZXing_ReaderOptions_setTryHarder(options, true);
            ffi::ZXing_ReaderOptions_setTryRotate(options, true);
            ffi::ZXing_ReaderOptions_setTryInvert(options, true);
            ffi::ZXing_ReaderOptions_setTryDownscale(options, true);
            ffi::ZXing_ReaderOptions_setTextMode(options, ffi::TEXT_MODE_HRI);
        }
        Ok(Options(options))
    }
}

impl Drop for Options {
    fn drop(&mut self) {
        unsafe { ffi::ZXing_ReaderOptions_delete(self.0) };
    }
}

struct View(*mut ffi::ImageView);

impl View {
    fn from_luma(luma: &Luma) -> Result<View> {
        let dimension = |value: usize| {
            c_int::try_from(value).map_err(|_| Error::TooLarge {
                width: luma.width as u32,
                height: luma.height as u32,
            })
        };
        let view = unsafe {
            ffi::ZXing_ImageView_new_checked(
                luma.pixels.as_ptr(),
                dimension(luma.pixels.len())?,
                dimension(luma.width)?,
                dimension(luma.height)?,
                ffi::IMAGE_FORMAT_LUM,
                dimension(luma.width)?,
                1,
            )
        };
        match view.is_null() {
            true => Err(last_error()),
            false => Ok(View(view)),
        }
    }
}

impl Drop for View {
    fn drop(&mut self) {
        unsafe { ffi::ZXing_ImageView_delete(self.0) };
    }
}

struct DecodeResults(*mut ffi::Barcodes);

impl Drop for DecodeResults {
    fn drop(&mut self) {
        unsafe { ffi::ZXing_Barcodes_delete(self.0) };
    }
}

fn decode(luma: &Luma) -> Result<Vec<Barcode>> {
    let options = Options::new()?;
    let view = View::from_luma(luma)?;
    let results = unsafe { ffi::ZXing_ReadBarcodes(view.0, options.0) };
    if results.is_null() {
        return Err(last_error());
    }
    let results = DecodeResults(results);
    let count = unsafe { ffi::ZXing_Barcodes_size(results.0) };
    Ok((0..count)
        .filter_map(|index| {
            let barcode = unsafe { ffi::ZXing_Barcodes_at(results.0, index) };
            detection(barcode)
        })
        .collect())
}

fn detection(barcode: *const ffi::Barcode) -> Option<Barcode> {
    if barcode.is_null() || !unsafe { ffi::ZXing_Barcode_isValid(barcode) } {
        return None;
    }
    let format = Format::from_zxing(unsafe { ffi::ZXing_Barcode_format(barcode) });
    let text = owned(unsafe { ffi::ZXing_Barcode_text(barcode) })?;
    let position = unsafe { ffi::ZXing_Barcode_position(barcode) };
    let point = |corner: ffi::PointI| Point::new(corner.x as f32, corner.y as f32);
    Some(Barcode {
        format,
        text,
        corners: [
            point(position.top_left),
            point(position.top_right),
            point(position.bottom_right),
            point(position.bottom_left),
        ],
    })
}

fn last_error() -> Error {
    Error::Reader(message::last())
}

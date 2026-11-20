use std::ffi::{CStr, c_int};

use crate::error::{Error, Result};
use crate::ffi;
use crate::message;

const QUIET_ZONE: usize = 4;
const ERROR_CORRECTION: &CStr = c"ecLevel=M";
const DARKEST_LIGHT: u8 = 128;
const INK: [u8; 4] = [0, 0, 0, 255];
const PAPER: [u8; 4] = [255, 255, 255, 255];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QrCode {
    side: usize,
    dark: Vec<bool>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QrImage {
    side: u32,
    rgba: Vec<u8>,
}

impl QrCode {
    pub fn encode(text: &str) -> Result<QrCode> {
        let symbol = Symbol::create(text)?;
        let image = Image::draw(&symbol)?;
        image.modules()
    }

    #[must_use]
    pub fn side(&self) -> usize {
        self.side
    }

    #[must_use]
    pub fn is_dark(&self, x: usize, y: usize) -> bool {
        x < self.side && y < self.side && self.dark[y * self.side + x]
    }

    #[must_use]
    pub fn rasterize(&self, module: u32) -> QrImage {
        let module = module.max(1) as usize;
        let side = (self.side + QUIET_ZONE * 2) * module;
        let mut rgba = vec![0; side * side * 4];
        for (index, pixel) in rgba.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            let (x, y) = (index % side / module, index / side / module);
            let dark =
                x >= QUIET_ZONE && y >= QUIET_ZONE && self.is_dark(x - QUIET_ZONE, y - QUIET_ZONE);
            *pixel = if dark { INK } else { PAPER };
        }
        QrImage {
            side: side as u32,
            rgba,
        }
    }
}

impl QrImage {
    #[must_use]
    pub fn side(&self) -> u32 {
        self.side
    }

    #[must_use]
    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }

    #[must_use]
    pub fn into_rgba(self) -> Vec<u8> {
        self.rgba
    }
}

struct Creator(*mut ffi::CreatorOptions);

impl Creator {
    fn new() -> Result<Creator> {
        let options = unsafe { ffi::ZXing_CreatorOptions_new(ffi::BARCODE_FORMAT_QR_CODE) };
        if options.is_null() {
            return Err(Error::Writer(message::last()));
        }
        unsafe { ffi::ZXing_CreatorOptions_setOptions(options, ERROR_CORRECTION.as_ptr()) };
        Ok(Creator(options))
    }
}

impl Drop for Creator {
    fn drop(&mut self) {
        unsafe { ffi::ZXing_CreatorOptions_delete(self.0) };
    }
}

struct Symbol(*mut ffi::Barcode);

impl Symbol {
    fn create(text: &str) -> Result<Symbol> {
        let creator = Creator::new()?;
        let size = c_int::try_from(text.len()).map_err(|_| Error::TooLong(text.len()))?;
        let barcode =
            unsafe { ffi::ZXing_CreateBarcodeFromText(text.as_ptr().cast(), size, creator.0) };
        if barcode.is_null() {
            return Err(Error::Writer(message::last()));
        }
        let symbol = Symbol(barcode);
        if !unsafe { ffi::ZXing_Barcode_isValid(symbol.0) } {
            return Err(Error::Writer(message::described(unsafe {
                ffi::ZXing_Barcode_errorMsg(symbol.0)
            })));
        }
        Ok(symbol)
    }
}

impl Drop for Symbol {
    fn drop(&mut self) {
        unsafe { ffi::ZXing_Barcode_delete(self.0) };
    }
}

struct Writer(*mut ffi::WriterOptions);

impl Writer {
    fn bare() -> Result<Writer> {
        let options = unsafe { ffi::ZXing_WriterOptions_new() };
        if options.is_null() {
            return Err(Error::Writer(message::last()));
        }
        unsafe {
            ffi::ZXing_WriterOptions_setScale(options, 1);
            ffi::ZXing_WriterOptions_setAddQuietZones(options, false);
        }
        Ok(Writer(options))
    }
}

impl Drop for Writer {
    fn drop(&mut self) {
        unsafe { ffi::ZXing_WriterOptions_delete(self.0) };
    }
}

struct Image(*mut ffi::Image);

impl Image {
    fn draw(symbol: &Symbol) -> Result<Image> {
        let writer = Writer::bare()?;
        let image = unsafe { ffi::ZXing_WriteBarcodeToImage(symbol.0, writer.0) };
        match image.is_null() {
            true => Err(Error::Writer(message::last())),
            false => Ok(Image(image)),
        }
    }

    fn modules(&self) -> Result<QrCode> {
        let (width, height, format, data) = unsafe {
            (
                ffi::ZXing_Image_width(self.0),
                ffi::ZXing_Image_height(self.0),
                ffi::ZXing_Image_format(self.0),
                ffi::ZXing_Image_data(self.0),
            )
        };
        let side = usize::try_from(width).unwrap_or(0);
        if side == 0 || width != height || format != ffi::IMAGE_FORMAT_LUM || data.is_null() {
            return Err(Error::Writer(format!(
                "unexpected {width}×{height} symbol image"
            )));
        }
        let shades = unsafe { std::slice::from_raw_parts(data, side * side) };
        Ok(QrCode {
            side,
            dark: shades.iter().map(|&shade| shade < DARKEST_LIGHT).collect(),
        })
    }
}

impl Drop for Image {
    fn drop(&mut self) {
        unsafe { ffi::ZXing_Image_delete(self.0) };
    }
}

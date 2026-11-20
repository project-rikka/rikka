use std::ffi::{c_char, c_int, c_void};

#[repr(C)]
pub(crate) struct ImageView {
    _opaque: [u8; 0],
}

#[repr(C)]
pub(crate) struct ReaderOptions {
    _opaque: [u8; 0],
}

#[repr(C)]
pub(crate) struct Barcodes {
    _opaque: [u8; 0],
}

#[repr(C)]
pub(crate) struct Barcode {
    _opaque: [u8; 0],
}

#[repr(C)]
pub(crate) struct CreatorOptions {
    _opaque: [u8; 0],
}

#[repr(C)]
pub(crate) struct WriterOptions {
    _opaque: [u8; 0],
}

#[repr(C)]
pub(crate) struct Image {
    _opaque: [u8; 0],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct PointI {
    pub(crate) x: c_int,
    pub(crate) y: c_int,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Position {
    pub(crate) top_left: PointI,
    pub(crate) top_right: PointI,
    pub(crate) bottom_right: PointI,
    pub(crate) bottom_left: PointI,
}

pub(crate) const IMAGE_FORMAT_LUM: c_int = 0x0100_0000;

pub(crate) const TEXT_MODE_HRI: c_int = 2;

pub(crate) const BARCODE_FORMAT_QR_CODE: c_int = b'Q' as c_int | (b' ' as c_int) << 8;

#[link(name = "ZXing")]
unsafe extern "C" {
    pub(crate) fn ZXing_ImageView_new_checked(
        data: *const u8,
        size: c_int,
        width: c_int,
        height: c_int,
        format: c_int,
        row_stride: c_int,
        pix_stride: c_int,
    ) -> *mut ImageView;
    pub(crate) fn ZXing_ImageView_delete(view: *mut ImageView);

    pub(crate) fn ZXing_ReaderOptions_new() -> *mut ReaderOptions;
    pub(crate) fn ZXing_ReaderOptions_delete(options: *mut ReaderOptions);
    pub(crate) fn ZXing_ReaderOptions_setTryHarder(options: *mut ReaderOptions, on: bool);
    pub(crate) fn ZXing_ReaderOptions_setTryRotate(options: *mut ReaderOptions, on: bool);
    pub(crate) fn ZXing_ReaderOptions_setTryInvert(options: *mut ReaderOptions, on: bool);
    pub(crate) fn ZXing_ReaderOptions_setTryDownscale(options: *mut ReaderOptions, on: bool);
    pub(crate) fn ZXing_ReaderOptions_setTextMode(options: *mut ReaderOptions, mode: c_int);

    pub(crate) fn ZXing_ReadBarcodes(
        view: *const ImageView,
        options: *const ReaderOptions,
    ) -> *mut Barcodes;
    pub(crate) fn ZXing_Barcodes_delete(barcodes: *mut Barcodes);
    pub(crate) fn ZXing_Barcodes_size(barcodes: *const Barcodes) -> c_int;
    pub(crate) fn ZXing_Barcodes_at(barcodes: *const Barcodes, index: c_int) -> *const Barcode;

    pub(crate) fn ZXing_Barcode_isValid(barcode: *const Barcode) -> bool;
    pub(crate) fn ZXing_Barcode_format(barcode: *const Barcode) -> c_int;
    pub(crate) fn ZXing_Barcode_text(barcode: *const Barcode) -> *mut c_char;
    pub(crate) fn ZXing_Barcode_position(barcode: *const Barcode) -> Position;

    pub(crate) fn ZXing_Barcode_errorMsg(barcode: *const Barcode) -> *mut c_char;
    pub(crate) fn ZXing_Barcode_delete(barcode: *mut Barcode);

    pub(crate) fn ZXing_CreatorOptions_new(format: c_int) -> *mut CreatorOptions;
    pub(crate) fn ZXing_CreatorOptions_delete(options: *mut CreatorOptions);
    pub(crate) fn ZXing_CreatorOptions_setOptions(
        options: *mut CreatorOptions,
        settings: *const c_char,
    );
    pub(crate) fn ZXing_CreateBarcodeFromText(
        text: *const c_char,
        size: c_int,
        options: *const CreatorOptions,
    ) -> *mut Barcode;

    pub(crate) fn ZXing_WriterOptions_new() -> *mut WriterOptions;
    pub(crate) fn ZXing_WriterOptions_delete(options: *mut WriterOptions);
    pub(crate) fn ZXing_WriterOptions_setScale(options: *mut WriterOptions, scale: c_int);
    pub(crate) fn ZXing_WriterOptions_setAddQuietZones(options: *mut WriterOptions, on: bool);
    pub(crate) fn ZXing_WriteBarcodeToImage(
        barcode: *const Barcode,
        options: *const WriterOptions,
    ) -> *mut Image;

    pub(crate) fn ZXing_Image_delete(image: *mut Image);
    pub(crate) fn ZXing_Image_data(image: *const Image) -> *const u8;
    pub(crate) fn ZXing_Image_width(image: *const Image) -> c_int;
    pub(crate) fn ZXing_Image_height(image: *const Image) -> c_int;
    pub(crate) fn ZXing_Image_format(image: *const Image) -> c_int;

    pub(crate) fn ZXing_LastErrorMsg() -> *mut c_char;
    pub(crate) fn ZXing_free(pointer: *mut c_void);
}

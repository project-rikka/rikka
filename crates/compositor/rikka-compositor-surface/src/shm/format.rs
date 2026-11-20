use rikka_compositor_types::Fourcc;

const WIRE_ARGB8888: u32 = 0;
const WIRE_XRGB8888: u32 = 1;

pub(crate) fn to_wire(format: Fourcc) -> u32 {
    match format {
        Fourcc::ARGB8888 => WIRE_ARGB8888,
        Fourcc::XRGB8888 => WIRE_XRGB8888,
        other => other.0,
    }
}

pub(crate) fn from_wire(value: u32) -> Fourcc {
    match value {
        WIRE_ARGB8888 => Fourcc::ARGB8888,
        WIRE_XRGB8888 => Fourcc::XRGB8888,
        code => Fourcc(code),
    }
}

pub(crate) fn bytes_per_pixel(format: Fourcc) -> Option<usize> {
    Some(match format {
        Fourcc::C8 | Fourcc::R8 | Fourcc::RGB332 | Fourcc::BGR233 => 1,
        Fourcc::ARGB4444
        | Fourcc::XRGB4444
        | Fourcc::ABGR4444
        | Fourcc::XBGR4444
        | Fourcc::RGBA4444
        | Fourcc::RGBX4444
        | Fourcc::BGRA4444
        | Fourcc::BGRX4444
        | Fourcc::ARGB1555
        | Fourcc::XRGB1555
        | Fourcc::ABGR1555
        | Fourcc::XBGR1555
        | Fourcc::RGBA5551
        | Fourcc::RGBX5551
        | Fourcc::BGRA5551
        | Fourcc::BGRX5551
        | Fourcc::RGB565
        | Fourcc::BGR565
        | Fourcc::R16
        | Fourcc::RG88
        | Fourcc::GR88 => 2,
        Fourcc::RGB888 | Fourcc::BGR888 => 3,
        Fourcc::ARGB8888
        | Fourcc::XRGB8888
        | Fourcc::ABGR8888
        | Fourcc::XBGR8888
        | Fourcc::RGBA8888
        | Fourcc::RGBX8888
        | Fourcc::BGRA8888
        | Fourcc::BGRX8888
        | Fourcc::ARGB2101010
        | Fourcc::XRGB2101010
        | Fourcc::ABGR2101010
        | Fourcc::XBGR2101010
        | Fourcc::RGBA1010102
        | Fourcc::RGBX1010102
        | Fourcc::BGRA1010102
        | Fourcc::BGRX1010102
        | Fourcc::RG1616
        | Fourcc::GR1616
        | Fourcc::AYUV
        | Fourcc::XYUV8888 => 4,
        Fourcc::ARGB16161616F
        | Fourcc::XRGB16161616F
        | Fourcc::ABGR16161616F
        | Fourcc::XBGR16161616F
        | Fourcc::AXBXGXRX106106106106 => 8,
        _ => return None,
    })
}

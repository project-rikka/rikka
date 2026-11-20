use std::fmt;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Fourcc(pub u32);

impl Fourcc {
    pub const ABGR1555: Self = Self::from_code(*b"AB15");
    pub const ABGR16161616F: Self = Self::from_code(*b"AB4H");
    pub const ABGR2101010: Self = Self::from_code(*b"AB30");
    pub const ABGR4444: Self = Self::from_code(*b"AB12");
    pub const ABGR8888: Self = Self::from_code(*b"AB24");
    pub const ARGB1555: Self = Self::from_code(*b"AR15");
    pub const ARGB16161616F: Self = Self::from_code(*b"AR4H");
    pub const ARGB2101010: Self = Self::from_code(*b"AR30");
    pub const ARGB4444: Self = Self::from_code(*b"AR12");
    pub const ARGB8888: Self = Self::from_code(*b"AR24");
    pub const AXBXGXRX106106106106: Self = Self::from_code(*b"AB10");
    pub const AYUV: Self = Self::from_code(*b"AYUV");
    pub const BGR233: Self = Self::from_code(*b"BGR8");
    pub const BGR565: Self = Self::from_code(*b"BG16");
    pub const BGR888: Self = Self::from_code(*b"BG24");
    pub const BGRA1010102: Self = Self::from_code(*b"BA30");
    pub const BGRA4444: Self = Self::from_code(*b"BA12");
    pub const BGRA5551: Self = Self::from_code(*b"BA15");
    pub const BGRA8888: Self = Self::from_code(*b"BA24");
    pub const BGRX1010102: Self = Self::from_code(*b"BX30");
    pub const BGRX4444: Self = Self::from_code(*b"BX12");
    pub const BGRX5551: Self = Self::from_code(*b"BX15");
    pub const BGRX8888: Self = Self::from_code(*b"BX24");
    pub const C8: Self = Self::from_code(*b"C8  ");
    pub const GR1616: Self = Self::from_code(*b"GR32");
    pub const GR88: Self = Self::from_code(*b"GR88");
    pub const NV12: Self = Self::from_code(*b"NV12");
    pub const NV15: Self = Self::from_code(*b"NV15");
    pub const NV16: Self = Self::from_code(*b"NV16");
    pub const NV21: Self = Self::from_code(*b"NV21");
    pub const NV24: Self = Self::from_code(*b"NV24");
    pub const NV42: Self = Self::from_code(*b"NV42");
    pub const NV61: Self = Self::from_code(*b"NV61");
    pub const P010: Self = Self::from_code(*b"P010");
    pub const P012: Self = Self::from_code(*b"P012");
    pub const P016: Self = Self::from_code(*b"P016");
    pub const P210: Self = Self::from_code(*b"P210");
    pub const Q401: Self = Self::from_code(*b"Q401");
    pub const Q410: Self = Self::from_code(*b"Q410");
    pub const R16: Self = Self::from_code(*b"R16 ");
    pub const R8: Self = Self::from_code(*b"R8  ");
    pub const RG1616: Self = Self::from_code(*b"RG32");
    pub const RG88: Self = Self::from_code(*b"RG88");
    pub const RGB332: Self = Self::from_code(*b"RGB8");
    pub const RGB565: Self = Self::from_code(*b"RG16");
    pub const RGB888: Self = Self::from_code(*b"RG24");
    pub const RGBA1010102: Self = Self::from_code(*b"RA30");
    pub const RGBA4444: Self = Self::from_code(*b"RA12");
    pub const RGBA5551: Self = Self::from_code(*b"RA15");
    pub const RGBA8888: Self = Self::from_code(*b"RA24");
    pub const RGBX1010102: Self = Self::from_code(*b"RX30");
    pub const RGBX4444: Self = Self::from_code(*b"RX12");
    pub const RGBX5551: Self = Self::from_code(*b"RX15");
    pub const RGBX8888: Self = Self::from_code(*b"RX24");
    pub const UYVY: Self = Self::from_code(*b"UYVY");
    pub const VUY101010: Self = Self::from_code(*b"VU30");
    pub const VUY888: Self = Self::from_code(*b"VU24");
    pub const VYUY: Self = Self::from_code(*b"VYUY");
    pub const X0L0: Self = Self::from_code(*b"X0L0");
    pub const X0L2: Self = Self::from_code(*b"X0L2");
    pub const XBGR1555: Self = Self::from_code(*b"XB15");
    pub const XBGR16161616F: Self = Self::from_code(*b"XB4H");
    pub const XBGR2101010: Self = Self::from_code(*b"XB30");
    pub const XBGR4444: Self = Self::from_code(*b"XB12");
    pub const XBGR8888: Self = Self::from_code(*b"XB24");
    pub const XRGB1555: Self = Self::from_code(*b"XR15");
    pub const XRGB16161616F: Self = Self::from_code(*b"XR4H");
    pub const XRGB2101010: Self = Self::from_code(*b"XR30");
    pub const XRGB4444: Self = Self::from_code(*b"XR12");
    pub const XRGB8888: Self = Self::from_code(*b"XR24");
    pub const XVYU12_16161616: Self = Self::from_code(*b"XV36");
    pub const XVYU16161616: Self = Self::from_code(*b"XV48");
    pub const XVYU2101010: Self = Self::from_code(*b"XV30");
    pub const XYUV8888: Self = Self::from_code(*b"XYUV");
    pub const Y0L0: Self = Self::from_code(*b"Y0L0");
    pub const Y0L2: Self = Self::from_code(*b"Y0L2");
    pub const Y210: Self = Self::from_code(*b"Y210");
    pub const Y212: Self = Self::from_code(*b"Y212");
    pub const Y216: Self = Self::from_code(*b"Y216");
    pub const Y410: Self = Self::from_code(*b"Y410");
    pub const Y412: Self = Self::from_code(*b"Y412");
    pub const Y416: Self = Self::from_code(*b"Y416");
    pub const YUV410: Self = Self::from_code(*b"YUV9");
    pub const YUV411: Self = Self::from_code(*b"YU11");
    pub const YUV420: Self = Self::from_code(*b"YU12");
    pub const YUV420_10BIT: Self = Self::from_code(*b"YU10");
    pub const YUV420_8BIT: Self = Self::from_code(*b"YU08");
    pub const YUV422: Self = Self::from_code(*b"YU16");
    pub const YUV444: Self = Self::from_code(*b"YU24");
    pub const YUYV: Self = Self::from_code(*b"YUYV");
    pub const YVU410: Self = Self::from_code(*b"YVU9");
    pub const YVU411: Self = Self::from_code(*b"YV11");
    pub const YVU420: Self = Self::from_code(*b"YV12");
    pub const YVU422: Self = Self::from_code(*b"YV16");
    pub const YVU444: Self = Self::from_code(*b"YV24");
    pub const YVYU: Self = Self::from_code(*b"YVYU");

    pub const fn from_code(code: [u8; 4]) -> Self {
        Self(u32::from_le_bytes(code))
    }
}

impl fmt::Debug for Fourcc {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}

impl fmt::Display for Fourcc {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let code: String = self
            .0
            .to_le_bytes()
            .iter()
            .map(|byte| match byte.is_ascii_graphic() {
                true => char::from(*byte),
                false => '?',
            })
            .collect();
        formatter.write_str(code.trim_end())
    }
}

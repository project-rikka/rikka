use rikka_compositor_types::Fourcc;

pub(crate) fn without_alpha(format: Fourcc) -> bool {
    matches!(
        format,
        Fourcc::XRGB8888
            | Fourcc::XBGR8888
            | Fourcc::RGBX8888
            | Fourcc::BGRX8888
            | Fourcc::XRGB2101010
            | Fourcc::XBGR2101010
            | Fourcc::RGBX1010102
            | Fourcc::BGRX1010102
            | Fourcc::XRGB16161616F
            | Fourcc::XBGR16161616F
            | Fourcc::XRGB4444
            | Fourcc::XBGR4444
            | Fourcc::RGBX4444
            | Fourcc::BGRX4444
            | Fourcc::XRGB1555
            | Fourcc::XBGR1555
            | Fourcc::RGBX5551
            | Fourcc::BGRX5551
            | Fourcc::RGB888
            | Fourcc::BGR888
            | Fourcc::RGB565
            | Fourcc::BGR565
            | Fourcc::RGB332
            | Fourcc::BGR233
            | Fourcc::NV12
            | Fourcc::NV15
            | Fourcc::NV16
            | Fourcc::NV21
            | Fourcc::NV24
            | Fourcc::NV42
            | Fourcc::NV61
            | Fourcc::P010
            | Fourcc::P012
            | Fourcc::P016
            | Fourcc::P210
            | Fourcc::YUYV
            | Fourcc::YVYU
            | Fourcc::UYVY
            | Fourcc::VYUY
            | Fourcc::XYUV8888
            | Fourcc::VUY888
            | Fourcc::VUY101010
            | Fourcc::YUV410
            | Fourcc::YUV411
            | Fourcc::YUV420
            | Fourcc::YUV422
            | Fourcc::YUV444
            | Fourcc::YVU410
            | Fourcc::YVU411
            | Fourcc::YVU420
            | Fourcc::YVU422
            | Fourcc::YVU444
    )
}

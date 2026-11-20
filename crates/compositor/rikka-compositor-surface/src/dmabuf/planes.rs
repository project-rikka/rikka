use rikka_compositor_types::Fourcc;

pub(super) fn plane_count(format: Fourcc) -> usize {
    match format {
        Fourcc::NV12
        | Fourcc::NV21
        | Fourcc::NV15
        | Fourcc::NV16
        | Fourcc::NV61
        | Fourcc::NV24
        | Fourcc::NV42
        | Fourcc::P010
        | Fourcc::P012
        | Fourcc::P016
        | Fourcc::P210 => 2,
        Fourcc::YUV410
        | Fourcc::YVU410
        | Fourcc::YUV411
        | Fourcc::YVU411
        | Fourcc::YUV420
        | Fourcc::YVU420
        | Fourcc::YUV422
        | Fourcc::YVU422
        | Fourcc::YUV444
        | Fourcc::YVU444
        | Fourcc::Q410
        | Fourcc::Q401 => 3,
        _ => 1,
    }
}

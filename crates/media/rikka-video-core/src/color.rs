use rikka_ffmpeg::avutil::{self, AVColorSpace};

pub fn matrix(colorspace: AVColorSpace, frame_height: i32) -> AVColorSpace {
    match colorspace {
        avutil::AVCOL_SPC_BT709
        | avutil::AVCOL_SPC_BT470BG
        | avutil::AVCOL_SPC_SMPTE170M
        | avutil::AVCOL_SPC_SMPTE240M
        | avutil::AVCOL_SPC_BT2020_NCL
        | avutil::AVCOL_SPC_BT2020_CL => colorspace,
        _ if frame_height <= 576 => avutil::AVCOL_SPC_BT470BG,
        _ => avutil::AVCOL_SPC_BT709,
    }
}

pub fn ycbcr_matrix(
    colorspace: i32,
    color_range: i32,
    bit_depth: u32,
    storage_scale: f64,
    frame_height: i32,
) -> [[f32; 4]; 3] {
    let (kr, kb) = match matrix(colorspace, frame_height) {
        avutil::AVCOL_SPC_BT709 => (0.2126, 0.0722),
        avutil::AVCOL_SPC_SMPTE240M => (0.212, 0.087),
        avutil::AVCOL_SPC_BT2020_NCL | avutil::AVCOL_SPC_BT2020_CL => (0.2627, 0.0593),
        _ => (0.299, 0.114),
    };
    let full = color_range == avutil::AVCOL_RANGE_JPEG;
    rikka_gpu::ycbcr_matrix(kr, kb, bit_depth, storage_scale, full)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Shader {
    Adjust,
    Arith,
    Blend,
    Box,
    Convert,
    Copy,
    Denoise,
    Dither,
    Gauss,
    Histogram,
    Kuwahara,
    Lens,
    Linear,
    Marks,
    Mask,
    Median,
    Mosaic,
    Motion,
    Noise,
    Resample,
    Sharpen,
    Stylize,
    Surface,
    Tonal,
    Warp,
    Gradient,
    Place,
    Key,
    Matte,
    Differ,
    Fractal,
    Distort,
}

impl Shader {
    pub(crate) const COUNT: usize = 32;

    pub(crate) fn index(self) -> usize {
        self as usize
    }

    pub(crate) fn compiled(self) -> rikka_spirv::Shader {
        match self {
            Shader::Adjust => kernels::ADJUST_COMP,
            Shader::Arith => kernels::ARITH_COMP,
            Shader::Blend => kernels::BLEND_COMP,
            Shader::Box => kernels::BOX_COMP,
            Shader::Convert => kernels::CONVERT_COMP,
            Shader::Copy => kernels::COPY_COMP,
            Shader::Denoise => kernels::DENOISE_COMP,
            Shader::Dither => kernels::DITHER_COMP,
            Shader::Gauss => kernels::GAUSS_COMP,
            Shader::Histogram => kernels::HISTOGRAM_COMP,
            Shader::Kuwahara => kernels::KUWAHARA_COMP,
            Shader::Lens => kernels::LENS_COMP,
            Shader::Linear => kernels::LINEAR_COMP,
            Shader::Marks => kernels::MARKS_COMP,
            Shader::Mask => kernels::MASK_COMP,
            Shader::Median => kernels::MEDIAN_COMP,
            Shader::Mosaic => kernels::MOSAIC_COMP,
            Shader::Motion => kernels::MOTION_COMP,
            Shader::Noise => kernels::NOISE_COMP,
            Shader::Resample => kernels::RESAMPLE_COMP,
            Shader::Sharpen => kernels::SHARPEN_COMP,
            Shader::Stylize => kernels::STYLIZE_COMP,
            Shader::Surface => kernels::SURFACE_COMP,
            Shader::Tonal => kernels::TONAL_COMP,
            Shader::Warp => kernels::WARP_COMP,
            Shader::Gradient => kernels::GRADIENT_COMP,
            Shader::Place => kernels::PLACE_COMP,
            Shader::Key => kernels::KEY_COMP,
            Shader::Matte => kernels::MATTE_COMP,
            Shader::Differ => kernels::DIFFER_COMP,
            Shader::Fractal => kernels::FRACTAL_COMP,
            Shader::Distort => kernels::DISTORT_COMP,
        }
    }
}

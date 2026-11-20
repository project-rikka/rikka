use rikka_compositor_protocols::wp_color_representation_surface_v1::{
    AlphaMode, ChromaLocation, Coefficients, Range,
};
use rikka_compositor_types::Fourcc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepresentationSupport {
    pub alpha_modes: Vec<AlphaMode>,
    pub coefficients: Vec<(Coefficients, Range)>,
}

impl Default for RepresentationSupport {
    fn default() -> Self {
        Self {
            alpha_modes: vec![AlphaMode::PremultipliedElectrical],
            coefficients: vec![(Coefficients::Identity, Range::Full)],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ColorRepresentation {
    pub alpha_mode: Option<AlphaMode>,
    pub coefficients: Option<(Coefficients, Range)>,
    pub chroma_location: Option<ChromaLocation>,
}

pub(crate) fn compatible(representation: &ColorRepresentation, format: Fourcc) -> bool {
    let ycbcr = is_ycbcr(format);
    let coefficients = representation
        .coefficients
        .is_none_or(|(coefficients, _)| (coefficients == Coefficients::Identity) != ycbcr);
    let chroma = representation.chroma_location.is_none() || is_420(format);
    coefficients && chroma
}

fn is_420(format: Fourcc) -> bool {
    matches!(
        format,
        Fourcc::NV12
            | Fourcc::NV21
            | Fourcc::NV15
            | Fourcc::P010
            | Fourcc::P012
            | Fourcc::P016
            | Fourcc::YUV420
            | Fourcc::YVU420
            | Fourcc::YUV420_8BIT
            | Fourcc::YUV420_10BIT
    )
}

fn is_ycbcr(format: Fourcc) -> bool {
    is_420(format)
        || matches!(
            format,
            Fourcc::YUYV
                | Fourcc::YVYU
                | Fourcc::UYVY
                | Fourcc::VYUY
                | Fourcc::AYUV
                | Fourcc::XYUV8888
                | Fourcc::VUY888
                | Fourcc::VUY101010
                | Fourcc::Y210
                | Fourcc::Y212
                | Fourcc::Y216
                | Fourcc::Y410
                | Fourcc::Y412
                | Fourcc::Y416
                | Fourcc::XVYU2101010
                | Fourcc::XVYU12_16161616
                | Fourcc::XVYU16161616
                | Fourcc::Y0L0
                | Fourcc::X0L0
                | Fourcc::Y0L2
                | Fourcc::X0L2
                | Fourcc::NV16
                | Fourcc::NV61
                | Fourcc::NV24
                | Fourcc::NV42
                | Fourcc::P210
                | Fourcc::Q410
                | Fourcc::Q401
                | Fourcc::YUV410
                | Fourcc::YVU410
                | Fourcc::YUV411
                | Fourcc::YVU411
                | Fourcc::YUV422
                | Fourcc::YVU422
                | Fourcc::YUV444
                | Fourcc::YVU444
        )
}

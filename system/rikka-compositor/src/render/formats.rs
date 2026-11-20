use rikka_compositor_types::{Format, Fourcc, Modifier};
use rikka_gpu::DrmFormat as RendererFormat;

pub(crate) fn renderer_format_to_kms(format: &RendererFormat) -> Format {
    Format::new(Fourcc(format.fourcc), Modifier(format.modifier))
}

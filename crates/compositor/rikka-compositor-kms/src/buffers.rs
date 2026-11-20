use rikka_compositor_types::{FormatSet, Fourcc};

use crate::dmabuf::Dmabuf;

#[derive(Debug)]
pub enum Buffers {
    Rendered(FormatSet),
    Shared(FormatSet),
    Imported(Vec<Dmabuf>),
    Mapped(Vec<Fourcc>),
}

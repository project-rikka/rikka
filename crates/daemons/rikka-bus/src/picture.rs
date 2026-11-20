use std::sync::Arc;

#[derive(Clone, Debug, PartialEq)]
pub struct Picture {
    pub width: u32,
    pub height: u32,
    pub pixels: Arc<[u8]>,
}

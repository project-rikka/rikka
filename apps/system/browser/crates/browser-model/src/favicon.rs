use std::sync::Arc;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Favicon {
    pub width: u32,
    pub height: u32,
    pub rgba: Arc<[u8]>,
    pub svg: Option<Arc<[u8]>>,
}

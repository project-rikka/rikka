use crate::cff::CffCache;
use rikka_types::Point;

#[derive(Debug, Default)]
pub struct OutlineScratch {
    pub(crate) points: Vec<Point>,
    pub(crate) flags: Vec<u8>,
    pub(crate) contour_ends: Vec<usize>,
    pub(crate) local_ends: Vec<u16>,
    pub(crate) deltas: Vec<Point>,
    pub(crate) tuple_deltas: Vec<Point>,
    pub(crate) touched: Vec<bool>,
    pub(crate) component_deltas: Vec<Point>,
    pub(crate) cff: Option<Box<CffCache>>,
}

impl OutlineScratch {
    pub fn new() -> Self {
        Self::default()
    }

    pub(crate) fn clear(&mut self) {
        self.points.clear();
        self.flags.clear();
        self.contour_ends.clear();
        self.component_deltas.clear();
    }
}

use rikka_text_font::Tag;

use crate::face::attributes::{Axis, AxisRange, ColorFormats, Slope};

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FaceRecord {
    pub(crate) index: u32,
    pub(crate) families: Vec<Box<str>>,
    pub(crate) style_name: Box<str>,
    pub(crate) full_name: Box<str>,
    pub(crate) weight: AxisRange,
    pub(crate) width: AxisRange,
    pub(crate) slope: Slope,
    pub(crate) monospace: bool,
    pub(crate) color: ColorFormats,
    pub(crate) axes: Vec<Axis>,
    pub(crate) scripts: Vec<Tag>,
    pub(crate) coverage: Vec<(u16, u32)>,
}

use std::cell::Cell;
use std::collections::HashMap;
use std::path::PathBuf;

use crate::budget::Budget;
use crate::parse::media::ColorScheme;
use crate::tree::PictureContent;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bitmap {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

pub type Decoder<'d> = dyn Fn(&[u8]) -> Option<Bitmap> + 'd;

pub(crate) type Prefetched = HashMap<String, Option<PictureContent>>;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum SvgGlyphs {
    #[default]
    Embedded,
    Skipped,
}

#[derive(Clone, Copy)]
pub(crate) struct Sources<'s> {
    pub(crate) folder: Option<&'s PathBuf>,
    pub(crate) decoder: Option<&'s Decoder<'s>>,
    pub(crate) prefetched: Option<&'s Prefetched>,
    pub(crate) scheme: ColorScheme,
    pub(crate) svg_glyphs: SvgGlyphs,
    pub(crate) scheme_asked: &'s Cell<bool>,
    pub(crate) depth: usize,
    pub(crate) budget: &'s Budget,
}

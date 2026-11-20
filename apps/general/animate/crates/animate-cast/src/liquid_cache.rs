use std::ops::Range;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LiquidCache {
    pub ready: Vec<Range<f32>>,
    pub computing: Option<Range<f32>>,
}

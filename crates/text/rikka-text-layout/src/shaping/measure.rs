#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Measure {
    pub(crate) em: f32,
    pub(crate) px: f32,
    pub(crate) spaced_clusters: u32,
}

impl Measure {
    #[inline]
    pub(crate) fn add(&mut self, advance: f32, size: Option<f32>, ends_cluster: bool) {
        match size {
            Some(size) => self.px += advance * size,
            None => self.em += advance,
        }
        self.spaced_clusters += u32::from(ends_cluster);
    }

    #[inline]
    pub(crate) fn append(&mut self, other: Measure) {
        self.em += other.em;
        self.px += other.px;
        self.spaced_clusters += other.spaced_clusters;
    }

    #[inline]
    pub(crate) fn to_px(self, font_size: f32, letter_spacing: f32) -> f32 {
        self.em * font_size + self.px + self.spaced_clusters as f32 * letter_spacing
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sample<'a> {
    pub data: &'a [u8],
    pub pts_us: i64,
    pub dts_us: i64,
    pub duration_us: i64,
    pub keyframe: bool,
}

impl Sample<'_> {
    #[must_use]
    pub fn shifted(self, by_us: i64) -> Self {
        Sample {
            pts_us: self.pts_us - by_us,
            dts_us: self.dts_us - by_us,
            ..self
        }
    }
}

use std::sync::Arc;

use crate::sample::Sample;

#[derive(Clone, PartialEq, Eq)]
pub struct Coded {
    pub data: Arc<[u8]>,
    pub pts_us: i64,
    pub dts_us: i64,
    pub duration_us: i64,
    pub keyframe: bool,
}

impl Coded {
    #[must_use]
    pub fn sample(&self) -> Sample<'_> {
        Sample {
            data: &self.data,
            pts_us: self.pts_us,
            dts_us: self.dts_us,
            duration_us: self.duration_us,
            keyframe: self.keyframe,
        }
    }

    #[must_use]
    pub fn end_us(&self) -> i64 {
        self.pts_us + self.duration_us.max(0)
    }

    #[must_use]
    pub fn shifted(&self, by_us: i64) -> Coded {
        Coded {
            pts_us: self.pts_us - by_us,
            dts_us: self.dts_us - by_us,
            ..self.clone()
        }
    }
}

impl std::fmt::Debug for Coded {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Coded")
            .field("pts_us", &self.pts_us)
            .field("dts_us", &self.dts_us)
            .field("duration_us", &self.duration_us)
            .field("keyframe", &self.keyframe)
            .field("bytes", &self.data.len())
            .finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Stage {
    Metadata,
    Java,
    Libraries,
    Assets,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Progress {
    pub stage: Stage,
    pub bytes: u64,
    pub total_bytes: u64,
    pub files: usize,
    pub total_files: usize,
}

impl Progress {
    #[must_use]
    pub fn starting(stage: Stage) -> Progress {
        Progress {
            stage,
            bytes: 0,
            total_bytes: 0,
            files: 0,
            total_files: 0,
        }
    }

    #[must_use]
    pub fn fraction(&self) -> Option<f32> {
        if self.total_bytes > 0 {
            return Some((self.bytes as f64 / self.total_bytes as f64) as f32);
        }
        (self.total_files > 0).then(|| self.files as f32 / self.total_files as f32)
    }
}

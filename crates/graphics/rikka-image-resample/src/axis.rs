#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Axis {
    pub source_len: usize,
    pub target_len: usize,
    pub origin: f32,
    pub scale: f32,
}

impl Axis {
    #[must_use]
    pub fn fit(source_len: usize, target_len: usize) -> Axis {
        Axis {
            source_len,
            target_len,
            origin: 0.0,
            scale: target_len as f32 / source_len.max(1) as f32,
        }
    }

    #[must_use]
    pub fn place(start: f32, source_len: usize, scale: f32) -> (i32, Axis) {
        let end = start + source_len as f32 * scale;
        let first = start.floor();
        let last = end.ceil();
        (
            first as i32,
            Axis {
                source_len,
                target_len: (last - first).max(0.0) as usize,
                origin: start - first,
                scale,
            },
        )
    }
}

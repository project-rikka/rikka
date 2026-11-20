#[derive(Clone, PartialEq, Eq)]
pub struct Preview {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl std::fmt::Debug for Preview {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Preview")
            .field("width", &self.width)
            .field("height", &self.height)
            .finish_non_exhaustive()
    }
}

pub(crate) fn fitted(
    (width, height): (u32, u32),
    (most_width, most_height): (u32, u32),
) -> (u32, u32) {
    let scale = (f64::from(most_width) / f64::from(width.max(1)))
        .min(f64::from(most_height) / f64::from(height.max(1)))
        .min(1.0);
    let side = |length: u32| ((f64::from(length) * scale).round() as u32).max(1);
    (side(width), side(height))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Fitted {
    pub(crate) offset: (u32, u32),
    pub(crate) extent: (u32, u32),
}

impl Fitted {
    pub(crate) fn compute(
        (source_width, source_height): (u32, u32),
        (width, height): (u32, u32),
    ) -> Fitted {
        let whole = source_width <= width + 1 && source_height <= height + 1;
        let extent = match whole {
            true => (source_width.min(width), source_height.min(height)),
            false => {
                let scale = (f64::from(width) / f64::from(source_width))
                    .min(f64::from(height) / f64::from(source_height));
                let side = |length: u32, most: u32| {
                    ((f64::from(length) * scale).round() as u32).clamp(1, most)
                };
                (side(source_width, width), side(source_height, height))
            }
        };
        Fitted {
            offset: (
                ((width - extent.0) / 2) & !1,
                ((height - extent.1) / 2) & !1,
            ),
            extent,
        }
    }
}

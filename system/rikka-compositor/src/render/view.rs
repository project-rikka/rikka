use rikka_compositor_geometry::{Logical, Physical, Rect, Size};
use rikka_types::{CornerRadii, Rect as SceneRect, RoundedRect};

use crate::outputs::Output;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct View {
    pub(crate) area: Rect<i32, Logical>,
    pub(crate) scale: f64,
}

impl View {
    pub(crate) fn from_output(output: &Output) -> Option<Self> {
        Some(Self {
            area: output.area()?,
            scale: output.scale(),
        })
    }

    pub(crate) fn rect(self, rect: Rect<i32, Logical>) -> SceneRect {
        self.exact(rect.to_f64())
    }

    pub(crate) fn screen(self) -> Rect<i32, Physical> {
        let extent = |length: i32| (f64::from(length) * self.scale).ceil() as i32;
        Rect::from_size(Size::new(
            extent(self.area.size.w),
            extent(self.area.size.h),
        ))
    }

    pub(crate) fn exact(self, rect: Rect<f64, Logical>) -> SceneRect {
        let scale = self.scale as f32;
        SceneRect::new(
            (rect.loc.x - f64::from(self.area.loc.x)) as f32 * scale,
            (rect.loc.y - f64::from(self.area.loc.y)) as f32 * scale,
            rect.size.w as f32 * scale,
            rect.size.h as f32 * scale,
        )
    }

    pub(crate) fn radii(self, radii: CornerRadii) -> CornerRadii {
        let scale = self.scale as f32;
        CornerRadii::new(
            radii.top_left * scale,
            radii.top_right * scale,
            radii.bottom_right * scale,
            radii.bottom_left * scale,
        )
    }

    pub(crate) fn shape(self, logical: RoundedRect) -> RoundedRect {
        let scale = self.scale as f32;
        let rect = logical.rect;
        RoundedRect::new(
            SceneRect::new(
                (rect.x() - self.area.loc.x as f32) * scale,
                (rect.y() - self.area.loc.y as f32) * scale,
                rect.width() * scale,
                rect.height() * scale,
            ),
            self.radii(logical.radii),
        )
    }

    pub(crate) fn measure(self, length: f32) -> f32 {
        (length * self.scale as f32).max(1.0)
    }
}

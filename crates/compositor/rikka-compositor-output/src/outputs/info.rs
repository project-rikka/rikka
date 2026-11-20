use rikka_compositor_geometry::{Logical, Physical, Point, Rect, Size, Transform};
use rikka_compositor_protocols::wl_output::Subpixel;
use rikka_compositor_types::OutputLayout;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Millimetres {
    pub width: i32,
    pub height: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Mode {
    pub size: Size<i32, Physical>,
    pub refresh_millihertz: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OutputInfo {
    pub description: String,
    pub make: String,
    pub model: String,
    pub serial: Option<String>,
    pub physical_size: Millimetres,
    pub subpixel: Subpixel,
    pub transform: Transform,
    pub mode: Mode,
    pub scale: f64,
    pub position: Point<i32, Logical>,
}

impl OutputInfo {
    pub fn logical_size(&self) -> Size<i32, Logical> {
        let size = self.mode.size.transformed(self.transform);
        let scale = self.scale_or_one();
        Size::new(
            (f64::from(size.w) / scale).round() as i32,
            (f64::from(size.h) / scale).round() as i32,
        )
    }

    pub fn layout(&self) -> OutputLayout {
        OutputLayout {
            geometry: Rect::new(self.position, self.logical_size()),
            scale: self.scale,
            transform: self.transform,
        }
    }

    pub fn integer_scale(&self) -> i32 {
        self.scale_or_one().ceil() as i32
    }

    fn scale_or_one(&self) -> f64 {
        if self.scale.is_finite() && self.scale > 0.0 {
            self.scale
        } else {
            1.0
        }
    }
}

impl Default for OutputInfo {
    fn default() -> Self {
        Self {
            description: String::new(),
            make: String::new(),
            model: String::new(),
            serial: None,
            physical_size: Millimetres::default(),
            subpixel: Subpixel::Unknown,
            transform: Transform::Normal,
            mode: Mode::default(),
            scale: 1.0,
            position: Point::zero(),
        }
    }
}

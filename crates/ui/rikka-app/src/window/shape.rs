use rikka_types::Size;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WindowShape {
    pub size: Size,
    pub restored: Size,
    pub maximized: bool,
    pub fullscreen: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Shaping {
    reported: Option<WindowShape>,
    restored: Size,
}

impl Shaping {
    pub(crate) fn new(initial: Size) -> Shaping {
        Shaping {
            reported: None,
            restored: initial,
        }
    }

    pub(crate) fn update(
        &mut self,
        size: Size,
        maximized: bool,
        fullscreen: bool,
    ) -> Option<WindowShape> {
        if size.width <= 0.0 || size.height <= 0.0 {
            return None;
        }
        if !maximized && !fullscreen {
            self.restored = size;
        }
        let shape = WindowShape {
            size,
            restored: self.restored,
            maximized,
            fullscreen,
        };
        if self.reported == Some(shape) {
            return None;
        }
        self.reported = Some(shape);
        Some(shape)
    }
}

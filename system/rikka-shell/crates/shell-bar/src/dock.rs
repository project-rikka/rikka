use rikka_settings::session::appearance::TaskbarPosition;
use rikka_types::{Point, Rect, Size};
use rikka_ui::Axis;
use shell_kit::taskbar::{Edge, Shape};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Dock {
    shape: Shape,
    output: Size,
}

impl Dock {
    #[must_use]
    pub(crate) fn new(shape: Shape, output: Size) -> Dock {
        Dock { shape, output }
    }

    #[must_use]
    pub(crate) fn on(self, output: Size) -> Dock {
        Dock { output, ..self }
    }

    #[must_use]
    pub(crate) fn edge(self) -> Edge {
        self.shape.edge()
    }

    #[must_use]
    pub(crate) fn axis(self) -> Axis {
        self.edge().axis()
    }

    #[must_use]
    pub(crate) fn is_vertical(self) -> bool {
        self.edge().is_vertical()
    }

    #[must_use]
    pub(crate) fn length(self) -> f32 {
        self.axis().length(self.output)
    }

    #[must_use]
    pub(crate) fn surface(self) -> Size {
        let extent = self.shape.extent();
        match self.is_vertical() {
            true => Size::new(extent, self.output.height),
            false => Size::new(self.output.width, extent),
        }
    }

    #[must_use]
    pub(crate) fn on_output(self, rect: Rect) -> Rect {
        let surface = self.surface();
        let origin = match self.edge().position() {
            TaskbarPosition::Top | TaskbarPosition::Left => Point::ZERO,
            TaskbarPosition::Bottom => Point::new(0.0, self.output.height - surface.height),
            TaskbarPosition::Right => Point::new(self.output.width - surface.width, 0.0),
        };
        rect.translate(origin)
    }

    #[must_use]
    pub(crate) fn facing(self, rect: Rect) -> Point {
        let rect = self.on_output(rect);
        let centre = rect.center();
        match self.edge().position() {
            TaskbarPosition::Top => Point::new(centre.x, rect.max_y()),
            TaskbarPosition::Bottom => Point::new(centre.x, rect.y()),
            TaskbarPosition::Left => Point::new(rect.max_x(), centre.y),
            TaskbarPosition::Right => Point::new(rect.x(), centre.y),
        }
    }
}

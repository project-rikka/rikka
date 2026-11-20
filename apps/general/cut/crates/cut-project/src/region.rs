use std::ops::RangeInclusive;

use rikka_figure::{Affine, Form, Frame, Grips, Look, Shape};
use rikka_image_edit::Path;
use rikka_types::{Color, Point, Rect, Size};

use crate::placement::{Placement, side};
use crate::project::Format;

const MIN_SCALE: f32 = 0.01;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Area {
    Whole,
    Shape {
        shape: Shape,
        width: f32,
        height: f32,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Region {
    pub placement: Placement,
    pub area: Area,
    pub feather: f32,
    pub inverted: bool,
}

impl Region {
    pub const FEATHERS: RangeInclusive<f32> = 0.0..=1000.0;

    #[must_use]
    pub fn whole() -> Region {
        Region {
            placement: Placement::default(),
            area: Area::Whole,
            feather: 0.0,
            inverted: false,
        }
    }

    #[must_use]
    pub fn shaped(shape: Shape, width: f32, height: f32) -> Region {
        Region {
            area: Area::Shape {
                shape,
                width,
                height,
            },
            ..Region::whole()
        }
    }

    #[must_use]
    pub fn with_placement(self, placement: Placement) -> Region {
        Region { placement, ..self }
    }

    #[must_use]
    pub fn checked(self) -> Region {
        Region {
            placement: self.placement.checked(),
            area: match self.area {
                Area::Whole => Area::Whole,
                Area::Shape {
                    shape,
                    width,
                    height,
                } => Area::Shape {
                    shape: shape.checked(),
                    width: side(width),
                    height: side(height),
                },
            },
            feather: match self.feather.is_finite() {
                true => self
                    .feather
                    .clamp(*Region::FEATHERS.start(), *Region::FEATHERS.end()),
                false => 0.0,
            },
            inverted: self.inverted,
        }
    }

    #[must_use]
    pub fn is_whole(&self) -> bool {
        matches!(self.area, Area::Whole)
    }

    #[must_use]
    pub fn frame(&self, format: Format) -> Frame {
        match self.area {
            Area::Whole => Frame::new(format.center(), format.width as f32, format.height as f32),
            Area::Shape { width, height, .. } => {
                self.placement.frame(Size::new(width, height), format)
            }
        }
    }

    #[must_use]
    pub fn path(&self, format: Format) -> Option<Path> {
        let Area::Shape { shape, .. } = self.area else {
            return None;
        };
        let shape = match shape {
            Shape::Rectangle { radius } => Shape::Rectangle {
                radius: radius * self.placement.unzoomed(),
            },
            other => other,
        };
        Some(shape.path(&self.frame(format)))
    }

    #[must_use]
    pub fn grips(&self, format: Format) -> Option<Grips> {
        match self.area {
            Area::Whole => None,
            Area::Shape { .. } => Some(Grips::Frame {
                frame: self.frame(format),
                sides: true,
                turns: true,
            }),
        }
    }

    #[must_use]
    pub fn bounds(&self, format: Format) -> Option<Rect> {
        match self.area {
            Area::Whole => None,
            Area::Shape { .. } => Some(self.frame(format).bounds()),
        }
    }

    #[must_use]
    pub fn hits(&self, format: Format, at: Point, reach: f32) -> bool {
        let Area::Shape {
            shape,
            width,
            height,
        } = self.area
        else {
            return false;
        };
        let Some(inverse) = self.placement.affine(format).inverse() else {
            return false;
        };
        let form = Form::Shape {
            shape,
            frame: Frame::new(Point::ZERO, width, height),
            look: Look {
                fill: Some(Color::WHITE),
                outline: None,
            },
        };
        form.hits(inverse.apply(at), reach / self.placement.least_scale())
    }

    #[must_use]
    pub fn mapped(&self, map: Affine, format: Format) -> Region {
        let Area::Shape { shape, .. } = self.area else {
            return *self;
        };
        let after = self.frame(format).mapped(map);
        let (across, down) = self.placement.scale();
        Region {
            placement: self.placement.followed(&after, format),
            area: Area::Shape {
                shape,
                width: side(after.width / across.max(MIN_SCALE)),
                height: side(after.height / down.max(MIN_SCALE)),
            },
            ..*self
        }
    }

    #[must_use]
    pub fn moved(&self, by: Point) -> Region {
        match self.area {
            Area::Whole => *self,
            Area::Shape { .. } => Region {
                placement: self.placement.moved(by),
                ..*self
            },
        }
    }
}

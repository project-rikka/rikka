use rikka_types::Point;

use crate::style::{Extent, Inset, Layout, Length};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Start,
    Center,
    End,
    Stretch,
}

impl Align {
    pub fn to_justify(self) -> Justify {
        match self {
            Self::Start | Self::Stretch => Justify::Start,
            Self::Center => Justify::Center,
            Self::End => Justify::End,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Justify {
    Start,
    Center,
    End,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Style {
    pub width: Option<Length>,
    pub height: Option<Length>,
    pub min_width: Option<Length>,
    pub min_height: Option<Length>,
    pub max_width: Option<Length>,
    pub max_height: Option<Length>,
    pub grow: Option<f32>,
    pub shrink: Option<f32>,
    pub default_grow: Option<f32>,
    pub align_self: Option<Align>,
    pub margin: Option<Inset>,
    pub position: Option<Point>,
}

impl Style {
    pub fn apply(&self, layout: &mut Layout) {
        if let Some(width) = self.width {
            layout.size.width = width;
        }
        if let Some(height) = self.height {
            layout.size.height = height;
        }
        if let Some(max_width) = self.max_width {
            layout.max.width = max_width;
        }
        if let Some(max_height) = self.max_height {
            layout.max.height = max_height;
        }
        if let Some(grow) = self.grow {
            layout.grow = grow;
            if grow > 0.0 {
                layout.basis = Length::px(0.0);
                layout.min = Extent::new(Length::px(0.0), Length::px(0.0));
            }
        } else if let Some(grow) = self.default_grow
            && self.width.is_none()
            && self.height.is_none()
        {
            layout.grow = grow;
        }
        if let Some(min_width) = self.min_width {
            layout.min.width = min_width;
        }
        if let Some(min_height) = self.min_height {
            layout.min.height = min_height;
        }
        if let Some(shrink) = self.shrink {
            layout.shrink = shrink;
        }
        if let Some(align) = self.align_self {
            layout.align_self = Some(align);
        }
        if let Some(margin) = self.margin {
            layout.margin = margin;
        }
        if let Some(position) = self.position {
            layout.inset = Some(position);
        }
    }
}

pub trait Styled: Sized {
    fn style_mut(&mut self) -> &mut Style;

    #[must_use]
    fn styled(mut self, style: Style) -> Self {
        *self.style_mut() = style;
        self
    }
    #[must_use]
    fn width(mut self, width: impl Into<Length>) -> Self {
        self.style_mut().width = Some(width.into());
        self
    }

    #[must_use]
    fn height(mut self, height: impl Into<Length>) -> Self {
        self.style_mut().height = Some(height.into());
        self
    }

    #[must_use]
    fn size(self, width: impl Into<Length>, height: impl Into<Length>) -> Self {
        self.width(width).height(height)
    }

    #[must_use]
    fn min_width(mut self, width: impl Into<Length>) -> Self {
        self.style_mut().min_width = Some(width.into());
        self
    }

    #[must_use]
    fn min_height(mut self, height: impl Into<Length>) -> Self {
        self.style_mut().min_height = Some(height.into());
        self
    }

    #[must_use]
    fn max_width(mut self, width: impl Into<Length>) -> Self {
        self.style_mut().max_width = Some(width.into());
        self
    }

    #[must_use]
    fn max_height(mut self, height: impl Into<Length>) -> Self {
        self.style_mut().max_height = Some(height.into());
        self
    }

    #[must_use]
    fn fill(self) -> Self {
        self.fill_width().fill_height()
    }

    #[must_use]
    fn fill_width(self) -> Self {
        self.width(Length::FILL)
    }

    #[must_use]
    fn fill_height(self) -> Self {
        self.height(Length::FILL)
    }

    #[must_use]
    fn grow(mut self, factor: f32) -> Self {
        self.style_mut().grow = Some(factor.max(0.0));
        self
    }

    #[must_use]
    fn shrink(mut self, factor: f32) -> Self {
        self.style_mut().shrink = Some(factor.max(0.0));
        self
    }

    #[must_use]
    fn align_self(mut self, align: Align) -> Self {
        self.style_mut().align_self = Some(align);
        self
    }

    #[must_use]
    fn margin(mut self, margin: impl Into<Inset>) -> Self {
        self.style_mut().margin = Some(margin.into());
        self
    }

    #[must_use]
    fn position(mut self, x: f32, y: f32) -> Self {
        self.style_mut().position = Some(Point::new(x, y));
        self
    }
}

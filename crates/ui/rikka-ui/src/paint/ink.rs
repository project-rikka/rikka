use rikka_types::{Color, Gradient};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Ink {
    Solid(Color),
    Gradient(Gradient),
}

impl Ink {
    pub(crate) fn color(&self) -> Color {
        match self {
            Ink::Solid(color) => *color,
            Ink::Gradient(_) => Color::WHITE,
        }
    }

    pub(crate) fn gradient(&self) -> Option<Box<Gradient>> {
        match self {
            Ink::Solid(_) => None,
            Ink::Gradient(gradient) => Some(Box::new(*gradient)),
        }
    }
}

impl From<Color> for Ink {
    fn from(color: Color) -> Self {
        Ink::Solid(color)
    }
}

impl From<Gradient> for Ink {
    fn from(gradient: Gradient) -> Self {
        Ink::Gradient(gradient)
    }
}

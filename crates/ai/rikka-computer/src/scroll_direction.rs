use std::str::FromStr;

use crate::error::Error;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScrollDirection {
    Up,
    Down,
    Left,
    Right,
}

impl ScrollDirection {
    #[must_use]
    pub fn delta(self, notches: f64) -> (f64, f64) {
        match self {
            ScrollDirection::Up => (0.0, -notches),
            ScrollDirection::Down => (0.0, notches),
            ScrollDirection::Left => (-notches, 0.0),
            ScrollDirection::Right => (notches, 0.0),
        }
    }
}

impl FromStr for ScrollDirection {
    type Err = Error;

    fn from_str(text: &str) -> Result<ScrollDirection, Error> {
        match text.trim().to_ascii_lowercase().as_str() {
            "up" => Ok(ScrollDirection::Up),
            "down" => Ok(ScrollDirection::Down),
            "left" => Ok(ScrollDirection::Left),
            "right" => Ok(ScrollDirection::Right),
            _ => Err(Error::UnknownScrollDirection(text.to_owned())),
        }
    }
}

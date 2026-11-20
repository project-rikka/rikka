use rikka_animation::Motion;
use rikka_types::{Color, Point};

use crate::motion::Transition;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Visual {
    pub(crate) opacity: Option<f32>,
    pub(crate) scale: Option<f32>,
    pub(crate) offset: Option<Point>,
    pub(crate) background: Option<Color>,
    pub(crate) foreground: Option<Color>,
    pub(crate) animate: Option<Motion>,
    pub(crate) transition: Option<Transition>,
    pub(crate) clipped: bool,
}

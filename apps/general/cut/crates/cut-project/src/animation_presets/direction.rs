use std::f32::consts::FRAC_1_SQRT_2;

use rikka_types::Point;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
    UpLeft,
    UpRight,
    DownLeft,
    DownRight,
}

impl Direction {
    pub const ALL: [Direction; 8] = [
        Direction::Up,
        Direction::Down,
        Direction::Left,
        Direction::Right,
        Direction::UpLeft,
        Direction::UpRight,
        Direction::DownLeft,
        Direction::DownRight,
    ];

    #[must_use]
    pub fn vector(self) -> Point {
        let slant = FRAC_1_SQRT_2;
        match self {
            Direction::Up => Point::new(0.0, -1.0),
            Direction::Down => Point::new(0.0, 1.0),
            Direction::Left => Point::new(-1.0, 0.0),
            Direction::Right => Point::new(1.0, 0.0),
            Direction::UpLeft => Point::new(-slant, -slant),
            Direction::UpRight => Point::new(slant, -slant),
            Direction::DownLeft => Point::new(-slant, slant),
            Direction::DownRight => Point::new(slant, slant),
        }
    }

    pub(crate) fn axes(self) -> Point {
        match self {
            Direction::Left | Direction::Right => Point::new(1.0, 0.0),
            Direction::Up | Direction::Down => Point::new(0.0, 1.0),
            Direction::UpLeft | Direction::UpRight | Direction::DownLeft | Direction::DownRight => {
                Point::new(1.0, 1.0)
            }
        }
    }
}

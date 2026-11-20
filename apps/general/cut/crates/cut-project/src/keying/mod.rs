mod piece;
mod selection;
mod single;
mod target;

pub use piece::KeyPiece;

use rikka_types::Point;

use crate::animation::{Bend, Handle, Interpolation, Side, Smooth, Value};
use crate::property::Property;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum KeyChange {
    Add { at: i64 },
    Remove { at: i64 },
    Clear,
    Value { at: i64, value: Value },
    Handle { at: i64, side: Side, handle: Handle },
    Smooth { at: i64, smooth: Smooth },
    Bend { at: i64, bend: Bend },
}

impl KeyChange {
    #[must_use]
    pub fn tangent(at: i64, bend: Bend, side: Side, tangent: Point) -> KeyChange {
        KeyChange::Bend {
            at,
            bend: bend.with_tangent(side, tangent),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct KeyRef {
    pub property: Property,
    pub at: i64,
}

impl KeyRef {
    #[must_use]
    pub fn new(property: Property, at: i64) -> KeyRef {
        KeyRef { property, at }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum KeysChange {
    Move { by: i64 },
    Duplicate { by: i64 },
    Delete,
    Interpolate(Interpolation),
}

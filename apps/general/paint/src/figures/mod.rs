pub(crate) mod clipboard;
mod manipulation;
mod picked;
mod style;

pub(crate) use manipulation::{ManipulatedFigures, Manipulation, grips_of};
pub(crate) use picked::FigureSelection;
pub(crate) use style::{Restyle, StyleSource, adopted, restyled};

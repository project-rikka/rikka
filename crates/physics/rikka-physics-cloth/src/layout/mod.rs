mod bake;
mod colouring;
mod records;
mod topology;
pub(crate) mod validate;

pub(crate) use bake::{Sections, bake};
pub(crate) use records::{
    CONTACT_WORDS, ColliderRecord, FigureRecord, GEOMETRY_VECS, JobRecord, NEAREST_WORDS, NONE,
    PieceRecord, WRAP_WORDS,
};
pub(crate) use topology::{Group, Topology};

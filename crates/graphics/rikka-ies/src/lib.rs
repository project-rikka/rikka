//! IESNA LM-63 photometric profiles: parsing and sampling of luminous intensity distributions.
//!
//! [`IesProfile::parse`] reads LM-63-1986, 1991, 1995, 2002 and 2019 files with `TILT=NONE` and
//! type C photometry; [`IesProfile::candela`] samples the intensity in any direction, folding
//! the file's symmetry and interpolating bilinearly between its angles.

mod error;
mod parse;
mod profile;

pub use error::IesError;
pub use profile::{Edition, IesProfile, Symmetry};

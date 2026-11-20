use glam::Vec3;
use rikka_file_format::DecodeError;

use crate::codec::{counted, finite, record, tagged};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SmoothType {
    #[default]
    Simple,
    LengthWeighted,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum RestSource {
    #[default]
    Original,
    Bind,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CorrectiveSmooth {
    pub factor: f32,
    pub iterations: u16,
    pub scale: f32,
    pub smooth_type: SmoothType,
    pub rest_source: RestSource,
    pub bind_coords: Vec<Vec3>,
    pub use_only_smooth: bool,
    pub use_pin_boundary: bool,
    pub vertex_group: String,
    pub invert_vertex_group: bool,
}

impl CorrectiveSmooth {
    #[must_use]
    pub fn is_bound(&self) -> bool {
        !self.bind_coords.is_empty()
    }
}

impl Default for CorrectiveSmooth {
    fn default() -> CorrectiveSmooth {
        CorrectiveSmooth {
            factor: 0.5,
            iterations: 5,
            scale: 1.0,
            smooth_type: SmoothType::Simple,
            rest_source: RestSource::Original,
            bind_coords: Vec::new(),
            use_only_smooth: false,
            use_pin_boundary: false,
            vertex_group: String::new(),
            invert_vertex_group: false,
        }
    }
}

fn finite_points() -> impl Fn(&Vec<Vec3>) -> Result<(), DecodeError> {
    |points| match points.iter().all(|point| point.is_finite()) {
        true => Ok(()),
        false => Err(DecodeError::Malformed("bind coordinate is not finite")),
    }
}

tagged!(SmoothType, "corrective smooth type" {
    Simple = 0,
    LengthWeighted = 1,
});

tagged!(RestSource, "corrective smooth rest source" {
    Original = 0,
    Bind = 1,
});

record!(CorrectiveSmooth {
    factor => finite("corrective smooth factor is not finite"),
    iterations => counted(0, i16::MAX as u16, "corrective smooth repeat is out of range"),
    scale => finite("corrective smooth scale is not finite"),
    smooth_type,
    rest_source,
    bind_coords => finite_points(),
    use_only_smooth,
    use_pin_boundary,
    vertex_group,
    invert_vertex_group,
});

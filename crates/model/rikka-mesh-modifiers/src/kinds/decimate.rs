use std::f32::consts::PI;

use bitflags::bitflags;

use crate::codec::{counted, flags_codec, record, tagged, within};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum DecimateType {
    #[default]
    Collapse,
    Unsubdivide,
    Planar,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SymmetryAxis {
    #[default]
    X,
    Y,
    Z,
}

bitflags! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
    pub struct Delimit: u8 {
        const NORMAL = 1 << 0;
        const MATERIAL = 1 << 1;
        const SEAM = 1 << 2;
        const SHARP = 1 << 3;
        const UV = 1 << 4;
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Decimate {
    pub decimate_type: DecimateType,
    pub ratio: f32,
    pub iterations: u16,
    pub angle_limit: f32,
    pub delimit: Delimit,
    pub use_dissolve_boundaries: bool,
    pub use_collapse_triangulate: bool,
    pub use_symmetry: bool,
    pub symmetry_axis: SymmetryAxis,
    pub vertex_group: String,
    pub invert_vertex_group: bool,
    pub vertex_group_factor: f32,
}

impl Default for Decimate {
    fn default() -> Decimate {
        Decimate {
            decimate_type: DecimateType::Collapse,
            ratio: 1.0,
            iterations: 0,
            angle_limit: 5.0f32.to_radians(),
            delimit: Delimit::empty(),
            use_dissolve_boundaries: false,
            use_collapse_triangulate: false,
            use_symmetry: false,
            symmetry_axis: SymmetryAxis::X,
            vertex_group: String::new(),
            invert_vertex_group: false,
            vertex_group_factor: 1.0,
        }
    }
}

tagged!(DecimateType, "decimate type" {
    Collapse = 0,
    Unsubdivide = 1,
    Planar = 2,
});

tagged!(SymmetryAxis, "symmetry axis" {
    X = 0,
    Y = 1,
    Z = 2,
});

flags_codec!(Delimit, u8, "decimate delimit flag");

record!(Decimate {
    decimate_type,
    ratio => within(0.0, 1.0, "decimate ratio is outside 0..1"),
    iterations => counted(0, i16::MAX as u16, "decimate iterations exceed 32767"),
    angle_limit => within(0.0, PI, "decimate angle limit is outside 0..180 degrees"),
    delimit,
    use_dissolve_boundaries,
    use_collapse_triangulate,
    use_symmetry,
    symmetry_axis,
    vertex_group,
    invert_vertex_group,
    vertex_group_factor => within(0.0, 1000.0, "decimate vertex group factor is outside 0..1000"),
});

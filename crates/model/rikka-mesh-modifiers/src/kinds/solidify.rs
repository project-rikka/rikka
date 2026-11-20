use crate::codec::{finite, record, tagged, within};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SolidifyMode {
    #[default]
    Simple,
    Complex,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ThicknessMode {
    Fixed,
    Even,
    #[default]
    Constraints,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum BoundaryShape {
    #[default]
    None,
    Round,
    Flat,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Solidify {
    pub solidify_mode: SolidifyMode,
    pub thickness: f32,
    pub offset: f32,
    pub thickness_clamp: f32,
    pub use_thickness_angle_clamp: bool,
    pub thickness_vertex_group: f32,
    pub vertex_group: String,
    pub invert_vertex_group: bool,
    pub shell_vertex_group: String,
    pub rim_vertex_group: String,
    pub use_rim: bool,
    pub use_rim_only: bool,
    pub use_even_offset: bool,
    pub use_quality_normals: bool,
    pub use_flip_normals: bool,
    pub use_flat_faces: bool,
    pub edge_crease_inner: f32,
    pub edge_crease_outer: f32,
    pub edge_crease_rim: f32,
    pub material_offset: i16,
    pub material_offset_rim: i16,
    pub nonmanifold_thickness_mode: ThicknessMode,
    pub nonmanifold_boundary_mode: BoundaryShape,
    pub nonmanifold_merge_threshold: f32,
    pub bevel_convex: f32,
}

impl Default for Solidify {
    fn default() -> Solidify {
        Solidify {
            solidify_mode: SolidifyMode::Simple,
            thickness: 0.01,
            offset: -1.0,
            thickness_clamp: 0.0,
            use_thickness_angle_clamp: false,
            thickness_vertex_group: 0.0,
            vertex_group: String::new(),
            invert_vertex_group: false,
            shell_vertex_group: String::new(),
            rim_vertex_group: String::new(),
            use_rim: true,
            use_rim_only: false,
            use_even_offset: false,
            use_quality_normals: false,
            use_flip_normals: false,
            use_flat_faces: false,
            edge_crease_inner: 0.0,
            edge_crease_outer: 0.0,
            edge_crease_rim: 0.0,
            material_offset: 0,
            material_offset_rim: 0,
            nonmanifold_thickness_mode: ThicknessMode::Constraints,
            nonmanifold_boundary_mode: BoundaryShape::None,
            nonmanifold_merge_threshold: 0.0001,
            bevel_convex: 0.0,
        }
    }
}

tagged!(SolidifyMode, "solidify mode" {
    Simple = 0,
    Complex = 1,
});

tagged!(ThicknessMode, "solidify thickness mode" {
    Fixed = 0,
    Even = 1,
    Constraints = 2,
});

tagged!(BoundaryShape, "solidify boundary shape" {
    None = 0,
    Round = 1,
    Flat = 2,
});

record!(Solidify {
    solidify_mode,
    thickness => finite("solidify thickness is not finite"),
    offset => finite("solidify offset is not finite"),
    thickness_clamp => within(0.0, 100.0, "solidify clamp is outside 0..100"),
    use_thickness_angle_clamp,
    thickness_vertex_group => within(0.0, 1.0, "solidify vertex group factor is outside 0..1"),
    vertex_group,
    invert_vertex_group,
    shell_vertex_group,
    rim_vertex_group,
    use_rim,
    use_rim_only,
    use_even_offset,
    use_quality_normals,
    use_flip_normals,
    use_flat_faces,
    edge_crease_inner => within(0.0, 1.0, "solidify inner crease is outside 0..1"),
    edge_crease_outer => within(0.0, 1.0, "solidify outer crease is outside 0..1"),
    edge_crease_rim => within(0.0, 1.0, "solidify rim crease is outside 0..1"),
    material_offset,
    material_offset_rim,
    nonmanifold_thickness_mode,
    nonmanifold_boundary_mode,
    nonmanifold_merge_threshold => within(0.0, 1.0, "solidify merge threshold is outside 0..1"),
    bevel_convex => within(-1.0, 1.0, "solidify bevel convex is outside -1..1"),
});

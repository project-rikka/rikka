use crate::codec::{counted, record, tagged, within};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SubdivisionType {
    #[default]
    CatmullClark,
    Simple,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum UvSmooth {
    None,
    PreserveCorners,
    PreserveCornersAndJunctions,
    PreserveCornersJunctionsAndConcave,
    #[default]
    PreserveBoundaries,
    All,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum BoundarySmooth {
    #[default]
    All,
    PreserveCorners,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum AdaptiveSpace {
    #[default]
    Pixel,
    Object,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Subdivision {
    pub subdivision_type: SubdivisionType,
    pub levels: u8,
    pub render_levels: u8,
    pub quality: u8,
    pub show_only_control_edges: bool,
    pub use_creases: bool,
    pub use_custom_normals: bool,
    pub use_limit_surface: bool,
    pub uv_smooth: UvSmooth,
    pub boundary_smooth: BoundarySmooth,
    pub use_adaptive_subdivision: bool,
    pub adaptive_space: AdaptiveSpace,
    pub adaptive_pixel_size: f32,
    pub adaptive_object_edge_length: f32,
}

impl Subdivision {
    pub const MAX_LEVELS: u8 = 11;
}

impl Default for Subdivision {
    fn default() -> Subdivision {
        Subdivision {
            subdivision_type: SubdivisionType::CatmullClark,
            levels: 1,
            render_levels: 2,
            quality: 3,
            show_only_control_edges: true,
            use_creases: true,
            use_custom_normals: false,
            use_limit_surface: true,
            uv_smooth: UvSmooth::PreserveBoundaries,
            boundary_smooth: BoundarySmooth::All,
            use_adaptive_subdivision: false,
            adaptive_space: AdaptiveSpace::Pixel,
            adaptive_pixel_size: 1.0,
            adaptive_object_edge_length: 0.01,
        }
    }
}

tagged!(SubdivisionType, "subdivision type" {
    CatmullClark = 0,
    Simple = 1,
});

tagged!(UvSmooth, "UV smooth mode" {
    None = 0,
    PreserveCorners = 1,
    PreserveCornersAndJunctions = 2,
    PreserveCornersJunctionsAndConcave = 3,
    PreserveBoundaries = 4,
    All = 5,
});

tagged!(BoundarySmooth, "boundary smooth mode" {
    All = 0,
    PreserveCorners = 1,
});

tagged!(AdaptiveSpace, "adaptive subdivision space" {
    Pixel = 0,
    Object = 1,
});

record!(Subdivision {
    subdivision_type,
    levels => counted(0, Subdivision::MAX_LEVELS, "subdivision levels exceed 11"),
    render_levels => counted(0, Subdivision::MAX_LEVELS, "subdivision render levels exceed 11"),
    quality => counted(1, 10, "subdivision quality is outside 1..10"),
    show_only_control_edges,
    use_creases,
    use_custom_normals,
    use_limit_surface,
    uv_smooth,
    boundary_smooth,
    use_adaptive_subdivision,
    adaptive_space,
    adaptive_pixel_size => within(0.1, 1000.0, "adaptive pixel size is outside 0.1..1000"),
    adaptive_object_edge_length => within(0.0001, 1000.0, "adaptive edge length is outside 0.0001..1000"),
});

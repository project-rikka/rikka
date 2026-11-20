use crate::codec::{finite, record, tagged};
use crate::reference::{ObjectRef, TextureRef};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TextureCoordinates {
    #[default]
    Local,
    Global,
    Object,
    Uv,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum DisplaceDirection {
    X,
    Y,
    Z,
    #[default]
    Normal,
    CustomNormal,
    RgbToXyz,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum DisplaceSpace {
    #[default]
    Local,
    Global,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Displace {
    pub texture: Option<TextureRef>,
    pub texture_coords: TextureCoordinates,
    pub texture_coords_object: Option<ObjectRef>,
    pub texture_coords_bone: String,
    pub uv_layer: String,
    pub direction: DisplaceDirection,
    pub space: DisplaceSpace,
    pub strength: f32,
    pub mid_level: f32,
    pub vertex_group: String,
    pub invert_vertex_group: bool,
}

impl Default for Displace {
    fn default() -> Displace {
        Displace {
            texture: None,
            texture_coords: TextureCoordinates::Local,
            texture_coords_object: None,
            texture_coords_bone: String::new(),
            uv_layer: String::new(),
            direction: DisplaceDirection::Normal,
            space: DisplaceSpace::Local,
            strength: 1.0,
            mid_level: 0.5,
            vertex_group: String::new(),
            invert_vertex_group: false,
        }
    }
}

tagged!(TextureCoordinates, "texture coordinates" {
    Local = 0,
    Global = 1,
    Object = 2,
    Uv = 3,
});

tagged!(DisplaceDirection, "displace direction" {
    X = 0,
    Y = 1,
    Z = 2,
    Normal = 3,
    RgbToXyz = 4,
    CustomNormal = 5,
});

tagged!(DisplaceSpace, "displace space" {
    Local = 0,
    Global = 1,
});

record!(Displace {
    texture,
    texture_coords,
    texture_coords_object,
    texture_coords_bone,
    uv_layer,
    direction,
    space,
    strength => finite("displace strength is not finite"),
    mid_level => finite("displace mid level is not finite"),
    vertex_group,
    invert_vertex_group,
});

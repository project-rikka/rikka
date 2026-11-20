use glam::{Vec2, Vec3, Vec4};

use crate::id::model_id;
use crate::texture::TextureId;
use crate::toon_surface::ToonSurface;

model_id!(MaterialId);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MaterialAlias {
    pub material: MaterialId,
    pub name: String,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Alpha {
    #[default]
    Opaque,
    Mask {
        cutoff: f32,
    },
    Blend,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pbr {
    pub metallic: f32,
    pub roughness: f32,
    pub normal_scale: f32,
    pub occlusion_strength: f32,
    pub diffuse_roughness: f32,
    pub thin_walled: bool,
    pub specular: Specular,
    pub anisotropy: Anisotropy,
    pub coat: Coat,
    pub sheen: Sheen,
    pub transmission: Transmission,
    pub subsurface: Subsurface,
    pub iridescence: Iridescence,
}

impl Default for Pbr {
    fn default() -> Self {
        Self {
            metallic: 1.0,
            roughness: 1.0,
            normal_scale: 1.0,
            occlusion_strength: 1.0,
            diffuse_roughness: 0.0,
            thin_walled: false,
            specular: Specular::default(),
            anisotropy: Anisotropy::default(),
            coat: Coat::default(),
            sheen: Sheen::default(),
            transmission: Transmission::default(),
            subsurface: Subsurface::default(),
            iridescence: Iridescence::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Specular {
    pub weight: f32,
    pub color: Vec3,
    pub ior: f32,
}

impl Default for Specular {
    fn default() -> Self {
        Self {
            weight: 1.0,
            color: Vec3::ONE,
            ior: 1.5,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Anisotropy {
    pub strength: f32,
    pub rotation: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Coat {
    pub weight: f32,
    pub roughness: f32,
    pub color: Vec3,
    pub ior: f32,
    pub normal_scale: f32,
    pub darkening: f32,
}

impl Default for Coat {
    fn default() -> Self {
        Self {
            weight: 0.0,
            roughness: 0.0,
            color: Vec3::ONE,
            ior: 1.5,
            normal_scale: 1.0,
            darkening: 1.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sheen {
    pub weight: f32,
    pub color: Vec3,
    pub roughness: f32,
}

impl Default for Sheen {
    fn default() -> Self {
        Self {
            weight: 0.0,
            color: Vec3::ONE,
            roughness: 0.5,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transmission {
    pub weight: f32,
    pub thickness: f32,
    pub attenuation_color: Vec3,
    pub attenuation_distance: f32,
    pub dispersion: f32,
}

impl Default for Transmission {
    fn default() -> Self {
        Self {
            weight: 0.0,
            thickness: 0.0,
            attenuation_color: Vec3::ONE,
            attenuation_distance: f32::INFINITY,
            dispersion: 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Subsurface {
    pub weight: f32,
    pub color: Vec3,
    pub radius: Vec3,
    pub anisotropy: f32,
}

impl Default for Subsurface {
    fn default() -> Self {
        Self {
            weight: 0.0,
            color: Vec3::splat(0.8),
            radius: Vec3::new(1.0, 0.5, 0.25),
            anisotropy: 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Iridescence {
    pub weight: f32,
    pub ior: f32,
    pub thickness_min: f32,
    pub thickness_max: f32,
}

impl Default for Iridescence {
    fn default() -> Self {
        Self {
            weight: 0.0,
            ior: 1.3,
            thickness_min: 100.0,
            thickness_max: 400.0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum OutlineWidth {
    #[default]
    None,
    World(f32),
    Screen(f32),
}

impl OutlineWidth {
    #[must_use]
    pub fn is_drawn(self) -> bool {
        !matches!(self, Self::None)
    }

    #[must_use]
    pub fn width(self) -> f32 {
        match self {
            Self::None => 0.0,
            Self::World(width) | Self::Screen(width) => width,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Outline {
    pub width: OutlineWidth,
    pub color: Vec3,
    pub lighting_mix: f32,
}

impl Default for Outline {
    fn default() -> Self {
        Self {
            width: OutlineWidth::None,
            color: Vec3::ZERO,
            lighting_mix: 1.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UvTransform {
    pub offset: Vec2,
    pub scale: Vec2,
    pub rotation: f32,
}

impl Default for UvTransform {
    fn default() -> Self {
        Self {
            offset: Vec2::ZERO,
            scale: Vec2::ONE,
            rotation: 0.0,
        }
    }
}

impl UvTransform {
    #[must_use]
    pub fn is_identity(&self) -> bool {
        self.offset == Vec2::ZERO && self.scale == Vec2::ONE && self.rotation == 0.0
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MatcapBlend {
    #[default]
    Add,
    Multiply,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ToonFlavor {
    #[default]
    MToon,
    Mmd,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Toon {
    pub flavor: ToonFlavor,
    pub shade_color: Vec3,
    pub shading_shift: f32,
    pub shading_shift_scale: f32,
    pub shading_toony: f32,
    pub gi_equalization: f32,
    pub normal_scale: f32,
    pub matcap_color: Vec3,
    pub matcap_blend: MatcapBlend,
    pub rim_color: Vec3,
    pub rim_fresnel_power: f32,
    pub rim_lift: f32,
    pub rim_lighting_mix: f32,
    pub outline: Outline,
    pub scroll: Vec2,
    pub spin: f32,
    pub transparent_depth_write: bool,
    pub render_order: i32,
    pub surface: ToonSurface,
    pub specular: Vec3,
    pub specular_power: f32,
    pub ambient: Vec3,
}

impl Default for Toon {
    fn default() -> Self {
        Self {
            flavor: ToonFlavor::MToon,
            shade_color: Vec3::ZERO,
            shading_shift: 0.0,
            shading_shift_scale: 1.0,
            shading_toony: 0.9,
            gi_equalization: 0.9,
            normal_scale: 1.0,
            matcap_color: Vec3::ONE,
            matcap_blend: MatcapBlend::Add,
            rim_color: Vec3::ZERO,
            rim_fresnel_power: 5.0,
            rim_lift: 0.0,
            rim_lighting_mix: 1.0,
            outline: Outline::default(),
            scroll: Vec2::ZERO,
            spin: 0.0,
            transparent_depth_write: false,
            render_order: 0,
            surface: ToonSurface::Cloth,
            specular: Vec3::ZERO,
            specular_power: 0.0,
            ambient: Vec3::ZERO,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shading {
    Pbr(Pbr),
    Toon(Toon),
}

impl Default for Shading {
    fn default() -> Self {
        Self::Pbr(Pbr::default())
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MaterialTextures {
    pub base_color: Option<TextureId>,
    pub normal: Option<TextureId>,
    pub metallic_roughness: Option<TextureId>,
    pub occlusion: Option<TextureId>,
    pub emissive: Option<TextureId>,
    pub shade: Option<TextureId>,
    pub shading_shift: Option<TextureId>,
    pub matcap: Option<TextureId>,
    pub rim: Option<TextureId>,
    pub outline_width: Option<TextureId>,
    pub uv_animation_mask: Option<TextureId>,
    pub shading_ramp: Option<TextureId>,
    pub specular: Option<TextureId>,
    pub specular_color: Option<TextureId>,
    pub anisotropy: Option<TextureId>,
    pub coat: Option<TextureId>,
    pub coat_roughness: Option<TextureId>,
    pub coat_normal: Option<TextureId>,
    pub sheen_color: Option<TextureId>,
    pub sheen_roughness: Option<TextureId>,
    pub transmission: Option<TextureId>,
    pub thickness: Option<TextureId>,
    pub iridescence: Option<TextureId>,
    pub iridescence_thickness: Option<TextureId>,
    pub channels: LayerChannels,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TextureChannel {
    #[default]
    Convention,
    Red,
    Green,
    Blue,
    Alpha,
}

impl TextureChannel {
    #[must_use]
    pub fn index(self) -> Option<usize> {
        match self {
            Self::Convention => None,
            Self::Red => Some(0),
            Self::Green => Some(1),
            Self::Blue => Some(2),
            Self::Alpha => Some(3),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LayerChannels {
    pub specular: TextureChannel,
    pub coat: TextureChannel,
    pub coat_roughness: TextureChannel,
    pub sheen_roughness: TextureChannel,
    pub transmission: TextureChannel,
    pub thickness: TextureChannel,
    pub iridescence: TextureChannel,
    pub iridescence_thickness: TextureChannel,
}

impl MaterialTextures {
    pub(crate) fn each(&self) -> [Option<TextureId>; 24] {
        [
            self.base_color,
            self.normal,
            self.metallic_roughness,
            self.occlusion,
            self.emissive,
            self.shade,
            self.shading_shift,
            self.matcap,
            self.rim,
            self.outline_width,
            self.uv_animation_mask,
            self.shading_ramp,
            self.specular,
            self.specular_color,
            self.anisotropy,
            self.coat,
            self.coat_roughness,
            self.coat_normal,
            self.sheen_color,
            self.sheen_roughness,
            self.transmission,
            self.thickness,
            self.iridescence,
            self.iridescence_thickness,
        ]
    }
}

#[derive(Clone, Debug, PartialEq)]
#[must_use]
pub struct Material {
    pub name: String,
    pub base_color: Vec4,
    pub emissive: Vec3,
    pub alpha: Alpha,
    pub double_sided: bool,
    pub casts_shadow: bool,
    pub receives_shadow: bool,
    pub shading: Shading,
    pub textures: MaterialTextures,
    pub uv_transform: UvTransform,
}

impl Default for Material {
    fn default() -> Self {
        Self {
            name: String::new(),
            base_color: Vec4::ONE,
            emissive: Vec3::ZERO,
            alpha: Alpha::Opaque,
            double_sided: false,
            casts_shadow: true,
            receives_shadow: true,
            shading: Shading::default(),
            textures: MaterialTextures::default(),
            uv_transform: UvTransform::default(),
        }
    }
}

impl Material {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Self::default()
        }
    }

    pub fn base_color(mut self, base_color: Vec4) -> Self {
        self.base_color = base_color;
        self
    }

    pub fn emissive(mut self, emissive: Vec3) -> Self {
        self.emissive = emissive;
        self
    }

    pub fn alpha(mut self, alpha: Alpha) -> Self {
        self.alpha = alpha;
        self
    }

    pub fn double_sided(mut self, double_sided: bool) -> Self {
        self.double_sided = double_sided;
        self
    }

    pub fn shading(mut self, shading: Shading) -> Self {
        self.shading = shading;
        self
    }

    pub fn textures(mut self, textures: MaterialTextures) -> Self {
        self.textures = textures;
        self
    }

    pub fn uv_transform(mut self, uv_transform: UvTransform) -> Self {
        self.uv_transform = uv_transform;
        self
    }

    #[must_use]
    pub fn is_transparent(&self) -> bool {
        matches!(self.alpha, Alpha::Blend)
    }

    #[must_use]
    pub fn render_order(&self) -> i32 {
        match self.shading {
            Shading::Toon(toon) => toon.render_order,
            Shading::Pbr(_) => 0,
        }
    }
}

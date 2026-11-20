use rikka_model_core::glam::{Vec3, Vec4};

use rikka_model_core::TextureId;
use rikka_model_core::{
    Alpha, MatcapBlend, Material, Outline, OutlineWidth, Shading, Toon, ToonFlavor, ToonSurface,
};

use super::super::EDGE_SCREEN_WIDTH;
use super::document::{
    PmxBlend, PmxMaterial, PmxMaterialFlags, PmxMaterialOffset, PmxSphereMode, PmxToon,
};

pub(crate) const SHARED_TOONS: usize = 10;

const LIGHT: f32 = 154.0 / 255.0;

const SHARED_SHADOWS: [[u8; 3]; SHARED_TOONS] = [
    [205, 205, 205],
    [245, 225, 225],
    [154, 154, 154],
    [248, 239, 235],
    [254, 231, 222],
    [195, 172, 3],
    [255, 255, 255],
    [255, 255, 255],
    [255, 255, 255],
    [255, 255, 255],
];

const STEP_TOONY: f32 = 0.95;

pub(crate) fn shared_toon_path(index: usize) -> String {
    format!("toon{:02}.bmp", index + 1)
}

pub(crate) struct SurfaceTextures<'a> {
    pub pictures: &'a [Option<TextureId>],
    pub names: &'a [String],
    pub ramps: &'a [Option<TextureId>],
    pub shared_ramps: &'a [Option<TextureId>; SHARED_TOONS],
}

impl SurfaceTextures<'_> {
    fn picture(&self, index: Option<usize>) -> Option<TextureId> {
        index.and_then(|index| self.pictures.get(index).copied().flatten())
    }

    fn name(&self, index: Option<usize>) -> Option<&str> {
        index.and_then(|index| self.names.get(index).map(String::as_str))
    }

    fn ramp(&self, toon: PmxToon) -> Option<TextureId> {
        match toon {
            PmxToon::Texture(index) => {
                index.and_then(|index| self.ramps.get(index).copied().flatten())
            }
            PmxToon::Shared(index) => self.shared_ramps.get(usize::from(index)).copied().flatten(),
        }
    }
}

pub(crate) fn material_of(
    pmx_material: &PmxMaterial,
    order: i32,
    textures: &SurfaceTextures<'_>,
) -> Material {
    let picture = textures.picture(pmx_material.texture);
    let mut material = Material::new(pmx_material.name.clone())
        .double_sided(pmx_material.flags.has(PmxMaterialFlags::DOUBLE_SIDED))
        .alpha(Alpha::Blend)
        .shading(Shading::Toon(Toon {
            flavor: ToonFlavor::Mmd,
            shading_toony: STEP_TOONY,
            transparent_depth_write: true,
            render_order: order,
            surface: ToonSurface::classify(
                &format!("{} {}", pmx_material.name, pmx_material.english_name),
                textures.name(pmx_material.texture),
            ),
            ..Toon::default()
        }));
    material.casts_shadow = pmx_material.flags.has(PmxMaterialFlags::CASTS_SHADOW);
    material.receives_shadow = pmx_material.flags.has(PmxMaterialFlags::RECEIVES_SHADOW);
    material.textures.base_color = picture;
    material.textures.shade = picture;
    material.textures.shading_ramp = textures.ramp(pmx_material.toon);
    let sphere = match pmx_material.sphere_mode {
        PmxSphereMode::Add => Some(MatcapBlend::Add),
        PmxSphereMode::Multiply => Some(MatcapBlend::Multiply),
        PmxSphereMode::None | PmxSphereMode::SubTexture => None,
    };
    if let Some(blend) = sphere
        && let Some(texture) = textures.picture(pmx_material.sphere)
    {
        material.textures.matcap = Some(texture);
        if let Shading::Toon(toon) = &mut material.shading {
            toon.matcap_blend = blend;
        }
    }
    Surface::from_material(pmx_material).paint(&mut material);
    material
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Surface {
    diffuse: Vec4,
    specular: Vec3,
    specular_power: f32,
    ambient: Vec3,
    flags: PmxMaterialFlags,
    edge_color: Vec4,
    edge_size: f32,
    toon: PmxToon,
}

impl Surface {
    pub(crate) fn from_material(material: &PmxMaterial) -> Self {
        Self {
            diffuse: material.diffuse,
            specular: material.specular,
            specular_power: material.specular_power,
            ambient: material.ambient,
            flags: material.flags,
            edge_color: material.edge_color,
            edge_size: material.edge_size,
            toon: material.toon,
        }
    }

    pub(crate) fn paint(&self, material: &mut Material) {
        let lit = (self.diffuse.truncate() * LIGHT + self.ambient).clamp(Vec3::ZERO, Vec3::ONE);
        material.base_color = lit.extend(self.diffuse.w.clamp(0.0, 1.0));
        let Shading::Toon(toon) = &mut material.shading else {
            return;
        };
        toon.shade_color = lit * shadow_of(self.toon);
        toon.specular = self.specular.clamp(Vec3::ZERO, Vec3::ONE);
        toon.specular_power = self.specular_power.max(0.0);
        toon.ambient = self.ambient.clamp(Vec3::ZERO, Vec3::ONE);
        toon.outline = match self.flags.has(PmxMaterialFlags::OUTLINE) {
            true => Outline {
                width: OutlineWidth::Screen(self.edge_size.max(0.0) * EDGE_SCREEN_WIDTH),
                color: self.edge_color.truncate().clamp(Vec3::ZERO, Vec3::ONE),
                lighting_mix: 0.0,
            },
            false => Outline::default(),
        };
    }

    pub(crate) fn blend(&mut self, offset: &PmxMaterialOffset, weight: f32) {
        let mixed = |base: Vec4, by: Vec4| match offset.blend {
            PmxBlend::Multiply => base * (Vec4::ONE + (by - Vec4::ONE) * weight),
            PmxBlend::Add => base + by * weight,
        };
        self.diffuse = mixed(self.diffuse, offset.diffuse);
        self.ambient = mixed(self.ambient.extend(1.0), offset.ambient.extend(1.0)).truncate();
        self.edge_color = mixed(self.edge_color, offset.edge_color);
        self.edge_size = mixed(Vec4::splat(self.edge_size), Vec4::splat(offset.edge_size)).x;
    }
}

fn shadow_of(toon: PmxToon) -> Vec3 {
    match toon {
        PmxToon::Texture(_) => Vec3::ONE,
        PmxToon::Shared(index) => SHARED_SHADOWS
            .get(usize::from(index))
            .map_or(Vec3::ONE, |[r, g, b]| {
                Vec3::new(f32::from(*r), f32::from(*g), f32::from(*b)) / 255.0
            }),
    }
}

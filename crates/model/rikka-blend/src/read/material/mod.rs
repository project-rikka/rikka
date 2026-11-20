mod generated;
mod image;
pub(crate) mod nodes;
mod png;

use rikka_model_core::glam::{Vec2, Vec3, Vec4};
use rikka_model_core::{
    Alpha, Filter, Image, ImageId, LayerChannels, Material, MaterialId, MaterialTextures,
    ModelBuilder, Pbr, Result, Sampler, Shading, Texture, TextureChannel, TextureId, UvTransform,
    Wrap,
};
use rustc_hash::FxHashMap;

use super::modifier::Notes;
use super::resources::Resources;
use crate::file::{Identity, Record};
pub(crate) use image::LoadedImage;
use nodes::{Link, Node, Placement, Tree};

const MA_BM_CLIP: i64 = 3;
const MA_BM_HASHED: i64 = 4;
const MA_BM_BLEND: i64 = 5;
const MA_BL_CULL_BACKFACE: i64 = 1 << 2;
const MA_SURFACE_FORWARD: i64 = 1;
const MA_TRANSP: i64 = 1 << 16;
const MA_VERTEXCOLP: i64 = 1 << 7;
const MAP_COL: i64 = 1;
const MAP_NORM: i64 = 2;
const MAP_ALPHA: i64 = 1 << 7;
const MAP_EMIT: i64 = 1 << 6;
const TEX_IMAGE: i64 = 8;
const SOCK_RGBA: i64 = 2;
const NODE_MATH_MULTIPLY: i64 = 2;
const NODE_MATH_SUBTRACT: i64 = 1;
const NODE_MATH_ROUND: i64 = 14;
const NODE_MATH_LESS_THAN: i64 = 15;
const NODE_MATH_GREATER_THAN: i64 = 16;

#[derive(Clone, Copy)]
pub(crate) struct Picture<'a> {
    image: Record<'a>,
    sampler: Sampler,
    placement: Option<Placement>,
    uv_map: Option<&'a str>,
}

#[derive(Clone, Copy, Default)]
struct Paint<'a> {
    constant: Vec4,
    texture: Option<Picture<'a>>,
    vertex: bool,
    alpha_texture: Option<Picture<'a>>,
}

#[derive(Clone, Copy)]
struct Amount<'a> {
    constant: f32,
    texture: Option<(Picture<'a>, usize)>,
}

#[derive(Clone, Copy)]
struct Surface<'a> {
    base: Paint<'a>,
    metallic: Amount<'a>,
    roughness: Amount<'a>,
    alpha: Amount<'a>,
    normal: Option<(Picture<'a>, f32)>,
    emission: Paint<'a>,
    emission_strength: f32,
    cutoff: Option<f32>,
    alpha_linked: bool,
    layers: Pbr,
    pictures: LayerPictures<'a>,
}

#[derive(Clone, Copy, Default)]
struct LayerPictures<'a> {
    specular: Option<(Picture<'a>, usize)>,
    specular_tint: Option<Picture<'a>>,
    coat: Option<(Picture<'a>, usize)>,
    coat_roughness: Option<(Picture<'a>, usize)>,
    coat_normal: Option<(Picture<'a>, f32)>,
    sheen_tint: Option<Picture<'a>>,
    sheen_roughness: Option<(Picture<'a>, usize)>,
    transmission: Option<(Picture<'a>, usize)>,
}

impl Surface<'_> {
    fn plain(color: Vec4) -> Self {
        Self {
            base: Paint {
                constant: color,
                ..Paint::default()
            },
            metallic: constant(0.0),
            roughness: constant(0.5),
            alpha: constant(1.0),
            normal: None,
            emission: Paint {
                constant: Vec4::ZERO,
                ..Paint::default()
            },
            emission_strength: 0.0,
            cutoff: None,
            alpha_linked: false,
            layers: Pbr::default(),
            pictures: LayerPictures::default(),
        }
    }
}

fn constant<'a>(value: f32) -> Amount<'a> {
    Amount {
        constant: value,
        texture: None,
    }
}

#[derive(Clone)]
pub(crate) struct Requirements {
    pub(crate) colors: bool,
    pub(crate) uv_map: Option<String>,
}

#[derive(Default)]
pub(crate) struct Library {
    materials: FxHashMap<Identity, (MaterialId, Requirements)>,
    images: FxHashMap<Identity, Option<ImageId>>,
    textures: FxHashMap<(ImageId, Sampler), TextureId>,
    fallback: Option<MaterialId>,
}

impl Library {
    pub(crate) fn fallback(&mut self, store: &mut dyn Store) -> MaterialId {
        *self.fallback.get_or_insert_with(|| {
            store.add_material(
                Material::new("Material")
                    .base_color(Vec4::new(0.8, 0.8, 0.8, 1.0))
                    .double_sided(true)
                    .shading(Shading::Pbr(Pbr {
                        metallic: 0.0,
                        roughness: 0.5,
                        ..Pbr::default()
                    })),
            )
        })
    }

    pub(crate) fn material(
        &mut self,
        material: Record<'_>,
        store: &mut dyn Store,
        resources: &mut dyn Resources,
        notes: &mut Notes,
    ) -> Result<(MaterialId, Requirements)> {
        let key = material.identity();
        if let Some((id, requirements)) = self.materials.get(&key) {
            return Ok((*id, requirements.clone()));
        }
        let (pbr_material, requirements) = self.build(material, store, resources, notes)?;
        let id = store.add_material(pbr_material);
        self.materials.insert(key, (id, requirements.clone()));
        Ok((id, requirements))
    }

    fn build(
        &mut self,
        material: Record<'_>,
        store: &mut dyn Store,
        resources: &mut dyn Resources,
        notes: &mut Notes,
    ) -> Result<(Material, Requirements)> {
        let name = material.id_name();
        let blend = material.blend();
        let diffuse = Vec4::new(
            material.float("r").unwrap_or(0.8),
            material.float("g").unwrap_or(0.8),
            material.float("b").unwrap_or(0.8),
            1.0,
        );
        let uses_nodes = match blend.at_least(500, 0) {
            true => true,
            false => material.int("use_nodes").unwrap_or(0) != 0,
        };
        let tree = material
            .deref("nodetree")
            .filter(|_| uses_nodes)
            .map(Tree::read);
        let surface = match &tree {
            Some(tree) => tree
                .output()
                .and_then(|output| {
                    let input = tree.nodes[output].input(&["Surface"])?;
                    tree.linked(output, input)
                })
                .and_then(|link| {
                    Walker {
                        tree,
                        notes: &mut *notes,
                        name: &name,
                    }
                    .shader(link, 0)
                }),
            None => None,
        };
        let surface = match (surface, &tree) {
            (Some(surface), _) => surface,
            (None, Some(_)) => {
                let mut surface = Surface::plain(diffuse);
                surface.metallic = constant(material.float("metallic").unwrap_or(0.0));
                surface.roughness = constant(material.float("roughness").unwrap_or(0.5));
                surface
            }
            (None, None) => legacy(&material, diffuse, blend.version()),
        };

        let blend_method = material.int("blend_method").unwrap_or(0);
        let alpha_picture = match surface.alpha.texture {
            Some((picture, 3)) => Some(picture),
            _ => None,
        };
        let alpha_used = surface.alpha.constant < 1.0
            || alpha_picture.is_some()
            || surface.base.alpha_texture.is_some();
        let mut alpha = match (blend.at_least(402, 0), blend_method) {
            (false, MA_BM_CLIP) => Alpha::Mask {
                cutoff: material.float("alpha_threshold").unwrap_or(0.5),
            },
            (false, MA_BM_BLEND | MA_BM_HASHED) => Alpha::Blend,
            (true, _) if alpha_used => Alpha::Blend,
            _ => Alpha::Opaque,
        };
        if tree.is_none() && material.int("mode").unwrap_or(0) & MA_TRANSP != 0 {
            alpha = Alpha::Blend;
        }
        if blend.at_least(402, 0)
            && material.int("surface_render_method").unwrap_or(0) == MA_SURFACE_FORWARD
            && (alpha_used || surface.alpha_linked)
        {
            alpha = Alpha::Blend;
        }
        if blend.at_least(402, 0)
            && let Some(cutoff) = surface.cutoff
        {
            alpha = Alpha::Mask { cutoff };
        }

        let mut placement = None;
        let mut uv_map = None;
        let packed_texture = match (surface.metallic.texture, surface.roughness.texture) {
            (Some((metal, 2)), Some((rough, 1))) if same(&metal, &rough) => Some(metal),
            (Some((metal, _)), None) => Some(metal),
            (None, Some((rough, _))) => Some(rough),
            (Some((metal, _)), Some(_)) => {
                notes.unsupported(format!(
                    "material \"{name}\": separate metallic and roughness images, keeping only metallic"
                ));
                Some(metal)
            }
            (None, None) => None,
        };
        let slots = [
            surface
                .base
                .texture
                .or(surface.base.alpha_texture)
                .or(alpha_picture),
            packed_texture,
            surface.normal.map(|(picture, _)| picture),
            surface.emission.texture,
        ];
        let mut slot_textures = [None; 4];
        for (slot, texture) in slots.into_iter().zip(&mut slot_textures) {
            let Some(picture) = slot else {
                continue;
            };
            *texture = self.texture(picture, store, resources, notes)?;
            if texture.is_some() {
                placement = placement.or(picture.placement);
                uv_map = uv_map.or_else(|| picture.uv_map.map(str::to_owned));
            }
        }
        let [base_color, metallic_roughness, normal, emissive] = slot_textures;
        let pictures = surface.pictures;
        let mut layer = |library: &mut Self, picture: Option<(Picture<'_>, usize)>| {
            let Some((picture, channel)) = picture else {
                return Ok((None, TextureChannel::Convention));
            };
            let channel = match channel {
                0 => TextureChannel::Red,
                1 => TextureChannel::Green,
                2 => TextureChannel::Blue,
                _ => TextureChannel::Alpha,
            };
            library
                .texture(picture, store, resources, notes)
                .map(|texture| (texture, channel))
        };
        let (specular, specular_channel) = layer(self, pictures.specular)?;
        let (coat, coat_channel) = layer(self, pictures.coat)?;
        let (coat_roughness, coat_roughness_channel) = layer(self, pictures.coat_roughness)?;
        let (sheen_roughness, sheen_roughness_channel) = layer(self, pictures.sheen_roughness)?;
        let (transmission, transmission_channel) = layer(self, pictures.transmission)?;
        let mut colour = |library: &mut Self, picture: Option<Picture<'_>>| match picture {
            Some(picture) => library.texture(picture, store, resources, notes),
            None => Ok(None),
        };
        let specular_color = colour(self, pictures.specular_tint)?;
        let sheen_color = colour(self, pictures.sheen_tint)?;
        let coat_normal = colour(self, pictures.coat_normal.map(|(picture, _)| picture))?;
        let textures = MaterialTextures {
            base_color,
            metallic_roughness,
            normal,
            emissive,
            specular,
            specular_color,
            coat,
            coat_roughness,
            coat_normal,
            sheen_color,
            sheen_roughness,
            transmission,
            channels: LayerChannels {
                specular: specular_channel,
                coat: coat_channel,
                coat_roughness: coat_roughness_channel,
                sheen_roughness: sheen_roughness_channel,
                transmission: transmission_channel,
                ..LayerChannels::default()
            },
            ..MaterialTextures::default()
        };

        let metallic = surface.metallic.constant;
        let roughness = surface.roughness.constant;
        let base = surface.base.constant;
        let alpha_value = surface.alpha.constant;
        let emissive = surface.emission.constant.truncate() * surface.emission_strength;
        let mut pbr_material = Material::new(name)
            .base_color(Vec4::new(base.x, base.y, base.z, base.w * alpha_value))
            .emissive(emissive)
            .alpha(alpha)
            .double_sided(material.int("blend_flag").unwrap_or(0) & MA_BL_CULL_BACKFACE == 0)
            .shading(Shading::Pbr(Pbr {
                metallic,
                roughness,
                normal_scale: surface.normal.map_or(1.0, |(_, strength)| strength),
                occlusion_strength: 1.0,
                ..surface.layers
            }))
            .textures(textures);
        if let Some(placement) = placement {
            pbr_material = pbr_material.uv_transform(uv_transform(placement));
        }
        Ok((
            pbr_material,
            Requirements {
                colors: surface.base.vertex,
                uv_map,
            },
        ))
    }

    fn texture(
        &mut self,
        picture: Picture<'_>,
        store: &mut dyn Store,
        resources: &mut dyn Resources,
        notes: &mut Notes,
    ) -> Result<Option<TextureId>> {
        self.image_texture(picture.image, picture.sampler, store, resources, notes)
    }

    pub(crate) fn image(
        &mut self,
        image: Record<'_>,
        store: &mut dyn Store,
        resources: &mut dyn Resources,
        notes: &mut Notes,
    ) -> Result<Option<ImageId>> {
        let key = image.identity();
        if let Some(image_id) = self.images.get(&key) {
            return Ok(*image_id);
        }
        let image_id =
            image::load(image, resources, notes)?.and_then(|loaded| store.add_image(loaded));
        self.images.insert(key, image_id);
        Ok(image_id)
    }

    pub(crate) fn image_texture(
        &mut self,
        image: Record<'_>,
        sampler: Sampler,
        store: &mut dyn Store,
        resources: &mut dyn Resources,
        notes: &mut Notes,
    ) -> Result<Option<TextureId>> {
        let Some(image) = self.image(image, store, resources, notes)? else {
            return Ok(None);
        };
        Ok(Some(*self.textures.entry((image, sampler)).or_insert_with(
            || store.add_texture(Texture::new(image).sampler(sampler)),
        )))
    }
}

pub(crate) trait Store {
    fn add_image(&mut self, image: LoadedImage) -> Option<ImageId>;

    fn add_texture(&mut self, texture: Texture) -> TextureId;

    fn add_material(&mut self, material: Material) -> MaterialId;
}

impl Store for ModelBuilder {
    fn add_image(&mut self, image: LoadedImage) -> Option<ImageId> {
        let bytes = image.bytes?;
        Some(ModelBuilder::add_image(self, Image::new(image.name, bytes)))
    }

    fn add_texture(&mut self, texture: Texture) -> TextureId {
        ModelBuilder::add_texture(self, texture)
    }

    fn add_material(&mut self, material: Material) -> MaterialId {
        ModelBuilder::add_material(self, material)
    }
}

fn same<'a>(first: &Picture<'a>, second: &Picture<'a>) -> bool {
    first.image.identity() == second.image.identity()
}

fn uv_transform(placement: Placement) -> UvTransform {
    let (sin, cos) = placement.rotation.sin_cos();
    UvTransform {
        offset: Vec2::new(
            placement.offset.x - placement.scale.y * sin,
            1.0 - placement.offset.y - placement.scale.y * cos,
        ),
        scale: placement.scale,
        rotation: placement.rotation,
    }
}

fn legacy<'a>(material: &Record<'a>, diffuse: Vec4, version: u32) -> Surface<'a> {
    let mut surface = Surface::plain(diffuse);
    if version >= 280 {
        surface.metallic = constant(material.float("metallic").unwrap_or(0.0));
        surface.roughness = constant(material.float("roughness").unwrap_or(0.4));
        return surface;
    }
    let hardness = material.int("har").unwrap_or(50) as f32;
    surface.roughness = constant((1.0 - (hardness / 511.0).sqrt()).clamp(0.05, 1.0));
    let alpha = material.float("alpha").unwrap_or(1.0);
    if material.int("mode").unwrap_or(0) & MA_TRANSP != 0 {
        surface.alpha = constant(alpha);
    }
    let emit = material.float("emit").unwrap_or(0.0);
    if emit > 0.0 {
        surface.emission.constant = diffuse;
        surface.emission_strength = emit;
    }
    surface.base.vertex = material.int("mode").unwrap_or(0) & MA_VERTEXCOLP != 0;
    for texture_slot in legacy_slots(material) {
        let Some(texture) = texture_slot.id("tex") else {
            continue;
        };
        if texture.int("type") != Some(TEX_IMAGE) {
            continue;
        }
        let Some(image) = texture.id("ima") else {
            continue;
        };
        let mapto = texture_slot.int("mapto").unwrap_or(MAP_COL);
        let size = texture_slot.vec3("size").unwrap_or(Vec3::ONE);
        let offset = texture_slot.vec3("ofs").unwrap_or(Vec3::ZERO);
        let picture = Picture {
            image,
            sampler: Sampler::new(
                Filter::Linear,
                match texture.int("extend").unwrap_or(2) {
                    1 | 4 => Wrap::Clamp,
                    _ => Wrap::Repeat,
                },
            ),
            placement: (size.truncate() != Vec2::ONE || offset.truncate() != Vec2::ZERO).then_some(
                Placement {
                    offset: offset.truncate() * 0.5 + Vec2::splat(0.5) - size.truncate() * 0.5,
                    scale: size.truncate(),
                    rotation: 0.0,
                },
            ),
            uv_map: None,
        };
        if mapto & MAP_COL != 0 && surface.base.texture.is_none() {
            surface.base.texture = Some(picture);
            surface.base.constant = Vec4::new(1.0, 1.0, 1.0, surface.base.constant.w);
        }
        if mapto & MAP_ALPHA != 0 {
            surface.alpha.texture = Some((picture, 3));
        }
        if mapto & MAP_NORM != 0 && surface.normal.is_none() {
            surface.normal = Some((picture, texture_slot.float("norfac").unwrap_or(1.0)));
        }
        if mapto & MAP_EMIT != 0 && surface.emission.texture.is_none() {
            surface.emission.texture = Some(picture);
        }
    }
    surface
}

fn legacy_slots<'a>(material: &Record<'a>) -> Vec<Record<'a>> {
    material
        .pointers_inline("mtex")
        .into_iter()
        .filter_map(|address| material.data_at(address))
        .collect()
}

struct Walker<'t, 'a, 'n> {
    tree: &'t Tree<'a>,
    notes: &'n mut Notes,
    name: &'t str,
}

impl<'t, 'a> Walker<'t, 'a, '_> {
    fn node(&self, link: Link) -> &'t Node<'a> {
        &self.tree.nodes[link.node]
    }

    fn input_link(&self, node: usize, names: &[&str]) -> Option<Link> {
        let input = self.tree.nodes[node].input(names)?;
        self.tree.linked(node, input)
    }

    fn shader(&mut self, link: Link, depth: usize) -> Option<Surface<'a>> {
        if depth > 32 {
            return None;
        }
        let index = link.node;
        let node = self.node(link);
        let idname = node.idname.as_str();
        match () {
            () if node.is("ShaderNodeBsdfPrincipled", 193) => {
                let mut surface = Surface::plain(Vec4::ONE);
                surface.base = self.paint(index, &["Base Color"]);
                surface.metallic = self.amount(index, &["Metallic"], 0.0);
                surface.roughness = self.amount(index, &["Roughness"], 0.5);
                let alpha_input = self.tree.nodes[index].input(&["Alpha"]);
                let alpha_link = alpha_input.and_then(|input| self.tree.linked(index, input));
                surface.alpha_linked = alpha_link.is_some();
                match alpha_link.and_then(|link| self.clip(link)) {
                    Some((cutoff, source)) => {
                        surface.cutoff = Some(cutoff);
                        surface.alpha = match source {
                            Some(source) => self.amount_from(source, 1.0, 0),
                            None => constant(1.0),
                        };
                    }
                    None => surface.alpha = self.amount(index, &["Alpha"], 1.0),
                }
                surface.normal = self.normal(index);
                let emission = match self.tree.nodes[index].input(&["Emission Color", "Emission"]) {
                    Some(_) => self.paint(index, &["Emission Color", "Emission"]),
                    None => Paint::default(),
                };
                let strength = self.amount(index, &["Emission Strength"], 1.0).constant;
                surface.emission = emission;
                surface.emission_strength = match emission.texture.is_some()
                    || emission.constant.truncate() != Vec3::ZERO
                {
                    true => strength,
                    false => 0.0,
                };
                (surface.layers, surface.pictures) =
                    self.principled(index, surface.base.constant.truncate());
                Some(surface)
            }
            () if node.is("ShaderNodeBsdfDiffuse", 132)
                || idname == "ShaderNodeBsdfToon"
                || idname == "ShaderNodeSubsurfaceScattering" =>
            {
                let mut surface = Surface::plain(Vec4::ONE);
                surface.base = self.paint(index, &["Color"]);
                surface.roughness = constant(1.0);
                surface.normal = self.normal(index);
                Some(surface)
            }
            () if node.is("ShaderNodeBsdfGlossy", 131)
                || idname == "ShaderNodeBsdfAnisotropic"
                || idname == "ShaderNodeBsdfMetallic" =>
            {
                let mut surface = Surface::plain(Vec4::ONE);
                surface.base = self.paint(index, &["Base Color", "Color"]);
                surface.metallic = constant(1.0);
                surface.roughness = self.amount(index, &["Roughness"], 0.5);
                surface.normal = self.normal(index);
                Some(surface)
            }
            () if idname == "ShaderNodeEeveeSpecular" => {
                let mut surface = Surface::plain(Vec4::ONE);
                surface.base = self.paint(index, &["Base Color"]);
                surface.roughness = self.amount(index, &["Roughness"], 0.5);
                surface.emission = self.paint(index, &["Emissive Color"]);
                surface.emission_strength = 1.0;
                let transparency = self.amount(index, &["Transparency"], 0.0);
                surface.alpha = constant(1.0 - transparency.constant);
                surface.normal = self.normal(index);
                Some(surface)
            }
            () if node.is("ShaderNodeEmission", 140) || idname == "ShaderNodeBackground" => {
                let mut surface = Surface::plain(Vec4::new(0.0, 0.0, 0.0, 1.0));
                surface.emission = self.paint(index, &["Color"]);
                surface.emission_strength = self.amount(index, &["Strength"], 1.0).constant;
                Some(surface)
            }
            () if node.is("ShaderNodeBsdfTransparent", 138) => {
                let mut surface = Surface::plain(Vec4::ONE);
                surface.alpha = constant(0.0);
                Some(surface)
            }
            () if node.is("ShaderNodeMixShader", 128) => self.mix(index, depth),
            () if node.is("ShaderNodeAddShader", 156) => {
                let first = self
                    .input_link_at(index, 0)
                    .and_then(|link| self.shader(link, depth + 1));
                let second = self
                    .input_link_at(index, 1)
                    .and_then(|link| self.shader(link, depth + 1));
                match (first, second) {
                    (Some(mut first), Some(second)) => {
                        if first.emission_strength == 0.0 && second.emission_strength > 0.0 {
                            first.emission = second.emission;
                            first.emission_strength = second.emission_strength;
                        }
                        Some(first)
                    }
                    (first, second) => first.or(second),
                }
            }
            () if idname == "ShaderNodeGroup" => {
                self.notes.unsupported(format!(
                    "material \"{}\": node group read with default values",
                    self.name
                ));
                None
            }
            () => {
                self.notes.unsupported(format!(
                    "material \"{}\": {} is approximated",
                    self.name,
                    match idname.is_empty() {
                        true => format!("node type {}", node.legacy),
                        false => idname.to_owned(),
                    }
                ));
                let mut surface = Surface::plain(Vec4::ONE);
                surface.base = self.paint(index, &["Color", "Base Color"]);
                Some(surface)
            }
        }
    }

    fn input_link_at(&self, node: usize, input: usize) -> Option<Link> {
        let inputs = &self.tree.nodes[node].inputs;
        let shader_inputs: Vec<usize> = inputs
            .iter()
            .enumerate()
            .filter(|(_, socket)| {
                socket.name == "Shader" || socket.identifier.starts_with("Shader")
            })
            .map(|(slot, _)| slot)
            .collect();
        self.tree.linked(node, *shader_inputs.get(input)?)
    }

    fn mix(&mut self, index: usize, depth: usize) -> Option<Surface<'a>> {
        let factor = self.amount(index, &["Fac"], 0.5);
        let first = self.input_link_at(index, 0);
        let second = self.input_link_at(index, 1);
        let transparent = |walker: &Self, link: Option<Link>| {
            link.is_some_and(|link| walker.node(link).is("ShaderNodeBsdfTransparent", 138))
        };
        match (transparent(self, first), transparent(self, second)) {
            (true, false) => {
                let mut surface = second.and_then(|link| self.shader(link, depth + 1))?;
                surface.alpha = factor;
                Some(surface)
            }
            (false, true) => {
                let mut surface = first.and_then(|link| self.shader(link, depth + 1))?;
                surface.alpha = Amount {
                    constant: 1.0 - factor.constant,
                    texture: factor.texture,
                };
                Some(surface)
            }
            _ => {
                let (main, other) = match factor.constant < 0.5 && factor.texture.is_none() {
                    true => (first, second),
                    false => (second, first),
                };
                main.and_then(|link| self.shader(link, depth + 1))
                    .or_else(|| other.and_then(|link| self.shader(link, depth + 1)))
            }
        }
    }

    fn paint(&mut self, node: usize, names: &[&str]) -> Paint<'a> {
        let Some(input) = self.tree.nodes[node].input(names) else {
            return Paint {
                constant: Vec4::ONE,
                ..Paint::default()
            };
        };
        let fallback = self.tree.nodes[node].inputs[input]
            .color()
            .unwrap_or(Vec4::ONE);
        match self.tree.linked(node, input) {
            Some(link) => self.paint_from(link, fallback, 0),
            None => Paint {
                constant: fallback,
                ..Paint::default()
            },
        }
    }

    fn paint_from(&mut self, link: Link, fallback: Vec4, depth: usize) -> Paint<'a> {
        if depth > 16 {
            return Paint {
                constant: fallback,
                ..Paint::default()
            };
        }
        let node = self.node(link);
        let output = node
            .outputs
            .get(link.socket)
            .map(|socket| socket.name.clone())
            .unwrap_or_default();
        match () {
            () if node.is("ShaderNodeTexImage", 143) => match self.picture(link.node) {
                Some(picture) if output == "Alpha" => Paint {
                    constant: Vec4::ONE,
                    alpha_texture: Some(picture),
                    ..Paint::default()
                },
                Some(picture) => Paint {
                    constant: Vec4::ONE,
                    texture: Some(picture),
                    ..Paint::default()
                },
                None => Paint {
                    constant: Vec4::ONE,
                    ..Paint::default()
                },
            },
            () if node.is("ShaderNodeRGB", 101) => Paint {
                constant: node
                    .outputs
                    .first()
                    .and_then(|socket| socket.color())
                    .unwrap_or(fallback),
                ..Paint::default()
            },
            () if node.idname == "ShaderNodeValue" => Paint {
                constant: node
                    .outputs
                    .first()
                    .and_then(|socket| socket.float())
                    .map_or(fallback, |value| Vec4::new(value, value, value, 1.0)),
                ..Paint::default()
            },
            () if node.is("ShaderNodeVertexColor", 706) || node.is("ShaderNodeAttribute", 129) => {
                Paint {
                    constant: Vec4::ONE,
                    vertex: true,
                    ..Paint::default()
                }
            }
            () if node.idname == "ShaderNodeMix" || node.is("ShaderNodeMixRGB", 103) => {
                self.mix_paint(link.node, fallback, depth)
            }
            () if node.is("ShaderNodeAmbientOcclusion", 179) => {
                let index = link.node;
                let input = self.tree.nodes[index].input(&["Color"]);
                let default = input
                    .and_then(|input| self.tree.nodes[index].inputs[input].color())
                    .unwrap_or(fallback);
                match input.and_then(|input| self.tree.linked(index, input)) {
                    Some(inner) => self.paint_from(inner, default, depth + 1),
                    None => Paint {
                        constant: default,
                        ..Paint::default()
                    },
                }
            }
            () if matches!(
                node.idname.as_str(),
                "ShaderNodeGamma"
                    | "ShaderNodeHueSaturation"
                    | "ShaderNodeBrightContrast"
                    | "ShaderNodeRGBCurve"
                    | "ShaderNodeValToRGB"
            ) =>
            {
                let index = link.node;
                let color = self.tree.nodes[index].input(&["Color", "Fac"]);
                match color.and_then(|input| self.tree.linked(index, input)) {
                    Some(inner) => self.paint_from(inner, fallback, depth + 1),
                    None => Paint {
                        constant: fallback,
                        ..Paint::default()
                    },
                }
            }
            () => Paint {
                constant: fallback,
                ..Paint::default()
            },
        }
    }

    fn mix_paint(&mut self, index: usize, fallback: Vec4, depth: usize) -> Paint<'a> {
        let node = &self.tree.nodes[index];
        let legacy = node.idname != "ShaderNodeMix";
        let blend = node.record.int("custom1").unwrap_or(0);
        let data_rgba = legacy
            || node
                .record
                .deref("storage")
                .and_then(|storage| storage.int("data_type"))
                .is_some_and(|data| data == SOCK_RGBA);
        let multiply = match legacy {
            true => blend == 2,
            false => node
                .record
                .deref("storage")
                .and_then(|storage| storage.int("blend_type"))
                .is_some_and(|blend| blend == 2),
        };
        let colors: Vec<usize> = node
            .inputs
            .iter()
            .enumerate()
            .filter(|(_, socket)| {
                matches!(socket.name.as_str(), "A" | "B" | "Color1" | "Color2") && socket.is_color()
            })
            .map(|(slot, _)| slot)
            .collect();
        if !data_rgba || colors.len() < 2 {
            return Paint {
                constant: fallback,
                ..Paint::default()
            };
        }
        let operand = |walker: &mut Self, slot: usize| {
            let default = walker.tree.nodes[index].inputs[slot]
                .color()
                .unwrap_or(Vec4::ONE);
            match walker.tree.linked(index, slot) {
                Some(link) => walker.paint_from(link, default, depth + 1),
                None => Paint {
                    constant: default,
                    ..Paint::default()
                },
            }
        };
        let first = operand(self, colors[0]);
        let second = operand(self, colors[1]);
        match multiply {
            true => Paint {
                constant: first.constant * second.constant,
                texture: first.texture.or(second.texture),
                vertex: first.vertex || second.vertex,
                alpha_texture: first.alpha_texture.or(second.alpha_texture),
            },
            false => match second.texture.is_some() || second.vertex {
                true => second,
                false => first,
            },
        }
    }

    fn amount(&mut self, node: usize, names: &[&str], default: f32) -> Amount<'a> {
        let Some(input) = self.tree.nodes[node].input(names) else {
            return constant(default);
        };
        let fallback = self.tree.nodes[node].inputs[input]
            .float()
            .unwrap_or(default);
        match self.tree.linked(node, input) {
            Some(link) => self.amount_from(link, fallback, 0),
            None => constant(fallback),
        }
    }

    fn constant(&mut self, node: usize, names: &[&str], default: f32) -> f32 {
        self.amount(node, names, default).constant
    }

    fn vector(&self, node: usize, names: &[&str], default: Vec3) -> Vec3 {
        let node = &self.tree.nodes[node];
        node.input(names)
            .and_then(|input| node.inputs[input].vector())
            .map_or(default, |vector| vector.truncate())
    }

    fn principled(&mut self, node: usize, base: Vec3) -> (Pbr, LayerPictures<'a>) {
        let defaults = Pbr::default();
        let fourth = self.tree.nodes[node].input(&["Coat Weight"]).is_some();
        let tinted = self.tree.nodes[node].input(&["Subsurface Color"]).is_some();
        let mut pictures = LayerPictures::default();
        let mut pbr = Pbr {
            diffuse_roughness: self.constant(node, &["Diffuse Roughness"], 0.0),
            thin_walled: self.constant(node, &["Thin Wall"], 0.0) != 0.0,
            ..defaults
        };
        pbr.specular.ior = self.constant(node, &["IOR"], defaults.specular.ior);
        let specular = self.amount(node, &["Specular IOR Level", "Specular"], 0.5);
        pbr.specular.weight = 2.0 * specular.constant;
        pictures.specular = specular.texture;
        if fourth {
            let tint = self.paint(node, &["Specular Tint"]);
            pbr.specular.color = tint.constant.truncate();
            pictures.specular_tint = tint.texture;
        }
        pbr.anisotropy.strength = self.constant(node, &["Anisotropic"], 0.0);
        pbr.anisotropy.rotation =
            self.constant(node, &["Anisotropic Rotation"], 0.0) * std::f32::consts::TAU;
        let coat = self.amount(node, &["Coat Weight", "Clearcoat"], 0.0);
        pbr.coat.weight = coat.constant;
        pictures.coat = coat.texture;
        let coat_roughness = self.amount(node, &["Coat Roughness", "Clearcoat Roughness"], 0.03);
        pbr.coat.roughness = coat_roughness.constant;
        pictures.coat_roughness = coat_roughness.texture;
        pbr.coat.ior = self.constant(node, &["Coat IOR"], 1.5);
        pictures.coat_normal = self.normal_from(node, &["Coat Normal", "Clearcoat Normal"]);
        if let Some((_, strength)) = pictures.coat_normal {
            pbr.coat.normal_scale = strength;
        }
        if fourth {
            pbr.coat.color = self.paint(node, &["Coat Tint"]).constant.truncate();
        }
        pbr.sheen.weight = self.constant(node, &["Sheen Weight", "Sheen"], 0.0);
        let sheen_roughness = self.amount(node, &["Sheen Roughness"], 0.5);
        pbr.sheen.roughness = sheen_roughness.constant;
        pictures.sheen_roughness = sheen_roughness.texture;
        if fourth {
            let tint = self.paint(node, &["Sheen Tint"]);
            pbr.sheen.color = tint.constant.truncate();
            pictures.sheen_tint = tint.texture;
        }
        let transmission = self.amount(node, &["Transmission Weight", "Transmission"], 0.0);
        pbr.transmission.weight = transmission.constant;
        pictures.transmission = transmission.texture;
        pbr.subsurface.weight = self.constant(node, &["Subsurface Weight", "Subsurface"], 0.0);
        pbr.subsurface.radius = self.vector(node, &["Subsurface Radius"], Vec3::new(1.0, 0.2, 0.1))
            * self.constant(node, &["Subsurface Scale"], 1.0);
        pbr.subsurface.anisotropy = self.constant(node, &["Subsurface Anisotropy"], 0.0);
        pbr.subsurface.color = match tinted {
            true => self.paint(node, &["Subsurface Color"]).constant.truncate(),
            false => base,
        };
        let film = self.constant(node, &["Thin Film Thickness"], 0.0);
        if film > 0.0 {
            pbr.iridescence.weight = 1.0;
            pbr.iridescence.thickness_max = film;
            pbr.iridescence.ior = self.constant(node, &["Thin Film IOR"], 1.33);
        }
        (pbr, pictures)
    }

    fn amount_from(&mut self, link: Link, fallback: f32, depth: usize) -> Amount<'a> {
        if depth > 16 {
            return constant(fallback);
        }
        let source = self.node(link);
        let output = source
            .outputs
            .get(link.socket)
            .map(|socket| socket.name.clone())
            .unwrap_or_default();
        if source.is("ShaderNodeTexImage", 143) {
            return match self.picture(link.node) {
                Some(picture) => Amount {
                    constant: 1.0,
                    texture: Some((picture, if output == "Alpha" { 3 } else { 1 })),
                },
                None => constant(1.0),
            };
        }
        if matches!(
            source.idname.as_str(),
            "ShaderNodeSeparateColor" | "ShaderNodeSeparateRGB" | "ShaderNodeSeparateXYZ"
        ) || source.is("ShaderNodeSeparateRGB", 120)
        {
            let channel = link.socket.min(2);
            let source_index = link.node;
            if let Some(inner) = self.tree.linked(source_index, 0)
                && self.node(inner).is("ShaderNodeTexImage", 143)
                && let Some(picture) = self.picture(inner.node)
            {
                return Amount {
                    constant: 1.0,
                    texture: Some((picture, channel)),
                };
            }
            return constant(fallback);
        }
        if source.is("ShaderNodeMath", 115)
            && source.record.int("custom1") == Some(NODE_MATH_MULTIPLY)
        {
            let index = link.node;
            let operand = |walker: &mut Self, slot: usize| {
                let default = walker.tree.nodes[index]
                    .inputs
                    .get(slot)
                    .and_then(|socket| socket.float())
                    .unwrap_or(1.0);
                match walker.tree.linked(index, slot) {
                    Some(inner) => walker.amount_from(inner, default, depth + 1),
                    None => constant(default),
                }
            };
            let first = operand(self, 0);
            let second = operand(self, 1);
            return Amount {
                constant: first.constant * second.constant,
                texture: first.texture.or(second.texture),
            };
        }
        if matches!(source.idname.as_str(), "ShaderNodeValue") {
            return constant(
                source
                    .outputs
                    .first()
                    .and_then(|socket| socket.float())
                    .unwrap_or(fallback),
            );
        }
        constant(fallback)
    }

    fn clip(&self, link: Link) -> Option<(f32, Option<Link>)> {
        let node = self.node(link);
        if !node.is("ShaderNodeMath", 115) {
            return None;
        }
        let index = link.node;
        let operand = |slot: usize| {
            self.tree.nodes[index]
                .inputs
                .get(slot)
                .and_then(|socket| socket.float())
        };
        match node.record.int("custom1")? {
            NODE_MATH_ROUND => Some((0.5, self.tree.linked(index, 0))),
            NODE_MATH_GREATER_THAN if self.tree.linked(index, 1).is_none() => {
                Some((operand(1)?, self.tree.linked(index, 0)))
            }
            NODE_MATH_SUBTRACT
                if self.tree.linked(index, 0).is_none() && operand(0) == Some(1.0) =>
            {
                let inner = self.tree.linked(index, 1)?;
                let less = self.node(inner);
                let inner_index = inner.node;
                (less.is("ShaderNodeMath", 115)
                    && less.record.int("custom1") == Some(NODE_MATH_LESS_THAN)
                    && self.tree.linked(inner_index, 1).is_none())
                .then(|| {
                    let cutoff = self.tree.nodes[inner_index]
                        .inputs
                        .get(1)
                        .and_then(|socket| socket.float())?;
                    Some((cutoff, self.tree.linked(inner_index, 0)))
                })
                .flatten()
            }
            _ => None,
        }
    }

    fn normal(&mut self, node: usize) -> Option<(Picture<'a>, f32)> {
        self.normal_from(node, &["Normal"])
    }

    fn normal_from(&mut self, node: usize, names: &[&str]) -> Option<(Picture<'a>, f32)> {
        let link = self.input_link(node, names)?;
        let map = self.node(link);
        if !map.is("ShaderNodeNormalMap", 175) {
            return None;
        }
        let map_index = link.node;
        let strength = self.amount(map_index, &["Strength"], 1.0).constant;
        let color = self.input_link(map_index, &["Color"])?;
        if !self.node(color).is("ShaderNodeTexImage", 143) {
            return None;
        }
        Some((self.picture(color.node)?, strength))
    }

    fn picture(&mut self, index: usize) -> Option<Picture<'a>> {
        let node = &self.tree.nodes[index];
        let image = node.record.id("id")?;
        let storage = node.record.deref("storage");
        let interpolation = storage
            .and_then(|storage| storage.int("interpolation"))
            .unwrap_or(0);
        let extension = storage
            .and_then(|storage| storage.int("extension"))
            .unwrap_or(0);
        let sampler = Sampler::new(
            match interpolation {
                1 => Filter::Nearest,
                _ => Filter::Linear,
            },
            match extension {
                1 | 2 => Wrap::Clamp,
                _ => Wrap::Repeat,
            },
        );
        let (placement, uv_map) = match self.input_link(index, &["Vector"]) {
            Some(link) => self.coordinates(link),
            None => (None, None),
        };
        Some(Picture {
            image,
            sampler,
            placement,
            uv_map,
        })
    }

    fn coordinates(&mut self, link: Link) -> (Option<Placement>, Option<&'a str>) {
        let node = self.node(link);
        if node.is("ShaderNodeUVMap", 187) {
            return (None, uv_name(node));
        }
        if !node.is("ShaderNodeMapping", 109) {
            return (None, None);
        }
        let index = link.node;
        let storage = node.record.deref("storage");
        let vector = |walker: &Self, name: &str, default: Vec3| {
            walker.tree.nodes[index]
                .input(&[name])
                .and_then(|input| walker.tree.nodes[index].inputs[input].vector())
                .map_or(default, |value| value.truncate())
        };
        let (location, rotation, scale) = match storage {
            Some(mapping) if mapping.kind() == "TexMapping" => (
                mapping.vec3("loc").unwrap_or(Vec3::ZERO),
                mapping.vec3("rot").unwrap_or(Vec3::ZERO),
                mapping.vec3("size").unwrap_or(Vec3::ONE),
            ),
            _ => (
                vector(self, "Location", Vec3::ZERO),
                vector(self, "Rotation", Vec3::ZERO),
                vector(self, "Scale", Vec3::ONE),
            ),
        };
        let uv = self
            .input_link(index, &["Vector"])
            .and_then(|inner| uv_name(self.node(inner)));
        (
            Some(Placement {
                offset: location.truncate(),
                scale: scale.truncate(),
                rotation: rotation.z,
            }),
            uv,
        )
    }
}

fn uv_name<'a>(node: &Node<'a>) -> Option<&'a str> {
    node.record
        .deref("storage")
        .and_then(|storage| storage.text_ref("uv_map"))
        .filter(|name| !name.is_empty())
}

use std::borrow::Cow;

use rikka_model_core::glam::{Mat4, Vec2, Vec3, Vec4};
use rikka_model_core::{
    Error, ExportCtx, INFLUENCES, Image, MatcapBlend, Material, MaterialId, Model, NodeId,
    OutlineWidth, Requirement, Result, Shading, TextureId, Warning,
};
use rustc_hash::FxHashMap;

use crate::pmx::document::{
    Pmx, PmxBone, PmxBoneFlags, PmxDisplayFrame, PmxDisplayTarget, PmxMaterial, PmxMaterialFlags,
    PmxMorph, PmxOffsets, PmxPanel, PmxSphereMode, PmxToon,
};
use crate::{EDGE_SCREEN_WIDTH, MMD_UNIT_SCALE};

const CENTRE: &str = "センター";
const ROOT_FRAME: &str = "Root";
const FACE_FRAME: &str = "表情";

pub(super) struct PmxGeometry {
    pub(super) positions: Vec<Vec3>,
    pub(super) normals: Vec<Vec3>,
    pub(super) uvs: Vec<Vec2>,
    pub(super) bindings: Vec<[(u16, f32); INFLUENCES]>,
    pub(super) indices: Vec<u32>,
    pub(super) runs: Vec<Run>,
    pub(super) bones: Vec<NodeId>,
    pub(super) morphs: Vec<Morph>,
}

pub(super) struct Run {
    pub(super) name: String,
    pub(super) material: Option<MaterialId>,
    pub(super) indices: u32,
}

pub(super) struct Morph {
    pub(super) name: String,
    pub(super) vertices: Vec<u32>,
    pub(super) offsets: Vec<Vec3>,
}

impl PmxGeometry {
    pub(super) fn gather(model: &Model) -> Result<Self> {
        let world = model.world_transforms();
        let bones = bone_nodes(model);
        let placement: FxHashMap<NodeId, u16> = bones
            .iter()
            .enumerate()
            .map(|(index, node)| (*node, index as u16))
            .collect();

        let mut geometry = Self {
            positions: Vec::new(),
            normals: Vec::new(),
            uvs: Vec::new(),
            bindings: Vec::new(),
            indices: Vec::new(),
            runs: Vec::new(),
            bones,
            morphs: Vec::new(),
        };

        let mut mesh_nodes = Vec::new();
        model.visit(|node| {
            if let Some(mesh) = model.node(node).mesh {
                mesh_nodes.push((node, mesh));
            }
        });
        for (node, mesh) in mesh_nodes {
            let source_mesh = model.mesh(mesh);
            let first = geometry.positions.len() as u32;
            let skinned = source_mesh.vertices.is_skinned() && model.node(node).skin.is_some();
            let place = match skinned {
                true => Mat4::IDENTITY,
                false => world[node.index()],
            };
            let joints = model
                .node(node)
                .skin
                .map(|skin| model.skin(skin).joints.as_slice())
                .unwrap_or_default();
            let fallback = model
                .node(node)
                .parent
                .and_then(|parent| placement.get(&parent).copied())
                .or_else(|| placement.get(&node).copied())
                .unwrap_or(0);

            for vertex in 0..source_mesh.vertices.len() {
                let position = source_mesh.vertices.positions[vertex];
                geometry.positions.push(place.transform_point3(position));
                let normal = source_mesh
                    .vertices
                    .normals
                    .get(vertex)
                    .copied()
                    .unwrap_or(Vec3::Y);
                geometry.normals.push(match skinned {
                    true => normal,
                    false => place.transform_vector3(normal).normalize_or(Vec3::Y),
                });
                geometry.uvs.push(
                    source_mesh
                        .vertices
                        .uvs
                        .get(vertex)
                        .copied()
                        .unwrap_or(Vec2::ZERO),
                );
                geometry.bindings.push(binding_of(
                    source_mesh.vertices.influences(vertex),
                    joints,
                    &placement,
                    fallback,
                ));
            }
            for target in &source_mesh.morph_targets {
                geometry.morphs.push(Morph {
                    name: target.name.clone(),
                    vertices: target
                        .vertices
                        .iter()
                        .map(|vertex| vertex + first)
                        .collect(),
                    offsets: target.positions.clone(),
                });
            }
            for submesh in &source_mesh.submeshes {
                let start = submesh.first_index as usize;
                let end = start + submesh.index_count as usize;
                geometry.indices.extend(
                    source_mesh.indices[start..end]
                        .iter()
                        .map(|index| index + first),
                );
                geometry.runs.push(Run {
                    name: match submesh.name.is_empty() {
                        true => source_mesh.name.clone(),
                        false => submesh.name.clone(),
                    },
                    material: submesh.material,
                    indices: submesh.index_count,
                });
            }
        }

        if geometry.positions.is_empty() {
            return Err(Error::Unmet {
                format: "PMX",
                requirement: Requirement::Vertices,
            });
        }
        Ok(geometry)
    }

    pub(super) fn matches(&self, pmx: &Pmx) -> bool {
        pmx.bones.len() == self.bones.len()
            && pmx.materials.len() == self.runs.len()
            && pmx.morphs.len() == self.morphs.len()
            && pmx.vertices.weights.len() == self.positions.len()
            && pmx
                .bones
                .iter()
                .zip(&self.bones)
                .all(|(bone, node)| bone.node == *node)
    }
}

pub(super) fn additional_uvs(pmx: Option<&Pmx>) -> u8 {
    pmx.map_or(0, |source| source.vertices.additional_uvs.len() as u8)
}

fn bone_nodes(model: &Model) -> Vec<NodeId> {
    let mut bones = Vec::new();
    let mut visited = vec![false; model.nodes.len()];
    let mut hold = |node: NodeId, bones: &mut Vec<NodeId>| {
        if let Some(slot) = visited.get_mut(node.index())
            && !std::mem::replace(slot, true)
        {
            bones.push(node);
        }
    };
    for skin in &model.skins {
        for joint in &skin.joints {
            hold(*joint, &mut bones);
        }
    }
    if bones.is_empty() {
        model.visit(|node| {
            if model.node(node).mesh.is_some() {
                hold(node, &mut bones);
            }
        });
    }
    bones
}

fn binding_of(
    influences: Option<[(u16, f32); INFLUENCES]>,
    skin: &[NodeId],
    placement: &FxHashMap<NodeId, u16>,
    fallback: u16,
) -> [(u16, f32); INFLUENCES] {
    let bone = |joint: u16| {
        skin.get(usize::from(joint))
            .and_then(|node| placement.get(node).copied())
            .unwrap_or(fallback)
    };
    let mut binding = influences.map_or([(0, 0.0); INFLUENCES], |vertex_influences| {
        vertex_influences.map(|(joint, weight)| (bone(joint), weight))
    });
    if binding.iter().all(|(_, weight)| *weight <= 0.0) {
        binding[0] = (fallback, 1.0);
    }
    binding
}

pub(super) fn write_textures(
    model: &Model,
    pmx: Option<&Pmx>,
    ctx: &mut ExportCtx<'_>,
) -> Result<Vec<String>> {
    let table: Vec<(String, Option<&Image>)> = match pmx {
        Some(pmx) => pmx
            .textures
            .iter()
            .map(|path| {
                let image = model.images.iter().find(|image| image.name == *path);
                (path.clone(), image)
            })
            .collect(),
        None => model
            .images
            .iter()
            .enumerate()
            .map(|(index, image)| (image.file_name(index).into_owned(), Some(image)))
            .collect(),
    };

    for (path, image) in &table {
        let Some(image) = image else {
            continue;
        };
        match ctx.write_resource(path, &image.bytes) {
            Ok(()) => {}
            Err(Error::Write { uri, reason }) => ctx.warn(Warning::MissingResource { uri, reason }),
            Err(error) => return Err(error),
        }
    }
    Ok(table.into_iter().map(|(path, _)| path).collect())
}

fn texture_index(model: &Model, texture: Option<TextureId>) -> Option<usize> {
    let texture = texture?;
    model
        .textures
        .get(texture.index())
        .map(|model_texture| model_texture.image.index())
}

fn ambient_of(material: &Material) -> Vec3 {
    match material.shading {
        Shading::Toon(toon) => toon.shade_color,
        Shading::Pbr(_) => material.emissive,
    }
}

pub(super) fn materials_of(
    model: &Model,
    geometry: &PmxGeometry,
    pmx: Option<&Pmx>,
    textures: &[String],
) -> Vec<PmxMaterial> {
    geometry
        .runs
        .iter()
        .enumerate()
        .map(|(index, run)| {
            if let Some(source_material) = pmx.and_then(|pmx| pmx.materials.get(index)) {
                return source_material.clone();
            }
            let material = run.material.map(|id| model.material(id));
            let mut pmx_material = PmxMaterial {
                name: material.map_or_else(|| run.name.clone(), |source| source.name.clone()),
                diffuse: material.map_or(Vec4::ONE, |source| source.base_color),
                specular: Vec3::ZERO,
                specular_power: 5.0,
                ambient: material.map_or(Vec3::splat(0.5), ambient_of),
                edge_color: Vec4::new(0.0, 0.0, 0.0, 1.0),
                edge_size: 1.0,
                ..PmxMaterial::default()
            };
            if let Some(material) = material {
                pmx_material
                    .flags
                    .set(PmxMaterialFlags::DOUBLE_SIDED, material.double_sided);
                pmx_material
                    .flags
                    .set(PmxMaterialFlags::GROUND_SHADOW, material.casts_shadow);
                pmx_material
                    .flags
                    .set(PmxMaterialFlags::CASTS_SHADOW, material.casts_shadow);
                pmx_material
                    .flags
                    .set(PmxMaterialFlags::RECEIVES_SHADOW, material.receives_shadow);
                let known =
                    |texture| texture_index(model, texture).filter(|index| *index < textures.len());
                pmx_material.texture = known(material.textures.base_color);
                pmx_material.toon = PmxToon::Texture(known(material.textures.shading_ramp));
                pmx_material.sphere = known(material.textures.matcap);
                if pmx_material.sphere.is_some() {
                    pmx_material.sphere_mode = match material.shading {
                        Shading::Toon(toon) if toon.matcap_blend == MatcapBlend::Multiply => {
                            PmxSphereMode::Multiply
                        }
                        _ => PmxSphereMode::Add,
                    };
                }
                if let Shading::Toon(toon) = material.shading {
                    let outline = toon.outline;
                    pmx_material
                        .flags
                        .set(PmxMaterialFlags::OUTLINE, outline.width.is_drawn());
                    pmx_material.edge_color = outline.color.extend(1.0);
                    pmx_material.edge_size = match outline.width {
                        OutlineWidth::Screen(width) => width / EDGE_SCREEN_WIDTH,
                        OutlineWidth::World(width) => width / MMD_UNIT_SCALE,
                        OutlineWidth::None => 1.0,
                    };
                }
            }
            pmx_material
        })
        .collect()
}

pub(super) fn bones_of<'a>(geometry: &PmxGeometry, pmx: Option<&'a Pmx>) -> Cow<'a, [PmxBone]> {
    if let Some(pmx) = pmx {
        return Cow::Borrowed(&pmx.bones);
    }
    geometry
        .bones
        .iter()
        .enumerate()
        .map(|(index, node)| PmxBone {
            name: match index {
                0 => CENTRE.to_owned(),
                _ => format!("bone{index}"),
            },
            node: *node,
            flags: PmxBoneFlags(
                PmxBoneFlags::ROTATABLE | PmxBoneFlags::VISIBLE | PmxBoneFlags::OPERABLE,
            ),
            ..PmxBone::default()
        })
        .collect()
}

pub(super) fn morphs_of<'a>(geometry: &PmxGeometry, pmx: Option<&'a Pmx>) -> Cow<'a, [PmxMorph]> {
    match pmx {
        Some(pmx) => Cow::Borrowed(&pmx.morphs),
        None => geometry
            .morphs
            .iter()
            .map(|morph| PmxMorph {
                name: morph.name.clone(),
                english_name: morph.name.clone(),
                panel: PmxPanel::Other,
                offsets: PmxOffsets::Vertex,
            })
            .collect(),
    }
}

pub(super) fn frames_of<'a>(
    pmx: Option<&'a Pmx>,
    bones: &[PmxBone],
    morphs: &[PmxMorph],
) -> Cow<'a, [PmxDisplayFrame]> {
    if let Some(pmx) = pmx.filter(|pmx| !pmx.display_frames.is_empty()) {
        return Cow::Borrowed(&pmx.display_frames);
    }
    let mut frames = vec![
        PmxDisplayFrame {
            name: ROOT_FRAME.to_owned(),
            english_name: ROOT_FRAME.to_owned(),
            special: true,
            targets: bones
                .first()
                .map(|_| vec![PmxDisplayTarget::Bone(0)])
                .unwrap_or_default(),
        },
        PmxDisplayFrame {
            name: FACE_FRAME.to_owned(),
            english_name: "Exp".to_owned(),
            special: true,
            targets: (0..morphs.len()).map(PmxDisplayTarget::Morph).collect(),
        },
    ];
    if bones.len() > 1 {
        frames.push(PmxDisplayFrame {
            name: "Bone".to_owned(),
            english_name: "Bone".to_owned(),
            special: false,
            targets: (1..bones.len()).map(PmxDisplayTarget::Bone).collect(),
        });
    }
    Cow::Owned(frames)
}

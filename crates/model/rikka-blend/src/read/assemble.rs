use rikka_model_core::glam::{Mat4, Quat};
use rikka_model_core::{
    Animation, CoordinateSystem, ImportCtx, Limit, MaterialId, Model, ModelBuilder, Node, NodeId,
    Pose, Result, Skin, Stage as Progress, Transform,
};
use rikka_polymesh::{ColorSource, ToMesh};
use rustc_hash::FxHashMap;

use super::animation::Clock;
use super::armature::{self, Channel, Skeleton};
use super::material::Library;
use super::modifier::{self, Notes, Scope};
use super::motion::Motion;
use super::scene::{self, OB_ARMATURE, OB_CAMERA, OB_EMPTY, OB_LAMP, OB_MESH, Stage};
use super::transform::{finite, inverse};
use super::{camera, light, mesh};
use crate::file::{Blend, Record};

const NAME: &str = "Blender";
const DEFORM_GROUPS: i64 = 1;
const DEFORM_ENVELOPE: i64 = 1 << 1;
const DEFORM_DUAL_QUATERNION: i64 = 1 << 2;
const UNITS_NONE: i64 = 0;

pub(crate) struct Rig {
    pub(crate) skeleton: Skeleton,
    pub(crate) channels: Vec<Channel>,
    pub(crate) joints: Vec<NodeId>,
    pub(crate) bone_worlds: Vec<Mat4>,
}

pub(crate) struct ShapeKeyBinding<'a> {
    pub(crate) node: NodeId,
    pub(crate) key: Record<'a>,
    pub(crate) shapes: Vec<(String, f32)>,
}

pub(crate) struct Assembly<'a> {
    pub(crate) stage: Stage<'a>,
    pub(crate) nodes: Vec<Option<NodeId>>,
    pub(crate) rigs: FxHashMap<usize, Rig>,
    pub(crate) morphed: Vec<ShapeKeyBinding<'a>>,
}

pub(crate) fn assemble<'a>(blend: &'a Blend<'a>, ctx: &mut ImportCtx<'_>) -> Result<Model> {
    let current = scene::scene(blend);
    let stage = scene::stage(blend, current);
    let version = blend.version();
    let mut builder = ModelBuilder::new()
        .importer(NAME)
        .generator(format!("Blender {}.{}", version / 100, version % 100))
        .coordinates(CoordinateSystem::RIGHT_HANDED_Z_UP)
        .unit_scale(unit_scale(current));
    let mut notes = Notes::default();
    let mut assembly = Assembly {
        nodes: vec![None; stage.placements.len()],
        stage,
        rigs: FxHashMap::default(),
        morphed: Vec::new(),
    };
    let needed = needed(&assembly.stage);
    for index in 0..assembly.stage.placements.len() {
        if needed[index] {
            place(&mut assembly, &mut builder, index);
            if assembly.stage.placements[index].object.int("type") == Some(OB_ARMATURE) {
                rig(&mut assembly, &mut builder, index);
            }
        }
    }
    let aspect = camera::aspect(current);
    let mut library = Library::default();
    let count = assembly.stage.placements.len().max(1);
    for index in 0..assembly.stage.placements.len() {
        ctx.checkpoint()?;
        ctx.report(Progress::Converting, index as f32 / count as f32);
        let placement = &assembly.stage.placements[index];
        if !placement.shown {
            continue;
        }
        let object = placement.object;
        let Some(node) = assembly.nodes[index] else {
            continue;
        };
        match object.int("type").unwrap_or(OB_EMPTY) {
            OB_MESH => geometry(
                &mut assembly,
                &mut builder,
                &mut library,
                &mut notes,
                ctx,
                index,
            )?,
            OB_CAMERA => {
                if let Some(data) = object.id("data") {
                    let mut lens = facing(&object.id_name(), node);
                    lens.camera =
                        Some(builder.add_camera(camera::camera(&data, aspect, &mut notes)));
                    builder.add_node(lens);
                }
            }
            OB_LAMP => {
                if let Some(data) = object.id("data")
                    && let Some(made) = light::light(&data, current, &mut notes)
                {
                    let mut lamp = facing(&object.id_name(), node);
                    lamp.light = Some(builder.add_light(made));
                    builder.add_node(lamp);
                }
            }
            OB_EMPTY | OB_ARMATURE => {}
            kind => notes.unsupported(format!(
                "skipping object \"{}\" of type {}",
                object.id_name(),
                kind_name(kind)
            )),
        }
    }
    let clock = Clock::from_scene(current);
    let motion = Motion::resolve(&assembly, builder.model_mut(), &mut notes);
    ctx.options()
        .limits
        .check(Limit::Keyframes, clock.frame_count())?;
    let animation = motion.bake(&clock, ctx)?;
    if let Some(shown) = motion.still(clock.current as f32) {
        show_frame(builder.model_mut(), &shown, 0.0);
    }
    if let Some(animation) = animation {
        builder.add_animation(animation);
    }
    for warning in notes.warnings {
        ctx.warn(warning);
    }
    ctx.report(Progress::Finishing, 1.0);
    builder.finish()
}

fn show_frame(model: &mut Model, animation: &Animation, time: f32) {
    let mut pose = Pose::rest(model);
    animation.apply(time, &mut pose);
    for (index, node) in model.nodes.iter_mut().enumerate() {
        let id = NodeId(index as u32);
        node.transform = pose.local(id);
        if !node.morph_weights.is_empty() {
            node.morph_weights = pose.weights(id).to_vec();
        }
    }
}

fn unit_scale(scene: Option<Record<'_>>) -> f32 {
    scene
        .and_then(|scene| scene.record("unit"))
        .filter(|unit| unit.int("system").unwrap_or(1) != UNITS_NONE)
        .and_then(|unit| unit.float("scale_length"))
        .filter(|scale| scale.is_finite() && *scale > 0.0)
        .unwrap_or(1.0)
}

fn kind_name(kind: i64) -> &'static str {
    match kind {
        2 => "curve",
        3 => "surface",
        4 => "text object",
        5 => "metaball",
        9 => "lattice",
        26 => "grease pencil object",
        27 => "hair curves object",
        28 => "point cloud",
        29 => "volume",
        30 => "grease pencil object",
        _ => "object type has no mesh representation",
    }
}

fn needed(stage: &Stage<'_>) -> Vec<bool> {
    let mut needed: Vec<bool> = stage
        .placements
        .iter()
        .map(|placement| placement.shown)
        .collect();
    for index in 0..stage.placements.len() {
        let placement = &stage.placements[index];
        if !placement.shown || placement.object.int("type") != Some(OB_MESH) {
            continue;
        }
        let armature = modifier::armature_of(&placement.object)
            .or_else(|| scene::skeletal_parent(&placement.object));
        if let Some(armature) = armature
            && let Some(rig) = stage.index_of(&armature)
        {
            needed[rig] = true;
        }
    }
    for index in (0..stage.placements.len()).rev() {
        if needed[index] {
            let mut parent = stage.placements[index].parent;
            while let Some(above) = parent {
                if needed[above] {
                    break;
                }
                needed[above] = true;
                parent = stage.placements[above].parent;
            }
        }
    }
    needed
}

fn place(assembly: &mut Assembly<'_>, builder: &mut ModelBuilder, index: usize) {
    let placement = &assembly.stage.placements[index];
    let mut parent = placement.parent.and_then(|parent| {
        Some((
            assembly.nodes[parent]?,
            assembly.stage.placements[parent].world,
        ))
    });
    if let Some(above) = placement.parent
        && !placement.instanced
        && let Some(bone) = scene::parent_bone(&placement.object)
        && let Some(rig) = assembly.rigs.get(&above)
        && let Some(bone_index) = rig.skeleton.find(&bone)
    {
        parent = Some((rig.joints[bone_index], rig.bone_worlds[bone_index]));
    }
    let local = match parent {
        Some((_, world)) => inverse(world) * placement.world,
        None => placement.world,
    };
    let mut node =
        Node::new(placement.object.id_name()).transform(Transform::from_matrix(finite(local)));
    node.parent = parent.map(|(node, _)| node);
    assembly.nodes[index] = Some(builder.add_node(node));
}

fn rig(assembly: &mut Assembly<'_>, builder: &mut ModelBuilder, index: usize) {
    let object = assembly.stage.placements[index].object;
    let Some(node) = assembly.nodes[index] else {
        return;
    };
    let skeleton = object.id("data").map(Skeleton::read).unwrap_or_default();
    let channels = armature::channels(object, &skeleton);
    let posed = armature::current(&skeleton, &channels);
    let armature_world = assembly.stage.placements[index].world;
    let mut joints: Vec<NodeId> = Vec::with_capacity(skeleton.bones.len());
    for (bone_index, bone) in skeleton.bones.iter().enumerate() {
        let (local, parent) = match bone.parent {
            Some(parent) => (inverse(posed[parent]) * posed[bone_index], joints[parent]),
            None => (posed[bone_index], node),
        };
        let mut joint =
            Node::new(bone.name.clone()).transform(Transform::from_matrix(finite(local)));
        joint.parent = Some(parent);
        joints.push(builder.add_node(joint));
    }
    assembly.rigs.insert(
        index,
        Rig {
            skeleton,
            channels,
            joints,
            bone_worlds: posed.iter().map(|pose| armature_world * *pose).collect(),
        },
    );
}

fn facing(name: &str, parent: NodeId) -> Node {
    let mut child = Node::new(name).transform(Transform {
        rotation: Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2),
        ..Transform::IDENTITY
    });
    child.parent = Some(parent);
    child
}

fn geometry<'a>(
    assembly: &mut Assembly<'a>,
    builder: &mut ModelBuilder,
    library: &mut Library,
    notes: &mut Notes,
    ctx: &mut ImportCtx<'_>,
    index: usize,
) -> Result<()> {
    let placement = &assembly.stage.placements[index];
    let object = placement.object;
    let world = placement.world;
    let Some(node) = assembly.nodes[index] else {
        return Ok(());
    };
    let Some(data) = object.id("data") else {
        return Ok(());
    };
    let name = object.id_name();
    let slots = material_slots(&object, &data);
    let mut materials: Vec<Option<MaterialId>> = Vec::with_capacity(slots.len());
    let mut colors = false;
    let mut uv_map: Option<String> = None;
    for slot in &slots {
        match slot {
            Some(material) => {
                let (id, wants) = library.material(*material, builder, ctx, notes)?;
                colors |= wants.colors;
                if let Some(uv_name) = wants.uv_map {
                    match &uv_map {
                        Some(current_uv) if *current_uv != uv_name => notes.unsupported(format!(
                            "materials of object \"{name}\" use different UV maps, using \"{current_uv}\""
                        )),
                        Some(_) => {}
                        None => uv_map = Some(uv_name),
                    }
                }
                materials.push(Some(id));
            }
            None => materials.push(None),
        }
    }

    let stage = &assembly.stage;
    let world_of = |record: Record<'a>| stage.world_of(record);
    let scope = Scope {
        world,
        world_of: &world_of,
    };
    let evaluated = mesh::evaluate(object, data, &scope, notes);
    if evaluated.mesh.face_count() == 0 {
        return Ok(());
    }

    let deform = evaluated
        .deform
        .as_ref()
        .map(|deform| (deform.armature, deform.flags))
        .or_else(|| scene::skeletal_parent(&object).map(|parent| (parent, DEFORM_GROUPS)));
    let rig = deform.and_then(|(armature, flags)| {
        if flags & DEFORM_DUAL_QUATERNION != 0 {
            notes.unsupported(format!(
                "object \"{name}\": dual quaternion skinning read as linear blend skinning"
            ));
        }
        if flags & DEFORM_GROUPS == 0 {
            if flags & DEFORM_ENVELOPE != 0 {
                notes.unsupported(format!("object \"{name}\": bone envelopes not evaluated"));
            }
            return None;
        }
        let at = stage.index_of(&armature)?;
        Some((at, assembly.rigs.get(&at)?))
    });
    let joint_of: Vec<Option<u16>> = match rig {
        Some((_, rig)) => evaluated
            .mesh
            .vertex_groups
            .groups
            .iter()
            .map(|group| {
                rig.skeleton
                    .find(&group.name)
                    .filter(|&bone| rig.skeleton.bones[bone].deform)
                    .and_then(|bone| u16::try_from(bone).ok())
            })
            .collect(),
        None => Vec::new(),
    };
    if let Some((_, rig)) = rig
        && rig
            .skeleton
            .bones
            .iter()
            .any(|bone| bone.deform && bone.segments > 1)
    {
        notes.unsupported(format!(
            "bendy bones deforming object \"{name}\" read as plain bones"
        ));
    }
    let skinned = rig.is_some()
        && joint_of.iter().any(Option::is_some)
        && evaluated.mesh.vertex_groups.has_weights();
    let mut options = ToMesh::new(name.clone())
        .shading(evaluated.shading)
        .materials(materials)
        .colors(match colors {
            true => ColorSource::Render,
            false => ColorSource::None,
        });
    if let Some(uv_name) = &uv_map {
        options = options.uv_map(uv_name.clone());
    }
    if skinned {
        options = options.joints(joint_of);
    }
    let mut converted = evaluated.mesh.to_mesh(&options);
    for submesh in &mut converted.mesh.submeshes {
        if submesh.material.is_none() {
            submesh.material = Some(library.fallback(builder));
        }
    }
    let keys = &evaluated.mesh.shape_keys.keys;
    let shapes: Vec<(String, f32)> = converted
        .shape_keys
        .iter()
        .map(|&key| {
            let weight = match evaluated.pinned_key {
                Some(pinned) => f32::from(u8::from(pinned == key as usize)),
                None => keys[key as usize].value,
            };
            (keys[key as usize].name.clone(), weight)
        })
        .collect();
    let mesh_id = builder.add_mesh(converted.mesh);
    let skin = match (skinned, rig) {
        (true, Some((at, rig))) => {
            let armature_world = assembly.stage.placements[at].world;
            let mut skin = Skin::new(name.clone());
            skin.joints = rig.joints.clone();
            skin.inverse_bind = rig
                .skeleton
                .bones
                .iter()
                .map(|bone| finite(inverse(armature_world * bone.rest) * world))
                .collect();
            skin.skeleton = assembly.nodes[at];
            Some(builder.add_skin(skin))
        }
        _ => None,
    };
    let target = &mut builder.model_mut().nodes[node.index()];
    target.mesh = Some(mesh_id);
    target.skin = skin;
    target.morph_weights = shapes.iter().map(|(_, weight)| *weight).collect();
    if !shapes.is_empty()
        && let Some(key) = data.id("key")
    {
        assembly.morphed.push(ShapeKeyBinding { node, key, shapes });
    }
    Ok(())
}

fn material_slots<'a>(object: &Record<'a>, data: &Record<'a>) -> Vec<Option<Record<'a>>> {
    let count = object
        .int("totcol")
        .unwrap_or(0)
        .max(data.int("totcol").unwrap_or(0))
        .max(0) as usize;
    let object_materials = object.pointers("mat");
    let data_materials = data.pointers("mat");
    let bits = object.raw("matbits").unwrap_or_default();
    (0..count)
        .map(|slot| {
            let from_object = bits.get(slot).is_some_and(|&bit| bit != 0);
            let address = match from_object {
                true => object_materials.get(slot),
                false => data_materials.get(slot),
            };
            address
                .filter(|&&address| address != 0)
                .and_then(|&address| object.id_at(address))
        })
        .collect()
}

use rikka_model_core::glam::{Mat4, Vec3};
use rustc_hash::{FxHashMap, FxHashSet};

use super::armature::{self, Skeleton};
use super::transform::{finite, inverse, object_basis, triangle_frame};
use crate::file::{Blend, Identity, Record};

pub(crate) const OB_EMPTY: i64 = 0;
pub(crate) const OB_MESH: i64 = 1;
pub(crate) const OB_LAMP: i64 = 10;
pub(crate) const OB_CAMERA: i64 = 11;
pub(crate) const OB_ARMATURE: i64 = 25;

const PAROBJECT: i64 = 0;
const PARSKEL: i64 = 4;
const PARVERT1: i64 = 5;
const PARVERT3: i64 = 6;
const PARBONE: i64 = 7;
const PARTYPE: i64 = 15;
const OB_HIDE_VIEWPORT: i64 = 1;
const OB_DUPLICOLLECTION: i64 = 1 << 8;
const COLLECTION_HIDE_VIEWPORT: i64 = 1;
const LAYER_COLLECTION_EXCLUDE: i64 = 1 << 4;
const LAYER_COLLECTION_HIDE: i64 = 1 << 7;
const BASE_HIDDEN: i64 = 1 << 8;
const INSTANCE_DEPTH: usize = 8;

pub(crate) struct ObjectPlacement<'a> {
    pub(crate) object: Record<'a>,
    pub(crate) parent: Option<usize>,
    pub(crate) world: Mat4,
    pub(crate) shown: bool,
    pub(crate) instanced: bool,
}

pub(crate) struct Stage<'a> {
    pub(crate) scene: Option<Record<'a>>,
    pub(crate) placements: Vec<ObjectPlacement<'a>>,
    index: FxHashMap<Identity, usize>,
}

impl<'a> Stage<'a> {
    pub(crate) fn index_of(&self, object: &Record<'a>) -> Option<usize> {
        self.index.get(&object.identity()).copied()
    }

    pub(crate) fn world_of(&self, object: Record<'a>) -> Mat4 {
        match self.index_of(&object) {
            Some(index) => self.placements[index].world,
            None => Placer::default().world(object, 0),
        }
    }
}

pub(crate) fn skeletal_parent<'a>(object: &Record<'a>) -> Option<Record<'a>> {
    (object.int("partype").unwrap_or(PAROBJECT) & PARTYPE == PARSKEL)
        .then(|| object.id("parent"))
        .flatten()
        .filter(|parent| parent.int("type") == Some(OB_ARMATURE))
}

pub(crate) fn parent_bone(object: &Record<'_>) -> Option<String> {
    (object.int("partype").unwrap_or(PAROBJECT) & PARTYPE == PARBONE)
        .then(|| object.text("parsubstr"))
        .flatten()
        .filter(|bone| !bone.is_empty())
}

pub(crate) fn scene<'a>(blend: &'a Blend<'a>) -> Option<Record<'a>> {
    blend
        .global()
        .and_then(|global| global.id("curscene"))
        .or_else(|| blend.ids(*b"SC").next())
}

#[derive(Default)]
struct Visible<'a> {
    objects: Vec<(Record<'a>, bool)>,
    seen: FxHashMap<Identity, usize>,
}

impl<'a> Visible<'a> {
    fn add(&mut self, object: Record<'a>, shown: bool) {
        if object.address().is_none() {
            return;
        }
        match self.seen.get(&object.identity()) {
            Some(&index) => self.objects[index].1 |= shown,
            None => {
                self.seen.insert(object.identity(), self.objects.len());
                self.objects.push((object, shown));
            }
        }
    }
}

pub(crate) fn stage<'a>(blend: &'a Blend<'a>, scene: Option<Record<'a>>) -> Stage<'a> {
    let mut visible = Visible::default();
    match scene {
        Some(scene) => {
            let mut scenes = vec![scene];
            let mut current = scene;
            while let Some(set) = current.id("set") {
                if scenes.len() > 16
                    || scenes
                        .iter()
                        .any(|visited| visited.identity() == set.identity())
                {
                    break;
                }
                scenes.push(set);
                current = set;
            }
            for scene in scenes {
                gather_scene(scene, &mut visible);
            }
        }
        None => {
            for object in blend.ids(*b"OB") {
                let hidden = object.int("restrictflag").unwrap_or(0) & OB_HIDE_VIEWPORT != 0;
                visible.add(object, !hidden);
            }
        }
    }

    let mut placer = Placer::default();
    for (object, shown) in visible.objects {
        placer.place(object, shown);
    }
    Stage {
        scene,
        placements: placer.placements,
        index: placer.index,
    }
}

fn gather_scene<'a>(scene: Record<'a>, visible: &mut Visible<'a>) {
    if let Some(master) = scene.deref("master_collection") {
        let layers = scene.list("view_layers");
        let current_layer = scene
            .blend()
            .global()
            .and_then(|global| global.pointer("cur_view_layer"));
        let layer = layers
            .iter()
            .find(|layer| layer.address().is_some() && layer.address() == current_layer)
            .or_else(|| layers.first())
            .copied();
        let hidden_bases: FxHashSet<u64> = layer
            .map(|layer| {
                layer
                    .list("object_bases")
                    .iter()
                    .filter(|base| base.int("flag").unwrap_or(0) & BASE_HIDDEN != 0)
                    .filter_map(|base| base.pointer("object"))
                    .collect()
            })
            .unwrap_or_default();
        let root = layer.and_then(|layer| layer.list("layer_collections").first().copied());
        collection(master, root, false, &hidden_bases, visible, 0);
        return;
    }
    let lay = scene.int("lay").unwrap_or(-1);
    for base in scene.list("base") {
        let Some(object) = base.id("object") else {
            continue;
        };
        let on_layer = base.int("lay").unwrap_or(-1) & lay != 0;
        let hidden = object.int("restrictflag").unwrap_or(0) & OB_HIDE_VIEWPORT != 0;
        visible.add(object, on_layer && !hidden);
    }
}

fn collection<'a>(
    collection: Record<'a>,
    layer: Option<Record<'a>>,
    hidden: bool,
    hidden_bases: &FxHashSet<u64>,
    visible: &mut Visible<'a>,
    depth: usize,
) {
    if depth > 64 {
        return;
    }
    let layer_flag = layer.and_then(|layer| layer.int("flag")).unwrap_or(0);
    if layer_flag & LAYER_COLLECTION_EXCLUDE != 0
        || collection.int("flag").unwrap_or(0) & COLLECTION_HIDE_VIEWPORT != 0
    {
        return;
    }
    let hidden = hidden || layer_flag & LAYER_COLLECTION_HIDE != 0;
    for entry in collection.list("gobject") {
        let Some(pointer) = entry.pointer("ob") else {
            continue;
        };
        let Some(object) = entry.id_at(pointer) else {
            continue;
        };
        let restricted = object.int("restrictflag").unwrap_or(0) & OB_HIDE_VIEWPORT != 0;
        let base_hidden = hidden_bases.contains(&pointer);
        visible.add(object, !(hidden || restricted || base_hidden));
    }
    let children = layer
        .map(|layer| layer.list("layer_collections"))
        .unwrap_or_default();
    for child in collection.list("children") {
        let Some(pointer) = child.pointer("collection") else {
            continue;
        };
        let Some(inner) = child.id_at(pointer) else {
            continue;
        };
        let inner_layer = children
            .iter()
            .find(|candidate| candidate.pointer("collection") == Some(pointer))
            .copied();
        self::collection(inner, inner_layer, hidden, hidden_bases, visible, depth + 1);
    }
}

pub(crate) fn collection_objects<'a>(
    collection: Record<'a>,
    out: &mut Vec<Record<'a>>,
    depth: usize,
) {
    if depth > 64 || collection.int("flag").unwrap_or(0) & COLLECTION_HIDE_VIEWPORT != 0 {
        return;
    }
    for entry in collection.list("gobject") {
        if let Some(object) = entry.id("ob")
            && object.int("restrictflag").unwrap_or(0) & OB_HIDE_VIEWPORT == 0
            && !out
                .iter()
                .any(|collected| collected.identity() == object.identity())
        {
            out.push(object);
        }
    }
    for child in collection.list("children") {
        if let Some(inner) = child.id("collection") {
            collection_objects(inner, out, depth + 1);
        }
    }
}

#[derive(Default)]
struct Placer<'a> {
    worlds: FxHashMap<Identity, Mat4>,
    skeletons: FxHashMap<Identity, (Skeleton, Vec<Mat4>)>,
    placements: Vec<ObjectPlacement<'a>>,
    index: FxHashMap<Identity, usize>,
}

impl<'a> Placer<'a> {
    fn place(&mut self, object: Record<'a>, shown: bool) -> usize {
        if let Some(&index) = self.index.get(&object.identity()) {
            if shown {
                self.placements[index].shown = true;
            }
            return index;
        }
        let parent = object.id("parent").map(|parent| self.place(parent, false));
        let world = self.world(object, 0);
        let index = self.placements.len();
        self.placements.push(ObjectPlacement {
            object,
            parent,
            world,
            shown,
            instanced: false,
        });
        self.index.insert(object.identity(), index);
        if shown {
            self.instance(index, object, world, 0);
        }
        index
    }

    fn instance(&mut self, holder: usize, object: Record<'a>, world: Mat4, depth: usize) {
        if depth >= INSTANCE_DEPTH || object.int("transflag").unwrap_or(0) & OB_DUPLICOLLECTION == 0
        {
            return;
        }
        let Some(collection) = object
            .id("dup_group")
            .or_else(|| object.id("instance_collection"))
        else {
            return;
        };
        let offset = collection
            .vec3("dupli_ofs")
            .or_else(|| collection.vec3("instance_offset"))
            .unwrap_or(Vec3::ZERO);
        let mut members = Vec::new();
        collection_objects(collection, &mut members, 0);
        let base = world * Mat4::from_translation(-offset);
        for member in members {
            let member_world = base * self.world(member, 0);
            let index = self.placements.len();
            self.placements.push(ObjectPlacement {
                object: member,
                parent: Some(holder),
                world: member_world,
                shown: true,
                instanced: true,
            });
            self.instance(index, member, member_world, depth + 1);
        }
    }

    fn world(&mut self, object: Record<'a>, depth: usize) -> Mat4 {
        if let Some(&world) = self.worlds.get(&object.identity()) {
            return world;
        }
        let local = object_basis(&object).matrix();
        let world = match object.id("parent").filter(|_| depth < 256) {
            Some(parent) => {
                let parent_world = self.world(parent, depth + 1);
                let parent_matrix = self.parent_matrix(object, parent, parent_world);
                let inverse = object.matrix("parentinv").unwrap_or(Mat4::IDENTITY);
                parent_matrix * finite(inverse) * local
            }
            None => local,
        };
        let world = finite(world);
        self.worlds.insert(object.identity(), world);
        world
    }

    fn parent_matrix(
        &mut self,
        object: Record<'a>,
        parent: Record<'a>,
        parent_world: Mat4,
    ) -> Mat4 {
        match object.int("partype").unwrap_or(PAROBJECT) & PARTYPE {
            PARBONE => {
                let name = object.text("parsubstr").unwrap_or_default();
                let factor = object.float("parent_bone_head_tail_factor").unwrap_or(1.0);
                match self.bone_parent(parent, &name, factor) {
                    Some(matrix) => parent_world * matrix,
                    None => parent_world,
                }
            }
            PARVERT1 => {
                let [point] = vertices(parent, [object.int("par1").unwrap_or(0)]);
                parent_world * Mat4::from_translation(point)
            }
            PARVERT3 => {
                let [a, b, c] = vertices(
                    parent,
                    ["par1", "par2", "par3"].map(|field| object.int(field).unwrap_or(0)),
                );
                parent_world * triangle_frame(a, b, c)
            }
            _ => parent_world,
        }
    }

    fn bone_parent(
        &mut self,
        armature_object: Record<'a>,
        bone: &str,
        factor: f32,
    ) -> Option<Mat4> {
        if armature_object.int("type") != Some(OB_ARMATURE) {
            return None;
        }
        let entry = self
            .skeletons
            .entry(armature_object.identity())
            .or_insert_with(|| {
                let skeleton = armature_object
                    .id("data")
                    .map(Skeleton::read)
                    .unwrap_or_default();
                let channels = armature::channels(armature_object, &skeleton);
                let posed = armature::current(&skeleton, &channels);
                (skeleton, posed)
            });
        let index = entry.0.find(bone)?;
        let posed = entry.1[index];
        let bone = &entry.0.bones[index];
        if bone.relative_parent {
            return Some(posed * inverse(bone.rest));
        }
        let mut matrix = posed;
        let along = matrix.y_axis.truncate() * bone.length * factor;
        matrix.w_axis += along.extend(0.0);
        Some(matrix)
    }
}

fn vertices<const N: usize>(parent: Record<'_>, indices: [i64; N]) -> [Vec3; N] {
    let positions = match parent.int("type") == Some(OB_MESH) {
        true => parent
            .id("data")
            .map(|mesh| super::mesh::source::read(mesh, Some(parent)).mesh.positions)
            .unwrap_or_default(),
        false => Vec::new(),
    };
    indices.map(|index| {
        positions
            .get(index.max(0) as usize)
            .copied()
            .unwrap_or(Vec3::ZERO)
    })
}

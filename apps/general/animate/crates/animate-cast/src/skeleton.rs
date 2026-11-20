use animate_body::Body;
use animate_motion::Part;
use animate_sheet::BoneDisplay;
use rikka_model::glam::{Mat4, Quat, Vec3};
use rikka_model::humanoid::Anatomy;
use rikka_model::mmd::{Pmx, PmxBoneFlags};
use rikka_model::{Instance, Model, NodeId};
use rustc_hash::FxHashSet;

#[derive(Clone, Debug, PartialEq)]
pub struct Bone {
    pub name: String,
    pub node: NodeId,
    pub parent: Option<usize>,
    pub rest: Vec3,
    pub rest_rotation: Quat,
    pub turns: bool,
    pub moves: bool,
    pub reaches: bool,
    pub shown: bool,
    pub driven: bool,
    pub fixed_axis: Option<Vec3>,
}

impl Bone {
    pub fn editable(&self) -> bool {
        self.shown && !self.driven
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Skeleton {
    bones: Vec<Bone>,
    driven: FxHashSet<String>,
    display: BoneDisplay,
}

impl Skeleton {
    pub fn new(model: &Model, anatomy: &Anatomy, body: Option<&Body>) -> Self {
        let driven_by = |node: NodeId| body.is_some_and(|body| body.mapping().is_driven(node));
        let (bones, display) = match model.extensions.get::<Pmx>() {
            Some(pmx) => (
                pmx_bones(model, pmx, &driven_by),
                BoneDisplay::from_pmx(pmx),
            ),
            None => {
                let bones = joint_bones(model, &driven_by);
                let display = BoneDisplay::by_region(bones.iter().map(|bone| {
                    let part = anatomy.region(bone.node).map_or(Part::Root, Part::holding);
                    (bone.name.clone(), part)
                }));
                (bones, display)
            }
        };
        let driven = bones
            .iter()
            .filter(|bone| bone.driven)
            .map(|bone| bone.name.clone())
            .collect();
        Self {
            bones,
            driven,
            display,
        }
    }

    pub fn bones(&self) -> &[Bone] {
        &self.bones
    }

    pub fn bone(&self, index: usize) -> Option<&Bone> {
        self.bones.get(index)
    }

    pub fn find(&self, name: &str) -> Option<usize> {
        self.bones.iter().position(|bone| bone.name == name)
    }

    pub fn world_of(&self, instance: &Instance, bone: &str) -> Option<Mat4> {
        let node = self.bone(self.find(bone)?)?.node;
        Some(instance.world(node))
    }

    pub fn is_driven(&self, name: &str) -> bool {
        self.driven.contains(name)
    }

    pub fn display(&self) -> &BoneDisplay {
        &self.display
    }
}

fn pmx_bones(model: &Model, pmx: &Pmx, driven_by: &dyn Fn(NodeId) -> bool) -> Vec<Bone> {
    pmx.bones
        .iter()
        .map(|bone| {
            let has = |flag| bone.flags.has(flag);
            Bone {
                name: bone.name.clone(),
                node: bone.node,
                parent: bone.parent,
                rest: model
                    .nodes
                    .get(bone.node.index())
                    .map_or(Vec3::ZERO, |node| node.transform.translation),
                rest_rotation: Quat::IDENTITY,
                turns: has(PmxBoneFlags::ROTATABLE),
                moves: has(PmxBoneFlags::TRANSLATABLE),
                reaches: has(PmxBoneFlags::IK),
                shown: has(PmxBoneFlags::VISIBLE) && has(PmxBoneFlags::OPERABLE),
                driven: driven_by(bone.node),
                fixed_axis: bone.fixed_axis.filter(|_| has(PmxBoneFlags::FIXED_AXIS)),
            }
        })
        .collect()
}

fn joint_bones(model: &Model, driven_by: &dyn Fn(NodeId) -> bool) -> Vec<Bone> {
    let mut jointed = vec![false; model.nodes.len()];
    for joint in model.skins.iter().flat_map(|skin| &skin.joints) {
        let mut cursor = Some(*joint);
        while let Some(node) =
            cursor.filter(|node| !jointed[node.index()] && model.node(*node).mesh.is_none())
        {
            jointed[node.index()] = true;
            cursor = model.node(node).parent;
        }
    }
    let order: Vec<NodeId> = model
        .order()
        .into_iter()
        .filter(|node| jointed[node.index()])
        .collect();
    let mut placed = vec![None; model.nodes.len()];
    for (index, node) in order.iter().enumerate() {
        placed[node.index()] = Some(index);
    }
    let index_of = |node: NodeId| placed[node.index()];
    order
        .iter()
        .map(|node| {
            let parent = model.node(*node).parent.and_then(index_of);
            Bone {
                name: model.node(*node).name.clone(),
                node: *node,
                parent,
                rest: model.node(*node).transform.translation,
                rest_rotation: model.node(*node).transform.rotation,
                turns: true,
                moves: parent.is_none(),
                reaches: false,
                shown: true,
                driven: driven_by(*node),
                fixed_axis: None,
            }
        })
        .collect()
}

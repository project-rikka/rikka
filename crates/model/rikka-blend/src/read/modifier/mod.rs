mod armature;
mod array;
mod bevel;
mod boolean;
mod context;
mod corrective_smooth;
mod data_transfer;
mod decimate;
mod displace;
mod edge_split;
mod lattice;
mod mirror;
mod shrinkwrap;
mod smooth;
mod solidify;
mod subdivision;
mod triangulate;
mod weighted_normal;
mod weld;

use rikka_mesh_modifiers::{
    CollectionRef, Modifier, ModifierKind, NoteKind, ObjectRef, Purpose, SkipReason, TextureRef,
    evaluate,
};
use rikka_model_core::Warning;
use rikka_model_core::glam::Mat4;
use rikka_polymesh::Polymesh;
use rustc_hash::FxHashSet;

use crate::file::Record;
use crate::write::Struct;
use context::{BlendContext, Objects};

pub(crate) use smooth::socket_identifier;

pub(crate) const REALTIME: i64 = 1;
pub(crate) const RENDER: i64 = 1 << 1;
pub(crate) const EDIT_MODE: i64 = 1 << 2;
pub(crate) const ON_CAGE: i64 = 1 << 3;
pub(crate) const PANEL_EXPANDED: i64 = 1;
pub(crate) const MODIFIER_PIN_LAST: i64 = 1 << 4;

const LATTICE: i64 = 2;
const SUBSURF: i64 = 1;
const MIRROR: i64 = 5;
const DECIMATE: i64 = 6;
const ARMATURE: i64 = 8;
const BOOLEAN: i64 = 11;
const ARRAY: i64 = 12;
const EDGE_SPLIT: i64 = 13;
const DISPLACE: i64 = 14;
const PARTICLES: i64 = 19;
const COLLISION: i64 = 23;
const BEVEL: i64 = 24;
const SHRINKWRAP: i64 = 25;
const SURFACE: i64 = 30;
const SHAPE_KEY: i64 = 32;
const SOLIDIFY: i64 = 33;
const TRIANGULATE: i64 = 44;
const CORRECTIVE_SMOOTH: i64 = 51;
const WEIGHTED_NORMAL: i64 = 54;
const DATA_TRANSFER: i64 = 49;
const WELD: i64 = 55;
const NODES: i64 = 57;

pub(crate) trait References<'a> {
    fn object(&self, record: Option<Record<'a>>) -> Option<ObjectRef>;
    fn texture(&self, record: Option<Record<'a>>) -> Option<TextureRef>;
    fn collection(&self, record: Option<Record<'a>>) -> Option<CollectionRef>;
}

pub(crate) trait Targets {
    fn object(&self, reference: Option<ObjectRef>) -> u64;
    fn texture(&self, reference: Option<TextureRef>) -> u64;
    fn collection(&self, reference: Option<CollectionRef>) -> u64;
}

pub(crate) enum Description {
    Kind {
        kind: ModifierKind,
        note: Option<String>,
    },
    Internal,
    Unsupported(String),
}

pub(crate) fn describe<'a>(
    modifier: &Record<'a>,
    owner: &Record<'a>,
    refs: &dyn References<'a>,
) -> Description {
    let kind = modifier
        .record("modifier")
        .and_then(|base| base.int("type"))
        .unwrap_or(0);
    let label = modifier
        .record("modifier")
        .and_then(|base| base.text("name"))
        .unwrap_or_default();
    let described: ModifierKind = match kind {
        MIRROR => mirror::read(modifier, refs).into(),
        SUBSURF => subdivision::read(modifier).into(),
        ARRAY => {
            let parsed = array::read(modifier, refs);
            return Description::Kind {
                kind: parsed.array.into(),
                note: parsed.fit_curve.then(|| {
                    format!(
                        "array modifier \"{label}\": fit-to-curve not supported, using fixed length"
                    )
                }),
            };
        }
        SOLIDIFY => solidify::read(modifier).into(),
        TRIANGULATE => triangulate::read(modifier).into(),
        WELD => weld::read(modifier).into(),
        EDGE_SPLIT => edge_split::read(modifier).into(),
        DISPLACE => displace::read(modifier, refs).into(),
        SHRINKWRAP => shrinkwrap::read(modifier, refs).into(),
        DATA_TRANSFER => data_transfer::read(modifier, refs).into(),
        ARMATURE => armature::read(modifier, refs).into(),
        BEVEL => bevel::read(modifier).into(),
        BOOLEAN => boolean::read(modifier, refs).into(),
        DECIMATE => decimate::read(modifier).into(),
        WEIGHTED_NORMAL => weighted_normal::read(modifier).into(),
        CORRECTIVE_SMOOTH => corrective_smooth::read(modifier).into(),
        LATTICE => {
            let parsed = lattice::read(modifier, owner);
            return Description::Kind {
                kind: parsed.lattice.into(),
                note: parsed
                    .note
                    .map(|note| format!("lattice modifier \"{label}\": {note}")),
            };
        }
        NODES => match smooth::read(modifier) {
            Some(smooth) => smooth.into(),
            None => return Description::Unsupported("geometry nodes".to_owned()),
        },
        SHAPE_KEY | COLLISION | SURFACE => return Description::Internal,
        PARTICLES => return Description::Unsupported("particle system".to_owned()),
        _ => {
            return Description::Unsupported(
                modifier.kind().trim_end_matches("ModifierData").to_owned(),
            );
        }
    };
    Description::Kind {
        kind: described,
        note: None,
    }
}

pub(crate) fn header(modifier: &Record<'_>) -> Modifier {
    let base = modifier.record("modifier");
    let mode = base.and_then(|base| base.int("mode")).unwrap_or(REALTIME);
    let mut entry = Modifier::new(
        base.and_then(|base| base.text("name")).unwrap_or_default(),
        ModifierKind::EdgeSplit(Default::default()),
    );
    entry.show_viewport = mode & REALTIME != 0;
    entry.show_render = mode & RENDER != 0;
    entry.show_in_editmode = mode & EDIT_MODE != 0;
    entry.show_on_cage = mode & ON_CAGE != 0;
    entry.show_expanded = base
        .and_then(|base| base.int("ui_expand_flag"))
        .unwrap_or(PANEL_EXPANDED)
        & PANEL_EXPANDED
        != 0;
    entry.pin_to_last =
        base.and_then(|base| base.int("flag")).unwrap_or(0) & MODIFIER_PIN_LAST != 0;
    entry
}

pub(crate) fn layout(kind: &ModifierKind) -> (&'static str, i64) {
    match kind {
        ModifierKind::Mirror(_) => ("MirrorModifierData", MIRROR),
        ModifierKind::Subdivision(_) => ("SubsurfModifierData", SUBSURF),
        ModifierKind::Solidify(_) => ("SolidifyModifierData", SOLIDIFY),
        ModifierKind::Array(_) => ("ArrayModifierData", ARRAY),
        ModifierKind::Weld(_) => ("WeldModifierData", WELD),
        ModifierKind::Triangulate(_) => ("TriangulateModifierData", TRIANGULATE),
        ModifierKind::EdgeSplit(_) => ("EdgeSplitModifierData", EDGE_SPLIT),
        ModifierKind::Displace(_) => ("DisplaceModifierData", DISPLACE),
        ModifierKind::Shrinkwrap(_) => ("ShrinkwrapModifierData", SHRINKWRAP),
        ModifierKind::DataTransfer(_) => ("DataTransferModifierData", DATA_TRANSFER),
        ModifierKind::SmoothByAngle(_) => ("NodesModifierData", NODES),
        ModifierKind::Armature(_) => ("ArmatureModifierData", ARMATURE),
        ModifierKind::Bevel(_) => ("BevelModifierData", BEVEL),
        ModifierKind::Boolean(_) => ("BooleanModifierData", BOOLEAN),
        ModifierKind::Decimate(_) => ("DecimateModifierData", DECIMATE),
        ModifierKind::WeightedNormal(_) => ("WeightedNormalModifierData", WEIGHTED_NORMAL),
        ModifierKind::CorrectiveSmooth(_) => ("CorrectiveSmoothModifierData", CORRECTIVE_SMOOTH),
        ModifierKind::Lattice(_) => ("LatticeModifierData", LATTICE),
    }
}

pub(crate) fn write(kind: &ModifierKind, record: &mut Struct<'_>, targets: &dyn Targets) {
    match kind {
        ModifierKind::Mirror(value) => mirror::write(value, record, targets),
        ModifierKind::Subdivision(value) => subdivision::write(value, record),
        ModifierKind::Solidify(value) => solidify::write(value, record),
        ModifierKind::Array(value) => array::write(value, record, targets),
        ModifierKind::Weld(value) => weld::write(value, record),
        ModifierKind::Triangulate(value) => triangulate::write(value, record),
        ModifierKind::EdgeSplit(value) => edge_split::write(value, record),
        ModifierKind::Displace(value) => displace::write(value, record, targets),
        ModifierKind::Shrinkwrap(value) => shrinkwrap::write(value, record, targets),
        ModifierKind::DataTransfer(value) => data_transfer::write(value, record, targets),
        ModifierKind::Armature(value) => armature::write(value, record, targets),
        ModifierKind::Bevel(value) => bevel::write(value, record),
        ModifierKind::Boolean(value) => boolean::write(value, record, targets),
        ModifierKind::Decimate(value) => decimate::write(value, record),
        ModifierKind::WeightedNormal(value) => weighted_normal::write(value, record),
        ModifierKind::CorrectiveSmooth(value) => corrective_smooth::write(value, record),
        ModifierKind::Lattice(value) => lattice::write(value, record),
        ModifierKind::SmoothByAngle(_) => {}
    }
}

pub(crate) use bevel::write_profile;
pub(crate) use corrective_smooth::bind_bytes;
pub(crate) use lattice::interpolation_code;

#[derive(Default)]
pub(crate) struct Notes {
    seen: FxHashSet<String>,
    pub(crate) warnings: Vec<Warning>,
}

impl Notes {
    pub(crate) fn unsupported(&mut self, message: String) {
        if self.seen.insert(message.clone()) {
            self.warnings.push(Warning::Unsupported(message));
        }
    }

    pub(crate) fn repaired(&mut self, message: String) {
        if self.seen.insert(message.clone()) {
            self.warnings.push(Warning::Repaired(message));
        }
    }
}

pub(crate) struct Deform<'a> {
    pub(crate) armature: Record<'a>,
    pub(crate) flags: i64,
}

pub(crate) struct Scope<'s, 'a> {
    pub(crate) world: Mat4,
    pub(crate) world_of: &'s dyn Fn(Record<'a>) -> Mat4,
}

pub(crate) fn apply<'a>(
    object: Record<'a>,
    mesh: &mut Polymesh,
    scope: &Scope<'_, 'a>,
    notes: &mut Notes,
) -> Option<Deform<'a>> {
    let name = object.id_name();
    let objects = Objects::new(object);
    let mut deform = None;
    let mut stack: Vec<Modifier> = Vec::new();
    let mut labels: Vec<(String, String)> = Vec::new();
    for modifier in object.list("modifiers") {
        let Some(base) = modifier.record("modifier") else {
            continue;
        };
        let label = base.text("name").unwrap_or_default();
        let mode = base.int("mode").unwrap_or(REALTIME);
        if mode & REALTIME == 0 {
            continue;
        }
        if base.int("type") == Some(ARMATURE) {
            if deform.is_some() {
                notes.unsupported(format!(
                    "object \"{name}\" has several armature modifiers, binding only the first"
                ));
            } else if let Some(armature) = modifier.id("object") {
                deform = Some(Deform {
                    armature,
                    flags: modifier.int("deformflag").unwrap_or(1),
                });
            }
            continue;
        }
        let kind_name = modifier
            .kind()
            .trim_end_matches("ModifierData")
            .to_ascii_lowercase();
        let parsed = match describe(&modifier, &object, &objects) {
            Description::Kind { kind, note } => {
                if let Some(note) = note {
                    notes.unsupported(note);
                }
                kind
            }
            Description::Internal => continue,
            Description::Unsupported(what) => {
                notes.unsupported(match base.int("type") {
                    Some(PARTICLES) => {
                        format!("particle system of object \"{name}\" not evaluated")
                    }
                    _ => format!("{what} modifier \"{label}\" of object \"{name}\" not evaluated"),
                });
                continue;
            }
        };
        let mut entry = header(&modifier);
        entry.kind = parsed;
        entry.show_viewport = true;
        labels.push((kind_name, label));
        stack.push(entry);
    }
    if stack.is_empty() {
        return deform;
    }
    let ctx = BlendContext {
        scope,
        objects: &objects,
        material_count: object.int("totcol").unwrap_or(0).max(0) as usize,
    };
    let evaluated = evaluate(mesh, &stack, &ctx, Purpose::Viewport);
    for note in evaluated.notes {
        let (kind, label) = &labels[note.modifier];
        if let NoteKind::Skipped(SkipReason::Hidden(_)) = note.kind {
            continue;
        }
        notes.unsupported(format!(
            "{kind} modifier \"{label}\" of object \"{name}\": {}",
            note.kind
        ));
    }
    *mesh = evaluated.mesh;
    deform
}

pub(crate) fn armature_of<'a>(object: &Record<'a>) -> Option<Record<'a>> {
    object
        .list("modifiers")
        .into_iter()
        .filter(|modifier| {
            modifier.record("modifier").is_some_and(|base| {
                base.int("type") == Some(ARMATURE)
                    && base.int("mode").unwrap_or(REALTIME) & REALTIME != 0
            })
        })
        .find_map(|modifier| modifier.id("object"))
}

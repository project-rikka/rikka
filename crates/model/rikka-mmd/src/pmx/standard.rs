use rikka_model_core::glam::Vec3;
use rikka_model_core::{CoordinateSystem, Model, ModelBuilder, Node, convert_coordinates};

use super::document::{
    Pmx, PmxAppend, PmxBone, PmxBoneFlags, PmxIk, PmxIkLink, PmxLimits, PmxTail,
};
use super::read::attach_bones;
use crate::MMD_UNIT_SCALE;

const NAME: &str = "標準ボーン";

const STANDARD_ARM_REST_DEGREES: f32 = 40.0;

const UPPER_ARM: f32 = 2.8;
const FOREARM: f32 = 2.3;
const UPPER_ARM_TWIST: f32 = 0.5;
const FOREARM_TWIST: f32 = 0.5;
const HAND: f32 = 0.78;
const LEG_LOOPS: u32 = 40;
const LEG_LIMIT: f32 = 2.0;
const TOE_LOOPS: u32 = 3;
const TOE_LIMIT: f32 = 4.0;
const KNEE_BENT: f32 = -0.5;
const ANKLE: Vec3 = Vec3::new(1.1, 1.43, 0.38);
const TOE: Vec3 = Vec3::new(0.99, 0.0, -1.26);

#[derive(Clone, Copy, PartialEq, Eq)]
enum Handle {
    Turn,
    Move,
    Reach,
    Hidden,
}

struct Bones {
    bones: Vec<PmxBone>,
}

impl Bones {
    fn index(&self, name: &str) -> usize {
        self.bones
            .iter()
            .position(|bone| bone.name == name)
            .unwrap_or_else(|| panic!("the standard skeleton names {name} before using it"))
    }

    fn add(&mut self, name: &str, english: &str, parent: Option<&str>, at: Vec3, handle: Handle) {
        let mut flags = PmxBoneFlags::default();
        flags.set(PmxBoneFlags::ROTATABLE, true);
        flags.set(
            PmxBoneFlags::TRANSLATABLE,
            matches!(handle, Handle::Move | Handle::Reach),
        );
        flags.set(PmxBoneFlags::VISIBLE, handle != Handle::Hidden);
        flags.set(PmxBoneFlags::OPERABLE, handle != Handle::Hidden);
        flags.set(PmxBoneFlags::IK, handle == Handle::Reach);
        let parent = parent.map(|parent| self.index(parent));
        self.bones.push(PmxBone {
            name: name.to_owned(),
            english_name: english.to_owned(),
            position: at,
            parent,
            flags,
            ..PmxBone::default()
        });
    }

    fn get(&mut self, name: &str) -> &mut PmxBone {
        let index = self.index(name);
        &mut self.bones[index]
    }

    fn append(&mut self, name: &str, source: &str, ratio: f32) {
        let parent = self.index(source);
        let bone = self.get(name);
        bone.append = Some(PmxAppend { parent, ratio });
        bone.flags.set(PmxBoneFlags::APPEND_ROTATE, true);
    }

    fn tail_to(&mut self, name: &str, tail: &str) {
        let tail = self.index(tail);
        let bone = self.get(name);
        bone.tail = PmxTail::Bone(Some(tail));
        bone.flags.set(PmxBoneFlags::TAIL_IS_BONE, true);
    }

    fn tail_by(&mut self, name: &str, offset: Vec3) {
        self.get(name).tail = PmxTail::Offset(offset);
    }

    fn twist(&mut self, name: &str, axis: Vec3) {
        let bone = self.get(name);
        bone.fixed_axis = Some(axis.normalize());
        bone.flags.set(PmxBoneFlags::FIXED_AXIS, true);
    }

    fn reach(
        &mut self,
        name: &str,
        target: &str,
        loops: u32,
        limit: f32,
        links: &[(&str, Option<PmxLimits>)],
    ) {
        let target = self.index(target);
        let links = links
            .iter()
            .map(|(link, limits)| PmxIkLink {
                bone: self.index(link),
                limits: *limits,
            })
            .collect();
        self.get(name).ik = Some(PmxIk {
            target,
            loops,
            limit_angle: limit,
            links,
        });
    }
}

struct Side {
    mark: &'static str,
    english: &'static str,
    lateral: f32,
}

impl Side {
    fn named(&self, base: &str) -> String {
        format!("{}{base}", self.mark)
    }

    fn english(&self, base: &str) -> String {
        format!("{base}{}", self.english)
    }

    fn mirrored(&self, at: Vec3) -> Vec3 {
        Vec3::new(at.x * self.lateral, at.y, at.z)
    }
}

const SIDES: [Side; 2] = [
    Side {
        mark: "左",
        english: "_L",
        lateral: 1.0,
    },
    Side {
        mark: "右",
        english: "_R",
        lateral: -1.0,
    },
];

const FINGERS: [(&str, &str, f32, [f32; 3]); 4] = [
    ("人指", "index", 0.21, [0.37, 0.21, 0.18]),
    ("中指", "middle", 0.03, [0.43, 0.24, 0.2]),
    ("薬指", "ring", -0.16, [0.4, 0.23, 0.19]),
    ("小指", "little", -0.3, [0.32, 0.18, 0.15]),
];

fn arm(bones: &mut Bones, side: &Side) {
    let angle = STANDARD_ARM_REST_DEGREES.to_radians();
    let along = side.mirrored(Vec3::new(angle.cos(), -angle.sin(), 0.0));
    let palm = side.mirrored(Vec3::new(-angle.sin(), -angle.cos(), 0.0));
    let forward = Vec3::NEG_Z;
    let shoulder = side.mirrored(Vec3::new(0.23, 16.16, -0.22));
    let upper = side.mirrored(Vec3::new(1.06, 16.04, -0.21));
    let elbow = upper + along * UPPER_ARM;
    let wrist = elbow + along * FOREARM;
    let chain = [
        ("肩P", "shoulderP", shoulder, Handle::Turn),
        ("肩", "shoulder", shoulder, Handle::Turn),
        ("肩C", "shoulderC", upper, Handle::Hidden),
        ("腕", "arm", upper, Handle::Turn),
        (
            "腕捩",
            "arm twist",
            upper + along * (UPPER_ARM * UPPER_ARM_TWIST),
            Handle::Turn,
        ),
        ("ひじ", "elbow", elbow, Handle::Turn),
        (
            "手捩",
            "wrist twist",
            elbow + along * (FOREARM * FOREARM_TWIST),
            Handle::Turn,
        ),
        ("手首", "wrist", wrist, Handle::Turn),
    ];
    let mut above = "上半身2".to_owned();
    for (base, english, at, handle) in chain {
        let name = side.named(base);
        bones.add(&name, &side.english(english), Some(&above), at, handle);
        above = name;
    }
    bones.append(&side.named("肩C"), &side.named("肩P"), -1.0);
    bones.twist(&side.named("腕捩"), along);
    bones.twist(&side.named("手捩"), along);
    bones.tail_to(&side.named("肩"), &side.named("腕"));
    bones.tail_to(&side.named("腕"), &side.named("ひじ"));
    bones.tail_to(&side.named("ひじ"), &side.named("手首"));
    bones.tail_by(&side.named("手首"), along * HAND);

    let thumb0 = wrist + along * 0.1 + palm * 0.1 + forward * 0.22;
    let thumb1 = thumb0 + along * 0.32 + palm * 0.1 + forward * 0.12;
    let thumb2 = thumb1 + along * 0.25 + palm * 0.09 + forward * 0.08;
    let mut above = side.named("手首");
    for (joint, at) in [thumb0, thumb1, thumb2].into_iter().enumerate() {
        let name = side.named(&format!("親指{}", full_width_digit(joint)));
        bones.add(
            &name,
            &side.english(&format!("thumb{joint}")),
            Some(&above),
            at,
            Handle::Turn,
        );
        above = name;
    }
    bones.tail_by(&above, along * 0.2 + forward * 0.05);

    for (finger, english_finger, spread, lengths) in FINGERS {
        let mut at = wrist + along * HAND + forward * spread;
        let mut above = side.named("手首");
        for (joint, length) in lengths.iter().enumerate() {
            let name = side.named(&format!("{finger}{}", full_width_digit(joint + 1)));
            bones.add(
                &name,
                &side.english(&format!("{english_finger}{}", joint + 1)),
                Some(&above),
                at,
                Handle::Turn,
            );
            at += along * *length;
            above = name;
        }
        bones.tail_by(&above, along * lengths[lengths.len() - 1]);
    }
}

fn full_width_digit(digit: usize) -> char {
    char::from_u32(u32::from('０') + digit as u32).unwrap_or('０')
}

fn leg(bones: &mut Bones, side: &Side) {
    let chain = [
        (
            "腰キャンセル",
            "waist cancel",
            Vec3::new(0.82, 11.61, 0.11),
            Handle::Hidden,
        ),
        ("足", "leg", Vec3::new(0.82, 11.61, 0.11), Handle::Turn),
        ("ひざ", "knee", Vec3::new(0.97, 6.73, 0.01), Handle::Turn),
        ("足首", "ankle", ANKLE, Handle::Turn),
        ("つま先", "toe", TOE, Handle::Hidden),
    ];
    let mut above = "下半身".to_owned();
    for (base, english, at, handle) in chain {
        let name = side.named(base);
        bones.add(
            &name,
            &side.english(english),
            Some(&above),
            side.mirrored(at),
            handle,
        );
        above = name;
    }
    bones.append(&side.named("腰キャンセル"), "腰", -1.0);
    bones.tail_to(&side.named("足"), &side.named("ひざ"));
    bones.tail_to(&side.named("ひざ"), &side.named("足首"));
    bones.tail_to(&side.named("足首"), &side.named("つま先"));
}

fn reach(bones: &mut Bones, side: &Side) {
    let ankle = side.mirrored(ANKLE);
    let chain = [
        (
            "足IK親",
            "leg IKP",
            Vec3::new(ankle.x, 0.0, ankle.z),
            Handle::Move,
        ),
        ("足ＩＫ", "leg IK", ankle, Handle::Reach),
        ("つま先ＩＫ", "toe IK", side.mirrored(TOE), Handle::Reach),
    ];
    let mut above = "全ての親".to_owned();
    for (base, english, at, handle) in chain {
        let name = side.named(base);
        bones.add(&name, &side.english(english), Some(&above), at, handle);
        above = name;
    }
    let knee = PmxLimits {
        lower: Vec3::new(-std::f32::consts::PI, 0.0, 0.0),
        upper: Vec3::new(KNEE_BENT.to_radians(), 0.0, 0.0),
    };
    bones.reach(
        &side.named("足ＩＫ"),
        &side.named("足首"),
        LEG_LOOPS,
        LEG_LIMIT,
        &[(&side.named("ひざ"), Some(knee)), (&side.named("足"), None)],
    );
    bones.reach(
        &side.named("つま先ＩＫ"),
        &side.named("つま先"),
        TOE_LOOPS,
        TOE_LIMIT,
        &[(&side.named("足首"), None)],
    );
    bones.tail_by(&side.named("足ＩＫ"), Vec3::new(0.0, 0.0, 1.0));
    bones.tail_by(&side.named("つま先ＩＫ"), Vec3::new(0.0, -1.0, 0.0));
}

fn bones() -> Vec<PmxBone> {
    let mut bones = Bones { bones: Vec::new() };
    bones.add("全ての親", "master", None, Vec3::ZERO, Handle::Move);
    let trunk = [
        (
            "センター",
            "center",
            Vec3::new(0.0, 7.72, 0.0),
            Handle::Move,
        ),
        (
            "グルーブ",
            "groove",
            Vec3::new(0.0, 7.92, 0.0),
            Handle::Move,
        ),
        ("腰", "waist", Vec3::new(0.0, 12.2, 0.26), Handle::Turn),
        (
            "上半身",
            "upper body",
            Vec3::new(0.0, 13.04, -0.7),
            Handle::Turn,
        ),
        (
            "上半身2",
            "upper body2",
            Vec3::new(0.0, 14.42, -0.81),
            Handle::Turn,
        ),
        ("首", "neck", Vec3::new(0.0, 16.44, -0.14), Handle::Turn),
        ("頭", "head", Vec3::new(0.0, 17.07, -0.14), Handle::Turn),
    ];
    let mut above = "全ての親";
    for (name, english, at, handle) in trunk {
        bones.add(name, english, Some(above), at, handle);
        above = name;
    }
    bones.add(
        "両目",
        "eyes",
        Some("頭"),
        Vec3::new(0.0, 20.5, -0.27),
        Handle::Turn,
    );
    for side in &SIDES {
        let eye = side.named("目");
        bones.add(
            &eye,
            &side.english("eye"),
            Some("頭"),
            side.mirrored(Vec3::new(0.42, 18.05, -0.57)),
            Handle::Hidden,
        );
        bones.append(&eye, "両目", 1.0);
        bones.tail_by(&eye, Vec3::new(0.0, 0.0, -0.5));
    }
    bones.tail_to("上半身", "上半身2");
    bones.tail_to("上半身2", "首");
    bones.tail_to("首", "頭");
    bones.tail_by("頭", Vec3::new(0.0, 1.5, 0.0));
    bones.tail_by("両目", Vec3::new(0.0, 0.0, -1.0));
    for side in &SIDES {
        arm(&mut bones, side);
    }
    bones.add(
        "下半身",
        "lower body",
        Some("腰"),
        Vec3::new(0.0, 13.04, -0.7),
        Handle::Turn,
    );
    bones.tail_by("下半身", Vec3::new(0.0, -1.3, 0.0));
    for side in &SIDES {
        leg(&mut bones, side);
    }
    for side in &SIDES {
        reach(&mut bones, side);
    }
    bones.bones
}

#[must_use]
pub fn standard_skeleton() -> Model {
    let mut pmx = Pmx {
        name: NAME.to_owned(),
        english_name: "Standard bones".to_owned(),
        bones: bones(),
        ..Pmx::default()
    };
    let mut builder = ModelBuilder::new()
        .coordinates(CoordinateSystem::LEFT_HANDED_Y_UP)
        .unit_scale(MMD_UNIT_SCALE)
        .generator(NAME.to_owned());
    let root = builder.add_node(Node::new(NAME));
    attach_bones(&mut builder, &mut pmx, root);
    pmx.root = root;
    builder.extend_with(pmx);
    let mut model = builder
        .finish()
        .expect("the standard skeleton is a valid model");
    convert_coordinates(&mut model, CoordinateSystem::CANONICAL, MMD_UNIT_SCALE);
    model.metadata.unit_scale = 1.0;
    model
}

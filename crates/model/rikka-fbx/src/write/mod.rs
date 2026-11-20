mod anim;
pub(crate) mod ascii;
pub(crate) mod binary;
mod deform;
mod geometry;
mod material;
mod scene;
mod topology;

use std::collections::hash_map::Entry;

use rustc_hash::FxHashMap;

use crate::KTIME;
use crate::document::{Node, Value};
use rikka_model_core::ExportCtx;
use rikka_model_core::Model;
use rikka_model_core::Result;
use rikka_model_core::Stage;
use rikka_model_core::{Axis, CoordinateSystem};

pub(crate) const VERSION: u32 = 7400;
const GENERATOR: &str = "rikka-model";
const HEADER_VERSION: i32 = 1003;

const FILE_ID: [u8; 16] = [
    0x28, 0xb3, 0x2a, 0xeb, 0xb6, 0x24, 0xcc, 0xc2, 0xbf, 0xc8, 0xb0, 0x2a, 0xa9, 0x2b, 0xfc, 0xf1,
];
const CREATION_TIME: &str = "1970-01-01 10:00:00:000";

pub(crate) fn write(model: &Model, ctx: &mut ExportCtx<'_>, text: bool) -> Result<Vec<u8>> {
    let mut composer = Composer::default();

    ctx.report(Stage::Writing, 0.1);
    let painted = material::write(model, &mut composer, ctx)?;
    ctx.report(Stage::Resources, 1.0);
    let meshes = geometry::write(model, &mut composer, ctx)?;
    ctx.checkpoint()?;
    let node_ids = scene::write(model, &mut composer, &meshes, &painted);
    let shaped = deform::write(model, &mut composer, &meshes, &node_ids, ctx)?;
    anim::write(model, &mut composer, &node_ids, &shaped, ctx)?;
    ctx.report(Stage::Writing, 0.8);

    let mut records = vec![header()];
    if !text {
        records.push(Node::new("FileId").with(Value::Raw(FILE_ID.to_vec().into())));
    }
    let [definitions, objects, connections] = composer.finish();
    records.extend([
        Node::new("CreationTime").with_text(CREATION_TIME),
        Node::new("Creator").with_text(GENERATOR),
        global_settings(ctx.options().coordinates, ctx.options().unit_scale),
        documents(),
        Node::new("References"),
        definitions,
        objects,
        connections,
        Node::new("Takes").adding(Node::new("Current").with_text("")),
    ]);

    Ok(match text {
        true => ascii::write(&records, VERSION),
        false => binary::write(&records, VERSION),
    })
}

#[derive(Default)]
pub(crate) struct Composer {
    objects: Vec<Node>,
    links: Vec<Node>,
    counts: FxHashMap<&'static str, usize>,
    order: Vec<&'static str>,
    next: i64,
}

impl Composer {
    pub(crate) fn id(&mut self) -> i64 {
        self.next += 1;
        self.next + 1_000_000
    }

    pub(crate) fn add(&mut self, kind: &'static str, node: Node) {
        match self.counts.entry(kind) {
            Entry::Occupied(mut count) => *count.get_mut() += 1,
            Entry::Vacant(slot) => {
                slot.insert(1);
                self.order.push(kind);
            }
        }
        self.objects.push(node);
    }

    pub(crate) fn connect(&mut self, src: i64, dst: i64) {
        self.links.push(
            Node::new("C")
                .with_text("OO")
                .with(Value::I64(src))
                .with(Value::I64(dst)),
        );
    }

    pub(crate) fn connect_property(&mut self, src: i64, dst: i64, property: &str) {
        self.links.push(
            Node::new("C")
                .with_text("OP")
                .with(Value::I64(src))
                .with(Value::I64(dst))
                .with_text(property),
        );
    }

    fn finish(self) -> [Node; 3] {
        let mut definitions = Node::new("Definitions")
            .adding(Node::new("Version").with_i32(100))
            .adding(Node::new("Count").with_i32(self.objects.len() as i32 + 1))
            .adding(
                Node::new("ObjectType")
                    .with_text("GlobalSettings")
                    .adding(Node::new("Count").with_i32(1)),
            );
        for kind in &self.order {
            definitions.add(
                Node::new("ObjectType")
                    .with_text(*kind)
                    .adding(Node::new("Count").with_i32(self.counts[kind] as i32)),
            );
        }

        [
            definitions,
            Node {
                name: "Objects".to_owned(),
                values: Vec::new(),
                children: self.objects,
            },
            Node {
                name: "Connections".to_owned(),
                values: Vec::new(),
                children: self.links,
            },
        ]
    }
}

pub(crate) fn named(name: &str, class: &str) -> String {
    format!("{name}\u{0}\u{1}{class}")
}

pub(crate) fn prop(name: &str, kind: &str, sub: &str, flags: &str) -> Node {
    Node::new("P")
        .with_text(name)
        .with_text(kind)
        .with_text(sub)
        .with_text(flags)
}

pub(crate) fn number(name: &str, value: f64) -> Node {
    prop(name, "double", "Number", "").with_f64(value)
}

pub(crate) fn integer(name: &str, value: i32) -> Node {
    prop(name, "int", "Integer", "").with_i32(value)
}

pub(crate) fn vector(name: &str, kind: &str, value: rikka_model_core::glam::Vec3) -> Node {
    prop(name, kind, "", "A")
        .with_f64(f64::from(value.x))
        .with_f64(f64::from(value.y))
        .with_f64(f64::from(value.z))
}

pub(crate) fn doubles(values: impl IntoIterator<Item = f32>) -> Value {
    Value::F64s(values.into_iter().map(f64::from).collect())
}

fn header() -> Node {
    Node::new("FBXHeaderExtension")
        .adding(Node::new("FBXHeaderVersion").with_i32(HEADER_VERSION))
        .adding(Node::new("FBXVersion").with_i32(VERSION as i32))
        .adding(Node::new("EncryptionType").with_i32(0))
        .adding(
            Node::new("CreationTimeStamp")
                .adding(Node::new("Version").with_i32(1000))
                .adding(Node::new("Year").with_i32(1970))
                .adding(Node::new("Month").with_i32(1))
                .adding(Node::new("Day").with_i32(1))
                .adding(Node::new("Hour").with_i32(10))
                .adding(Node::new("Minute").with_i32(0))
                .adding(Node::new("Second").with_i32(0))
                .adding(Node::new("Millisecond").with_i32(0)),
        )
        .adding(Node::new("Creator").with_text(GENERATOR))
}

fn documents() -> Node {
    Node::new("Documents")
        .adding(Node::new("Count").with_i32(1))
        .adding(
            Node::new("Document")
                .with_i64(1)
                .with_text("")
                .with_text("Scene")
                .adding(Node::new("RootNode").with_i64(0)),
        )
}

fn global_settings(coordinates: CoordinateSystem, unit_scale: f32) -> Node {
    let (up, up_sign) = axis(coordinates.up);
    let (front, front_sign) = axis(coordinates.forward.opposite());
    let (right, right_sign) = axis(coordinates.right);
    let centimetres = f64::from(unit_scale) * 100.0;

    Node::new("GlobalSettings")
        .adding(Node::new("Version").with_i32(1000))
        .adding(
            Node::new("Properties70")
                .adding(integer("UpAxis", up))
                .adding(integer("UpAxisSign", up_sign))
                .adding(integer("FrontAxis", front))
                .adding(integer("FrontAxisSign", front_sign))
                .adding(integer("CoordAxis", right))
                .adding(integer("CoordAxisSign", right_sign))
                .adding(integer("OriginalUpAxis", up))
                .adding(integer("OriginalUpAxisSign", up_sign))
                .adding(number("UnitScaleFactor", centimetres))
                .adding(number("OriginalUnitScaleFactor", centimetres))
                .adding(vector(
                    "AmbientColor",
                    "ColorRGB",
                    rikka_model_core::glam::Vec3::ZERO,
                ))
                .adding(prop("DefaultCamera", "KString", "", "").with_text("Producer Perspective"))
                .adding(prop("TimeMode", "enum", "", "").with_i32(6))
                .adding(prop("TimeSpanStart", "KTime", "Time", "").with_i64(0))
                .adding(prop("TimeSpanStop", "KTime", "Time", "").with_i64(KTIME))
                .adding(number("CustomFrameRate", -1.0)),
        )
}

fn axis(axis: Axis) -> (i32, i32) {
    match axis {
        Axis::X => (0, 1),
        Axis::NegX => (0, -1),
        Axis::Y => (1, 1),
        Axis::NegY => (1, -1),
        Axis::Z => (2, 1),
        Axis::NegZ => (2, -1),
    }
}

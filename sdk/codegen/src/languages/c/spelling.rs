use crate::abi::{self, Function, Origin, Return, Slot, SlotType};
use crate::model::{Api, Element, Enum, Flags, Id, Paragraph, Primitive, Target, Value};
use crate::names::screaming;

pub(crate) struct Spelling<'a> {
    pub(crate) api: &'a Api,
}

impl<'a> Spelling<'a> {
    pub(crate) fn new(api: &'a Api) -> Self {
        Self { api }
    }

    pub(crate) fn prefix(&self) -> &str {
        &self.api.abi_prefix
    }

    pub(crate) fn upper(&self) -> String {
        screaming(&self.api.abi_prefix)
    }

    pub(crate) fn type_name(&self, name: &str) -> String {
        format!("{}_{name}", self.prefix())
    }

    pub(crate) fn macro_name(&self, name: &str) -> String {
        format!("{}_{}", self.upper(), screaming(name))
    }

    pub(crate) fn function(&self, origin: Origin) -> String {
        self.type_name(&abi::name(self.api, origin))
    }

    pub(crate) fn abi_function(&self, function: &Function) -> String {
        self.type_name(&function.name)
    }

    pub(crate) fn variant(&self, id: Id<Enum>, index: usize) -> String {
        let item = &self.api[id];
        self.macro_name(&format!(
            "{}_{}",
            item.value_prefix, item.variants[index].name
        ))
    }

    pub(crate) fn flag(&self, id: Id<Flags>, index: usize) -> String {
        let item = &self.api[id];
        self.macro_name(&format!("{}_{}", item.value_prefix, item.flags[index].name))
    }

    pub(crate) fn value(&self, value: Value) -> String {
        match value {
            Value::Primitive(primitive) => self::primitive(primitive).to_owned(),
            Value::Struct(id) => self.type_name(&self.api[id].name),
            Value::Enum(id) => self.type_name(&self.api[id].name),
            Value::Flags(id) => self.type_name(&self.api[id].name),
        }
    }

    pub(crate) fn slot(&self, slot: &Slot) -> String {
        let name = &slot.name;
        match slot.ty {
            SlotType::Value(value) => format!("{} {name}", self.value(value)),
            SlotType::Str => format!("{} {name}", self.type_name("str")),
            SlotType::Object(class) => format!("{} *{name}", self.type_name(&self.api[class].name)),
            SlotType::Pointer(Element::Primitive(primitive)) => {
                format!("const {} *{name}", self::primitive(primitive))
            }
            SlotType::Pointer(Element::String) => {
                format!("const {} *{name}", self.type_name("str"))
            }
            SlotType::Count => format!("size_t {name}"),
            SlotType::Callback(callback) => {
                format!("{} {name}", self.type_name(&self.api[callback].name))
            }
            SlotType::Userdata => format!("void *{name}"),
            SlotType::Destroy => format!("{} {name}", self.type_name("destroy_fn")),
        }
    }

    pub(crate) fn returns(&self, returns: Return) -> String {
        match returns {
            Return::Void => "void".to_owned(),
            Return::Value(value) => self.value(value),
            Return::Str => self.type_name("str"),
            Return::String => self.type_name("string"),
            Return::Object(class) => format!("{} *", self.type_name(&self.api[class].name)),
        }
    }

    pub(crate) fn link(&self, target: Target) -> String {
        let api = self.api;
        match target {
            Target::Constant(id) => self.macro_name(&api[id].name),
            Target::Struct(id) => self.type_name(&api[id].name),
            Target::Field(id, index) => format!(
                "{}.{}",
                self.type_name(&api[id].name),
                api[id].fields[index].name
            ),
            Target::Enum(id) => self.type_name(&api[id].name),
            Target::Variant(id, index) => self.variant(id, index),
            Target::Flags(id) => self.type_name(&api[id].name),
            Target::Flag(id, index) => self.flag(id, index),
            Target::Callback(id) => self.type_name(&api[id].name),
            Target::Class(id) => self.type_name(&api[id].name),
            Target::Member(class, index) => {
                format!("{}()", self.function(Origin::Member(class, index)))
            }
            Target::Function(id) => format!("{}()", self.function(Origin::Function(id))),
        }
    }

    pub(crate) fn text(&self, paragraph: &Paragraph) -> String {
        paragraph.render(&|target| self.link(target))
    }

    pub(crate) fn body(&self, paragraphs: &[Paragraph]) -> Vec<String> {
        paragraphs
            .iter()
            .map(|paragraph| self.text(paragraph))
            .collect()
    }
}

pub(crate) fn primitive(primitive: Primitive) -> &'static str {
    match primitive {
        Primitive::Bool => "bool",
        Primitive::U8 => "uint8_t",
        Primitive::U16 => "uint16_t",
        Primitive::U32 => "uint32_t",
        Primitive::U64 => "uint64_t",
        Primitive::I32 => "int32_t",
        Primitive::I64 => "int64_t",
        Primitive::Usize => "size_t",
        Primitive::F32 => "float",
        Primitive::F64 => "double",
    }
}

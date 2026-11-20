use crate::abi::{Slot, SlotType};
use crate::languages::keywords;
use crate::languages::projection::{self, Linker};
use crate::model::{
    Api, Callback, Class, DeclarationKind, Element, Flags, Id, Input, Member, Output, Primitive,
    Property, Target, Value,
};
use crate::names::{pascal, screaming};

pub(super) const ELEMENT: &str = "Element";

pub(super) fn camel(name: &str) -> String {
    let pascal = pascal(name);
    let mut characters = pascal.chars();
    let joined = characters.next().map_or_else(String::new, |first| {
        first.to_ascii_lowercase().to_string() + characters.as_str()
    });
    match keywords::JAVA.contains(&joined.as_str()) {
        true => format!("{joined}Value"),
        false => joined,
    }
}

pub(super) struct Spelling<'a> {
    pub(super) api: &'a Api,
}

impl<'a> Spelling<'a> {
    pub(super) fn class(&self, class: Id<Class>) -> String {
        pascal(&self.api[class].name)
    }

    pub(super) fn statics(&self) -> String {
        pascal(&self.api.name)
    }

    pub(super) fn exception(&self) -> String {
        format!("{}Exception", pascal(&self.api.name))
    }

    pub(super) fn interface(&self, callback: Id<Callback>) -> String {
        projection::callback_name(&self.api[callback])
    }

    pub(super) fn flag(&self, flags: Id<Flags>) -> String {
        let item = &self.api[flags];
        match item.value_prefix == item.name {
            true => format!("{}Flag", pascal(&item.name)),
            false => pascal(&item.value_prefix),
        }
    }

    pub(super) fn getter(&self, property: &Property) -> String {
        let boolean = matches!(
            property.ty,
            Output::Value(Value::Primitive(Primitive::Bool))
        );
        let predicate = ["is_", "can_", "has_"]
            .iter()
            .any(|prefix| property.name.starts_with(prefix));
        match (boolean, predicate) {
            (true, true) => camel(&property.name),
            (true, false) => format!("is{}", pascal(&property.name)),
            (false, _) => format!("get{}", pascal(&property.name)),
        }
    }

    pub(super) fn member(&self, class: Id<Class>, index: usize) -> String {
        let owner = &self.api[class];
        match self.api.member(class, index) {
            Member::Constructor(constructor) => match &constructor.name {
                None => self.class(class),
                Some(name) => format!("from{}", pascal(name)),
            },
            Member::Method(method) => {
                let chained = crate::languages::projection::chained(owner, method);
                camel(crate::languages::projection::builder_name(method, chained))
            }
            Member::Property(property) => self.getter(property),
            Member::Declaration(declaration) => camel(&declaration.name),
        }
    }

    pub(super) fn setter(&self, class: Id<Class>, index: usize) -> String {
        match self.api.member(class, index) {
            Member::Property(property) => format!("set{}", pascal(&property.name)),
            _ => unreachable!("only properties have setters"),
        }
    }

    pub(super) fn trampoline(&self, callback: Id<Callback>, position: usize) -> String {
        format!("{}{position}", camel(&self.api[callback].name))
    }

    pub(super) fn handle(&self, function: &str) -> String {
        screaming(&format!("{}_{function}", self.api.abi_prefix))
    }

    pub(super) fn primitive(&self, primitive: Primitive) -> &'static str {
        match primitive {
            Primitive::Bool => "boolean",
            Primitive::U8 => "byte",
            Primitive::U16 => "short",
            Primitive::U32 | Primitive::I32 | Primitive::Usize => "int",
            Primitive::U64 | Primitive::I64 => "long",
            Primitive::F32 => "float",
            Primitive::F64 => "double",
        }
    }

    pub(super) fn carrier(&self, primitive: Primitive) -> &'static str {
        match primitive {
            Primitive::Usize => "long",
            _ => self.primitive(primitive),
        }
    }

    pub(super) fn layout(&self, primitive: Primitive) -> &'static str {
        match primitive {
            Primitive::Bool => "JAVA_BOOLEAN",
            Primitive::U8 => "JAVA_BYTE",
            Primitive::U16 => "JAVA_SHORT",
            Primitive::U32 | Primitive::I32 => "JAVA_INT",
            Primitive::U64 | Primitive::I64 | Primitive::Usize => "JAVA_LONG",
            Primitive::F32 => "JAVA_FLOAT",
            Primitive::F64 => "JAVA_DOUBLE",
        }
    }

    pub(super) fn value(&self, value: Value) -> String {
        match value {
            Value::Primitive(primitive) => self.primitive(primitive).to_owned(),
            Value::Struct(id) => pascal(&self.api[id].name),
            Value::Enum(id) => pascal(&self.api[id].name),
            Value::Flags(id) => format!("Set<{}>", self.flag(id)),
        }
    }

    pub(super) fn value_layout(&self, value: Value) -> String {
        match value {
            Value::Primitive(primitive) => self.layout(primitive).to_owned(),
            Value::Struct(id) => screaming(&self.api[id].name),
            Value::Enum(_) | Value::Flags(_) => "JAVA_INT".to_owned(),
        }
    }

    pub(super) fn value_carrier(&self, value: Value) -> String {
        match value {
            Value::Primitive(primitive) => self.carrier(primitive).to_owned(),
            Value::Struct(_) => "MemorySegment".to_owned(),
            Value::Enum(_) | Value::Flags(_) => "int".to_owned(),
        }
    }

    pub(super) fn input(&self, input: &Input) -> String {
        match input {
            Input::Value(value) => self.value(*value),
            Input::String => "String".to_owned(),
            Input::Object { class, .. } => self.class(*class),
            Input::Array(Element::Primitive(primitive)) => {
                format!("{}[]", self.primitive(*primitive))
            }
            Input::Array(Element::String) => "List<String>".to_owned(),
            Input::Closure(closure) => self.interface(closure.functions[0].callback),
        }
    }

    pub(super) fn slot_layout(&self, slot: &Slot) -> String {
        match slot.ty {
            SlotType::Value(value) => self.value_layout(value),
            SlotType::Str => "STR".to_owned(),
            SlotType::Count => "JAVA_LONG".to_owned(),
            SlotType::Object(_)
            | SlotType::Pointer(_)
            | SlotType::Callback(_)
            | SlotType::Userdata
            | SlotType::Destroy => "ADDRESS".to_owned(),
        }
    }

    pub(super) fn slot_carrier(&self, slot: &Slot) -> String {
        match slot.ty {
            SlotType::Value(value) => self.value_carrier(value),
            SlotType::Count => "long".to_owned(),
            _ => "MemorySegment".to_owned(),
        }
    }
}

impl Linker for Spelling<'_> {
    fn link(&self, target: Target) -> String {
        let api = self.api;
        let link = |path: String| format!("{{@link {path}}}");
        match target {
            Target::Constant(id) => {
                link(format!("{}#{}", self.statics(), screaming(&api[id].name)))
            }
            Target::Struct(id) => link(pascal(&api[id].name)),
            Target::Field(id, index) => link(format!(
                "{}#{}",
                pascal(&api[id].name),
                camel(&api[id].fields[index].name)
            )),
            Target::Enum(id) => link(pascal(&api[id].name)),
            Target::Variant(id, index) => link(format!(
                "{}#{}",
                pascal(&api[id].name),
                screaming(&api[id].variants[index].name)
            )),
            Target::Flags(id) => link(self.flag(id)),
            Target::Flag(id, index) => link(format!(
                "{}#{}",
                self.flag(id),
                screaming(&api[id].flags[index].name)
            )),
            Target::Callback(id) => link(self.interface(id)),
            Target::Class(id) => link(self.class(id)),
            Target::Member(class, index) => {
                let member = self.member(class, index);
                match api.member(class, index) {
                    Member::Declaration(declaration)
                        if declaration.kind == DeclarationKind::Modifier =>
                    {
                        link(format!("{ELEMENT}#{member}"))
                    }
                    Member::Constructor(constructor) if constructor.name.is_none() => {
                        link(self.class(class))
                    }
                    _ => link(format!("{}#{member}", self.class(class))),
                }
            }
            Target::Function(id) => link(format!("{}#{}", self.statics(), camel(&api[id].name))),
        }
    }
}

pub(super) fn escape(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(start) = rest.find("{@link ") {
        out.push_str(&escape_plain(&rest[..start]));
        let end = rest[start..]
            .find('}')
            .map_or(rest.len(), |end| start + end + 1);
        out.push_str(&rest[start..end]);
        rest = &rest[end..];
    }
    out.push_str(&escape_plain(rest));
    out
}

fn escape_plain(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('@', "&#64;")
        .replace("*/", "*&#47;")
}

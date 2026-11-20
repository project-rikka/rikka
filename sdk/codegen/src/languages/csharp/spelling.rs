use crate::abi::{Slot, SlotType};
use crate::languages::keywords;
use crate::languages::projection::{self, Linker};
use crate::model::{
    Api, Callback, Class, DeclarationKind, Element, Id, Input, Member, Primitive, Target, Value,
};
use crate::names::pascal_with_acronyms;

const ACRONYMS: &[&str] = &["io", "ok", "ui"];

pub(super) const ELEMENT: &str = "Element";
pub(super) const STATICS: &str = "Sdk";

pub(super) fn pascal(name: &str) -> String {
    pascal_with_acronyms(name, ACRONYMS)
}

pub(super) fn camel(name: &str) -> String {
    let pascal = pascal(name);
    let mut characters = pascal.chars();
    let joined = characters.next().map_or_else(String::new, |first| {
        first.to_ascii_lowercase().to_string() + characters.as_str()
    });
    match keywords::CSHARP.contains(&joined.as_str()) {
        true => format!("@{joined}"),
        false => joined,
    }
}

pub(super) fn bare(name: &str) -> String {
    camel(name).trim_start_matches('@').to_owned()
}

pub(super) struct Spelling<'a> {
    pub(super) api: &'a Api,
}

impl<'a> Spelling<'a> {
    pub(super) fn class(&self, class: Id<Class>) -> String {
        pascal(&self.api[class].name)
    }

    pub(super) fn exception(&self) -> String {
        format!("{}Exception", pascal(&self.api.name))
    }

    pub(super) fn delegate(&self, callback: Id<Callback>) -> String {
        projection::callback_name(&self.api[callback])
    }

    pub(super) fn member(&self, class: Id<Class>, index: usize) -> String {
        match self.api.member(class, index) {
            Member::Constructor(constructor) => match &constructor.name {
                None => self.class(class),
                Some(name) => format!("From{}", pascal(name)),
            },
            Member::Method(method) => pascal(&method.name),
            Member::Property(property) => pascal(&property.name),
            Member::Declaration(declaration) => pascal(&declaration.name),
        }
    }

    pub(super) fn trampoline(&self, callback: Id<Callback>, position: usize) -> String {
        format!("{}{position}", pascal(&self.api[callback].name))
    }

    pub(super) fn primitive(&self, primitive: Primitive) -> &'static str {
        match primitive {
            Primitive::Bool => "bool",
            Primitive::U8 => "byte",
            Primitive::U16 => "ushort",
            Primitive::U32 => "uint",
            Primitive::U64 => "ulong",
            Primitive::I32 => "int",
            Primitive::I64 => "long",
            Primitive::Usize => "int",
            Primitive::F32 => "float",
            Primitive::F64 => "double",
        }
    }

    pub(super) fn native_primitive(&self, primitive: Primitive) -> &'static str {
        match primitive {
            Primitive::Bool => "byte",
            Primitive::Usize => "nint",
            _ => self.primitive(primitive),
        }
    }

    pub(super) fn value(&self, value: Value) -> String {
        match value {
            Value::Primitive(primitive) => self.primitive(primitive).to_owned(),
            Value::Struct(id) => pascal(&self.api[id].name),
            Value::Enum(id) => pascal(&self.api[id].name),
            Value::Flags(id) => pascal(&self.api[id].name),
        }
    }

    pub(super) fn native_value(&self, value: Value) -> String {
        match value {
            Value::Primitive(primitive) => self.native_primitive(primitive).to_owned(),
            Value::Struct(id) => format!("Native.{}", pascal(&self.api[id].name)),
            Value::Enum(_) | Value::Flags(_) => "uint".to_owned(),
        }
    }

    pub(super) fn input(&self, input: &Input) -> String {
        match input {
            Input::Value(value) => self.value(*value),
            Input::String => "string".to_owned(),
            Input::Object { class, .. } => self.class(*class),
            Input::Array(Element::Primitive(primitive)) => {
                format!("ReadOnlySpan<{}>", self.primitive(*primitive))
            }
            Input::Array(Element::String) => "IReadOnlyList<string>".to_owned(),
            Input::Closure(closure) => self.delegate(closure.functions[0].callback),
        }
    }

    pub(super) fn received(&self, input: &Input) -> String {
        match input {
            Input::Array(Element::Primitive(primitive)) => {
                format!("IReadOnlyList<{}>", self.primitive(*primitive))
            }
            _ => self.input(input),
        }
    }

    pub(super) fn pointer(&self, callback: Id<Callback>) -> String {
        let mut types: Vec<String> = crate::abi::callback_slots(&self.api[callback])
            .iter()
            .map(|slot| self.native_slot(slot))
            .collect();
        types.push("void".to_owned());
        format!("delegate* unmanaged[Cdecl]<{}>", types.join(", "))
    }

    pub(super) fn native_slot(&self, slot: &Slot) -> String {
        match slot.ty {
            SlotType::Value(value) => self.native_value(value),
            SlotType::Str => "Str".to_owned(),
            SlotType::Object(_) | SlotType::Userdata => "nint".to_owned(),
            SlotType::Pointer(Element::Primitive(primitive)) => {
                format!("{}*", self.native_primitive(primitive))
            }
            SlotType::Pointer(Element::String) => "Str*".to_owned(),
            SlotType::Count => "nint".to_owned(),
            SlotType::Callback(callback) => self.pointer(callback),
            SlotType::Destroy => "delegate* unmanaged[Cdecl]<nint, void>".to_owned(),
        }
    }
}

impl Linker for Spelling<'_> {
    fn link(&self, target: Target) -> String {
        let api = self.api;
        let cref = |path: String| format!("<see cref=\"{path}\"/>");
        match target {
            Target::Constant(id) => cref(format!("{STATICS}.{}", pascal(&api[id].name))),
            Target::Struct(id) => cref(pascal(&api[id].name)),
            Target::Field(id, index) => cref(format!(
                "{}.{}",
                pascal(&api[id].name),
                pascal(&api[id].fields[index].name)
            )),
            Target::Enum(id) => cref(pascal(&api[id].name)),
            Target::Variant(id, index) => cref(format!(
                "{}.{}",
                pascal(&api[id].name),
                pascal(&api[id].variants[index].name)
            )),
            Target::Flags(id) => cref(pascal(&api[id].name)),
            Target::Flag(id, index) => cref(format!(
                "{}.{}",
                pascal(&api[id].name),
                pascal(&api[id].flags[index].name)
            )),
            Target::Callback(id) => cref(self.delegate(id)),
            Target::Class(id) => cref(self.class(id)),
            Target::Member(class, index) => {
                let member = self.member(class, index);
                match api.member(class, index) {
                    Member::Declaration(declaration)
                        if declaration.kind == DeclarationKind::Modifier =>
                    {
                        cref(format!("{ELEMENT}.{member}"))
                    }
                    Member::Constructor(constructor) if constructor.name.is_none() => {
                        cref(self.class(class))
                    }
                    _ => cref(format!("{}.{member}", self.class(class))),
                }
            }
            Target::Function(id) => cref(format!("{STATICS}.{}", pascal(&api[id].name))),
        }
    }
}

pub(super) fn escape(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(start) = rest.find("<see cref=\"") {
        out.push_str(&escape_plain(&rest[..start]));
        let end = rest[start..]
            .find("/>")
            .map_or(rest.len(), |end| start + end + 2);
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
}

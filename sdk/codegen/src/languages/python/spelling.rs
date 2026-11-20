use crate::abi::{Slot, SlotType};
use crate::languages::keywords;
use crate::languages::projection::{self, Linker};
use crate::model::{
    Api, Callback, Class, DeclarationKind, Element, Id, Input, Member, Openness, Primitive, Target,
    Value,
};
use crate::names::{pascal, screaming};

pub(super) const ELEMENT: &str = "Element";
pub(super) const CONTAINER: &str = "Container";
pub(super) const ERROR: &str = "Error";

pub(super) struct Spelling<'a> {
    pub(super) api: &'a Api,
}

impl<'a> Spelling<'a> {
    pub(super) fn ident(&self, name: &str) -> String {
        match keywords::PYTHON.contains(&name) {
            true => format!("{name}_"),
            false => name.to_owned(),
        }
    }

    pub(super) fn class(&self, class: Id<Class>) -> String {
        pascal(&self.api[class].name)
    }

    pub(super) fn callback(&self, callback: Id<Callback>) -> String {
        projection::callback_name(&self.api[callback])
    }

    pub(super) fn callback_type(&self, callback: Id<Callback>) -> String {
        pascal(&self.api[callback].name)
    }

    pub(super) fn trampoline(&self, callback: Id<Callback>, position: usize) -> String {
        format!("_{}_{position}", self.api[callback].name)
    }

    pub(super) fn member(&self, class: Id<Class>, index: usize) -> String {
        match self.api.member(class, index) {
            Member::Constructor(constructor) => match &constructor.name {
                None => "__init__".to_owned(),
                Some(name) => self.ident(&format!("from_{name}")),
            },
            Member::Method(method) => self.ident(&method.name),
            Member::Property(property) => self.ident(&property.name),
            Member::Declaration(declaration) => self.ident(&declaration.name),
        }
    }

    pub(super) fn native(&self, function: &str) -> String {
        format!("_native.{}_{function}", self.api.abi_prefix)
    }

    pub(super) fn primitive(&self, primitive: Primitive) -> &'static str {
        match primitive {
            Primitive::Bool => "bool",
            Primitive::F32 | Primitive::F64 => "float",
            _ => "int",
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

    pub(super) fn accepted(&self, value: Value) -> String {
        match value {
            Value::Enum(id) if self.api[id].openness == Openness::Open => {
                format!("{} | int | str", pascal(&self.api[id].name))
            }
            Value::Flags(id) => format!("{} | int", pascal(&self.api[id].name)),
            _ => self.value(value),
        }
    }

    pub(super) fn input(&self, input: &Input) -> String {
        match input {
            Input::Value(value) => self.accepted(*value),
            Input::String => "str".to_owned(),
            Input::Object { class, .. } => self.class(*class),
            Input::Array(Element::Primitive(Primitive::U8)) => {
                "bytes | bytearray | memoryview".to_owned()
            }
            Input::Array(Element::Primitive(primitive)) => {
                format!("Sequence[{}]", self.primitive(*primitive))
            }
            Input::Array(Element::String) => "Sequence[str]".to_owned(),
            Input::Closure(closure) => self.callback(closure.functions[0].callback),
        }
    }

    pub(super) fn received(&self, input: &Input) -> String {
        match input {
            Input::Value(value) => self.value(*value),
            Input::Array(Element::Primitive(primitive)) => {
                format!("list[{}]", self.primitive(*primitive))
            }
            Input::Array(Element::String) => "list[str]".to_owned(),
            _ => self.input(input),
        }
    }

    pub(super) fn ctype(&self, primitive: Primitive) -> &'static str {
        match primitive {
            Primitive::Bool => "ctypes.c_bool",
            Primitive::U8 => "ctypes.c_uint8",
            Primitive::U16 => "ctypes.c_uint16",
            Primitive::U32 => "ctypes.c_uint32",
            Primitive::U64 => "ctypes.c_uint64",
            Primitive::I32 => "ctypes.c_int32",
            Primitive::I64 => "ctypes.c_int64",
            Primitive::Usize => "ctypes.c_ssize_t",
            Primitive::F32 => "ctypes.c_float",
            Primitive::F64 => "ctypes.c_double",
        }
    }

    pub(super) fn c_value(&self, value: Value) -> String {
        match value {
            Value::Primitive(primitive) => self.ctype(primitive).to_owned(),
            Value::Struct(id) => pascal(&self.api[id].name),
            Value::Enum(_) | Value::Flags(_) => "ctypes.c_uint32".to_owned(),
        }
    }

    pub(super) fn c_slot(&self, slot: &Slot, received: bool) -> String {
        match slot.ty {
            SlotType::Value(value) => self.c_value(value),
            SlotType::Str if received => "_runtime.StrView".to_owned(),
            SlotType::Str => "_runtime.Str".to_owned(),
            SlotType::Object(_) | SlotType::Userdata => "ctypes.c_void_p".to_owned(),
            SlotType::Pointer(Element::Primitive(Primitive::U8)) if !received => {
                "ctypes.c_char_p".to_owned()
            }
            SlotType::Pointer(Element::Primitive(primitive)) => {
                format!("ctypes.POINTER({})", self.ctype(primitive))
            }
            SlotType::Pointer(Element::String) => "ctypes.POINTER(_runtime.StrView)".to_owned(),
            SlotType::Count => "ctypes.c_size_t".to_owned(),
            SlotType::Callback(callback) => self.callback_type(callback),
            SlotType::Destroy => "DestroyFn".to_owned(),
        }
    }
}

impl Linker for Spelling<'_> {
    fn link(&self, target: Target) -> String {
        let api = self.api;
        match target {
            Target::Constant(id) => format!(":data:`{}`", screaming(&api[id].name)),
            Target::Struct(id) => format!(":class:`{}`", pascal(&api[id].name)),
            Target::Field(id, index) => format!(
                ":attr:`{}.{}`",
                pascal(&api[id].name),
                self.ident(&api[id].fields[index].name)
            ),
            Target::Enum(id) => format!(":class:`{}`", pascal(&api[id].name)),
            Target::Variant(id, index) => format!(
                ":attr:`{}.{}`",
                pascal(&api[id].name),
                screaming(&api[id].variants[index].name)
            ),
            Target::Flags(id) => format!(":class:`{}`", pascal(&api[id].name)),
            Target::Flag(id, index) => format!(
                ":attr:`{}.{}`",
                pascal(&api[id].name),
                screaming(&api[id].flags[index].name)
            ),
            Target::Callback(id) => format!(":data:`{}`", self.callback(id)),
            Target::Class(id) => format!(":class:`{}`", self.class(id)),
            Target::Member(class, index) => {
                let member = self.member(class, index);
                match api.member(class, index) {
                    Member::Declaration(declaration)
                        if declaration.kind == DeclarationKind::Modifier =>
                    {
                        format!(":meth:`{ELEMENT}.{member}`")
                    }
                    Member::Property(_) => format!(":attr:`{}.{member}`", self.class(class)),
                    Member::Constructor(constructor) if constructor.name.is_none() => {
                        format!(":class:`{}`", self.class(class))
                    }
                    _ => format!(":meth:`{}.{member}`", self.class(class)),
                }
            }
            Target::Function(id) => format!(":func:`{}`", self.ident(&api[id].name)),
        }
    }
}

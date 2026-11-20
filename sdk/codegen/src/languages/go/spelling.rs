use crate::abi::{Slot, SlotType};
use crate::languages::keywords;
use crate::languages::projection::Linker;
use crate::model::{
    Api, Callback, Class, DeclarationKind, Element, Id, Input, Member, Primitive, Target, Value,
};
use crate::names::{pascal_with_acronyms, spaced};

const INITIALISMS: &[&str] = &[
    "api", "cpu", "gpu", "html", "http", "id", "json", "mime", "ok", "rgba", "ui", "uri", "url",
    "utf8",
];

pub(super) const ELEMENT: &str = "Element";
pub(super) const ERROR: &str = "Error";

pub(super) fn exported(name: &str) -> String {
    pascal_with_acronyms(name, INITIALISMS)
}

pub(super) fn unexported(name: &str) -> String {
    let (first, rest) = name.split_once('_').unwrap_or((name, ""));
    let joined = format!("{first}{}", exported(rest));
    match keywords::GO.contains(&joined.as_str()) {
        true => format!("{joined}_"),
        false => joined,
    }
}

pub(super) struct Spelling<'a> {
    pub(super) api: &'a Api,
}

impl<'a> Spelling<'a> {
    pub(super) fn class(&self, class: Id<Class>) -> String {
        exported(&self.api[class].name)
    }

    pub(super) fn enum_value(&self, prefix: &str, variant: &str) -> String {
        exported(&format!("{prefix}_{variant}"))
    }

    pub(super) fn constructor(&self, class: Id<Class>, name: Option<&str>) -> String {
        match name {
            None => format!("New{}", self.class(class)),
            Some(name) => format!("New{}From{}", self.class(class), exported(name)),
        }
    }

    pub(super) fn member(&self, class: Id<Class>, index: usize) -> String {
        match self.api.member(class, index) {
            Member::Constructor(constructor) => {
                self.constructor(class, constructor.name.as_deref())
            }
            Member::Method(method) => {
                let owner = &self.api[class];
                let chained = crate::languages::projection::chained(owner, method);
                exported(crate::languages::projection::builder_name(method, chained))
            }
            Member::Property(property) => exported(&property.name),
            Member::Declaration(declaration) => exported(&declaration.name),
        }
    }

    pub(super) fn setter(&self, class: Id<Class>, index: usize) -> String {
        match self.api.member(class, index) {
            Member::Property(property) => format!("Set{}", exported(&property.name)),
            _ => unreachable!("only properties have setters"),
        }
    }

    pub(super) fn trampoline(&self, callback: Id<Callback>, position: usize) -> String {
        format!(
            "{}{}{position}",
            self.api.name,
            exported(&self.api[callback].name)
        )
    }

    pub(super) fn primitive(&self, primitive: Primitive) -> &'static str {
        match primitive {
            Primitive::Bool => "bool",
            Primitive::U8 => "uint8",
            Primitive::U16 => "uint16",
            Primitive::U32 => "uint32",
            Primitive::U64 => "uint64",
            Primitive::I32 => "int32",
            Primitive::I64 => "int64",
            Primitive::Usize => "int",
            Primitive::F32 => "float32",
            Primitive::F64 => "float64",
        }
    }

    pub(super) fn c_primitive(&self, primitive: Primitive) -> &'static str {
        match primitive {
            Primitive::Bool => "C.bool",
            Primitive::U8 => "C.uint8_t",
            Primitive::U16 => "C.uint16_t",
            Primitive::U32 => "C.uint32_t",
            Primitive::U64 => "C.uint64_t",
            Primitive::I32 => "C.int32_t",
            Primitive::I64 => "C.int64_t",
            Primitive::Usize => "C.size_t",
            Primitive::F32 => "C.float",
            Primitive::F64 => "C.double",
        }
    }

    pub(super) fn c_type(&self, name: &str) -> String {
        format!("C.{}_{name}", self.api.abi_prefix)
    }

    pub(super) fn c_value(&self, value: Value) -> String {
        match value {
            Value::Primitive(primitive) => self.c_primitive(primitive).to_owned(),
            Value::Struct(id) => self.c_type(&self.api[id].name),
            Value::Enum(id) => self.c_type(&self.api[id].name),
            Value::Flags(id) => self.c_type(&self.api[id].name),
        }
    }

    pub(super) fn is_error(&self, value: Value) -> bool {
        matches!(value, Value::Enum(id) if self.api[id].error.is_some())
    }

    pub(super) fn value(&self, value: Value) -> String {
        match value {
            Value::Primitive(primitive) => self.primitive(primitive).to_owned(),
            Value::Enum(_) if self.is_error(value) => "error".to_owned(),
            Value::Struct(id) => exported(&self.api[id].name),
            Value::Enum(id) => exported(&self.api[id].name),
            Value::Flags(id) => exported(&self.api[id].name),
        }
    }

    pub(super) fn input(&self, input: &Input) -> String {
        match input {
            Input::Value(value) => match value {
                Value::Enum(id) => exported(&self.api[*id].name),
                _ => self.value(*value),
            },
            Input::String => "string".to_owned(),
            Input::Object { class, .. } => format!("*{}", self.class(*class)),
            Input::Array(Element::Primitive(Primitive::U8)) => "[]byte".to_owned(),
            Input::Array(Element::Primitive(primitive)) => {
                format!("[]{}", self.primitive(*primitive))
            }
            Input::Array(Element::String) => "[]string".to_owned(),
            Input::Closure(closure) => self.signature(closure.functions[0].callback),
        }
    }

    pub(super) fn received(&self, input: &Input) -> String {
        match input {
            Input::Value(value) => self.value(*value),
            _ => self.input(input),
        }
    }

    pub(super) fn signature(&self, callback: Id<Callback>) -> String {
        let params: Vec<String> = self.api[callback]
            .params
            .iter()
            .map(|param| format!("{} {}", unexported(&param.name), self.received(&param.ty)))
            .collect();
        format!("func({})", params.join(", "))
    }

    pub(super) fn bare_signature(&self, callback: Id<Callback>) -> String {
        let params: Vec<String> = self.api[callback]
            .params
            .iter()
            .map(|param| self.received(&param.ty))
            .collect();
        format!("func({})", params.join(", "))
    }

    pub(super) fn go_slot(&self, slot: &Slot) -> String {
        let ty = match slot.ty {
            SlotType::Value(value) => self.c_value(value),
            SlotType::Str => self.c_type("str"),
            SlotType::Object(class) => format!("*{}", self.c_type(&self.api[class].name)),
            SlotType::Pointer(Element::Primitive(primitive)) => {
                format!("*{}", self.c_primitive(primitive))
            }
            SlotType::Pointer(Element::String) => format!("*{}", self.c_type("str")),
            SlotType::Count => "C.size_t".to_owned(),
            SlotType::Callback(callback) => self.c_type(&self.api[callback].name),
            SlotType::Userdata => "unsafe.Pointer".to_owned(),
            SlotType::Destroy => self.c_type("destroy_fn"),
        };
        format!("{} {ty}", unexported(&slot.name))
    }
}

impl Linker for Spelling<'_> {
    fn link(&self, target: Target) -> String {
        let api = self.api;
        match target {
            Target::Constant(id) => format!("[{}]", exported(&api[id].name)),
            Target::Struct(id) => format!("[{}]", exported(&api[id].name)),
            Target::Field(id, index) => format!(
                "[{}].{}",
                exported(&api[id].name),
                exported(&api[id].fields[index].name)
            ),
            Target::Enum(id) => format!("[{}]", exported(&api[id].name)),
            Target::Variant(id, index) => format!(
                "[{}]",
                self.enum_value(&api[id].value_prefix, &api[id].variants[index].name)
            ),
            Target::Flags(id) => format!("[{}]", exported(&api[id].name)),
            Target::Flag(id, index) => format!(
                "[{}]",
                self.enum_value(&api[id].value_prefix, &api[id].flags[index].name)
            ),
            Target::Callback(id) => format!("the {} callback", spaced(&api[id].name)),
            Target::Class(id) => format!("[{}]", self.class(id)),
            Target::Member(class, index) => {
                let member = self.member(class, index);
                match api.member(class, index) {
                    Member::Declaration(declaration)
                        if declaration.kind == DeclarationKind::Modifier =>
                    {
                        format!("[{ELEMENT}.{member}]")
                    }
                    Member::Constructor(_) => format!("[{member}]"),
                    _ => format!("[{}.{member}]", self.class(class)),
                }
            }
            Target::Function(id) => format!("[{}]", exported(&api[id].name)),
        }
    }
}

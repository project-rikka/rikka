use crate::abi::{Origin, Slot, SlotType};
use crate::languages::{c, keywords};
use crate::model::{
    Api, Callback, Class, Configuration, DeclarationKind, Element, Id, Input, Member, Method,
    Ownership, Paragraph, Primitive, Target, Value,
};
use crate::names::pascal;

pub(crate) const ELEMENT: &str = "Element";
pub(crate) const ERROR: &str = "Error";
const SETTER: &str = "set_";
const CONSTRUCTOR: &str = "from_";

pub(crate) struct Spelling<'a> {
    pub(crate) api: &'a Api,
    pub(crate) c: c::Spelling<'a>,
}

impl<'a> Spelling<'a> {
    pub(crate) fn new(api: &'a Api) -> Self {
        Self {
            api,
            c: c::Spelling::new(api),
        }
    }

    pub(crate) fn namespace(&self) -> &str {
        &self.api.name
    }

    pub(crate) fn ident(&self, name: &str) -> String {
        match keywords::CPP.contains(&name) {
            true => format!("{name}_"),
            false => name.to_owned(),
        }
    }

    pub(crate) fn class(&self, class: Id<Class>) -> String {
        pascal(&self.api[class].name)
    }

    pub(crate) fn c_class(&self, class: Id<Class>) -> String {
        self.c.type_name(&self.api[class].name)
    }

    pub(crate) fn member(&self, class: Id<Class>, index: usize) -> String {
        let owner = &self.api[class];
        match &owner.members[index] {
            Member::Constructor(constructor) => match &constructor.name {
                None => self.class(class),
                Some(name) => self.ident(&format!("{CONSTRUCTOR}{name}")),
            },
            Member::Method(method) => {
                match (configures(owner, method), method.name.strip_prefix(SETTER)) {
                    (true, Some(stripped)) => self.ident(stripped),
                    _ => self.ident(&method.name),
                }
            }
            Member::Property(property) => self.ident(&property.name),
            Member::Declaration(declaration) => self.ident(&declaration.name),
        }
    }

    pub(crate) fn setter(&self, class: Id<Class>, index: usize) -> String {
        match self.api.member(class, index) {
            Member::Property(property) => self.ident(&format!("{SETTER}{}", property.name)),
            _ => unreachable!("only properties have setters"),
        }
    }

    pub(crate) fn primitive(&self, primitive: Primitive) -> &'static str {
        match primitive {
            Primitive::Bool => "bool",
            Primitive::U8 => "std::uint8_t",
            Primitive::U16 => "std::uint16_t",
            Primitive::U32 => "std::uint32_t",
            Primitive::U64 => "std::uint64_t",
            Primitive::I32 => "std::int32_t",
            Primitive::I64 => "std::int64_t",
            Primitive::Usize => "std::size_t",
            Primitive::F32 => "float",
            Primitive::F64 => "double",
        }
    }

    pub(crate) fn value(&self, value: Value) -> String {
        match value {
            Value::Primitive(primitive) => self.primitive(primitive).to_owned(),
            Value::Struct(id) => pascal(&self.api[id].name),
            Value::Enum(id) => pascal(&self.api[id].name),
            Value::Flags(id) => pascal(&self.api[id].name),
        }
    }

    pub(crate) fn c_value(&self, value: Value) -> String {
        self.c.value(value)
    }

    pub(crate) fn c_slot(&self, slot: &Slot) -> String {
        let name = self.ident(&slot.name);
        let ty = match slot.ty {
            SlotType::Value(Value::Primitive(primitive)) => self.primitive(primitive).to_owned(),
            SlotType::Value(value) => self.c.value(value),
            SlotType::Str => self.c.type_name("str"),
            SlotType::Object(class) => format!("{}*", self.c_class(class)),
            SlotType::Pointer(Element::Primitive(primitive)) => {
                format!("const {}*", c::primitive(primitive))
            }
            SlotType::Pointer(Element::String) => format!("const {}*", self.c.type_name("str")),
            SlotType::Count => "std::size_t".to_owned(),
            SlotType::Callback(callback) => self.c.type_name(&self.api[callback].name),
            SlotType::Userdata => "void*".to_owned(),
            SlotType::Destroy => self.c.type_name("destroy_fn"),
        };
        format!("{ty} {name}")
    }

    pub(crate) fn argument(&self, input: &Input) -> String {
        match input {
            Input::Value(value) => self.value(*value),
            Input::String => "std::string_view".to_owned(),
            Input::Object { class, .. } => format!("{}&", self.class(*class)),
            Input::Array(Element::String) => "std::span<const std::string_view>".to_owned(),
            Input::Array(Element::Primitive(primitive)) => {
                format!("std::span<const {}>", self.primitive(*primitive))
            }
            Input::Closure(_) => unreachable!("validated: callbacks receive no closures"),
        }
    }

    pub(crate) fn link(&self, target: Target) -> String {
        let api = self.api;
        let namespace = self.namespace();
        match target {
            Target::Constant(id) => format!("{namespace}::{}", self.ident(&api[id].name)),
            Target::Struct(id) => format!("{namespace}::{}", pascal(&api[id].name)),
            Target::Field(id, index) => format!(
                "{namespace}::{}::{}",
                pascal(&api[id].name),
                self.ident(&api[id].fields[index].name)
            ),
            Target::Enum(id) => format!("{namespace}::{}", pascal(&api[id].name)),
            Target::Variant(id, index) => format!(
                "{namespace}::{}::{}",
                pascal(&api[id].name),
                self.ident(&api[id].variants[index].name)
            ),
            Target::Flags(id) => format!("{namespace}::{}", pascal(&api[id].name)),
            Target::Flag(id, index) => format!(
                "{namespace}::{}::{}",
                pascal(&api[id].name),
                self.ident(&api[id].flags[index].name)
            ),
            Target::Callback(id) => format!("`{}`", api[id].name),
            Target::Class(id) => format!("{namespace}::{}", self.class(id)),
            Target::Member(class, index) => {
                let owner = match api.member(class, index) {
                    Member::Declaration(declaration)
                        if declaration.kind == DeclarationKind::Modifier =>
                    {
                        ELEMENT.to_owned()
                    }
                    _ => self.class(class),
                };
                format!("{namespace}::{owner}::{}()", self.member(class, index))
            }
            Target::Function(id) => format!("{namespace}::{}()", self.ident(&api[id].name)),
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

    pub(crate) fn trampoline(&self, callback: Id<Callback>) -> String {
        self.ident(&self.api[callback].name)
    }

    pub(crate) fn c_function(&self, origin: Origin) -> String {
        self.c.function(origin)
    }
}

pub(crate) fn configures(owner: &Class, method: &Method) -> bool {
    owner.configuration == Configuration::Builder
        && method.receiver == Ownership::Borrowed
        && method.signature.returns.is_none()
        && method.signature.throws.is_none()
}

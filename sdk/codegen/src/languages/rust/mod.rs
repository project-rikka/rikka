use askama::Template;

use crate::abi::{self, Abi, Function, Lifecycle, Origin, Return, Slot, SlotType};
use crate::diagnostic::{Diagnostic, Span};
use crate::language::{Error, Language, Output};
use crate::languages::unique;
use crate::model::{
    Api, Callback, Class, ClosureFunction, ConstantValue, Element, Id, Input, Lifetime, Openness,
    Output as Returned, Ownership, Param, Primitive, Threading, Value,
};
use crate::names::{pascal, screaming};

const KEYWORDS: &[&str] = &[
    "abstract", "as", "async", "await", "become", "box", "break", "const", "continue", "do", "dyn",
    "else", "enum", "extern", "false", "final", "fn", "for", "gen", "if", "impl", "in", "let",
    "loop", "macro", "match", "mod", "move", "mut", "override", "priv", "pub", "ref", "return",
    "static", "struct", "trait", "true", "try", "type", "typeof", "unsafe", "unsized", "use",
    "virtual", "where", "while", "yield",
];
const UNESCAPABLE: &[&str] = &["self", "super", "crate", "Self"];
const TYPES: &[&str] = &["Str", "String", "DestroyFn"];
const CONSTANTS: &[&str] = &["VERSION", "STRLEN"];
const FLAG_CONSTANTS: &[&str] = &["ALL", "C_NAME"];
const OPTION: &str = "::core::option::Option";

pub(crate) struct Rust;

impl Language for Rust {
    fn name(&self) -> &'static str {
        "rust"
    }

    fn generate(&self, api: &Api) -> Result<Vec<Output>, Error> {
        let abi = Abi::new(api)?;
        validate(api, &abi)?;
        let bindings = Bindings::new(api, &abi);
        Ok(vec![Output::render("ffi.rs".to_owned(), &bindings)?])
    }
}

fn validate(api: &Api, abi: &Abi) -> Result<(), Diagnostic> {
    let escapable = |name: &str, span: Span| match UNESCAPABLE.contains(&name) {
        true => Err(Diagnostic::new(
            span,
            format!("`{name}` cannot name a Rust variable or field"),
        )),
        false => Ok(()),
    };
    for field in api.structs.iter().flat_map(|item| &item.fields) {
        escapable(&field.name, field.span)?;
    }
    for param in api.callbacks.iter().flat_map(|callback| &callback.params) {
        for slot in abi::param_slots(param) {
            escapable(&slot.name, param.span)?;
        }
    }
    for function in &abi.functions {
        for (slot, span) in function.located_slots(api) {
            escapable(&slot.name, span)?;
        }
    }

    let reserved = |names: &[&str]| -> Vec<(String, Span)> {
        names
            .iter()
            .map(|name| ((*name).to_owned(), Span::default()))
            .collect()
    };
    let types = api
        .structs
        .iter()
        .map(|item| (pascal(&item.name), item.span))
        .chain(api.enums.iter().map(|item| (pascal(&item.name), item.span)))
        .chain(api.flags.iter().map(|item| (pascal(&item.name), item.span)))
        .chain(
            api.callbacks
                .iter()
                .map(|item| (pascal(&item.name), item.span)),
        );
    unique(reserved(TYPES).into_iter().chain(types), |name| {
        format!("this becomes the Rust type `{name}`, which is taken")
    })?;
    let constants = api
        .constants
        .iter()
        .map(|item| (screaming(&item.name), item.span));
    unique(reserved(CONSTANTS).into_iter().chain(constants), |name| {
        format!("this becomes the Rust constant `{name}`, which is taken")
    })?;
    for item in api
        .enums
        .iter()
        .filter(|item| item.openness == Openness::Closed)
    {
        let variants = item
            .variants
            .iter()
            .map(|variant| (pascal(&variant.name), variant.span));
        unique(variants, |name| {
            format!("this becomes the Rust variant `{name}`, which another value also does")
        })?;
    }
    for item in &api.flags {
        let flags = item
            .flags
            .iter()
            .map(|flag| (screaming(&flag.name), flag.span));
        unique(reserved(FLAG_CONSTANTS).into_iter().chain(flags), |name| {
            format!("this becomes the Rust constant `{name}`, which is taken")
        })?;
    }
    let rust = Spelling { api };
    let mut shapes: Vec<(String, Vec<String>, &str)> = Vec::new();
    for callback in &api.callbacks {
        let signature = rust.signature(callback);
        let types = rust.call(callback, String::new(), String::new()).types;
        match shapes.iter().find(|(shared, ..)| *shared == signature) {
            Some((_, expected, first)) if *expected != types => {
                return Err(Diagnostic::new(
                    callback.span,
                    format!(
                        "this shares its C signature with `{first}` but not its parameter types"
                    ),
                ));
            }
            Some(_) => {}
            None => shapes.push((signature, types, &callback.name)),
        }
    }
    Ok(())
}

struct Named {
    name: String,
    ty: String,
}

struct Constant {
    name: String,
    ty: String,
    value: String,
}

struct Variant {
    name: String,
    value: String,
}

struct Record {
    name: String,
    fields: Vec<Named>,
}

struct Enumeration {
    name: String,
    c_name: String,
    open: bool,
    variants: Vec<Variant>,
}

struct FlagSet {
    name: String,
    c_name: String,
    all: String,
    flags: Vec<Variant>,
}

struct Shape {
    ty: String,
    methods: Vec<Call>,
}

struct Call {
    name: String,
    params: String,
    types: Vec<String>,
    prelude: Vec<String>,
    access: String,
    arguments: String,
}

struct Export {
    name: String,
    params: String,
    returns: String,
    body: Vec<String>,
}

#[derive(Template)]
#[template(path = "rust/ffi.rs.j2", escape = "none")]
struct Bindings {
    prefix: String,
    version: u32,
    constants: Vec<Constant>,
    structs: Vec<Record>,
    enums: Vec<Enumeration>,
    flags: Vec<FlagSet>,
    callbacks: Vec<Named>,
    shapes: Vec<Shape>,
    exports: Vec<Export>,
}

impl Bindings {
    fn new(api: &Api, abi: &Abi) -> Self {
        let rust = Spelling { api };
        Self {
            prefix: api.abi_prefix.clone(),
            version: api.version.packed(),
            constants: api
                .constants
                .iter()
                .map(|constant| {
                    let ty = primitive(constant.ty);
                    Constant {
                        name: screaming(&constant.name),
                        ty: ty.to_owned(),
                        value: match constant.value {
                            ConstantValue::Maximum => format!("{ty}::MAX"),
                            ConstantValue::Integer(value) => value.to_string(),
                            ConstantValue::Float(value) => format!("{value:?}"),
                        },
                    }
                })
                .collect(),
            structs: api
                .structs
                .iter()
                .map(|item| Record {
                    name: pascal(&item.name),
                    fields: item
                        .fields
                        .iter()
                        .map(|field| Named {
                            name: ident(&field.name),
                            ty: primitive(field.ty).to_owned(),
                        })
                        .collect(),
                })
                .collect(),
            enums: api
                .enums
                .iter()
                .map(|item| {
                    let open = item.openness == Openness::Open;
                    Enumeration {
                        name: pascal(&item.name),
                        c_name: format!("{}_{}", api.abi_prefix, item.name),
                        open,
                        variants: item
                            .variants
                            .iter()
                            .map(|variant| Variant {
                                name: match open {
                                    true => screaming(&variant.name),
                                    false => pascal(&variant.name),
                                },
                                value: variant.value.to_string(),
                            })
                            .collect(),
                    }
                })
                .collect(),
            flags: api
                .flags
                .iter()
                .map(|item| FlagSet {
                    name: pascal(&item.name),
                    c_name: format!("{}_{}", api.abi_prefix, item.name),
                    all: item
                        .flags
                        .iter()
                        .map(|flag| format!("0x{:x}", flag.value))
                        .collect::<Vec<_>>()
                        .join(" | "),
                    flags: item
                        .flags
                        .iter()
                        .map(|flag| Variant {
                            name: screaming(&flag.name),
                            value: format!("0x{:x}", flag.value),
                        })
                        .collect(),
                })
                .collect(),
            callbacks: api
                .callbacks
                .iter()
                .map(|callback| Named {
                    name: pascal(&callback.name),
                    ty: rust.signature(callback),
                })
                .collect(),
            shapes: rust.shapes(abi),
            exports: abi
                .functions
                .iter()
                .map(|function| rust.export(function))
                .collect(),
        }
    }
}

struct Spelling<'a> {
    api: &'a Api,
}

impl Spelling<'_> {
    fn handle(&self, class: Id<Class>) -> String {
        format!("crate::api::{}", pascal(&self.api[class].name))
    }

    fn value(&self, value: Value) -> String {
        match value {
            Value::Primitive(primitive) => self::primitive(primitive).to_owned(),
            Value::Struct(id) => pascal(&self.api[id].name),
            Value::Enum(_) | Value::Flags(_) => "u32".to_owned(),
        }
    }

    fn element(&self, element: Element) -> String {
        match element {
            Element::Primitive(primitive) => self::primitive(primitive).to_owned(),
            Element::String => "Str".to_owned(),
        }
    }

    fn slot(&self, slot: &Slot) -> String {
        match slot.ty {
            SlotType::Value(value) => self.value(value),
            SlotType::Str => "Str".to_owned(),
            SlotType::Object(class) => format!("*mut {}", self.handle(class)),
            SlotType::Pointer(element) => format!("*const {}", self.element(element)),
            SlotType::Count => "usize".to_owned(),
            SlotType::Callback(callback) => {
                format!("{OPTION}<{}>", pascal(&self.api[callback].name))
            }
            SlotType::Userdata => "*mut c_void".to_owned(),
            SlotType::Destroy => format!("{OPTION}<DestroyFn>"),
        }
    }

    fn signature(&self, callback: &Callback) -> String {
        abi::callback_slots(callback)
            .iter()
            .map(|slot| self.slot(slot))
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn shapes(&self, abi: &Abi) -> Vec<Shape> {
        let mut seen: Vec<Vec<String>> = Vec::new();
        let mut shapes = Vec::new();
        for function in &abi.functions {
            for param in &function.signature.params {
                let Input::Closure(closure) = &param.ty else {
                    continue;
                };
                let callbacks: Vec<Id<Callback>> = closure
                    .functions
                    .iter()
                    .map(|function| function.callback)
                    .collect();
                let signatures: Vec<String> = callbacks
                    .iter()
                    .map(|callback| self.signature(&self.api[*callback]))
                    .collect();
                if seen.contains(&signatures) {
                    continue;
                }
                seen.push(signatures);
                let types: Vec<String> = callbacks
                    .iter()
                    .map(|callback| pascal(&self.api[*callback].name))
                    .collect();
                let ty = match types.as_slice() {
                    [single] => single.clone(),
                    many => format!("({})", many.join(", ")),
                };
                let methods = closure
                    .functions
                    .iter()
                    .enumerate()
                    .map(|(position, part)| {
                        let (name, access) = match closure.functions.len() {
                            1 => ("call".to_owned(), "self.function()".to_owned()),
                            _ => (
                                format!("call_{}", part.name),
                                format!("self.function().{position}"),
                            ),
                        };
                        self.call(&self.api[part.callback], name, access)
                    })
                    .collect();
                shapes.push(Shape { ty, methods });
            }
        }
        shapes
    }

    fn call(&self, callback: &Callback, name: String, access: String) -> Call {
        let mut types = Vec::new();
        let mut prelude = Vec::new();
        let mut arguments = Vec::new();
        for param in &callback.params {
            let name = ident(&param.name);
            match &param.ty {
                Input::Value(Value::Primitive(primitive)) => {
                    types.push(self::primitive(*primitive).to_owned());
                    arguments.push(name.clone());
                }
                Input::Value(Value::Struct(id)) => {
                    types.push(pascal(&self.api[*id].name));
                    arguments.push(name.clone());
                }
                Input::Value(Value::Enum(id)) => {
                    let item = &self.api[*id];
                    types.push(pascal(&item.name));
                    arguments.push(match item.openness {
                        Openness::Open => format!("{name}.0"),
                        Openness::Closed => format!("{name}.into_raw()"),
                    });
                }
                Input::Value(Value::Flags(id)) => {
                    types.push(pascal(&self.api[*id].name));
                    arguments.push(format!("{name}.0"));
                }
                Input::String => {
                    types.push("&str".to_owned());
                    arguments.push(format!("crate::abi::view({name})"));
                }
                Input::Object { class, .. } => {
                    types.push(format!("&mut {}", self.handle(*class)));
                    arguments.push(format!("std::ptr::from_mut({name})"));
                }
                Input::Array(Element::String) => {
                    types.push("&[&str]".to_owned());
                    prelude.push(format!("let {name} = crate::abi::views({name});"));
                    arguments.push(format!("{name}.as_ptr()"));
                    arguments.push(format!("{name}.len()"));
                }
                Input::Array(Element::Primitive(primitive)) => {
                    types.push(format!("&[{}]", self::primitive(*primitive)));
                    arguments.push(format!("{name}.as_ptr()"));
                    arguments.push(format!("{name}.len()"));
                }
                Input::Closure(_) => unreachable!("validated: callbacks receive no closures"),
            }
        }
        arguments.push("self.userdata()".to_owned());
        let params = std::iter::once("&self".to_owned())
            .chain(
                callback
                    .params
                    .iter()
                    .zip(&types)
                    .map(|(param, ty)| format!("{}: {ty}", ident(&param.name))),
            )
            .collect::<Vec<_>>()
            .join(", ");
        Call {
            name,
            params,
            types,
            prelude,
            access,
            arguments: arguments.join(", "),
        }
    }

    fn export(&self, function: &Function) -> Export {
        let api = self.api;
        let name = format!("{}_{}", api.abi_prefix, function.name);
        let params = function
            .slots(api)
            .iter()
            .map(|slot| format!("{}: {}", ident(&slot.name), self.slot(slot)))
            .collect::<Vec<_>>()
            .join(", ");
        let returns = match function.returns() {
            Return::Void => String::new(),
            Return::Value(value) => format!(" -> {}", self.value(value)),
            Return::Str => " -> Str".to_owned(),
            Return::String => " -> String".to_owned(),
            Return::Object(class) => format!(" -> *mut {}", self.handle(class)),
        };
        let body = match function.origin {
            Origin::Lifecycle(class, lifecycle) => vec![self.lifecycle(class, lifecycle)],
            _ => self.forward(function),
        };
        Export {
            name,
            params,
            returns,
            body,
        }
    }

    fn lifecycle(&self, class: Id<Class>, lifecycle: Lifecycle) -> String {
        let label = abi::receiver_name(&self.api[class]);
        let receiver = ident(&label);
        let (kind, arguments) = match self.api[class].lifetime {
            Lifetime::Owned => ("owned", receiver),
            Lifetime::Shared => ("shared", receiver),
            Lifetime::Local => ("local", format!("{receiver}, \"{label}\"")),
            Lifetime::Scoped => unreachable!("scoped classes have no lifecycle"),
        };
        let operation = lifecycle.suffix();
        format!("unsafe {{ crate::abi::{operation}_{kind}({arguments}) }}")
    }

    fn forward(&self, function: &Function) -> Vec<String> {
        let api = self.api;
        let mut body = Vec::new();
        let mut arguments = Vec::new();
        if let Some(receiver) = function.receiver {
            let label = abi::receiver_name(&api[receiver.class]);
            body.push(self.object(&label, receiver.class, receiver.ownership));
            arguments.push(ident(&label));
        }
        for param in &function.signature.params {
            body.extend(self.convert(param));
            arguments.push(ident(&param.name));
        }
        let call = format!("crate::api::{}({})", function.name, arguments.join(", "));
        body.push(match function.returns() {
            Return::Void => format!("{call};"),
            Return::Value(Value::Enum(id)) => match api[id].openness {
                Openness::Open => format!("{call}.0"),
                Openness::Closed => format!("{call}.into_raw()"),
            },
            Return::Value(Value::Flags(_)) => format!("{call}.0"),
            Return::Value(_) | Return::Str => call,
            Return::String => format!("crate::abi::give_string({call})"),
            Return::Object(class) => match function.signature.returns {
                Some(Returned::Object {
                    ownership: Ownership::Borrowed,
                    ..
                }) => format!("crate::abi::lend({call})"),
                _ => match api[class].lifetime {
                    Lifetime::Owned => format!("crate::abi::give_owned({call})"),
                    Lifetime::Shared => format!("crate::abi::give_shared({call})"),
                    Lifetime::Local => format!("crate::abi::give_local({call})"),
                    Lifetime::Scoped => {
                        unreachable!("validated: scoped classes are never returned")
                    }
                },
            },
        });
        body
    }

    fn object(&self, label: &str, class: Id<Class>, ownership: Ownership) -> String {
        let helper = match (self.api[class].lifetime, ownership) {
            (Lifetime::Owned, Ownership::Borrowed) => "owned_mut",
            (Lifetime::Owned, Ownership::Owned) => "take_owned",
            (Lifetime::Shared, _) => "shared_ref",
            (Lifetime::Local, _) => "local_ref",
            (Lifetime::Scoped, _) => "scoped_mut",
        };
        let name = ident(label);
        format!("let {name} = unsafe {{ crate::abi::{helper}({name}, \"{label}\") }};")
    }

    fn convert(&self, param: &Param) -> Option<String> {
        let api = self.api;
        let label = &param.name;
        let name = ident(label);
        Some(match &param.ty {
            Input::String => {
                format!("let {name} = unsafe {{ crate::abi::text({name}, \"{label}\") }};")
            }
            Input::Value(Value::Enum(id)) => {
                let item = &api[*id];
                let ty = pascal(&item.name);
                match item.openness {
                    Openness::Open => format!("let {name} = {ty}({name});"),
                    Openness::Closed => format!(
                        "let {name} = crate::abi::enumerated({name}, {ty}::from_raw, {ty}::C_NAME, \"{label}\");"
                    ),
                }
            }
            Input::Value(Value::Flags(id)) => {
                let ty = pascal(&api[*id].name);
                format!(
                    "let {name} = crate::abi::enumerated({name}, {ty}::from_raw, {ty}::C_NAME, \"{label}\");"
                )
            }
            Input::Value(_) => return None,
            Input::Object { class, ownership } => self.object(label, *class, *ownership),
            Input::Array(_) => {
                let count = ident(&abi::count_name(label));
                format!(
                    "let {name} = unsafe {{ crate::abi::slice({name}, {count}, \"{label}\") }};"
                )
            }
            Input::Closure(closure) => {
                let threads = match closure.threading {
                    Threading::AnyThread => "AnyThread",
                    Threading::ThisThread => "ThisThread",
                };
                let required = |part: &ClosureFunction| {
                    let function = ident(&part.name);
                    format!("crate::abi::required({function}, \"{}\")", part.name)
                };
                let functions = match closure.functions.as_slice() {
                    [single] => required(single),
                    many => format!(
                        "({})",
                        many.iter().map(required).collect::<Vec<_>>().join(", ")
                    ),
                };
                format!(
                    "let {name} = unsafe {{ crate::abi::Closure::<_, crate::abi::{threads}>::new({functions}, {}, {}) }};",
                    abi::USERDATA,
                    abi::DESTROY
                )
            }
        })
    }
}

fn ident(name: &str) -> String {
    match KEYWORDS.contains(&name) {
        true => format!("r#{name}"),
        false => name.to_owned(),
    }
}

fn primitive(primitive: Primitive) -> &'static str {
    match primitive {
        Primitive::Bool => "bool",
        Primitive::U8 => "u8",
        Primitive::U16 => "u16",
        Primitive::U32 => "u32",
        Primitive::U64 => "u64",
        Primitive::I32 => "i32",
        Primitive::I64 => "i64",
        Primitive::Usize => "usize",
        Primitive::F32 => "f32",
        Primitive::F64 => "f64",
    }
}

mod docs;
mod spelling;

use askama::Template;

use crate::abi::{self, Abi, Lifecycle, Origin};
use crate::diagnostic::{Diagnostic, Span};
use crate::language::{Error, Language, Output};
use crate::languages::{doxygen, keywords, unique};
use crate::model::{Api, ConstantValue, Doc, Id, Lifetime, Primitive, Version};
use crate::names::screaming;

pub(crate) use spelling::{Spelling, primitive};

const FORCE32: &str = "0x7FFFFFFF";

const BUILTIN_TYPES: &[&str] = &["str", "string", "destroy_fn"];
const BUILTIN_FUNCTIONS: &[&str] = &["cstr", "string_free"];
const BUILTIN_MACROS: &[&str] = &[
    "api",
    "nodiscard",
    "make_version",
    "version_major",
    "version_minor",
    "version_patch",
    "header_version",
    "str",
    "strlen",
];

pub(crate) struct C;

impl Language for C {
    fn name(&self) -> &'static str {
        "c"
    }

    fn generate(&self, api: &Api) -> Result<Vec<Output>, Error> {
        let abi = Abi::new(api)?;
        validate(api, &abi)?;
        let spelling = Spelling::new(api);
        let header = Header::new(&spelling, &abi);
        let pkgconfig = PkgConfig {
            library: &api.name,
            version: api.version,
        };
        Ok(vec![
            Output::render(format!("include/{0}/{0}.h", api.name), &header)?,
            Output::render(format!("{}.pc.in", api.name), &pkgconfig)?,
        ])
    }
}

fn validate(api: &Api, abi: &Abi) -> Result<(), Diagnostic> {
    let keyword = |name: &str, span: Span| match keywords::is_c_or_cpp(name) {
        true => Err(Diagnostic::new(
            span,
            format!("`{name}` is a C or C++ keyword and cannot name a C parameter or field"),
        )),
        false => Ok(()),
    };
    for field in api.structs.iter().flat_map(|item| &item.fields) {
        keyword(&field.name, field.span)?;
    }
    for param in api.callbacks.iter().flat_map(|callback| &callback.params) {
        for slot in abi::param_slots(param) {
            keyword(&slot.name, param.span)?;
        }
    }
    for function in &abi.functions {
        for (slot, span) in function.located_slots(api) {
            keyword(&slot.name, span)?;
        }
    }

    let types = api
        .structs
        .iter()
        .map(|item| (item.name.clone(), item.span))
        .chain(api.enums.iter().map(|item| (item.name.clone(), item.span)))
        .chain(api.flags.iter().map(|item| (item.name.clone(), item.span)))
        .chain(
            api.callbacks
                .iter()
                .map(|item| (item.name.clone(), item.span)),
        )
        .chain(
            api.classes
                .iter()
                .map(|item| (item.name.clone(), item.span)),
        );
    let ordinary = builtins(BUILTIN_TYPES.iter().chain(BUILTIN_FUNCTIONS))
        .chain(types)
        .chain(
            abi.functions
                .iter()
                .map(|function| (function.name.clone(), function.span(api))),
        );
    unique(ordinary, |name| {
        format!(
            "`{}_{name}` is declared twice in the C namespace",
            api.abi_prefix
        )
    })?;

    let values = api
        .enums
        .iter()
        .flat_map(|item| {
            item.variants
                .iter()
                .map(|variant| {
                    (
                        format!("{}_{}", item.value_prefix, variant.name),
                        variant.span,
                    )
                })
                .chain([(format!("{}_force32", item.value_prefix), item.span)])
        })
        .chain(api.flags.iter().flat_map(|item| {
            item.flags
                .iter()
                .map(|flag| (format!("{}_{}", item.value_prefix, flag.name), flag.span))
                .chain([(format!("{}_none", item.value_prefix), item.span)])
        }));
    let macros = builtins(BUILTIN_MACROS.iter())
        .chain(
            api.constants
                .iter()
                .map(|constant| (constant.name.clone(), constant.span)),
        )
        .chain(values);
    unique(macros, |name| {
        format!(
            "`{}_{}` is defined twice among the C macros",
            screaming(&api.abi_prefix),
            screaming(name)
        )
    })
}

fn builtins<'a>(
    names: impl Iterator<Item = &'a &'static str>,
) -> impl Iterator<Item = (String, Span)> {
    names.map(|name| ((*name).to_owned(), Span::default()))
}

struct Define {
    doc: String,
    name: String,
    value: String,
}

struct Line {
    doc: String,
    text: String,
}

struct Typed {
    doc: String,
    name: String,
    lines: Vec<Line>,
}

struct Declared {
    doc: String,
    text: String,
}

#[derive(Template)]
#[template(path = "c/header.h.j2", escape = "none")]
struct Header {
    library: String,
    guard: String,
    prefix: String,
    upper: String,
    doc: String,
    version: Version,
    constants: Vec<Define>,
    structs: Vec<Typed>,
    enums: Vec<Typed>,
    flags: Vec<Typed>,
    handles: Vec<Declared>,
    callbacks: Vec<Declared>,
    functions: Vec<Declared>,
}

impl Header {
    fn new(spelling: &Spelling, abi: &Abi) -> Self {
        let api = spelling.api;
        let prefix = spelling.prefix().to_owned();
        let upper = spelling.upper();
        let body = |doc: &Doc| spelling.body(&doc.body);
        let constants = api
            .constants
            .iter()
            .map(|constant| Define {
                doc: doxygen::block("", &body(&constant.doc)),
                name: spelling.macro_name(&constant.name),
                value: match constant.value {
                    ConstantValue::Maximum => maximum(constant.ty).to_owned(),
                    ConstantValue::Integer(value) => integer(constant.ty, value),
                    ConstantValue::Float(value) => float(constant.ty, value),
                },
            })
            .collect();
        let structs = api
            .structs
            .iter()
            .map(|item| Typed {
                doc: doxygen::block("", &body(&item.doc)),
                name: spelling.type_name(&item.name),
                lines: item
                    .fields
                    .iter()
                    .map(|field| Line {
                        doc: doxygen::block("    ", &body(&field.doc)),
                        text: format!("{} {};", spelling::primitive(field.ty), field.name),
                    })
                    .collect(),
            })
            .collect();
        let enums = Id::all(&api.enums)
            .map(|id| {
                let item = &api[id];
                let mut lines: Vec<Line> = item
                    .variants
                    .iter()
                    .enumerate()
                    .map(|(index, variant)| Line {
                        doc: doxygen::block("    ", &body(&variant.doc)),
                        text: format!("{} = {},", spelling.variant(id, index), variant.value),
                    })
                    .collect();
                lines.push(Line {
                    doc: String::new(),
                    text: format!(
                        "{} = {FORCE32}",
                        spelling.macro_name(&format!("{}_force32", item.value_prefix))
                    ),
                });
                Typed {
                    doc: doxygen::block("", &body(&item.doc)),
                    name: spelling.type_name(&item.name),
                    lines,
                }
            })
            .collect();
        let flags = Id::all(&api.flags)
            .map(|id| {
                let item = &api[id];
                let none = Line {
                    doc: String::new(),
                    text: format!(
                        "{} = 0,",
                        spelling.macro_name(&format!("{}_none", item.value_prefix))
                    ),
                };
                let lines = std::iter::once(none)
                    .chain(item.flags.iter().enumerate().map(|(index, flag)| Line {
                        doc: doxygen::block("    ", &body(&flag.doc)),
                        text: format!("{} = 0x{:X},", spelling.flag(id, index), flag.value),
                    }))
                    .collect();
                Typed {
                    doc: doxygen::block("", &body(&item.doc)),
                    name: spelling.type_name(&item.name),
                    lines,
                }
            })
            .collect();
        let handles = Id::all(&api.classes)
            .map(|id| {
                let class = &api[id];
                let name = spelling.type_name(&class.name);
                let lifetime = match class.lifetime {
                    Lifetime::Owned => format!(
                        "Owned by whoever holds it until handed over; {}() destroys one that never was.",
                        spelling.function(Origin::Lifecycle(id, Lifecycle::Free))
                    ),
                    Lifetime::Shared => format!(
                        "Reference-counted with {}() and {}(); usable from any thread.",
                        spelling.function(Origin::Lifecycle(id, Lifecycle::Ref)),
                        spelling.function(Origin::Lifecycle(id, Lifecycle::Unref))
                    ),
                    Lifetime::Local => format!(
                        "Reference-counted with {}() and {}(); usable only on the thread that made it.",
                        spelling.function(Origin::Lifecycle(id, Lifecycle::Ref)),
                        spelling.function(Origin::Lifecycle(id, Lifecycle::Unref))
                    ),
                    Lifetime::Scoped => {
                        "Borrowed for the duration of the callback that receives it.".to_owned()
                    }
                };
                let mut paragraphs = body(&class.doc);
                paragraphs.push(lifetime);
                Declared {
                    doc: doxygen::block("", &paragraphs),
                    text: format!("typedef struct {name} {name};"),
                }
            })
            .collect();
        let callbacks = api
            .callbacks
            .iter()
            .map(|callback| {
                let params: Vec<String> = abi::callback_slots(callback)
                    .iter()
                    .map(|slot| spelling.slot(slot))
                    .collect();
                Declared {
                    doc: doxygen::block("", &docs::callback(spelling, callback)),
                    text: format!(
                        "typedef void (*{})({});",
                        spelling.type_name(&callback.name),
                        params.join(", ")
                    ),
                }
            })
            .collect();
        let functions = abi
            .functions
            .iter()
            .map(|function| {
                let params: Vec<String> = function
                    .slots(api)
                    .iter()
                    .map(|slot| spelling.slot(slot))
                    .collect();
                let params = match params.is_empty() {
                    true => "void".to_owned(),
                    false => params.join(", "),
                };
                let attributes = match function.must_use(api) {
                    true => format!("{upper}_API {upper}_NODISCARD"),
                    false => format!("{upper}_API"),
                };
                let returns = spelling.returns(function.returns());
                let spacing = match returns.ends_with('*') {
                    true => "",
                    false => " ",
                };
                Declared {
                    doc: doxygen::block("", &docs::function(spelling, function)),
                    text: format!(
                        "{attributes} {returns}{spacing}{}({params});",
                        spelling.abi_function(function)
                    ),
                }
            })
            .collect();
        Self {
            library: api.name.clone(),
            guard: format!("{}_H", screaming(&api.name)),
            prefix,
            upper,
            doc: doxygen::block("", &body(&api.doc)),
            version: api.version,
            constants,
            structs,
            enums,
            flags,
            handles,
            callbacks,
            functions,
        }
    }
}

#[derive(Template)]
#[template(path = "c/library.pc.in.j2", escape = "none")]
struct PkgConfig<'a> {
    library: &'a str,
    version: Version,
}

fn maximum(primitive: Primitive) -> &'static str {
    match primitive {
        Primitive::U8 => "UINT8_MAX",
        Primitive::U16 => "UINT16_MAX",
        Primitive::U32 => "UINT32_MAX",
        Primitive::U64 => "UINT64_MAX",
        Primitive::I32 => "INT32_MAX",
        Primitive::I64 => "INT64_MAX",
        Primitive::Usize => "SIZE_MAX",
        Primitive::Bool | Primitive::F32 | Primitive::F64 => {
            unreachable!("validated: only integers have a maximum")
        }
    }
}

fn integer(primitive: Primitive, value: i128) -> String {
    match primitive {
        Primitive::U8 | Primitive::U16 | Primitive::U32 => format!("{value}U"),
        Primitive::U64 => format!("UINT64_C({value})"),
        Primitive::I32 if value == i128::from(i32::MIN) => format!("({} - 1)", value + 1),
        Primitive::I32 => format!("{value}"),
        Primitive::I64 if value == i128::from(i64::MIN) => {
            format!("(INT64_C({}) - 1)", value + 1)
        }
        Primitive::I64 => format!("INT64_C({value})"),
        Primitive::Usize => format!("((size_t){value})"),
        Primitive::Bool | Primitive::F32 | Primitive::F64 => {
            unreachable!("validated: integer constants have integer types")
        }
    }
}

fn float(primitive: Primitive, value: f64) -> String {
    match primitive {
        Primitive::F32 => format!("{value:?}f"),
        _ => format!("{value:?}"),
    }
}

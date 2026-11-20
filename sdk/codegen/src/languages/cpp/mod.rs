mod members;
mod spelling;

use std::collections::BTreeSet;

use askama::Template;

use crate::abi::{self, Abi, Lifecycle, Origin, USERDATA};
use crate::diagnostic::Diagnostic;
use crate::language::{Error, Language, Output};
use crate::languages::doxygen;
use crate::model::{
    Api, Callback, Declaration, DeclarationKind, Doc, Element, Id, Input, Lifetime, Member,
    Openness, Value,
};
use crate::names::pascal;

use spelling::{ELEMENT, ERROR, Spelling};

const BODY: &str = "body";
const NONE: &str = "none";
const THROWING_CALLABLES: &str =
    "Callables passed to the SDK must not throw: an exception leaving one calls std::terminate().";

pub(crate) struct Cpp;

impl Language for Cpp {
    fn name(&self) -> &'static str {
        "cpp"
    }

    fn generate(&self, api: &Api) -> Result<Vec<Output>, Error> {
        let abi = Abi::new(api)?;
        let spelling = Spelling::new(api);
        validate(&spelling)?;
        let header = Header::new(&spelling, &abi);
        Ok(vec![Output::render(
            format!("include/{0}/{0}.hpp", api.name),
            &header,
        )?])
    }
}

fn validate(spelling: &Spelling) -> Result<(), Diagnostic> {
    let api = spelling.api;
    let errors: Vec<_> = api
        .enums
        .iter()
        .filter(|item| item.error.is_some())
        .collect();
    if let [_, second, ..] = errors.as_slice() {
        return Err(Diagnostic::new(
            second.span,
            format!(
                "the C++ binding maps one `@error` enum to `{}::{ERROR}`; `{}` is a second one",
                api.name, second.name
            ),
        ));
    }

    let mut types = BTreeSet::from([ERROR.to_owned(), ELEMENT.to_owned()]);
    let names = api
        .structs
        .iter()
        .map(|item| (item.name.as_str(), item.span))
        .chain(api.enums.iter().map(|item| (item.name.as_str(), item.span)))
        .chain(api.flags.iter().map(|item| (item.name.as_str(), item.span)))
        .chain(
            api.classes
                .iter()
                .map(|item| (item.name.as_str(), item.span)),
        );
    for (name, span) in names {
        if !types.insert(pascal(name)) {
            return Err(Diagnostic::new(
                span,
                format!(
                    "`{name}` becomes the C++ type `{}`, which is taken",
                    pascal(name)
                ),
            ));
        }
    }

    let declaring: Vec<_> = api
        .classes
        .iter()
        .filter(|class| {
            class
                .members
                .iter()
                .any(|member| matches!(member, Member::Declaration(_)))
        })
        .collect();
    if let [_, second, ..] = declaring.as_slice() {
        return Err(Diagnostic::new(
            second.span,
            format!("the C++ binding has one `{ELEMENT}` for one declaring class"),
        ));
    }

    for class in Id::all(&api.classes) {
        let mut seen: BTreeSet<(String, String)> = builtin_members(api[class].lifetime)
            .iter()
            .map(|name| (spelling.class(class), (*name).to_owned()))
            .collect();
        for (index, member) in api[class].members.iter().enumerate() {
            let name = spelling.member(class, index);
            let owner = match member {
                Member::Declaration(Declaration {
                    kind: DeclarationKind::Modifier,
                    ..
                }) => ELEMENT.to_owned(),
                _ => spelling.class(class),
            };
            if !seen.insert((owner, name.clone())) {
                return Err(Diagnostic::new(
                    member.span(),
                    format!("this becomes the C++ member `{name}`, which another member also does"),
                ));
            }
            if let Member::Declaration(declaration) = member
                && declaration.kind == DeclarationKind::Container
                && let Some(param) = declaration.params.iter().find(|param| param.name == BODY)
            {
                return Err(Diagnostic::new(
                    param.span,
                    format!("a C++ container takes its children as `{BODY}`"),
                ));
            }
        }
    }

    for callback in &api.callbacks {
        let slots: BTreeSet<String> = abi::callback_slots(callback)
            .into_iter()
            .map(|slot| slot.name)
            .collect();
        for param in &callback.params {
            if let Some(local) = trampoline_local(&param.name, &param.ty)
                && slots.contains(&local)
            {
                return Err(Diagnostic::new(
                    param.span,
                    format!(
                        "the C++ trampoline of `{}` needs a local `{local}`",
                        callback.name
                    ),
                ));
            }
        }
    }

    for item in &api.flags {
        if let Some(flag) = item.flags.iter().find(|flag| flag.name == NONE) {
            return Err(Diagnostic::new(
                flag.span,
                format!("the C++ binding gives every flag set the value `{NONE}`"),
            ));
        }
    }
    Ok(())
}

fn trampoline_local(name: &str, input: &Input) -> Option<String> {
    match input {
        Input::Object { .. } => Some(format!("{name}_object")),
        Input::Array(Element::String) => Some(format!("{name}_views")),
        _ => None,
    }
}

struct Line {
    doc: String,
    text: String,
}

struct Record {
    doc: String,
    name: String,
    c_name: String,
    fields: Vec<Line>,
    to_c: String,
    from_c: String,
}

struct Enumeration {
    doc: String,
    name: String,
    lines: Vec<Line>,
}

struct ErrorType {
    status: String,
    c_status: String,
    success: String,
    message: String,
}

enum Kind {
    Owned {
        free: String,
    },
    Counted {
        reference: String,
        unreference: String,
    },
    Scoped,
}

fn builtin_members(lifetime: Lifetime) -> &'static [&'static str] {
    match lifetime {
        Lifetime::Owned | Lifetime::Shared | Lifetime::Local => {
            &["adopt", "raw", "release", "swap"]
        }
        Lifetime::Scoped => &["raw"],
    }
}

struct ClassItem {
    doc: String,
    name: String,
    raw: String,
    kind: Kind,
    declarations: Vec<String>,
    definitions: Vec<String>,
}

struct ElementItem {
    raw: String,
    declarations: Vec<String>,
    definitions: Vec<String>,
}

#[derive(Template)]
#[template(path = "cpp/header.hpp.j2", escape = "none")]
struct Header {
    library: String,
    guard: String,
    namespace: String,
    c_header: String,
    c_prefix: String,
    c_upper: String,
    doc: String,
    constants: Vec<Line>,
    structs: Vec<Record>,
    enums: Vec<Enumeration>,
    flags: Vec<Enumeration>,
    error: Option<ErrorType>,
    classes: Vec<ClassItem>,
    element: Option<ElementItem>,
    trampolines: Vec<String>,
    functions: Vec<String>,
}

impl Header {
    fn new(spelling: &Spelling, abi: &Abi) -> Self {
        let api = spelling.api;
        let c = &spelling.c;
        let body = |doc: &Doc| spelling.body(&doc.body);
        let mut doc = body(&api.doc);
        doc.push(THROWING_CALLABLES.to_owned());
        let constants = api
            .constants
            .iter()
            .map(|constant| Line {
                doc: doxygen::block("", &body(&constant.doc)),
                text: format!(
                    "inline constexpr {} {} = {};",
                    spelling.primitive(constant.ty),
                    spelling.ident(&constant.name),
                    c.macro_name(&constant.name)
                ),
            })
            .collect();
        let structs = api
            .structs
            .iter()
            .map(|item| {
                let fields: Vec<String> = item
                    .fields
                    .iter()
                    .map(|field| format!("value.{}", spelling.ident(&field.name)))
                    .collect();
                Record {
                    doc: doxygen::block("", &body(&item.doc)),
                    name: pascal(&item.name),
                    c_name: c.type_name(&item.name),
                    fields: item
                        .fields
                        .iter()
                        .map(|field| Line {
                            doc: doxygen::block("    ", &body(&field.doc)),
                            text: format!(
                                "{} {}{{}};",
                                spelling.primitive(field.ty),
                                spelling.ident(&field.name)
                            ),
                        })
                        .collect(),
                    to_c: fields.join(", "),
                    from_c: fields.join(", "),
                }
            })
            .collect();
        let enums = Id::all(&api.enums)
            .map(|id| {
                let item = &api[id];
                let mut paragraphs = body(&item.doc);
                if item.openness == Openness::Open {
                    paragraphs.push(format!(
                        "Open: any value converts, as in `{}{{0x61}}`.",
                        pascal(&item.name)
                    ));
                }
                Enumeration {
                    doc: doxygen::block("", &paragraphs),
                    name: pascal(&item.name),
                    lines: item
                        .variants
                        .iter()
                        .enumerate()
                        .map(|(index, variant)| Line {
                            doc: doxygen::block("    ", &body(&variant.doc)),
                            text: format!(
                                "{} = {},",
                                spelling.ident(&variant.name),
                                c.variant(id, index)
                            ),
                        })
                        .collect(),
                }
            })
            .collect();
        let flags = Id::all(&api.flags)
            .map(|id| {
                let item = &api[id];
                let none = Line {
                    doc: String::new(),
                    text: format!(
                        "{NONE} = {},",
                        c.macro_name(&format!("{}_{NONE}", item.value_prefix))
                    ),
                };
                Enumeration {
                    doc: doxygen::block("", &body(&item.doc)),
                    name: pascal(&item.name),
                    lines: std::iter::once(none)
                        .chain(item.flags.iter().enumerate().map(|(index, flag)| Line {
                            doc: doxygen::block("    ", &body(&flag.doc)),
                            text: format!(
                                "{} = {},",
                                spelling.ident(&flag.name),
                                c.flag(id, index)
                            ),
                        }))
                        .collect(),
                }
            })
            .collect();
        let error = Id::all(&api.enums).find_map(|id| {
            let codes = api[id].error?;
            Some(ErrorType {
                status: pascal(&api[id].name),
                c_status: c.type_name(&api[id].name),
                success: c.variant(id, codes.success),
                message: c.function(Origin::Function(codes.message)),
            })
        });
        let classes = Id::all(&api.classes)
            .map(|id| {
                let class = &api[id];
                let members = members::class(spelling, abi, id);
                let mut paragraphs = body(&class.doc);
                paragraphs.push(members::lifetime_note(class.lifetime).to_owned());
                let lifecycle = |lifecycle| c.function(Origin::Lifecycle(id, lifecycle));
                ClassItem {
                    doc: doxygen::block("", &paragraphs),
                    name: spelling.class(id),
                    raw: spelling.c_class(id),
                    kind: match class.lifetime {
                        Lifetime::Owned => Kind::Owned {
                            free: lifecycle(Lifecycle::Free),
                        },
                        Lifetime::Shared | Lifetime::Local => Kind::Counted {
                            reference: lifecycle(Lifecycle::Ref),
                            unreference: lifecycle(Lifecycle::Unref),
                        },
                        Lifetime::Scoped => Kind::Scoped,
                    },
                    declarations: members.declarations,
                    definitions: members.definitions,
                }
            })
            .collect();
        let element = Id::all(&api.classes)
            .find(|id| {
                api[*id]
                    .members
                    .iter()
                    .any(|member| matches!(member, Member::Declaration(_)))
            })
            .map(|id| {
                let members = members::element(spelling, abi);
                ElementItem {
                    raw: spelling.c_class(id),
                    declarations: members.declarations,
                    definitions: members.definitions,
                }
            });
        let trampolines = Id::all(&api.callbacks)
            .map(|callback| trampoline(spelling, callback))
            .collect();
        let functions = abi
            .functions
            .iter()
            .filter(|function| matches!(function.origin, Origin::Function(_)))
            .map(|function| members::free(spelling, function))
            .collect();
        Self {
            library: api.name.clone(),
            guard: format!("{}_HPP", api.name.to_ascii_uppercase()),
            namespace: spelling.namespace().to_owned(),
            c_header: format!("{0}/{0}.h", api.name),
            c_prefix: c.prefix().to_owned(),
            c_upper: c.upper(),
            doc: doxygen::block("", &doc),
            constants,
            structs,
            enums,
            flags,
            error,
            classes,
            element,
            trampolines,
            functions,
        }
    }
}

fn trampoline(spelling: &Spelling, id: Id<Callback>) -> String {
    let callback = &spelling.api[id];
    let params: Vec<String> = abi::callback_slots(callback)
        .iter()
        .map(|slot| spelling.c_slot(slot))
        .collect();
    let mut prelude = Vec::new();
    let mut arguments = Vec::new();
    for param in &callback.params {
        let name = spelling.ident(&param.name);
        let local = trampoline_local(&name, &param.ty);
        match &param.ty {
            Input::Object { class, .. } => {
                let local = local.expect("objects need a local");
                prelude.push(format!("{} {local}({name});", spelling.class(*class)));
                arguments.push(local);
            }
            Input::Value(Value::Primitive(_)) => arguments.push(name),
            Input::Value(Value::Struct(_)) => arguments.push(format!("detail::convert({name})")),
            Input::Value(value) => {
                arguments.push(format!("static_cast<{}>({name})", spelling.value(*value)));
            }
            Input::String => arguments.push(format!("detail::view({name})")),
            Input::Array(Element::String) => {
                let local = local.expect("string arrays need a local");
                let count = spelling.ident(&abi::count_name(&param.name));
                prelude.push(format!("std::vector<std::string_view> {local};"));
                prelude.push(format!("{local}.reserve({count});"));
                prelude.push(format!(
                    "for (std::size_t index_ = 0; index_ < {count}; ++index_) {{"
                ));
                prelude.push(format!(
                    "    {local}.push_back(detail::view({name}[index_]));"
                ));
                prelude.push("}".to_owned());
                arguments.push(format!("std::span<const std::string_view>({local})"));
            }
            Input::Array(Element::Primitive(primitive)) => {
                let count = spelling.ident(&abi::count_name(&param.name));
                arguments.push(format!(
                    "std::span<const {}>({name}, {count})",
                    spelling.primitive(*primitive)
                ));
            }
            Input::Closure(_) => unreachable!("validated: callbacks receive no closures"),
        }
    }
    let mut out = format!(
        "template <class Pack, std::size_t Index>\nvoid {}({}) noexcept {{\n",
        spelling.trampoline(id),
        params.join(", ")
    );
    for line in prelude {
        out.push_str(&format!("    {line}\n"));
    }
    out.push_str(&format!(
        "    std::get<Index>(*static_cast<Pack*>({USERDATA}))({});\n}}",
        arguments.join(", ")
    ));
    out
}

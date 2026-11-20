mod spelling;

use askama::Template;

use crate::abi::{self, Abi, Function, Lifecycle, Origin, Return};
use crate::diagnostic::{Diagnostic, Span};
use crate::language::{Error, Language, Output};
use crate::languages::projection::{self, Item, Linker, Literal, Trampoline};
use crate::languages::{c, unique, wrap};
use crate::model::{
    Api, Class, DeclarationKind, Doc, Element, Id, Input, Lifetime, Openness, Output as Result,
    Ownership, Primitive, Value,
};
use crate::names::spaced;

use spelling::{ELEMENT, ERROR, Spelling, exported, unexported};

const WIDTH: usize = 100;
const MODULE: &str = "github.com/project-rikka/rikka/sdk/bindings/go";
const GO_VERSION: &str = "1.24";
const BODY: &str = "body";

pub(crate) struct Go;

impl Language for Go {
    fn name(&self) -> &'static str {
        "go"
    }

    fn generate(&self, api: &Api) -> std::result::Result<Vec<Output>, Error> {
        let abi = Abi::new(api)?;
        let spelling = Spelling { api };
        validate(&spelling)?;
        let package = api.name.clone();
        let trampolines = projection::trampolines(api, &abi);
        Ok(vec![
            Output::render(
                "go.mod".to_owned(),
                &Module {
                    module: MODULE,
                    version: GO_VERSION,
                },
            )?,
            Output::render(
                "doc.go".to_owned(),
                &Package {
                    library: &api.name,
                    package: &package,
                    doc: comment("", &package_doc(&spelling)),
                },
            )?,
            Output::render(
                "runtime.go".to_owned(),
                &Runtime {
                    library: &api.name,
                    package: &package,
                    prefix: &api.abi_prefix,
                },
            )?,
            Output::render(
                "callbacks.go".to_owned(),
                &Callbacks::new(&spelling, &trampolines),
            )?,
            Output::render(
                format!("{}.go", api.name),
                &Api_::new(&spelling, &abi, &trampolines),
            )?,
        ])
    }
}

fn validate(spelling: &Spelling) -> std::result::Result<(), Diagnostic> {
    let api = spelling.api;
    projection::single_error(api, ERROR)?;
    let mut names: Vec<(String, Span)> = vec![
        (ELEMENT.to_owned(), Span::default()),
        (ERROR.to_owned(), Span::default()),
        ("BindingVersion".to_owned(), Span::default()),
    ];
    names.extend(
        api.constants
            .iter()
            .map(|item| (exported(&item.name), item.span)),
    );
    names.extend(
        api.structs
            .iter()
            .map(|item| (exported(&item.name), item.span)),
    );
    for item in &api.enums {
        names.push((exported(&item.name), item.span));
        names.extend(item.variants.iter().map(|variant| {
            (
                spelling.enum_value(&item.value_prefix, &variant.name),
                variant.span,
            )
        }));
    }
    for item in &api.flags {
        names.push((exported(&item.name), item.span));
        names.extend(item.flags.iter().map(|flag| {
            (
                spelling.enum_value(&item.value_prefix, &flag.name),
                flag.span,
            )
        }));
    }
    for id in Id::all(&api.classes) {
        names.push((spelling.class(id), api[id].span));
        for member in &api[id].members {
            if let crate::model::Member::Constructor(constructor) = member {
                names.push((
                    spelling.constructor(id, constructor.name.as_deref()),
                    constructor.span,
                ));
            }
        }
    }
    names.extend(
        api.functions
            .iter()
            .map(|item| (exported(&item.name), item.span)),
    );
    unique(names, |name| {
        format!("this becomes the Go name `{name}`, which is taken")
    })?;
    for class in Id::all(&api.classes) {
        let members = api[class]
            .members
            .iter()
            .enumerate()
            .filter(|(index, member)| {
                !projection::is_modifier(api, class, *index)
                    && !matches!(member, crate::model::Member::Constructor(_))
            })
            .map(|(index, member)| (spelling.member(class, index), member.span()));
        unique(members, |name| {
            format!("this becomes the Go method `{name}`, which another member also does")
        })?;
    }
    Ok(())
}

#[derive(Template)]
#[template(path = "go/go.mod.j2", escape = "none")]
struct Module<'a> {
    module: &'a str,
    version: &'a str,
}

#[derive(Template)]
#[template(path = "go/doc.go.j2", escape = "none")]
struct Package<'a> {
    library: &'a str,
    package: &'a str,
    doc: String,
}

#[derive(Template)]
#[template(path = "go/runtime.go.j2", escape = "none")]
struct Runtime<'a> {
    library: &'a str,
    package: &'a str,
    prefix: &'a str,
}

#[derive(Template)]
#[template(path = "go/callbacks.go.j2", escape = "none")]
struct Callbacks {
    library: String,
    package: String,
    trampolines: Vec<String>,
}

impl Callbacks {
    fn new(spelling: &Spelling, trampolines: &[Trampoline]) -> Self {
        let api = spelling.api;
        Self {
            library: api.name.clone(),
            package: api.name.clone(),
            trampolines: trampolines
                .iter()
                .map(|trampoline| render_trampoline(spelling, *trampoline))
                .collect(),
        }
    }
}

struct Struct {
    doc: String,
    name: String,
    c_name: String,
    fields: Vec<String>,
    to_c: String,
    from_c: String,
}

struct Enumeration {
    doc: String,
    name: String,
    values: Vec<String>,
    names: Vec<String>,
    flags: bool,
}

struct ErrorItem {
    status: String,
    success: String,
    texts: Vec<(String, String)>,
}

struct ClassItem {
    doc: String,
    name: String,
    raw: String,
    adopt: String,
    release: Option<String>,
    bound: bool,
    owned: bool,
    scoped: bool,
    declaring: bool,
    methods: Vec<String>,
}

#[derive(Template)]
#[template(path = "go/api.go.j2", escape = "none")]
struct Api_ {
    library: String,
    package: String,
    prefix: String,
    externs: Vec<String>,
    version: u32,
    constants: Vec<String>,
    structs: Vec<Struct>,
    enums: Vec<Enumeration>,
    error: Option<ErrorItem>,
    last_error: String,
    classes: Vec<ClassItem>,
    element: Option<Vec<String>>,
    end: String,
    imports: Vec<&'static str>,
    view: String,
    view_raw: String,
    functions: Vec<String>,
}

impl Api_ {
    fn new(spelling: &Spelling, abi: &Abi, trampolines: &[Trampoline]) -> Self {
        let api = spelling.api;
        let body = |doc: &Doc| spelling.body(&doc.body);
        let c = c::Spelling::new(api);
        let externs = trampolines
            .iter()
            .map(|trampoline| {
                let callback = &api[trampoline.callback];
                let slots: Vec<String> = abi::callback_slots(callback)
                    .iter()
                    .map(|slot| c.slot(slot))
                    .collect();
                format!(
                    "extern void {}({});",
                    spelling.trampoline(trampoline.callback, trampoline.position),
                    slots.join(", ")
                )
            })
            .chain([format!("extern void {}Destroy(void* userdata);", api.name)])
            .collect();

        let constants: Vec<String> = api
            .constants
            .iter()
            .map(|constant| {
                let name = exported(&constant.name);
                let value = match projection::literal(constant) {
                    Literal::Integer(value) => value.to_string(),
                    Literal::Float(value) => format!("{value:?}"),
                };
                format!(
                    "{}const {name} {} = {value}",
                    comment("", &described(&name, &body(&constant.doc), Subject::Value)),
                    spelling.primitive(constant.ty)
                )
            })
            .collect();

        let structs = api
            .structs
            .iter()
            .map(|item| {
                let name = exported(&item.name);
                Struct {
                    doc: comment("", &described(&name, &body(&item.doc), Subject::Type)),
                    name: name.clone(),
                    c_name: spelling.c_type(&item.name),
                    fields: aligned(
                        item.fields
                            .iter()
                            .map(|field| {
                                (
                                    comment("\t", &body(&field.doc)),
                                    vec![
                                        exported(&field.name),
                                        spelling.primitive(field.ty).to_owned(),
                                    ],
                                )
                            })
                            .collect(),
                    ),
                    to_c: item
                        .fields
                        .iter()
                        .map(|field| {
                            format!(
                                "{}: {}(value.{})",
                                field.name,
                                spelling.c_primitive(field.ty),
                                exported(&field.name)
                            )
                        })
                        .collect::<Vec<_>>()
                        .join(", "),
                    from_c: item
                        .fields
                        .iter()
                        .map(|field| {
                            format!(
                                "{}: {}(value.{})",
                                exported(&field.name),
                                spelling.primitive(field.ty),
                                field.name
                            )
                        })
                        .collect::<Vec<_>>()
                        .join(", "),
                }
            })
            .collect();

        let mut enums: Vec<Enumeration> = api
            .enums
            .iter()
            .map(|item| {
                let name = exported(&item.name);
                let open = item.openness == Openness::Open;
                let mut doc = described(&name, &body(&item.doc), Subject::Type);
                if open {
                    doc.push(format!("Open: any value converts, as in {name}('a')."));
                }
                let names: Vec<String> = item
                    .variants
                    .iter()
                    .map(|variant| spelling.enum_value(&item.value_prefix, &variant.name))
                    .collect();
                Enumeration {
                    doc: comment("", &doc),
                    name: name.clone(),
                    values: aligned(
                        item.variants
                            .iter()
                            .zip(&names)
                            .map(|(variant, constant)| {
                                (
                                    comment("\t", &body(&variant.doc)),
                                    vec![
                                        constant.clone(),
                                        name.clone(),
                                        format!("= {}", variant.value),
                                    ],
                                )
                            })
                            .collect(),
                    ),
                    names,
                    flags: false,
                }
            })
            .collect();
        enums.extend(api.flags.iter().map(|item| {
            let name = exported(&item.name);
            let names: Vec<String> = item
                .flags
                .iter()
                .map(|flag| spelling.enum_value(&item.value_prefix, &flag.name))
                .collect();
            Enumeration {
                doc: comment("", &described(&name, &body(&item.doc), Subject::Type)),
                name: name.clone(),
                values: aligned(
                    item.flags
                        .iter()
                        .zip(&names)
                        .map(|(flag, constant)| {
                            (
                                comment("\t", &body(&flag.doc)),
                                vec![
                                    constant.clone(),
                                    name.clone(),
                                    format!("= {:#x}", flag.value),
                                ],
                            )
                        })
                        .collect(),
                ),
                names,
                flags: true,
            }
        }));

        let error = Id::all(&api.enums).find_map(|id| {
            let item = &api[id];
            let codes = item.error?;
            Some(ErrorItem {
                status: exported(&item.name),
                success: spelling
                    .enum_value(&item.value_prefix, &item.variants[codes.success].name),
                texts: item
                    .variants
                    .iter()
                    .map(|variant| {
                        (
                            spelling.enum_value(&item.value_prefix, &variant.name),
                            spaced(&variant.name),
                        )
                    })
                    .collect(),
            })
        });
        let last_error = api
            .enums
            .iter()
            .find_map(|item| item.error)
            .map(|codes| format!("C.{}_{}", api.abi_prefix, api[codes.message].name))
            .unwrap_or_default();

        let declaring = projection::declaring(api);
        let mut element = None;
        let classes: Vec<ClassItem> = Id::all(&api.classes)
            .map(|id| {
                let class = &api[id];
                let name = spelling.class(id);
                let mut doc = described(&name, &body(&class.doc), Subject::Type);
                doc.push(lifetime_note(class.lifetime).to_owned());
                ClassItem {
                    doc: comment("", &doc),
                    name: name.clone(),
                    raw: spelling.c_type(&class.name),
                    adopt: format!("new{name}"),
                    release: projection::release(class).map(|lifecycle| {
                        format!(
                            "C.{}_{}",
                            api.abi_prefix,
                            abi::name(api, Origin::Lifecycle(id, lifecycle))
                        )
                    }),
                    bound: class.lifetime == Lifetime::Local,
                    owned: class.lifetime == Lifetime::Owned,
                    scoped: class.lifetime == Lifetime::Scoped,
                    declaring: declaring == Some(id),
                    methods: {
                        let items = projection::items(api, abi, id);
                        if declaring == Some(id) {
                            element = Some(modifiers(spelling, id, &items));
                        }
                        methods(spelling, id, &items)
                    },
                }
            })
            .collect();
        let end = declaring
            .map(|id| format!("C.{}_{}", api.abi_prefix, abi::name(api, Origin::End(id))))
            .unwrap_or_default();

        let functions: Vec<String> = abi
            .functions
            .iter()
            .filter_map(|function| match function.origin {
                Origin::Function(id) => {
                    let name = exported(&api[id].name);
                    let call = call(spelling, function, None);
                    let (returns, tail) = result(spelling, function, &call);
                    Some(render(&Method {
                        doc: docs(spelling, function, &name),
                        receiver: None,
                        name,
                        params: call.params.clone(),
                        returns,
                        body: call.body(tail),
                    }))
                }
                _ => None,
            })
            .collect();
        let generated = classes
            .iter()
            .flat_map(|class| &class.methods)
            .chain(element.iter().flatten())
            .chain(&functions)
            .chain(&constants)
            .flat_map(|code| code.lines())
            .filter(|line| !line.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        let imports = [
            ("fmt", !enums.is_empty() || error.is_some()),
            (
                "runtime",
                classes.iter().any(|class| !class.scoped) || generated.contains("runtime."),
            ),
            ("strings", enums.iter().any(|item| item.flags)),
            ("syscall", classes.iter().any(|class| class.bound)),
            ("unsafe", generated.contains("unsafe.")),
        ]
        .into_iter()
        .filter(|(_, used)| *used)
        .map(|(name, _)| name)
        .collect();
        Self {
            library: api.name.clone(),
            package: api.name.clone(),
            prefix: api.abi_prefix.clone(),
            externs,
            imports,
            version: api.version.packed(),
            constants,
            structs,
            enums,
            error,
            last_error,
            classes,
            view: declaring.map_or_else(String::new, |id| spelling.class(id)),
            view_raw: declaring.map_or_else(String::new, |id| spelling.c_type(&api[id].name)),
            element,
            end,
            functions,
        }
    }
}

fn lifetime_note(lifetime: Lifetime) -> &'static str {
    match lifetime {
        Lifetime::Owned => {
            "It owns its native object until it is handed over; the garbage collector frees an \
             object never handed over."
        }
        Lifetime::Shared => {
            "It holds one reference to a native object usable from any goroutine; the garbage \
             collector drops the reference."
        }
        Lifetime::Local => {
            "It holds one reference to a native object usable only on the thread that made it; \
             the garbage collector drops the reference there."
        }
        Lifetime::Scoped => "It is lent to a callback and valid only while the callback runs.",
    }
}

fn package_doc(spelling: &Spelling) -> Vec<String> {
    let api = spelling.api;
    let mut doc = described(
        &format!("Package {}", api.name),
        &spelling.body(&api.doc.body),
        Subject::Type,
    );
    doc.push(format!(
        "The package links lib{} through pkg-config and locks the main goroutine to the main \
         thread, so run the application from main. Callbacks run on that thread unless their \
         documentation says otherwise; a panic in a callback ends the program.",
        api.name
    ));
    doc
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Subject {
    Type,
    Function,
    Declaration,
    Value,
}

fn lowered(text: &str) -> String {
    let mut characters = text.chars();
    let Some(first) = characters.next() else {
        return String::new();
    };
    let second = characters.clone().next();
    match second.is_some_and(char::is_uppercase) {
        true => text.to_owned(),
        false => first.to_lowercase().to_string() + characters.as_str(),
    }
}

fn described(name: &str, paragraphs: &[String], subject: Subject) -> Vec<String> {
    let mut out: Vec<String> = paragraphs
        .iter()
        .filter(|text| !text.is_empty())
        .cloned()
        .collect();
    let Some(first) = out.first_mut() else {
        return out;
    };
    let article = ["A ", "An ", "The "]
        .iter()
        .any(|article| first.starts_with(article));
    *first = match (subject, article) {
        (Subject::Type, true) => format!("{name} is {}", lowered(first)),
        (Subject::Type, false) if !name.starts_with("Package ") => {
            format!("{name} describes {}", lowered(first))
        }
        (Subject::Declaration, true) => format!("{name} declares {}", lowered(first)),
        (Subject::Function, true) => format!("{name} returns {}", lowered(first)),
        (Subject::Function, false) if first.starts_with("Whether ") => {
            format!("{name} reports {}", lowered(first))
        }
        (Subject::Function | Subject::Declaration, false) => format!("{name} {}", lowered(first)),
        (_, _) => format!("{name}: {first}"),
    };
    out
}

fn aligned(entries: Vec<(String, Vec<String>)>) -> Vec<String> {
    let mut sections: Vec<Vec<usize>> = Vec::new();
    for (index, (doc, _)) in entries.iter().enumerate() {
        match (sections.last_mut(), doc.is_empty()) {
            (Some(section), true) => section.push(index),
            _ => sections.push(vec![index]),
        }
    }
    let mut widths = vec![Vec::new(); entries.len()];
    for section in &sections {
        let columns = entries[section[0]].1.len();
        let maximum: Vec<usize> = (0..columns)
            .map(|column| {
                section
                    .iter()
                    .map(|index| entries[*index].1[column].len())
                    .max()
                    .unwrap_or_default()
            })
            .collect();
        for index in section {
            widths[*index] = maximum.clone();
        }
    }
    entries
        .iter()
        .zip(widths)
        .map(|((doc, cells), widths)| {
            let last = cells.len() - 1;
            let line: Vec<String> = cells
                .iter()
                .enumerate()
                .map(|(column, cell)| match column == last {
                    true => cell.clone(),
                    false => format!("{cell:<width$}", width = widths[column]),
                })
                .collect();
            format!("{doc}\t{}", line.join(" "))
        })
        .collect()
}

fn comment(indent: &str, paragraphs: &[String]) -> String {
    let mut out = String::new();
    for (index, paragraph) in paragraphs
        .iter()
        .filter(|text| !text.is_empty())
        .enumerate()
    {
        if index > 0 {
            out.push_str(&format!("{indent}//\n"));
        }
        let (prefix, text) = match paragraph.strip_prefix("  - ") {
            Some(rest) => ("  - ", rest),
            None => ("", paragraph.as_str()),
        };
        for (line, part) in wrap(text, WIDTH - indent.len() - 7).iter().enumerate() {
            let lead = match (line, prefix.is_empty()) {
                (0, _) => prefix,
                (_, true) => "",
                (_, false) => "    ",
            };
            out.push_str(&format!("{indent}// {lead}{part}\n"));
        }
    }
    out
}

fn docs(spelling: &Spelling, function: &Function, name: &str) -> String {
    documented(
        spelling,
        function,
        name,
        projection::docs(spelling.api, function, spelling),
    )
}

fn documented(
    spelling: &Spelling,
    function: &Function,
    name: &str,
    docs: projection::Docs,
) -> String {
    let declares = matches!(
        function.origin,
        Origin::Member(class, index)
            if matches!(spelling.api.member(class, index), crate::model::Member::Declaration(_))
    );
    let subject = match declares {
        true => Subject::Declaration,
        false => Subject::Function,
    };
    let mut lines = described(name, &docs.body, subject);
    for (param, text) in docs.params.iter().filter(|(_, text)| !text.is_empty()) {
        lines.push(format!("  - {}: {text}", unexported(param)));
    }
    if !docs.returns.is_empty() {
        lines.push(format!("It returns {}", lowered(&docs.returns)));
    }
    if docs.throws {
        lines.push(format!(
            "It returns an [*{ERROR}] carrying the failing status."
        ));
    }
    let mut merged: Vec<String> = Vec::new();
    for line in lines {
        match (merged.last_mut(), line.starts_with("  - ")) {
            (Some(last), true) if last.starts_with("  - ") || last.contains("\n  - ") => {
                last.push('\n');
                last.push_str(&line);
            }
            _ => merged.push(line),
        }
    }
    let mut out = String::new();
    for (index, paragraph) in merged.iter().enumerate() {
        if index > 0 {
            out.push_str("//\n");
        }
        for item in paragraph.split('\n') {
            out.push_str(&comment("", &[item.to_owned()]));
        }
    }
    out
}

struct Method {
    doc: String,
    receiver: Option<String>,
    name: String,
    params: Vec<String>,
    returns: Option<String>,
    body: Vec<String>,
}

fn render(method: &Method) -> String {
    let receiver = method
        .receiver
        .as_ref()
        .map_or(String::new(), |receiver| format!("({receiver}) "));
    let returns = method
        .returns
        .as_ref()
        .map_or(String::new(), |returns| format!(" {returns}"));
    let mut out = format!(
        "{}func {receiver}{}({}){returns} {{\n",
        method.doc,
        method.name,
        method.params.join(", ")
    );
    for line in &method.body {
        out.push('\t');
        out.push_str(line);
        out.push('\n');
    }
    out.push('}');
    out
}

#[derive(Default)]
struct Call {
    params: Vec<String>,
    checks: Vec<String>,
    late: Vec<String>,
    arguments: Vec<String>,
    kept: Vec<String>,
}

impl Call {
    fn body(&self, tail: Vec<String>) -> Vec<String> {
        self.checks
            .iter()
            .chain(&self.late)
            .cloned()
            .chain(tail)
            .collect()
    }
}

fn call(spelling: &Spelling, function: &Function, receiver: Option<Receiver>) -> Call {
    let api = spelling.api;
    let mut call = Call::default();
    if let Some(receiver) = receiver {
        match receiver.ownership {
            Ownership::Owned => {
                call.late.push(format!("raw := {}.take()", receiver.name));
                call.arguments.push("raw".to_owned());
            }
            Ownership::Borrowed => {
                call.arguments
                    .push(format!("{}.{}", receiver.name, receiver.access));
                if receiver.kept {
                    call.kept.push(receiver.name.clone());
                }
            }
        }
    }
    for param in &function.signature.params {
        let name = unexported(&param.name);
        match &param.ty {
            Input::Value(Value::Primitive(primitive)) => {
                call.params
                    .push(format!("{name} {}", spelling.primitive(*primitive)));
                call.arguments
                    .push(format!("{}({name})", spelling.c_primitive(*primitive)));
            }
            Input::Value(value @ Value::Struct(_)) => {
                call.params
                    .push(format!("{name} {}", spelling.value(*value)));
                call.arguments.push(format!("{name}.native()"));
            }
            Input::Value(value) => {
                call.params
                    .push(format!("{name} {}", spelling.input(&param.ty)));
                call.arguments
                    .push(format!("{}({name})", spelling.c_value(*value)));
            }
            Input::String => {
                call.params.push(format!("{name} string"));
                call.arguments.push(format!("cStr({name})"));
            }
            Input::Object {
                class,
                ownership: Ownership::Borrowed,
            } => {
                call.params
                    .push(format!("{name} *{}", spelling.class(*class)));
                call.arguments.push(format!("{name}.handle()"));
                call.kept.push(name);
            }
            Input::Object {
                class,
                ownership: Ownership::Owned,
            } => {
                call.params
                    .push(format!("{name} *{}", spelling.class(*class)));
                let local = format!("{name}Raw");
                call.late.push(format!("{local} := {name}.take()"));
                call.arguments.push(local);
            }
            Input::Array(Element::Primitive(primitive)) => {
                call.params
                    .push(format!("{name} []{}", go_element(spelling, *primitive)));
                call.arguments.push(format!(
                    "(*{})(unsafe.Pointer(unsafe.SliceData({name})))",
                    spelling.c_primitive(*primitive)
                ));
                call.arguments.push(format!("C.size_t(len({name}))"));
            }
            Input::Array(Element::String) => {
                unreachable!("validated: only callbacks receive string arrays")
            }
            Input::Closure(closure) => {
                let names: Vec<String> = closure
                    .functions
                    .iter()
                    .map(|part| unexported(&part.name))
                    .collect();
                for (part, part_name) in closure.functions.iter().zip(&names) {
                    call.params
                        .push(format!("{part_name} {}", spelling.signature(part.callback)));
                    call.checks.push(format!("if {part_name} == nil {{"));
                    call.checks
                        .push(format!("\tpanic(\"{}: {part_name} is nil\")", api.name));
                    call.checks.push("}".to_owned());
                }
                call.late
                    .push(format!("userdata := retain({})", names.join(", ")));
                for (position, part) in closure.functions.iter().enumerate() {
                    call.arguments.push(format!(
                        "C.{}_{}(C.{})",
                        api.abi_prefix,
                        api[part.callback].name,
                        spelling.trampoline(part.callback, position)
                    ));
                }
                call.arguments.push("userdata".to_owned());
                call.arguments.push(format!(
                    "C.{}_destroy_fn(C.{}Destroy)",
                    api.abi_prefix, api.name
                ));
            }
        }
    }
    call
}

fn go_element(spelling: &Spelling, primitive: Primitive) -> &'static str {
    match primitive {
        Primitive::U8 => "byte",
        _ => spelling.primitive(primitive),
    }
}

struct Receiver {
    name: String,
    access: &'static str,
    ownership: Ownership,
    kept: bool,
}

fn invoke(spelling: &Spelling, function: &Function, call: &Call) -> String {
    format!(
        "C.{}_{}({})",
        spelling.api.abi_prefix,
        function.name,
        call.arguments.join(", ")
    )
}

fn result(spelling: &Spelling, function: &Function, call: &Call) -> (Option<String>, Vec<String>) {
    let api = spelling.api;
    let expression = invoke(spelling, function, call);
    let keep: Vec<String> = call
        .kept
        .iter()
        .map(|name| format!("runtime.KeepAlive({name})"))
        .collect();
    let finish = |ty: Option<String>, converted: Option<String>| {
        let mut lines = Vec::new();
        match converted {
            None => {
                lines.push(expression.clone());
                lines.extend(keep.iter().cloned());
            }
            Some(converted) if keep.is_empty() => lines.push(format!("return {converted}")),
            Some(converted) => {
                lines.push(format!("result := {converted}"));
                lines.extend(keep.iter().cloned());
                lines.push("return result".to_owned());
            }
        }
        (ty, lines)
    };
    if function.signature.throws.is_some() {
        return finish(
            Some("error".to_owned()),
            Some(format!("checkStatus({expression})")),
        );
    }
    match (function.returns(), function.signature.returns) {
        (Return::Void, _) => finish(None, None),
        (Return::Value(Value::Primitive(primitive)), _) => finish(
            Some(spelling.primitive(primitive).to_owned()),
            Some(format!("{}({expression})", spelling.primitive(primitive))),
        ),
        (Return::Value(value @ Value::Struct(id)), _) => finish(
            Some(spelling.value(value)),
            Some(format!("go{}({expression})", exported(&api[id].name))),
        ),
        (Return::Value(value @ Value::Enum(id)), _) if spelling.is_error(value) => finish(
            Some("error".to_owned()),
            Some(format!(
                "statusError({}({expression}))",
                exported(&api[id].name)
            )),
        ),
        (Return::Value(value), _) => {
            let ty = spelling.value(value);
            finish(Some(ty.clone()), Some(format!("{ty}({expression})")))
        }
        (Return::Str, _) => finish(
            Some("string".to_owned()),
            Some(format!("goString({expression})")),
        ),
        (Return::String, _) => finish(
            Some("string".to_owned()),
            Some(format!("takeString({expression})")),
        ),
        (
            Return::Object(class),
            Some(Result::Object {
                ownership: Ownership::Borrowed,
                ..
            }),
        ) => finish(
            Some(format!("*{}", spelling.class(class))),
            Some(format!(
                "new{}(C.{}_{}({expression}))",
                spelling.class(class),
                api.abi_prefix,
                abi::name(api, Origin::Lifecycle(class, Lifecycle::Ref))
            )),
        ),
        (Return::Object(class), _) => finish(
            Some(format!("*{}", spelling.class(class))),
            Some(format!("new{}({expression})", spelling.class(class))),
        ),
    }
}

fn receiver_name(spelling: &Spelling, class: Id<Class>) -> String {
    spelling.api[class]
        .name
        .chars()
        .next()
        .map_or_else(|| "x".to_owned(), |first| first.to_string())
}

fn methods(spelling: &Spelling, class: Id<Class>, items: &[Item]) -> Vec<String> {
    let api = spelling.api;
    let owner = &api[class];
    let name = spelling.class(class);
    let me = receiver_name(spelling, class);
    let receiver = format!("{me} *{name}");
    let counted = owner.lifetime != Lifetime::Scoped;
    let mut out = Vec::new();
    for (index, item) in items.iter().enumerate() {
        let member = spelling.member(class, index);
        match item {
            Item::Constructor { function, .. } => {
                let call = call(spelling, function, None);
                let (returns, tail) = result(spelling, function, &call);
                out.push(render(&Method {
                    doc: docs(spelling, function, &member),
                    receiver: None,
                    name: member,
                    params: call.params.clone(),
                    returns,
                    body: call.body(tail),
                }));
            }
            Item::Method {
                function,
                method,
                chained,
            } => {
                let call = call(
                    spelling,
                    function,
                    Some(Receiver {
                        name: me.clone(),
                        access: "handle()",
                        ownership: method.receiver,
                        kept: counted,
                    }),
                );
                let (mut returns, mut tail) = result(spelling, function, &call);
                if *chained {
                    tail.push(format!("return {me}"));
                    returns = Some(format!("*{name}"));
                }
                out.push(render(&Method {
                    doc: docs(spelling, function, &member),
                    receiver: Some(receiver.clone()),
                    name: member,
                    params: call.params.clone(),
                    returns,
                    body: call.body(tail),
                }));
            }
            Item::Property { getter, setter, .. } => {
                let read = call(
                    spelling,
                    getter,
                    Some(Receiver {
                        name: me.clone(),
                        access: "handle()",
                        ownership: Ownership::Borrowed,
                        kept: counted,
                    }),
                );
                let (returns, tail) = result(spelling, getter, &read);
                out.push(render(&Method {
                    doc: docs(spelling, getter, &member),
                    receiver: Some(receiver.clone()),
                    name: member,
                    params: Vec::new(),
                    returns,
                    body: read.body(tail),
                }));
                if let Some(setter) = setter {
                    let write = call(
                        spelling,
                        setter,
                        Some(Receiver {
                            name: me.clone(),
                            access: "handle()",
                            ownership: Ownership::Borrowed,
                            kept: counted,
                        }),
                    );
                    let (returns, tail) = result(spelling, setter, &write);
                    let setter_name = spelling.setter(class, index);
                    out.push(render(&Method {
                        doc: docs(spelling, setter, &setter_name),
                        receiver: Some(receiver.clone()),
                        name: setter_name,
                        params: write.params.clone(),
                        returns,
                        body: write.body(tail),
                    }));
                }
            }
            Item::Declaration {
                function,
                declaration,
            } => {
                let mut call = call(
                    spelling,
                    function,
                    Some(Receiver {
                        name: me.clone(),
                        access: "handle()",
                        ownership: Ownership::Borrowed,
                        kept: false,
                    }),
                );
                let expression = invoke(spelling, function, &call);
                let keep = call
                    .kept
                    .iter()
                    .map(|name| format!("runtime.KeepAlive({name})"));
                match declaration.kind {
                    DeclarationKind::Container => {
                        call.params.push(format!("{BODY} func()"));
                        call.checks.push(format!("if {BODY} == nil {{"));
                        call.checks
                            .push(format!("\tpanic(\"{}: {BODY} is nil\")", api.name));
                        call.checks.push("}".to_owned());
                        let mut tail = vec![expression];
                        tail.extend(keep);
                        tail.push(format!("return {me}.contain({BODY})"));
                        out.push(render(&Method {
                            doc: documented(
                                spelling,
                                function,
                                &member,
                                projection::docs(api, function, spelling).with_paragraph(format!(
                                    "The {BODY} function declares its children."
                                )),
                            ),
                            receiver: Some(receiver.clone()),
                            name: member,
                            params: call.params.clone(),
                            returns: Some(ELEMENT.to_owned()),
                            body: call.body(tail),
                        }));
                    }
                    DeclarationKind::Element => {
                        let mut tail = vec![expression];
                        tail.extend(keep);
                        tail.push(format!("return {me}.declared()"));
                        out.push(render(&Method {
                            doc: docs(spelling, function, &member),
                            receiver: Some(receiver.clone()),
                            name: member,
                            params: call.params.clone(),
                            returns: Some(ELEMENT.to_owned()),
                            body: call.body(tail),
                        }));
                    }
                    DeclarationKind::Modifier => {}
                }
            }
        }
    }
    out
}

fn modifiers(spelling: &Spelling, class: Id<Class>, items: &[Item]) -> Vec<String> {
    let mut out = Vec::new();
    for (index, function) in projection::modifiers(items) {
        let member = spelling.member(class, index);
        let call = call(
            spelling,
            function,
            Some(Receiver {
                name: "e".to_owned(),
                access: "current()",
                ownership: Ownership::Borrowed,
                kept: false,
            }),
        );
        let (_, mut tail) = result(spelling, function, &call);
        tail.push("return e".to_owned());
        out.push(render(&Method {
            doc: docs(spelling, function, &member),
            receiver: Some(format!("e {ELEMENT}")),
            name: member,
            params: call.params.clone(),
            returns: Some(ELEMENT.to_owned()),
            body: call.body(tail),
        }));
    }
    out
}

fn render_trampoline(spelling: &Spelling, trampoline: Trampoline) -> String {
    let api = spelling.api;
    let callback = &api[trampoline.callback];
    let slots: Vec<String> = abi::callback_slots(callback)
        .iter()
        .map(|slot| spelling.go_slot(slot))
        .collect();
    let mut lines = vec!["drainReleases()".to_owned()];
    let mut arguments = Vec::new();
    for param in &callback.params {
        let name = unexported(&param.name);
        let count = || unexported(&abi::count_name(&param.name));
        match &param.ty {
            Input::Object { class, .. } => {
                let local = format!("{name}Lent");
                lines.push(format!(
                    "{local} := &{}{{raw: {name}}}",
                    spelling.class(*class)
                ));
                lines.push(format!("defer {local}.expire()"));
                arguments.push(local);
            }
            Input::Value(Value::Primitive(primitive)) => {
                arguments.push(format!("{}({name})", spelling.primitive(*primitive)));
            }
            Input::Value(Value::Struct(id)) => {
                arguments.push(format!("go{}({name})", exported(&api[*id].name)));
            }
            Input::Value(value @ Value::Enum(id)) if spelling.is_error(*value) => {
                arguments.push(format!("statusError({}({name}))", exported(&api[*id].name)));
            }
            Input::Value(value) => arguments.push(format!("{}({name})", spelling.value(*value))),
            Input::String => arguments.push(format!("goString({name})")),
            Input::Array(Element::String) => {
                arguments.push(format!("goStrings({name}, {})", count()));
            }
            Input::Array(Element::Primitive(primitive)) => arguments.push(format!(
                "goValues[{}](unsafe.Pointer({name}), {})",
                go_element(spelling, *primitive),
                count()
            )),
            Input::Closure(_) => unreachable!("validated: callbacks receive no closures"),
        }
    }
    lines.push(format!(
        "retained[{}](userdata, {})({})",
        spelling.bare_signature(trampoline.callback),
        trampoline.position,
        arguments.join(", ")
    ));
    let name = spelling.trampoline(trampoline.callback, trampoline.position);
    let mut out = format!("//export {name}\nfunc {name}({}) {{\n", slots.join(", "));
    for line in lines {
        out.push('\t');
        out.push_str(&line);
        out.push('\n');
    }
    out.push('}');
    out
}

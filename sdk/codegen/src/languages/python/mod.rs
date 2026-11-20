mod spelling;

use askama::Template;

use crate::abi::{self, Abi, Function, Lifecycle, Origin, Return};
use crate::diagnostic::Diagnostic;
use crate::language::{Error, Language, Output};
use crate::languages::projection::{self, Item, Linker, Literal, Naming, Trampoline, Vocabulary};
use crate::languages::wrap;
use crate::model::{
    Api, Callback, Class, DeclarationKind, Doc, Element, Flags, Id, Input, Lifetime, Openness,
    Output as Result, Ownership, Primitive, Value,
};
use crate::names::{pascal, screaming};

use spelling::{CONTAINER, ELEMENT, ERROR, Spelling};

const WIDTH: usize = 99;
const INDENT: &str = "    ";
const OPTIONAL: &str = " | None = None";
const SELF_TYPE: &str = "_ElementT";
const TAKEN: &str = "self_raw";

pub(crate) struct Python;

impl Language for Python {
    fn name(&self) -> &'static str {
        "python"
    }

    fn generate(&self, api: &Api) -> std::result::Result<Vec<Output>, Error> {
        let abi = Abi::new(api)?;
        let spelling = Spelling { api };
        validate(&spelling)?;
        let package = format!("src/{}", api.name);
        Ok(vec![
            Output::render(
                "pyproject.toml".to_owned(),
                &Project {
                    library: &api.name,
                    version: api.version.to_string(),
                    summary: projection::summary(api, &spelling).replace('"', "'"),
                },
            )?,
            Output::render(format!("{package}/py.typed"), &Typed)?,
            Output::render(
                format!("{package}/_runtime.py"),
                &Runtime {
                    library: &api.name,
                    soname: projection::soname(api),
                },
            )?,
            Output::render(
                format!("{package}/_native.py"),
                &Native::new(&spelling, &abi),
            )?,
            Output::render(
                format!("{package}/__init__.py"),
                &Module::new(&spelling, &abi),
            )?,
        ])
    }
}

fn validate(spelling: &Spelling) -> std::result::Result<(), Diagnostic> {
    projection::validate(
        spelling.api,
        Vocabulary {
            language: "Python",
            type_noun: "name",
            member_noun: "member",
            error: ERROR.to_owned(),
            reserved_types: [ELEMENT, CONTAINER, ERROR].map(str::to_owned).to_vec(),
            reserved_members: &[],
        },
        spelling,
    )
}

impl Naming for Spelling<'_> {
    fn flags(&self, flags: Id<Flags>) -> String {
        pascal(&self.api[flags].name)
    }

    fn callback(&self, callback: Id<Callback>) -> String {
        Spelling::callback(self, callback)
    }

    fn members(
        &self,
        class: Id<Class>,
        index: usize,
    ) -> std::result::Result<Vec<String>, Diagnostic> {
        Ok(vec![self.member(class, index)])
    }
}

#[derive(Template)]
#[template(path = "python/pyproject.toml.j2", escape = "none")]
struct Project<'a> {
    library: &'a str,
    version: String,
    summary: String,
}

#[derive(Template)]
#[template(path = "python/py.typed.j2", escape = "none")]
struct Typed;

#[derive(Template)]
#[template(path = "python/_runtime.py.j2", escape = "none")]
struct Runtime<'a> {
    library: &'a str,
    soname: String,
}

struct Prototype {
    name: String,
    returns: String,
    arguments: String,
}

struct Record {
    name: String,
    fields: Vec<(String, String)>,
}

#[derive(Template)]
#[template(path = "python/_native.py.j2", escape = "none")]
struct Native {
    library: String,
    prefix: String,
    structs: Vec<Record>,
    callbacks: Vec<Prototype>,
    functions: Vec<Prototype>,
}

impl Native {
    fn new(spelling: &Spelling, abi: &Abi) -> Self {
        let api = spelling.api;
        let structs = api
            .structs
            .iter()
            .map(|item| Record {
                name: pascal(&item.name),
                fields: item
                    .fields
                    .iter()
                    .map(|field| (field.name.clone(), spelling.ctype(field.ty).to_owned()))
                    .collect(),
            })
            .collect();
        let callbacks = Id::all(&api.callbacks)
            .map(|id| Prototype {
                name: spelling.callback_type(id),
                returns: "None".to_owned(),
                arguments: abi::callback_slots(&api[id])
                    .iter()
                    .map(|slot| spelling.c_slot(slot, true))
                    .collect::<Vec<_>>()
                    .join(", "),
            })
            .collect();
        let functions = abi
            .functions
            .iter()
            .map(|function| Prototype {
                name: function.name.clone(),
                returns: match function.returns() {
                    Return::Void => "None".to_owned(),
                    Return::Value(value) => spelling.c_value(value),
                    Return::Str => "_runtime.StrView".to_owned(),
                    Return::String => "_runtime.String".to_owned(),
                    Return::Object(_) => "ctypes.c_void_p".to_owned(),
                },
                arguments: function
                    .slots(api)
                    .iter()
                    .map(|slot| spelling.c_slot(slot, false))
                    .collect::<Vec<_>>()
                    .join(", "),
            })
            .collect();
        Self {
            library: api.name.clone(),
            prefix: api.abi_prefix.clone(),
            structs,
            callbacks,
            functions,
        }
    }
}

struct Line {
    doc: String,
    text: String,
}

struct Struct {
    doc: String,
    name: String,
    snake: String,
    fields: Vec<String>,
    values: String,
}

struct Enumeration {
    doc: String,
    name: String,
    base: &'static str,
    members: Vec<String>,
    open: bool,
}

struct ClassItem {
    doc: String,
    name: String,
    base: &'static str,
    release: Option<String>,
    bound: bool,
    end: Option<String>,
    members: Vec<String>,
}

struct ErrorItem {
    status: String,
    success: String,
}

#[derive(Template)]
#[template(path = "python/__init__.py.j2", escape = "none")]
struct Module {
    library: String,
    doc: String,
    version: String,
    exports: Vec<String>,
    constants: Vec<Line>,
    structs: Vec<Struct>,
    enums: Vec<Enumeration>,
    error: Option<ErrorItem>,
    last_error: String,
    aliases: Vec<Line>,
    variables: Vec<String>,
    classes: Vec<ClassItem>,
    view: String,
    element: Option<Vec<String>>,
    functions: Vec<String>,
    trampolines: Vec<String>,
}

impl Module {
    fn new(spelling: &Spelling, abi: &Abi) -> Self {
        let api = spelling.api;
        let body = |doc: &Doc| spelling.body(&doc.body);
        let mut exports: Vec<String> = Vec::new();

        let constants = api
            .constants
            .iter()
            .map(|constant| {
                let name = screaming(&constant.name);
                exports.push(name.clone());
                let value = match projection::literal(constant) {
                    Literal::Integer(value) => value.to_string(),
                    Literal::Float(value) => format!("{value:?}"),
                };
                Line {
                    doc: comments("", &body(&constant.doc)),
                    text: format!(
                        "{name}: Final[{}] = {value}",
                        spelling.primitive(constant.ty)
                    ),
                }
            })
            .collect();

        let structs = api
            .structs
            .iter()
            .map(|item| {
                let name = pascal(&item.name);
                exports.push(name.clone());
                Struct {
                    doc: docstring(INDENT, &body(&item.doc)),
                    name: name.clone(),
                    snake: item.name.clone(),
                    fields: item
                        .fields
                        .iter()
                        .map(|field| {
                            format!(
                                "{}{INDENT}{}: {}",
                                comments(INDENT, &body(&field.doc)),
                                spelling.ident(&field.name),
                                spelling.primitive(field.ty)
                            )
                        })
                        .collect(),
                    values: item
                        .fields
                        .iter()
                        .map(|field| format!("value.{}", spelling.ident(&field.name)))
                        .collect::<Vec<_>>()
                        .join(", "),
                }
            })
            .collect();

        let mut enums: Vec<Enumeration> = api
            .enums
            .iter()
            .map(|item| {
                let name = pascal(&item.name);
                exports.push(name.clone());
                let open = item.openness == Openness::Open;
                let mut described = body(&item.doc);
                if open {
                    described.push(format!(
                        "Open: any value converts, as in ``{name}(0x61)``, and so does a \
                         one-character string, as in ``{name}(\"a\")``."
                    ));
                }
                Enumeration {
                    doc: docstring(INDENT, &described),
                    name,
                    base: "enum.IntEnum",
                    members: item
                        .variants
                        .iter()
                        .map(|variant| {
                            format!(
                                "{}{INDENT}{} = {}",
                                comments(INDENT, &body(&variant.doc)),
                                screaming(&variant.name),
                                variant.value
                            )
                        })
                        .collect(),
                    open,
                }
            })
            .collect();
        enums.extend(api.flags.iter().map(|item| {
            let name = pascal(&item.name);
            exports.push(name.clone());
            Enumeration {
                doc: docstring(INDENT, &body(&item.doc)),
                name,
                base: "enum.IntFlag",
                members: item
                    .flags
                    .iter()
                    .map(|flag| {
                        format!(
                            "{}{INDENT}{} = {:#x}",
                            comments(INDENT, &body(&flag.doc)),
                            screaming(&flag.name),
                            flag.value
                        )
                    })
                    .collect(),
                open: false,
            }
        }));

        let error = Id::all(&api.enums).find_map(|id| {
            let codes = api[id].error?;
            exports.push(ERROR.to_owned());
            Some(ErrorItem {
                status: pascal(&api[id].name),
                success: screaming(&api[id].variants[codes.success].name),
            })
        });
        let last_error = api
            .enums
            .iter()
            .find_map(|item| item.error)
            .map(|codes| spelling.native(&api[codes.message].name))
            .unwrap_or_default();

        let mut variables = Vec::new();
        let aliases = Id::all(&api.callbacks)
            .map(|id| {
                let callback = &api[id];
                let name = spelling.callback(id);
                exports.push(name.clone());
                variables.push(format!("_{name}T = TypeVar(\"_{name}T\", bound={name})"));
                let arguments: Vec<String> = callback
                    .params
                    .iter()
                    .map(|param| alias_argument(spelling, &param.ty))
                    .collect();
                let docs = projection::callback_docs(callback, spelling);
                let mut lines = docs.body;
                lines.extend(
                    docs.params
                        .into_iter()
                        .filter(|(_, text)| !text.is_empty())
                        .map(|(param, text)| format!("``{}``: {text}", spelling.ident(&param))),
                );
                Line {
                    doc: comments("", &lines),
                    text: format!(
                        "{name}: TypeAlias = Callable[[{}], object]",
                        arguments.join(", ")
                    ),
                }
            })
            .collect();

        let declaring = projection::declaring(api);
        let mut element = None;
        let classes = Id::all(&api.classes)
            .map(|id| {
                let class = &api[id];
                let name = spelling.class(id);
                exports.push(name.clone());
                let items = projection::items(api, abi, id);
                if declaring == Some(id) {
                    element = Some(modifiers(spelling, id, &items));
                }
                let mut described = body(&class.doc);
                described.push(lifetime_note(class.lifetime).to_owned());
                ClassItem {
                    doc: docstring(INDENT, &described),
                    name,
                    base: match class.lifetime {
                        Lifetime::Owned => "_runtime.Handle",
                        Lifetime::Shared | Lifetime::Local => "_runtime.Counted",
                        Lifetime::Scoped if declaring == Some(id) => "_runtime.Declaring",
                        Lifetime::Scoped => "_runtime.Lent",
                    },
                    release: projection::release(class).map(|lifecycle| {
                        spelling.native(&abi::name(api, Origin::Lifecycle(id, lifecycle)))
                    }),
                    bound: class.lifetime == Lifetime::Local,
                    end: class
                        .has_containers()
                        .then(|| spelling.native(&abi::name(api, Origin::End(id)))),
                    members: members(spelling, id, &items),
                }
            })
            .collect();
        if declaring.is_some() {
            exports.push(ELEMENT.to_owned());
            exports.push(CONTAINER.to_owned());
        }

        let functions = abi
            .functions
            .iter()
            .filter_map(|function| match function.origin {
                Origin::Function(id) => {
                    let name = spelling.ident(&api[id].name);
                    exports.push(name.clone());
                    let call = call(spelling, function, None);
                    let (returns, tail) =
                        result(spelling, function, invoke(spelling, function, &call));
                    Some(render(
                        &Method {
                            decorators: Vec::new(),
                            name,
                            params: call.params.clone(),
                            returns,
                            doc: described(spelling, function),
                            body: call.body(tail),
                        },
                        "",
                    ))
                }
                _ => None,
            })
            .collect();

        let trampolines = projection::trampolines(api, abi)
            .into_iter()
            .map(|trampoline| render_trampoline(spelling, trampoline))
            .collect();
        exports.sort();
        Self {
            library: api.name.clone(),
            doc: docstring("", &body(&api.doc)),
            version: api.version.to_string(),
            exports,
            constants,
            structs,
            enums,
            error,
            last_error,
            aliases,
            variables,
            classes,
            view: declaring.map_or_else(String::new, |id| spelling.class(id)),
            element,
            functions,
            trampolines,
        }
    }
}

fn alias_argument(spelling: &Spelling, input: &Input) -> String {
    match input {
        Input::Object { .. } => format!("\"{}\"", spelling.received(input)),
        _ => spelling.received(input),
    }
}

fn lifetime_note(lifetime: Lifetime) -> &'static str {
    match lifetime {
        Lifetime::Owned => "It owns its native object until it is consumed or released.",
        Lifetime::Shared => "It holds one reference to a native object usable from any thread.",
        Lifetime::Local => {
            "It holds one reference to a native object usable only on the thread that made it."
        }
        Lifetime::Scoped => "It is lent to a callback and valid only while the callback runs.",
    }
}

struct Method {
    decorators: Vec<String>,
    name: String,
    params: Vec<String>,
    returns: String,
    doc: Vec<String>,
    body: Vec<String>,
}

fn render(method: &Method, indent: &str) -> String {
    let mut out = String::new();
    for decorator in &method.decorators {
        out.push_str(&format!("{indent}@{decorator}\n"));
    }
    let head = format!("{indent}def {}(", method.name);
    let tail = format!(") -> {}:", method.returns);
    let single = format!("{head}{}{tail}", method.params.join(", "));
    if single.len() <= WIDTH {
        out.push_str(&single);
    } else {
        out.push_str(&head);
        out.push('\n');
        for param in &method.params {
            out.push_str(&format!("{indent}{INDENT}{param},\n"));
        }
        out.push_str(&format!("{indent}{tail}"));
    }
    out.push('\n');
    let inner = format!("{indent}{INDENT}");
    if !method.doc.is_empty() {
        out.push_str(&method_docstring(&inner, &method.doc));
    }
    if method.body.is_empty() {
        out.push_str(&format!("{inner}...\n"));
    }
    for line in &method.body {
        if line.is_empty() {
            out.push('\n');
            continue;
        }
        for part in wrap_call(line, inner.len()) {
            out.push_str(&format!("{inner}{part}\n"));
        }
    }
    out.trim_end().to_owned()
}

fn wrap_call(line: &str, indent: usize) -> Vec<String> {
    let leading = line.len() - line.trim_start().len();
    let start = line
        .find("_native.")
        .and_then(|at| line[at..].find('(').map(|open| at + open));
    let Some(open) = start.filter(|_| indent + line.len() > WIDTH) else {
        return vec![line.to_owned()];
    };
    let mut depth = 0usize;
    let mut close = None;
    let mut cuts = Vec::new();
    for (offset, character) in line[open..].char_indices() {
        let at = open + offset;
        match character {
            '(' | '[' => depth += 1,
            ')' | ']' => {
                depth -= 1;
                if depth == 0 {
                    close = Some(at);
                    break;
                }
            }
            ',' if depth == 1 => cuts.push(at),
            _ => {}
        }
    }
    let Some(close) = close else {
        return vec![line.to_owned()];
    };
    let pad = " ".repeat(leading);
    let mut parts = vec![line[..=open].to_owned()];
    let mut from = open + 1;
    for cut in cuts.into_iter().chain([close]) {
        let argument = line[from..cut].trim();
        if !argument.is_empty() {
            parts.push(format!("{pad}{INDENT}{argument},"));
        }
        from = cut + 1;
    }
    parts.push(format!("{pad}{}", &line[close..]));
    parts
}

fn docstring(indent: &str, paragraphs: &[String]) -> String {
    let mut lines = Vec::new();
    for paragraph in paragraphs.iter().filter(|text| !text.is_empty()) {
        if !lines.is_empty() {
            lines.push(String::new());
        }
        lines.push(paragraph.clone());
    }
    match lines.is_empty() {
        true => String::new(),
        false => method_docstring(indent, &lines),
    }
}

fn method_docstring(indent: &str, lines: &[String]) -> String {
    let mut rendered: Vec<String> = Vec::new();
    for line in lines {
        if line.is_empty() {
            rendered.push(String::new());
            continue;
        }
        let (prefix, text) = match line.strip_prefix("  ") {
            Some(rest) => (INDENT, rest),
            None => ("", line.as_str()),
        };
        let opening = if rendered.is_empty() { 3 } else { 0 };
        let width = WIDTH - indent.len() - prefix.len() - opening;
        for (index, part) in wrap(&escape(text), width).iter().enumerate() {
            let hanging = match (index, prefix.is_empty()) {
                (0, _) | (_, true) => "",
                _ => INDENT,
            };
            rendered.push(format!("{prefix}{hanging}{part}"));
        }
    }
    let mut out = format!("{indent}\"\"\"{}", rendered[0]);
    if rendered.len() == 1 {
        out.push_str("\"\"\"\n");
        return out;
    }
    for line in &rendered[1..] {
        out.push('\n');
        if !line.is_empty() {
            out.push_str(&format!("{indent}{line}"));
        }
    }
    out.push_str(&format!("\n{indent}\"\"\"\n"));
    out
}

fn escape(text: &str) -> String {
    text.replace('\\', "\\\\").replace("\"\"\"", "\\\"\\\"\\\"")
}

fn comments(indent: &str, paragraphs: &[String]) -> String {
    let mut out = String::new();
    for (index, paragraph) in paragraphs
        .iter()
        .filter(|text| !text.is_empty())
        .enumerate()
    {
        if index > 0 {
            out.push_str(&format!("{indent}#:\n"));
        }
        for line in wrap(paragraph, WIDTH - indent.len() - 3) {
            out.push_str(&format!("{indent}#: {line}\n"));
        }
    }
    out
}

fn docs(spelling: &Spelling, docs: projection::Docs) -> Vec<String> {
    let mut lines = Vec::new();
    for paragraph in &docs.body {
        if !lines.is_empty() {
            lines.push(String::new());
        }
        lines.push(paragraph.clone());
    }
    let mut section = |title: &str, items: Vec<String>| {
        if items.is_empty() {
            return;
        }
        if !lines.is_empty() {
            lines.push(String::new());
        }
        lines.push(format!("{title}:"));
        lines.extend(items.into_iter().map(|item| format!("  {item}")));
    };
    section(
        "Args",
        docs.params
            .iter()
            .filter(|(_, text)| !text.is_empty())
            .map(|(name, text)| format!("{}: {text}", spelling.ident(name)))
            .collect(),
    );
    if !docs.returns.is_empty() {
        section("Returns", vec![docs.returns.clone()]);
    }
    if docs.throws {
        section("Raises", vec![format!("{ERROR}: With the failing status.")]);
    }
    lines
}

fn described(spelling: &Spelling, function: &Function) -> Vec<String> {
    docs(spelling, projection::docs(spelling.api, function, spelling))
}

#[derive(Default)]
struct Call {
    params: Vec<String>,
    early: Vec<String>,
    late: Vec<String>,
    arguments: Vec<String>,
    hoisted: Vec<Hoisted>,
    callback: Option<CallbackParam>,
    forwarded: Vec<String>,
}

struct Hoisted {
    argument: usize,
    local: String,
    checked: String,
}

struct CallbackParam {
    name: String,
    alias: String,
    optional: bool,
}

impl Call {
    fn convert(&mut self, name: &str, inline: String, checked: String) {
        self.hoisted.push(Hoisted {
            argument: self.arguments.len(),
            local: format!("{name}_arg"),
            checked,
        });
        self.arguments.push(inline);
    }

    fn hoists(&self) -> bool {
        !self.late.is_empty()
    }

    fn passed(&self) -> Vec<String> {
        let mut arguments = self.arguments.clone();
        if self.hoists() {
            for hoisted in &self.hoisted {
                arguments[hoisted.argument] = hoisted.local.clone();
            }
        }
        arguments
    }

    fn body(&self, tail: Vec<String>) -> Vec<String> {
        let mut lines = self.early.clone();
        if self.hoists() {
            lines.extend(
                self.hoisted
                    .iter()
                    .map(|hoisted| format!("{} = {}", hoisted.local, hoisted.checked)),
            );
        }
        lines.extend(self.late.iter().cloned());
        lines.extend(tail);
        lines
    }
}

fn call(spelling: &Spelling, function: &Function, receiver: Option<String>) -> Call {
    let api = spelling.api;
    let mut call = Call::default();
    match receiver {
        Some(receiver) if receiver == TAKEN => call.arguments.push(receiver),
        Some(receiver) => call.convert("self", receiver.clone(), receiver),
        None => {}
    }
    let params = &function.signature.params;
    for (position, param) in params.iter().enumerate() {
        let name = spelling.ident(&param.name);
        if !matches!(param.ty, Input::Closure(_)) {
            call.forwarded.push(name.clone());
        }
        match &param.ty {
            Input::Value(Value::Primitive(Primitive::Bool)) => {
                call.params.push(format!("{name}: bool"));
                let converted = format!("bool({name})");
                call.convert(&name, converted.clone(), converted);
            }
            Input::Value(Value::Primitive(primitive @ (Primitive::F32 | Primitive::F64))) => {
                call.params
                    .push(format!("{name}: {}", spelling.primitive(*primitive)));
                call.convert(&name, name.clone(), format!("float({name})"));
            }
            Input::Value(Value::Primitive(primitive)) => {
                call.params
                    .push(format!("{name}: {}", spelling.primitive(*primitive)));
                call.convert(&name, name.clone(), format!("operator.index({name})"));
            }
            Input::Value(value @ Value::Struct(id)) => {
                call.params
                    .push(format!("{name}: {}", spelling.value(*value)));
                let converted = format!("_to_{}({name})", api[*id].name);
                call.convert(&name, converted.clone(), converted);
            }
            Input::Value(value) => {
                call.params
                    .push(format!("{name}: {}", spelling.accepted(*value)));
                let converted = format!("{}({name})", spelling.value(*value));
                call.convert(&name, converted.clone(), converted);
            }
            Input::String => {
                call.params.push(format!("{name}: str"));
                let converted = format!("_runtime.text({name})");
                call.convert(&name, converted.clone(), converted);
            }
            Input::Object {
                class,
                ownership: Ownership::Borrowed,
            } => {
                let ty = spelling.class(*class);
                call.params.push(format!("{name}: {ty}"));
                let converted = format!("_runtime.handle_of({name}, {ty})");
                call.convert(&name, converted.clone(), converted);
            }
            Input::Object {
                class,
                ownership: Ownership::Owned,
            } => {
                let ty = spelling.class(*class);
                call.params.push(format!("{name}: {ty}"));
                call.early.push(format!("_runtime.expect({name}, {ty})"));
                call.late.push(format!("{name}_raw = {name}._take()"));
                call.arguments.push(format!("{name}_raw"));
            }
            Input::Array(element) => {
                call.params
                    .push(format!("{name}: {}", spelling.input(&param.ty)));
                let local = format!("{name}_items");
                match element {
                    Element::Primitive(Primitive::U8) => {
                        call.early
                            .push(format!("{local} = _runtime.bytes_of({name})"));
                    }
                    Element::Primitive(primitive) => {
                        call.early.push(format!(
                            "{local} = ({} * len({name}))(*{name})",
                            spelling.ctype(*primitive)
                        ));
                    }
                    Element::String => {
                        unreachable!("validated: only callbacks receive string arrays")
                    }
                }
                call.arguments.push(local.clone());
                call.arguments.push(format!("len({local})"));
            }
            Input::Closure(closure) => {
                let single = closure.functions.len() == 1;
                let optional = single && position > 0 && position + 1 == params.len();
                let names: Vec<String> = closure
                    .functions
                    .iter()
                    .map(|part| spelling.ident(&part.name))
                    .collect();
                for (part, part_name) in closure.functions.iter().zip(&names) {
                    let alias = spelling.callback(part.callback);
                    call.params.push(match optional {
                        true => format!("{part_name}: {alias}{OPTIONAL}"),
                        false => format!("{part_name}: {alias}"),
                    });
                    call.early
                        .push(format!("_runtime.expect_callable({part_name})"));
                }
                if single {
                    call.callback = Some(CallbackParam {
                        name: names[0].clone(),
                        alias: spelling.callback(closure.functions[0].callback),
                        optional,
                    });
                }
                call.late.push(format!(
                    "{} = _runtime.register({})",
                    abi::USERDATA,
                    names.join(", ")
                ));
                for (position, part) in closure.functions.iter().enumerate() {
                    call.arguments
                        .push(spelling.trampoline(part.callback, position));
                }
                call.arguments.push(abi::USERDATA.to_owned());
                call.arguments.push("_destroy".to_owned());
            }
        }
    }
    call
}

fn invoke(spelling: &Spelling, function: &Function, call: &Call) -> String {
    format!(
        "{}({})",
        spelling.native(&function.name),
        call.passed().join(", ")
    )
}

fn result(spelling: &Spelling, function: &Function, expression: String) -> (String, Vec<String>) {
    let api = spelling.api;
    if function.signature.throws.is_some() {
        return ("None".to_owned(), vec![format!("_check({expression})")]);
    }
    match (function.returns(), function.signature.returns) {
        (Return::Void, _) => ("None".to_owned(), vec![expression]),
        (Return::Value(Value::Primitive(primitive)), _) => (
            spelling.primitive(primitive).to_owned(),
            vec![format!("return {expression}")],
        ),
        (Return::Value(value @ Value::Struct(id)), _) => (
            spelling.value(value),
            vec![format!("return _from_{}({expression})", api[id].name)],
        ),
        (Return::Value(value), _) => {
            let ty = spelling.value(value);
            (ty.clone(), vec![format!("return {ty}({expression})")])
        }
        (Return::Str, _) => (
            "str".to_owned(),
            vec![format!("return _runtime.read({expression})")],
        ),
        (Return::String, _) => (
            "str".to_owned(),
            vec![format!("return _native.take_string({expression})")],
        ),
        (
            Return::Object(class),
            Some(Result::Object {
                ownership: Ownership::Borrowed,
                ..
            }),
        ) => {
            let ty = spelling.class(class);
            let reference =
                spelling.native(&abi::name(api, Origin::Lifecycle(class, Lifecycle::Ref)));
            (
                ty.clone(),
                vec![format!("return {ty}._adopt({reference}({expression}))")],
            )
        }
        (Return::Object(class), _) => {
            let ty = spelling.class(class);
            (
                ty.clone(),
                vec![format!("return {ty}._adopt({expression})")],
            )
        }
    }
}

fn decorated(method: Method, call: &Call) -> Vec<Method> {
    let Some(callback) = call.callback.as_ref().filter(|callback| callback.optional) else {
        return vec![method];
    };
    let ty = format!("_{}T", callback.alias);
    let decorator = format!("Callable[[{ty}], {ty}]");
    let receiver = method.params[0].clone();
    let required: Vec<String> = std::iter::once(receiver.clone())
        .chain(
            call.params
                .iter()
                .map(|param| param.trim_end_matches(OPTIONAL).to_owned()),
        )
        .collect();
    let leading: Vec<String> = std::iter::once(receiver)
        .chain(call.params[..call.params.len() - 1].iter().cloned())
        .collect();
    let typed = Method {
        decorators: vec!["overload".to_owned()],
        name: method.name.clone(),
        params: required,
        returns: method.returns.clone(),
        doc: Vec::new(),
        body: Vec::new(),
    };
    let decorating = Method {
        decorators: vec!["overload".to_owned()],
        name: method.name.clone(),
        params: leading,
        returns: decorator.clone(),
        doc: Vec::new(),
        body: Vec::new(),
    };
    let forwarded = call
        .forwarded
        .iter()
        .cloned()
        .chain(["function".to_owned()])
        .collect::<Vec<_>>()
        .join(", ");
    let mut body = vec![
        format!("if {} is None:", callback.name),
        String::new(),
        format!("{INDENT}def decorate(function: {ty}) -> {ty}:"),
        format!("{INDENT}{INDENT}self.{}({forwarded})", method.name),
        format!("{INDENT}{INDENT}return function"),
        String::new(),
        format!("{INDENT}return decorate"),
    ];
    body.extend(method.body);
    let general = Method {
        returns: format!("{} | {decorator}", method.returns),
        body,
        ..method
    };
    vec![typed, decorating, general]
}

fn with_self(params: &[String], receiver: &str) -> Vec<String> {
    std::iter::once(receiver.to_owned())
        .chain(params.iter().cloned())
        .collect()
}

fn members(spelling: &Spelling, class: Id<Class>, items: &[Item]) -> Vec<String> {
    let api = spelling.api;
    let setters: Vec<(&crate::model::Method, &Function)> = items
        .iter()
        .filter_map(|item| match item {
            Item::Method {
                method,
                function,
                chained: true,
            } if method.name.starts_with(projection::SETTER) => Some((*method, *function)),
            _ => None,
        })
        .collect();
    let mut out = Vec::new();
    for (index, item) in items.iter().enumerate() {
        let name = spelling.member(class, index);
        let methods = match item {
            Item::Constructor {
                function,
                constructor,
            } => vec![constructor_method(
                spelling,
                class,
                function,
                constructor.name.is_none(),
                &setters,
                name,
            )],
            Item::Method {
                function, method, ..
            } => {
                let receiver = match method.receiver {
                    Ownership::Owned => TAKEN,
                    Ownership::Borrowed => "self._handle",
                };
                let mut call = call(spelling, function, Some(receiver.to_owned()));
                if method.receiver == Ownership::Owned {
                    call.late.insert(0, format!("{TAKEN} = self._take()"));
                }
                let (mut returns, mut tail) =
                    result(spelling, function, invoke(spelling, function, &call));
                if method.receiver == Ownership::Owned && function.signature.throws.is_some() {
                    tail = std::iter::once("with _runtime.interruptible():".to_owned())
                        .chain(tail.into_iter().map(|line| format!("{INDENT}{line}")))
                        .collect();
                }
                if let Some(callback) = &call.callback
                    && function.returns() == Return::Void
                {
                    tail.push(format!("return {}", callback.name));
                    returns = callback.alias.clone();
                }
                decorated(
                    Method {
                        decorators: Vec::new(),
                        name,
                        params: with_self(&call.params, "self"),
                        returns,
                        doc: described(spelling, function),
                        body: call.body(tail),
                    },
                    &call,
                )
            }
            Item::Property { getter, setter, .. } => {
                let read = call(spelling, getter, Some("self._handle".to_owned()));
                let (returns, tail) = result(spelling, getter, invoke(spelling, getter, &read));
                let mut methods = vec![Method {
                    decorators: vec!["property".to_owned()],
                    name: name.clone(),
                    params: vec!["self".to_owned()],
                    returns,
                    doc: described(spelling, getter),
                    body: read.body(tail),
                }];
                if let Some(setter) = setter {
                    let write = call(spelling, setter, Some("self._handle".to_owned()));
                    methods.push(Method {
                        decorators: vec![format!("{name}.setter")],
                        name,
                        params: with_self(&write.params, "self"),
                        returns: "None".to_owned(),
                        doc: Vec::new(),
                        body: write.body(vec![invoke(spelling, setter, &write)]),
                    });
                }
                methods
            }
            Item::Declaration {
                function,
                declaration,
            } => {
                let call = call(spelling, function, Some("self._handle".to_owned()));
                let expression = invoke(spelling, function, &call);
                match declaration.kind {
                    DeclarationKind::Container => vec![Method {
                        decorators: Vec::new(),
                        name,
                        params: with_self(&call.params, "self"),
                        returns: CONTAINER.to_owned(),
                        doc: docs(
                            spelling,
                            projection::docs(api, function, spelling)
                                .with_paragraph("Declare its children in a ``with`` block."),
                        ),
                        body: call.body(vec![expression, format!("return {CONTAINER}(self)")]),
                    }],
                    DeclarationKind::Element => decorated(
                        Method {
                            decorators: Vec::new(),
                            name,
                            params: with_self(&call.params, "self"),
                            returns: ELEMENT.to_owned(),
                            doc: described(spelling, function),
                            body: call.body(vec![expression, format!("return {ELEMENT}(self)")]),
                        },
                        &call,
                    ),
                    DeclarationKind::Modifier => Vec::new(),
                }
            }
        };
        out.extend(methods.iter().map(|method| render(method, INDENT)));
    }
    out
}

fn constructor_method(
    spelling: &Spelling,
    class: Id<Class>,
    function: &Function,
    default: bool,
    setters: &[(&crate::model::Method, &Function)],
    name: String,
) -> Method {
    let api = spelling.api;
    let call = call(spelling, function, None);
    let expression = invoke(spelling, function, &call);
    if !default {
        return Method {
            decorators: vec!["classmethod".to_owned()],
            name,
            params: with_self(&call.params, "cls"),
            returns: spelling.class(class),
            doc: described(spelling, function),
            body: call.body(vec![format!("return cls._adopt({expression})")]),
        };
    }
    let mut params = with_self(&call.params, "self");
    let mut documented = projection::docs(api, function, spelling);
    let mut tail = vec![format!("self._attach({expression})")];
    if !setters.is_empty() {
        params.push("*".to_owned());
    }
    for (method, setter) in setters {
        let stripped = method.name.trim_start_matches(projection::SETTER);
        let key = spelling.ident(stripped);
        let types: Vec<String> = method
            .signature
            .params
            .iter()
            .map(|param| spelling.input(&param.ty))
            .collect();
        let (ty, spread) = match types.as_slice() {
            [single] => (single.clone(), ""),
            _ => (format!("tuple[{}]", types.join(", ")), "*"),
        };
        params.push(format!("{key}: {ty}{OPTIONAL}"));
        documented =
            documented.with_param(stripped, spelling.body(setter.description(api)).join(" "));
        tail.push(format!("if {key} is not None:"));
        tail.push(format!(
            "{INDENT}self.{}({spread}{key})",
            spelling.ident(&method.name)
        ));
    }
    Method {
        decorators: Vec::new(),
        name,
        params,
        returns: "None".to_owned(),
        doc: docs(spelling, documented),
        body: call.body(tail),
    }
}

fn modifiers(spelling: &Spelling, class: Id<Class>, items: &[Item]) -> Vec<String> {
    let mut out = Vec::new();
    for (index, function) in projection::modifiers(items) {
        let call = call(
            spelling,
            function,
            Some("self._view._current(self._serial)".to_owned()),
        );
        let methods = decorated(
            Method {
                decorators: Vec::new(),
                name: spelling.member(class, index),
                params: with_self(&call.params, &format!("self: {SELF_TYPE}")),
                returns: SELF_TYPE.to_owned(),
                doc: described(spelling, function),
                body: call.body(vec![
                    invoke(spelling, function, &call),
                    "return self".to_owned(),
                ]),
            },
            &call,
        );
        out.extend(methods.iter().map(|method| render(method, INDENT)));
    }
    out
}

fn render_trampoline(spelling: &Spelling, trampoline: Trampoline) -> String {
    let api = spelling.api;
    let callback = &api[trampoline.callback];
    let slots: Vec<String> = abi::callback_slots(callback)
        .iter()
        .map(|slot| spelling.ident(&slot.name))
        .collect();
    let mut prelude = Vec::new();
    let mut arguments = Vec::new();
    let mut lent = Vec::new();
    for param in &callback.params {
        let name = spelling.ident(&param.name);
        let count = || spelling.ident(&abi::count_name(&param.name));
        match &param.ty {
            Input::Object { class, .. } => {
                let local = format!("{name}_lent");
                prelude.push(format!(
                    "{local} = {}._lend({name})",
                    spelling.class(*class)
                ));
                arguments.push(local.clone());
                lent.push(local);
            }
            Input::Value(Value::Primitive(_)) => arguments.push(name),
            Input::Value(Value::Struct(id)) => {
                arguments.push(format!("_from_{}({name})", api[*id].name));
            }
            Input::Value(value) => arguments.push(format!("{}({name})", spelling.value(*value))),
            Input::String => arguments.push(format!("_runtime.read({name})")),
            Input::Array(Element::String) => {
                arguments.push(format!("_runtime.read_all({name}, {})", count()));
            }
            Input::Array(Element::Primitive(_)) => {
                arguments.push(format!("{name}[: {}]", count()));
            }
            Input::Closure(_) => unreachable!("validated: callbacks receive no closures"),
        }
    }
    let mut lines = vec!["_runtime.drain()".to_owned()];
    lines.extend(prelude);
    lines.push("try:".to_owned());
    lines.push(format!(
        "{INDENT}_runtime.closure({}, {})({})",
        abi::USERDATA,
        trampoline.position,
        arguments.join(", ")
    ));
    lines.push("except BaseException:".to_owned());
    lines.push(format!("{INDENT}_runtime.report()"));
    if !lent.is_empty() {
        lines.push("finally:".to_owned());
        lines.extend(
            lent.iter()
                .map(|local| format!("{INDENT}{local}._expire()")),
        );
    }
    let method = Method {
        decorators: vec![format!(
            "_native.{}",
            spelling.callback_type(trampoline.callback)
        )],
        name: spelling.trampoline(trampoline.callback, trampoline.position),
        params: slots,
        returns: "None".to_owned(),
        doc: Vec::new(),
        body: lines,
    };
    render(&method, "")
}

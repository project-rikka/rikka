mod spelling;

use askama::Template;

use crate::abi::{self, Abi, Function, Lifecycle, Origin, Return};
use crate::diagnostic::Diagnostic;
use crate::language::{Error, Language, Output};
use crate::languages::projection::{self, Item, Linker, Literal, Naming, Trampoline, Vocabulary};
use crate::languages::wrap;
use crate::model::{
    Api, Callback, Class, DeclarationKind, Doc, Element, Flags, Id, Input, Lifetime, Member,
    Openness, Output as Result, Ownership, Primitive, Value,
};
use crate::names::spaced;

use spelling::{ELEMENT, STATICS, Spelling, bare, camel, escape, pascal};

const WIDTH: usize = 110;
const INDENT: &str = "    ";
const TARGET: &str = "net8.0";
const LANGUAGE: &str = "12";
const BODY: &str = "body";
const LENT: &str = "Raw";
const OBJECT_MEMBERS: &[&str] = &["Equals", "GetHashCode", "GetType", "ToString", "Dispose"];

pub(crate) struct CSharp;

impl Language for CSharp {
    fn name(&self) -> &'static str {
        "csharp"
    }

    fn generate(&self, api: &Api) -> std::result::Result<Vec<Output>, Error> {
        let abi = Abi::new(api)?;
        let spelling = Spelling { api };
        validate(&spelling)?;
        let namespace = pascal(&api.name);
        Ok(vec![
            Output::render(
                format!("{namespace}.csproj"),
                &Project {
                    namespace: namespace.clone(),
                    version: api.version.to_string(),
                    summary: escape(&projection::summary(api, &spelling)),
                    target: TARGET,
                    language: LANGUAGE,
                },
            )?,
            Output::render(
                "src/Runtime.cs".to_owned(),
                &Runtime {
                    library: &api.name,
                    namespace: &namespace,
                    prefix: &api.abi_prefix,
                },
            )?,
            Output::render("src/Native.cs".to_owned(), &Native::new(&spelling, &abi))?,
            Output::render(format!("src/{namespace}.cs"), &Public::new(&spelling, &abi))?,
        ])
    }
}

fn validate(spelling: &Spelling) -> std::result::Result<(), Diagnostic> {
    projection::validate(
        spelling.api,
        Vocabulary {
            language: "C#",
            type_noun: "type",
            member_noun: "member",
            error: spelling.exception(),
            reserved_types: vec![ELEMENT.to_owned(), STATICS.to_owned(), spelling.exception()],
            reserved_members: OBJECT_MEMBERS,
        },
        spelling,
    )
}

impl Naming for Spelling<'_> {
    fn flags(&self, flags: Id<Flags>) -> String {
        pascal(&self.api[flags].name)
    }

    fn callback(&self, callback: Id<Callback>) -> String {
        self.delegate(callback)
    }

    fn members(
        &self,
        class: Id<Class>,
        index: usize,
    ) -> std::result::Result<Vec<String>, Diagnostic> {
        let member = self.api.member(class, index);
        if matches!(member, Member::Constructor(constructor) if constructor.name.is_none()) {
            return Ok(Vec::new());
        }
        let name = self.member(class, index);
        if name == self.class(class) {
            return Err(Diagnostic::new(
                member.span(),
                format!("this becomes the C# member `{name}`, which names its own class"),
            ));
        }
        Ok(vec![name])
    }
}

#[derive(Template)]
#[template(path = "csharp/project.csproj.j2", escape = "none")]
struct Project {
    namespace: String,
    version: String,
    summary: String,
    target: &'static str,
    language: &'static str,
}

#[derive(Template)]
#[template(path = "csharp/Runtime.cs.j2", escape = "none")]
struct Runtime<'a> {
    library: &'a str,
    namespace: &'a str,
    prefix: &'a str,
}

struct Record {
    name: String,
    fields: Vec<(String, String)>,
    to_native: String,
    to_public: String,
}

#[derive(Template)]
#[template(path = "csharp/Native.cs.j2", escape = "none")]
struct Native {
    library: String,
    namespace: String,
    soname: String,
    dll: String,
    prefix: String,
    structs: Vec<Record>,
    functions: Vec<String>,
    error: Option<ErrorItem>,
    trampolines: Vec<String>,
}

struct ErrorItem {
    status: String,
    success: String,
    exception: String,
    message: String,
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
                    .map(|field| {
                        (
                            spelling.native_primitive(field.ty).to_owned(),
                            pascal(&field.name),
                        )
                    })
                    .collect(),
                to_native: item
                    .fields
                    .iter()
                    .map(|field| {
                        let name = pascal(&field.name);
                        match field.ty {
                            Primitive::Bool => {
                                format!("{name} = value.{name} ? (byte)1 : (byte)0")
                            }
                            Primitive::Usize => format!("{name} = (nint)value.{name}"),
                            _ => format!("{name} = value.{name}"),
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(", "),
                to_public: item
                    .fields
                    .iter()
                    .map(|field| {
                        let name = pascal(&field.name);
                        match field.ty {
                            Primitive::Bool => format!("{name} != 0"),
                            Primitive::Usize => format!("checked((int){name})"),
                            _ => name,
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(", "),
            })
            .collect();
        let functions = abi
            .functions
            .iter()
            .map(|function| {
                let returns = match function.returns() {
                    Return::Void => "void".to_owned(),
                    Return::Value(value) => spelling.native_value(value),
                    Return::Str => "Str".to_owned(),
                    Return::String => "OwnedString".to_owned(),
                    Return::Object(_) => "nint".to_owned(),
                };
                let params: Vec<String> = function
                    .slots(api)
                    .iter()
                    .map(|slot| format!("{} {}", spelling.native_slot(slot), camel(&slot.name)))
                    .collect();
                format!(
                    "{INDENT}[LibraryImport(Library, EntryPoint = \"{0}_{1}\")]\n{INDENT}internal static partial {returns} {0}_{1}({2});",
                    api.abi_prefix,
                    function.name,
                    params.join(", ")
                )
            })
            .collect();
        let error = Id::all(&api.enums).find_map(|id| {
            let item = &api[id];
            let codes = item.error?;
            Some(ErrorItem {
                status: pascal(&item.name),
                success: pascal(&item.variants[codes.success].name),
                exception: spelling.exception(),
                message: format!("{}_{}", api.abi_prefix, api[codes.message].name),
            })
        });
        Self {
            library: api.name.clone(),
            namespace: pascal(&api.name),
            soname: projection::soname(api),
            dll: projection::dll(api),
            prefix: api.abi_prefix.clone(),
            structs,
            functions,
            error,
            trampolines: projection::trampolines(api, abi)
                .into_iter()
                .map(|trampoline| render_trampoline(spelling, trampoline))
                .collect(),
        }
    }
}

struct Enumeration {
    doc: String,
    name: String,
    flags: bool,
    members: Vec<String>,
}

struct Delegate {
    doc: String,
    text: String,
}

struct ClassItem {
    doc: String,
    name: String,
    kind: Kind,
    members: Vec<String>,
}

enum Kind {
    Handle {
        release: String,
        bound: bool,
        counted: bool,
    },
    Lent {
        declaring: bool,
        end: Option<String>,
    },
}

struct ExceptionItem {
    name: String,
    status: String,
}

#[derive(Template)]
#[template(path = "csharp/Api.cs.j2", escape = "none")]
struct Public {
    library: String,
    namespace: String,
    doc: String,
    version: u32,
    constants: Vec<String>,
    structs: Vec<String>,
    enums: Vec<Enumeration>,
    exception: Option<ExceptionItem>,
    delegates: Vec<Delegate>,
    classes: Vec<ClassItem>,
    view: String,
    element: Option<Vec<String>>,
    functions: Vec<String>,
}

impl Public {
    fn new(spelling: &Spelling, abi: &Abi) -> Self {
        let api = spelling.api;
        let body = |doc: &Doc| spelling.body(&doc.body);
        let constants = api
            .constants
            .iter()
            .map(|constant| {
                let value = match (projection::literal(constant), constant.ty) {
                    (Literal::Integer(value), _) => value.to_string(),
                    (Literal::Float(value), Primitive::F32) => format!("{value:?}f"),
                    (Literal::Float(value), _) => format!("{value:?}"),
                };
                format!(
                    "{}{INDENT}public const {} {} = {value};",
                    summary(INDENT, &body(&constant.doc)),
                    spelling.primitive(constant.ty),
                    pascal(&constant.name)
                )
            })
            .collect();
        let structs = api
            .structs
            .iter()
            .map(|item| {
                let mut doc = summary("", &body(&item.doc));
                let params: Vec<String> = item
                    .fields
                    .iter()
                    .map(|field| {
                        let described = body(&field.doc).join(" ");
                        if !described.is_empty() {
                            doc.push_str(&format!(
                                "/// <param name=\"{}\">{}</param>\n",
                                pascal(&field.name),
                                escape(&described)
                            ));
                        }
                        format!("{} {}", spelling.primitive(field.ty), pascal(&field.name))
                    })
                    .collect();
                format!(
                    "{doc}public readonly record struct {}({});",
                    pascal(&item.name),
                    params.join(", ")
                )
            })
            .collect();
        let mut enums: Vec<Enumeration> = api
            .enums
            .iter()
            .map(|item| {
                let name = pascal(&item.name);
                let mut paragraphs = body(&item.doc);
                if item.openness == Openness::Open {
                    paragraphs.push(format!(
                        "Open: any value converts, as in <c>({name})'a'</c>."
                    ));
                }
                Enumeration {
                    doc: summary("", &paragraphs),
                    name,
                    flags: false,
                    members: item
                        .variants
                        .iter()
                        .map(|variant| {
                            format!(
                                "{}{INDENT}{} = {},",
                                summary(INDENT, &body(&variant.doc)),
                                pascal(&variant.name),
                                variant.value
                            )
                        })
                        .collect(),
                }
            })
            .collect();
        enums.extend(api.flags.iter().map(|item| {
            Enumeration {
                doc: summary("", &body(&item.doc)),
                name: pascal(&item.name),
                flags: true,
                members: std::iter::once(format!(
                    "{}{INDENT}None = 0,",
                    summary(INDENT, &["No flag.".to_owned()])
                ))
                .chain(item.flags.iter().map(|flag| {
                    format!(
                        "{}{INDENT}{} = {:#x},",
                        summary(INDENT, &body(&flag.doc)),
                        pascal(&flag.name),
                        flag.value
                    )
                }))
                .collect(),
            }
        }));
        let exception = api
            .enums
            .iter()
            .find(|item| item.error.is_some())
            .map(|item| ExceptionItem {
                name: spelling.exception(),
                status: pascal(&item.name),
            });
        let delegates = Id::all(&api.callbacks)
            .map(|id| {
                let callback = &api[id];
                let docs = projection::callback_docs(callback, spelling);
                let mut doc = summary("", &docs.body);
                for (param, text) in &docs.params {
                    doc.push_str(&tagged(
                        "",
                        &format!("<param name=\"{}\">", bare(param)),
                        "</param>",
                        &described_param(param, text),
                    ));
                }
                let params: Vec<String> = callback
                    .params
                    .iter()
                    .map(|param| format!("{} {}", spelling.received(&param.ty), camel(&param.name)))
                    .collect();
                Delegate {
                    doc,
                    text: format!(
                        "public delegate void {}({});",
                        spelling.delegate(id),
                        params.join(", ")
                    ),
                }
            })
            .collect();
        let declaring = projection::declaring(api);
        let mut element = None;
        let classes = Id::all(&api.classes)
            .map(|id| {
                let class = &api[id];
                let items = projection::items(api, abi, id);
                if declaring == Some(id) {
                    element = Some(modifiers(spelling, id, &items));
                }
                let mut paragraphs = body(&class.doc);
                paragraphs.push(lifetime_note(class.lifetime).to_owned());
                ClassItem {
                    doc: summary("", &paragraphs),
                    name: spelling.class(id),
                    kind: match projection::release(class) {
                        Some(lifecycle) => Kind::Handle {
                            release: format!(
                                "{}_{}",
                                api.abi_prefix,
                                abi::name(api, Origin::Lifecycle(id, lifecycle))
                            ),
                            bound: class.lifetime == Lifetime::Local,
                            counted: class.lifetime != Lifetime::Owned,
                        },
                        None => Kind::Lent {
                            declaring: declaring == Some(id),
                            end: class.has_containers().then(|| {
                                format!("{}_{}", api.abi_prefix, abi::name(api, Origin::End(id)))
                            }),
                        },
                    },
                    members: members(spelling, id, &items),
                }
            })
            .collect();
        let functions = abi
            .functions
            .iter()
            .filter_map(|function| match function.origin {
                Origin::Function(id) => {
                    let name = pascal(&api[id].name);
                    let call = call(spelling, function, None);
                    let (returns, tail) =
                        result(spelling, function, &call.expression(spelling, function));
                    Some(render(&Method {
                        doc: described(spelling, function),
                        head: format!("public static {returns} {name}"),
                        params: call.params.clone(),
                        body: call.body(tail),
                        indent: INDENT,
                    }))
                }
                _ => None,
            })
            .collect();
        Self {
            library: api.name.clone(),
            namespace: pascal(&api.name),
            doc: summary("", &body(&api.doc)),
            version: api.version.packed(),
            constants,
            structs,
            enums,
            exception,
            delegates,
            classes,
            view: declaring.map_or_else(String::new, |id| spelling.class(id)),
            element,
            functions,
        }
    }
}

fn lifetime_note(lifetime: Lifetime) -> &'static str {
    match lifetime {
        Lifetime::Owned => {
            "It owns its native object until it is handed over or disposed; the finalizer frees an \
             object never handed over."
        }
        Lifetime::Shared => {
            "It holds one reference to a native object usable from any thread; disposing it or the \
             finalizer drops the reference."
        }
        Lifetime::Local => {
            "It holds one reference to a native object usable only on the thread that made it; \
             disposing it drops the reference, and the finalizer hands it back to that thread."
        }
        Lifetime::Scoped => "It is lent to a callback and valid only while the callback runs.",
    }
}

fn summary(indent: &str, paragraphs: &[String]) -> String {
    let paragraphs: Vec<&String> = paragraphs.iter().filter(|text| !text.is_empty()).collect();
    if paragraphs.is_empty() {
        return String::new();
    }
    let mut out = format!("{indent}/// <summary>\n");
    for (index, paragraph) in paragraphs.iter().enumerate() {
        let wrapped = wrap(&escape(paragraph), WIDTH - indent.len() - 4);
        match (paragraphs.len(), index) {
            (1, _) => {
                for line in wrapped {
                    out.push_str(&format!("{indent}/// {line}\n"));
                }
            }
            _ => {
                out.push_str(&format!("{indent}/// <para>\n"));
                for line in wrapped {
                    out.push_str(&format!("{indent}/// {line}\n"));
                }
                out.push_str(&format!("{indent}/// </para>\n"));
            }
        }
    }
    out.push_str(&format!("{indent}/// </summary>\n"));
    out
}

fn tagged(indent: &str, open: &str, close: &str, text: &str) -> String {
    let single = format!("{indent}/// {open}{}{close}", escape(text));
    if single.len() <= WIDTH {
        return format!("{single}\n");
    }
    let mut out = format!("{indent}/// {open}\n");
    for line in wrap(&escape(text), WIDTH - indent.len() - 4) {
        out.push_str(&format!("{indent}/// {line}\n"));
    }
    out.push_str(&format!("{indent}/// {close}\n"));
    out
}

fn described_param(name: &str, text: &str) -> String {
    match text.is_empty() {
        true => format!("The {}.", spaced(name)),
        false => text.to_owned(),
    }
}

fn described(spelling: &Spelling, function: &Function) -> String {
    docs(spelling, projection::docs(spelling.api, function, spelling))
}

fn docs(spelling: &Spelling, docs: projection::Docs) -> String {
    let mut out = summary(INDENT, &docs.body);
    for (name, text) in &docs.params {
        out.push_str(&tagged(
            INDENT,
            &format!("<param name=\"{}\">", bare(name)),
            "</param>",
            &described_param(name, text),
        ));
    }
    if !docs.returns.is_empty() {
        out.push_str(&tagged(INDENT, "<returns>", "</returns>", &docs.returns));
    }
    if docs.throws {
        out.push_str(&tagged(
            INDENT,
            &format!("<exception cref=\"{}\">", spelling.exception()),
            "</exception>",
            "It failed; the exception carries the status.",
        ));
    }
    out
}

struct Method {
    doc: String,
    head: String,
    params: Vec<String>,
    body: Vec<String>,
    indent: &'static str,
}

fn render(method: &Method) -> String {
    let indent = method.indent;
    let mut out = method.doc.clone();
    let single = format!("{indent}{}({})", method.head, method.params.join(", "));
    if single.len() <= WIDTH {
        out.push_str(&single);
    } else {
        out.push_str(&format!("{indent}{}(\n", method.head));
        let last = method.params.len().saturating_sub(1);
        for (index, param) in method.params.iter().enumerate() {
            let separator = if index == last { ")" } else { "," };
            out.push_str(&format!("{indent}{INDENT}{param}{separator}\n"));
        }
        out = out.trim_end().to_owned();
    }
    out.push_str(&format!("\n{indent}{{\n"));
    for line in &method.body {
        match line.is_empty() {
            true => out.push('\n'),
            false => out.push_str(&format!("{indent}{INDENT}{line}\n")),
        }
    }
    out.push_str(&format!("{indent}}}"));
    out
}

#[derive(Default)]
struct Call {
    params: Vec<String>,
    checks: Vec<String>,
    leases: Vec<String>,
    late: Vec<String>,
    pinned: Vec<String>,
    arguments: Vec<String>,
}

impl Call {
    fn expression(&self, spelling: &Spelling, function: &Function) -> String {
        format!(
            "Native.{}_{}({})",
            spelling.api.abi_prefix,
            function.name,
            self.arguments.join(", ")
        )
    }

    fn body(&self, tail: Vec<String>) -> Vec<String> {
        let mut lines: Vec<String> = self
            .checks
            .iter()
            .chain(&self.leases)
            .chain(&self.late)
            .cloned()
            .collect();
        let mut depth = String::new();
        for pin in &self.pinned {
            lines.push(format!("{depth}{pin}"));
            lines.push(format!("{depth}{{"));
            depth.push_str(INDENT);
        }
        lines.extend(tail.into_iter().map(|line| format!("{depth}{line}")));
        while !depth.is_empty() {
            depth.truncate(depth.len() - INDENT.len());
            lines.push(format!("{depth}}}"));
        }
        lines
    }
}

enum Receiver {
    Handle(Ownership),
    Lent,
    Current,
}

fn call(spelling: &Spelling, function: &Function, receiver: Option<Receiver>) -> Call {
    let mut call = Call::default();
    match receiver {
        None => {}
        Some(Receiver::Handle(Ownership::Owned)) => {
            call.late.push("nint self = handle.Take();".to_owned());
            call.arguments.push("self".to_owned());
        }
        Some(Receiver::Handle(Ownership::Borrowed)) => {
            call.leases
                .push("using var self = handle.Lease();".to_owned());
            call.arguments.push("self.Raw".to_owned());
        }
        Some(Receiver::Lent | Receiver::Current) => call.arguments.push(LENT.to_owned()),
    }
    for param in &function.signature.params {
        let name = camel(&param.name);
        let local = bare(&param.name);
        match &param.ty {
            Input::Value(Value::Primitive(Primitive::Bool)) => {
                call.params.push(format!("bool {name}"));
                call.arguments.push(format!("{name} ? (byte)1 : (byte)0"));
            }
            Input::Value(Value::Primitive(Primitive::Usize)) => {
                call.params.push(format!("int {name}"));
                call.arguments.push(format!("(nint){name}"));
            }
            Input::Value(Value::Primitive(primitive)) => {
                call.params
                    .push(format!("{} {name}", spelling.primitive(*primitive)));
                call.arguments.push(name);
            }
            Input::Value(value @ Value::Struct(_)) => {
                call.params
                    .push(format!("{} {name}", spelling.value(*value)));
                call.arguments
                    .push(format!("{}.From({name})", spelling.native_value(*value)));
            }
            Input::Value(value) => {
                call.params
                    .push(format!("{} {name}", spelling.value(*value)));
                call.arguments.push(format!("(uint){name}"));
            }
            Input::String => {
                call.params.push(format!("string {name}"));
                call.leases.push(format!(
                    "Span<byte> {local}Utf8 = Interop.Utf8({name}, stackalloc byte[Interop.StackLimit], nameof({local}));"
                ));
                call.pinned
                    .push(format!("fixed (byte* {local}Bytes = {local}Utf8)"));
                call.arguments
                    .push(format!("new Str({local}Bytes, (nuint){local}Utf8.Length)"));
            }
            Input::Object {
                class,
                ownership: Ownership::Borrowed,
            } => {
                call.params
                    .push(format!("{} {name}", spelling.class(*class)));
                call.checks
                    .push(format!("ArgumentNullException.ThrowIfNull({name});"));
                call.leases
                    .push(format!("using var {local}Lease = {name}.handle.Lease();"));
                call.arguments.push(format!("{local}Lease.Raw"));
            }
            Input::Object {
                class,
                ownership: Ownership::Owned,
            } => {
                call.params
                    .push(format!("{} {name}", spelling.class(*class)));
                call.checks
                    .push(format!("ArgumentNullException.ThrowIfNull({name});"));
                call.late
                    .push(format!("nint {local}Raw = {name}.handle.Take();"));
                call.arguments.push(format!("{local}Raw"));
            }
            Input::Array(Element::Primitive(primitive)) => {
                call.params.push(format!(
                    "ReadOnlySpan<{}> {name}",
                    spelling.primitive(*primitive)
                ));
                call.pinned.push(format!(
                    "fixed ({}* {local}Items = {name})",
                    spelling.native_primitive(*primitive)
                ));
                call.arguments.push(format!("{local}Items"));
                call.arguments.push(format!("(nint){name}.Length"));
            }
            Input::Array(Element::String) => {
                unreachable!("validated: only callbacks receive string arrays")
            }
            Input::Closure(closure) => {
                let names: Vec<String> = closure
                    .functions
                    .iter()
                    .map(|part| camel(&part.name))
                    .collect();
                for (part, part_name) in closure.functions.iter().zip(&names) {
                    call.params
                        .push(format!("{} {part_name}", spelling.delegate(part.callback)));
                    call.checks
                        .push(format!("ArgumentNullException.ThrowIfNull({part_name});"));
                }
                call.late.push(format!(
                    "nint userdata = Closures.Retain({});",
                    names.join(", ")
                ));
                for (position, part) in closure.functions.iter().enumerate() {
                    call.arguments.push(format!(
                        "&Trampolines.{}",
                        spelling.trampoline(part.callback, position)
                    ));
                }
                call.arguments.push("userdata".to_owned());
                call.arguments.push("&Closures.Destroy".to_owned());
            }
        }
    }
    if call.arguments.first().is_some_and(|first| first == LENT) && !call.late.is_empty() {
        call.arguments[0] = "self".to_owned();
        call.leases.push(format!("nint self = {LENT};"));
    }
    call
}

fn result(spelling: &Spelling, function: &Function, expression: &str) -> (String, Vec<String>) {
    let api = spelling.api;
    if function.signature.throws.is_some() {
        return (
            "void".to_owned(),
            vec![format!("Native.Check({expression});")],
        );
    }
    let value = |ty: String, converted: String| (ty, vec![format!("return {converted};")]);
    match (function.returns(), function.signature.returns) {
        (Return::Void, _) => ("void".to_owned(), vec![format!("{expression};")]),
        (Return::Value(Value::Primitive(Primitive::Bool)), _) => {
            value("bool".to_owned(), format!("{expression} != 0"))
        }
        (Return::Value(Value::Primitive(Primitive::Usize)), _) => {
            value("int".to_owned(), format!("checked((int){expression})"))
        }
        (Return::Value(Value::Primitive(primitive)), _) => value(
            spelling.primitive(primitive).to_owned(),
            expression.to_owned(),
        ),
        (Return::Value(found @ Value::Struct(_)), _) => {
            value(spelling.value(found), format!("{expression}.ToPublic()"))
        }
        (Return::Value(found), _) => {
            let ty = spelling.value(found);
            value(ty.clone(), format!("({ty}){expression}"))
        }
        (Return::Str, _) => value("string".to_owned(), format!("Interop.Read({expression})")),
        (Return::String, _) => value("string".to_owned(), format!("Interop.Take({expression})")),
        (
            Return::Object(class),
            Some(Result::Object {
                ownership: Ownership::Borrowed,
                ..
            }),
        ) => {
            let ty = spelling.class(class);
            value(
                ty.clone(),
                format!(
                    "{ty}.Adopt(Native.{}_{}({expression}))",
                    api.abi_prefix,
                    abi::name(api, Origin::Lifecycle(class, Lifecycle::Ref))
                ),
            )
        }
        (Return::Object(class), _) => {
            let ty = spelling.class(class);
            value(ty.clone(), format!("{ty}.Adopt({expression})"))
        }
    }
}

fn members(spelling: &Spelling, class: Id<Class>, items: &[Item]) -> Vec<String> {
    let api = spelling.api;
    let owner = &api[class];
    let name = spelling.class(class);
    let scoped = owner.lifetime == Lifetime::Scoped;
    let receiver = |ownership| match scoped {
        true => Receiver::Lent,
        false => Receiver::Handle(ownership),
    };
    let mut out = Vec::new();
    for (index, item) in items.iter().enumerate() {
        let member = spelling.member(class, index);
        match item {
            Item::Constructor {
                function,
                constructor,
            } => {
                let call = call(spelling, function, None);
                let expression = call.expression(spelling, function);
                match constructor.name {
                    None => out.push(render(&Method {
                        doc: described(spelling, function),
                        head: format!("public {member}"),
                        params: call.params.clone(),
                        body: call.body(vec![format!("handle = Handle({expression});")]),
                        indent: INDENT,
                    })),
                    Some(_) => out.push(render(&Method {
                        doc: described(spelling, function),
                        head: format!("public static {name} {member}"),
                        params: call.params.clone(),
                        body: call.body(vec![format!("return Adopt({expression});")]),
                        indent: INDENT,
                    })),
                }
            }
            Item::Method {
                function,
                method,
                chained,
            } => {
                let call = call(spelling, function, Some(receiver(method.receiver)));
                let (mut returns, tail) =
                    result(spelling, function, &call.expression(spelling, function));
                let mut body = Vec::new();
                if *chained {
                    returns = name.clone();
                    body = vec!["return this;".to_owned()];
                }
                let mut lines = call.body(tail);
                lines.extend(body);
                out.push(render(&Method {
                    doc: described(spelling, function),
                    head: format!("public {returns} {member}"),
                    params: call.params.clone(),
                    body: lines,
                    indent: INDENT,
                }));
            }
            Item::Property { getter, setter, .. } => {
                let read = call(spelling, getter, Some(receiver(Ownership::Borrowed)));
                let (returns, tail) = result(spelling, getter, &read.expression(spelling, getter));
                let mut text = described(spelling, getter);
                if let Some(setter) = setter {
                    let described = spelling.body(setter.description(api)).join(" ");
                    if described != spelling.body(getter.description(api)).join(" ") {
                        text.push_str(&tagged(INDENT, "<remarks>", "</remarks>", &described));
                    }
                }
                text.push_str(&format!("{INDENT}public {returns} {member}\n{INDENT}{{\n"));
                text.push_str(&format!("{INDENT}{INDENT}get\n{INDENT}{INDENT}{{\n"));
                for line in read.body(tail) {
                    text.push_str(&format!("{INDENT}{INDENT}{INDENT}{line}\n"));
                }
                text.push_str(&format!("{INDENT}{INDENT}}}\n"));
                if let Some(setter) = setter {
                    let mut renamed = (*setter).clone();
                    renamed.signature.params[0].name = "value".to_owned();
                    let write = call(spelling, &renamed, Some(receiver(Ownership::Borrowed)));
                    let expression = write.expression(spelling, &renamed);
                    let lines = write.body(vec![format!("{expression};")]);
                    text.push_str(&format!("\n{INDENT}{INDENT}set\n{INDENT}{INDENT}{{\n"));
                    for line in lines {
                        text.push_str(&format!("{INDENT}{INDENT}{INDENT}{line}\n"));
                    }
                    text.push_str(&format!("{INDENT}{INDENT}}}\n"));
                }
                text.push_str(&format!("{INDENT}}}"));
                out.push(text);
            }
            Item::Declaration {
                function,
                declaration,
            } => {
                let mut call = call(spelling, function, Some(Receiver::Lent));
                let expression = call.expression(spelling, function);
                match declaration.kind {
                    DeclarationKind::Container => {
                        call.params.push(format!("Action {BODY}"));
                        call.checks
                            .push(format!("ArgumentNullException.ThrowIfNull({BODY});"));
                        out.push(render(&Method {
                            doc: docs(
                                spelling,
                                projection::docs(api, function, spelling)
                                    .with_param(BODY, "Declares the children."),
                            ),
                            head: format!("public {ELEMENT} {member}"),
                            params: call.params.clone(),
                            body: call.body(vec![
                                format!("{expression};"),
                                format!("return Contain({BODY});"),
                            ]),
                            indent: INDENT,
                        }));
                    }
                    DeclarationKind::Element => out.push(render(&Method {
                        doc: described(spelling, function),
                        head: format!("public {ELEMENT} {member}"),
                        params: call.params.clone(),
                        body: call.body(vec![
                            format!("{expression};"),
                            "return Declared();".to_owned(),
                        ]),
                        indent: INDENT,
                    })),
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
        let call = call(spelling, function, Some(Receiver::Current));
        let expression = call.expression(spelling, function);
        out.push(render(&Method {
            doc: described(spelling, function),
            head: format!("public {ELEMENT} {}", spelling.member(class, index)),
            params: call.params.clone(),
            body: call.body(vec![format!("{expression};"), "return this;".to_owned()]),
            indent: INDENT,
        }));
    }
    out
}

fn render_trampoline(spelling: &Spelling, trampoline: Trampoline) -> String {
    let api = spelling.api;
    let callback = &api[trampoline.callback];
    let params: Vec<String> = abi::callback_slots(callback)
        .iter()
        .map(|slot| format!("{} {}", spelling.native_slot(slot), camel(&slot.name)))
        .collect();
    let mut prelude = vec!["Reclaimer.Drain();".to_owned()];
    let mut arguments = Vec::new();
    let mut lent = Vec::new();
    for param in &callback.params {
        let name = camel(&param.name);
        let local = bare(&param.name);
        let count = || camel(&abi::count_name(&param.name));
        match &param.ty {
            Input::Object { class, .. } => {
                let ty = spelling.class(*class);
                prelude.push(format!("var {local}Lent = new {ty}({name});"));
                arguments.push(format!("{local}Lent"));
                lent.push(format!("{local}Lent.Expire();"));
            }
            Input::Value(Value::Primitive(Primitive::Bool)) => {
                arguments.push(format!("{name} != 0"))
            }
            Input::Value(Value::Primitive(Primitive::Usize)) => {
                arguments.push(format!("checked((int){name})"));
            }
            Input::Value(Value::Primitive(_)) => arguments.push(name),
            Input::Value(Value::Struct(_)) => arguments.push(format!("{name}.ToPublic()")),
            Input::Value(value) => arguments.push(format!("({}){name}", spelling.value(*value))),
            Input::String => arguments.push(format!("Interop.Read({name})")),
            Input::Array(Element::String) => {
                arguments.push(format!("Interop.ReadAll({name}, {})", count()));
            }
            Input::Array(Element::Primitive(_)) => {
                arguments.push(format!("Interop.Copy({name}, {})", count()));
            }
            Input::Closure(_) => unreachable!("validated: callbacks receive no closures"),
        }
    }
    let mut out = format!(
        "{INDENT}[UnmanagedCallersOnly(CallConvs = [typeof(CallConvCdecl)])]\n{INDENT}internal static void {}({})\n{INDENT}{{\n",
        spelling.trampoline(trampoline.callback, trampoline.position),
        params.join(", ")
    );
    let inner = format!("{INDENT}{INDENT}");
    for line in &prelude {
        out.push_str(&format!("{inner}{line}\n"));
    }
    out.push_str(&format!("{inner}try\n{inner}{{\n"));
    out.push_str(&format!(
        "{inner}{INDENT}Closures.Get<{}>(userdata, {})({});\n",
        spelling.delegate(trampoline.callback),
        trampoline.position,
        arguments.join(", ")
    ));
    out.push_str(&format!(
        "{inner}}}\n{inner}catch (Exception exception)\n{inner}{{\n"
    ));
    out.push_str(&format!(
        "{inner}{INDENT}Closures.Fail(exception);\n{inner}}}\n"
    ));
    if !lent.is_empty() {
        out.push_str(&format!("{inner}finally\n{inner}{{\n"));
        for line in &lent {
            out.push_str(&format!("{inner}{INDENT}{line}\n"));
        }
        out.push_str(&format!("{inner}}}\n"));
    }
    out.push_str(&format!("{INDENT}}}"));
    out
}

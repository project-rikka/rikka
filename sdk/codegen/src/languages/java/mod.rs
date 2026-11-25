mod spelling;

use askama::Template;

use crate::abi::{self, Abi, Function, Lifecycle, Origin, Return};
use crate::diagnostic::Diagnostic;
use crate::language::{Error, Language, Output};
use crate::languages::doxygen;
use crate::languages::projection::{self, Item, Linker, Literal, Naming, Trampoline, Vocabulary};
use crate::model::{
    Access, Api, Callback, Class, DeclarationKind, Element, Flags, Id, Input, Lifetime, Member,
    Openness, Output as Result, Ownership, Primitive, Value,
};
use crate::names::{pascal, screaming, spaced};

use spelling::{ELEMENT, Spelling, camel, escape};

const INDENT: &str = "    ";
const WIDTH: usize = 110;
const DOMAIN: &str = "os";
const RELEASE: &str = "22";
const BODY: &str = "body";
const CONSTRUCT: &str = "construct";
const INTERNAL: &[&str] = &["Native", "Handle", "Trampolines"];
const OBJECT_METHODS: &[&str] = &[
    "close",
    "equals",
    "getClass",
    "hashCode",
    "notify",
    "notifyAll",
    "toString",
    "wait",
];
const IMPORTS: &[(&str, &str)] = &[
    ("java.lang.foreign.Arena", "Arena."),
    ("java.lang.foreign.MemorySegment", "MemorySegment"),
    ("java.lang.foreign.SegmentAllocator", "SegmentAllocator"),
    ("java.lang.foreign.ValueLayout", "ValueLayout."),
    ("java.lang.ref.Reference", "Reference."),
    ("java.util.List", "List<"),
    ("java.util.Objects", "Objects."),
    ("java.util.Set", "Set<"),
];

pub(crate) struct Java;

impl Language for Java {
    fn name(&self) -> &'static str {
        "java"
    }

    fn generate(&self, api: &Api) -> std::result::Result<Vec<Output>, Error> {
        let abi = Abi::new(api)?;
        let spelling = Spelling { api };
        validate(&spelling)?;
        let package = format!("{DOMAIN}.{}", api.name);
        let folder = format!("src/main/java/{}", package.replace('.', "/"));
        let header = Header {
            library: api.name.clone(),
            package: package.clone(),
        };
        let file = |name: &str| format!("{folder}/{name}.java");
        let mut outputs = vec![
            Output::render(
                "pom.xml".to_owned(),
                &Pom {
                    package: package.clone(),
                    artifact: api.name.clone(),
                    version: api.version.to_string(),
                    summary: escape(&projection::summary(api, &spelling)),
                    release: RELEASE,
                },
            )?,
            Output::render(
                "src/main/java/module-info.java".to_owned(),
                &ModuleInfo {
                    library: api.name.clone(),
                    module: package.clone(),
                    doc: javadoc("", &spelling.body(&api.doc.body))
                        .replace("{@link ", &format!("{{@link {package}.")),
                },
            )?,
            Output::render(
                file(&spelling.statics()),
                &Statics::new(&spelling, &abi, &header),
            )?,
            Output::render(file("Native"), &Native::new(&spelling, &abi, &header))?,
            Output::render(file("Handle"), &Runtime { header: &header })?,
            Output::render(
                file("Trampolines"),
                &Trampolines::new(&spelling, &abi, &header),
            )?,
        ];
        if let Some(error) = Id::all(&api.enums).find(|id| api[*id].error.is_some()) {
            outputs.push(Output::render(
                file(&spelling.exception()),
                &Exception {
                    header: &header,
                    name: spelling.exception(),
                    status: pascal(&api[error].name),
                },
            )?);
        }
        for item in &api.structs {
            outputs.push(Output::render(
                file(&pascal(&item.name)),
                &record(&spelling, item, &header),
            )?);
        }
        for id in Id::all(&api.enums) {
            let item = &api[id];
            let name = pascal(&item.name);
            let closed = constants(
                &spelling,
                item.variants
                    .iter()
                    .map(|variant| (variant, variant.value.to_string())),
            );
            match item.openness {
                Openness::Open => {
                    let mut lines = spelling.body(&item.doc.body);
                    lines.push(format!(
                        "Open: any value is a key, as in {{@code {name}.of('a')}}."
                    ));
                    lines.push(tag("param", Some("value"), "the raw value"));
                    outputs.push(Output::render(
                        file(&name),
                        &OpenEnum {
                            header: &header,
                            doc: javadoc("", &lines),
                            name: name.clone(),
                            constants: constants(
                                &spelling,
                                item.variants
                                    .iter()
                                    .map(|variant| (variant, format!("{:#x}", variant.value))),
                            ),
                        },
                    )?);
                }
                Openness::Closed => outputs.push(Output::render(
                    file(&name),
                    &Enum {
                        header: &header,
                        doc: javadoc("", &spelling.body(&item.doc.body)),
                        name: name.clone(),
                        constants: closed,
                    },
                )?),
            }
        }
        for id in Id::all(&api.flags) {
            let item = &api[id];
            let mut lines = spelling.body(&item.doc.body);
            lines.push(
                "Combine them in a {@link java.util.Set}, such as an {@link java.util.EnumSet}."
                    .to_owned(),
            );
            outputs.push(Output::render(
                file(&spelling.flag(id)),
                &Flag {
                    header: &header,
                    doc: javadoc("", &lines),
                    name: spelling.flag(id),
                    constants: constants(
                        &spelling,
                        item.flags
                            .iter()
                            .map(|flag| (flag, format!("{:#x}", flag.value))),
                    ),
                },
            )?);
        }
        for id in Id::all(&api.callbacks) {
            outputs.push(Output::render(
                file(&spelling.interface(id)),
                &callback(&spelling, id, &header),
            )?);
        }
        let declaring = projection::declaring(api);
        for id in Id::all(&api.classes) {
            let items = projection::items(api, &abi, id);
            outputs.push(Output::render(
                file(&spelling.class(id)),
                &class(&spelling, id, &items, declaring == Some(id), &header),
            )?);
            if declaring == Some(id) {
                let modifiers = modifiers(&spelling, id, &items);
                outputs.push(Output::render(
                    file(ELEMENT),
                    &ElementFile {
                        header: &header,
                        imports: imports(&modifiers),
                        view: spelling.class(id),
                        modifiers,
                    },
                )?);
            }
        }
        Ok(outputs)
    }
}

fn validate(spelling: &Spelling) -> std::result::Result<(), Diagnostic> {
    let reserved_types = [ELEMENT.to_owned(), spelling.statics(), spelling.exception()]
        .into_iter()
        .chain(INTERNAL.iter().map(|name| (*name).to_owned()))
        .collect();
    projection::validate(
        spelling.api,
        Vocabulary {
            language: "Java",
            type_noun: "type",
            member_noun: "method",
            error: spelling.exception(),
            reserved_types,
            reserved_members: OBJECT_METHODS,
        },
        spelling,
    )
}

impl Naming for Spelling<'_> {
    fn flags(&self, flags: Id<Flags>) -> String {
        self.flag(flags)
    }

    fn callback(&self, callback: Id<Callback>) -> String {
        self.interface(callback)
    }

    fn members(
        &self,
        class: Id<Class>,
        index: usize,
    ) -> std::result::Result<Vec<String>, Diagnostic> {
        Ok(match self.api.member(class, index) {
            Member::Constructor(constructor) if constructor.name.is_none() => Vec::new(),
            Member::Property(property) if property.access == Access::ReadWrite => {
                vec![self.member(class, index), self.setter(class, index)]
            }
            _ => vec![self.member(class, index)],
        })
    }
}

fn imports(code: &[String]) -> Vec<&'static str> {
    let statements: Vec<&str> = code
        .iter()
        .flat_map(|text| text.lines())
        .map(str::trim_start)
        .filter(|line| !line.starts_with('*') && !line.starts_with("/**"))
        .collect();
    IMPORTS
        .iter()
        .filter(|(_, used)| statements.iter().any(|line| line.contains(used)))
        .map(|(import, _)| *import)
        .collect()
}

struct Header {
    library: String,
    package: String,
}

#[derive(Template)]
#[template(path = "java/pom.xml.j2", escape = "none")]
struct Pom {
    package: String,
    artifact: String,
    version: String,
    summary: String,
    release: &'static str,
}

#[derive(Template)]
#[template(path = "java/module-info.java.j2", escape = "none")]
struct ModuleInfo {
    library: String,
    module: String,
    doc: String,
}

#[derive(Template)]
#[template(path = "java/Handle.java.j2", escape = "none")]
struct Runtime<'a> {
    header: &'a Header,
}

#[derive(Template)]
#[template(path = "java/Exception.java.j2", escape = "none")]
struct Exception<'a> {
    header: &'a Header,
    name: String,
    status: String,
}

struct Constant {
    doc: String,
    name: String,
    value: String,
}

fn constants<'a>(
    spelling: &Spelling,
    variants: impl Iterator<Item = (&'a crate::model::Variant, String)>,
) -> Vec<Constant> {
    variants
        .map(|(variant, value)| Constant {
            doc: javadoc(INDENT, &spelling.body(&variant.doc.body)),
            name: screaming(&variant.name),
            value,
        })
        .collect()
}

#[derive(Template)]
#[template(path = "java/Record.java.j2", escape = "none")]
struct Record<'a> {
    header: &'a Header,
    doc: String,
    name: String,
    fields: String,
}

fn record<'a>(spelling: &Spelling, item: &crate::model::Struct, header: &'a Header) -> Record<'a> {
    let mut lines = spelling.body(&item.doc.body);
    for field in &item.fields {
        let text = spelling.body(&field.doc.body).join(" ");
        if !text.is_empty() {
            lines.push(tag("param", Some(&camel(&field.name)), &text));
        }
    }
    Record {
        header,
        doc: javadoc("", &lines),
        name: pascal(&item.name),
        fields: item
            .fields
            .iter()
            .map(|field| format!("{} {}", spelling.primitive(field.ty), camel(&field.name)))
            .collect::<Vec<_>>()
            .join(", "),
    }
}

#[derive(Template)]
#[template(path = "java/Enum.java.j2", escape = "none")]
struct Enum<'a> {
    header: &'a Header,
    doc: String,
    name: String,
    constants: Vec<Constant>,
}

#[derive(Template)]
#[template(path = "java/OpenEnum.java.j2", escape = "none")]
struct OpenEnum<'a> {
    header: &'a Header,
    doc: String,
    name: String,
    constants: Vec<Constant>,
}

#[derive(Template)]
#[template(path = "java/Flag.java.j2", escape = "none")]
struct Flag<'a> {
    header: &'a Header,
    doc: String,
    name: String,
    constants: Vec<Constant>,
}

#[derive(Template)]
#[template(path = "java/Callback.java.j2", escape = "none")]
struct CallbackFile<'a> {
    header: &'a Header,
    imports: Vec<&'static str>,
    doc: String,
    name: String,
    method_doc: String,
    method: &'static str,
    params: String,
}

fn invoked(params: usize) -> &'static str {
    match params {
        0 => "run",
        _ => "accept",
    }
}

fn callback<'a>(
    spelling: &Spelling,
    id: Id<crate::model::Callback>,
    header: &'a Header,
) -> CallbackFile<'a> {
    let callback = &spelling.api[id];
    let docs = projection::callback_docs(callback, spelling);
    let summary: Vec<String> = docs.body.iter().take(1).cloned().collect();
    let mut lines = docs.body;
    for (name, text) in &docs.params {
        lines.push(tag(
            "param",
            Some(&camel(name)),
            &described_param(name, text),
        ));
    }
    let params: Vec<String> = callback
        .params
        .iter()
        .map(|param| format!("{} {}", spelling.input(&param.ty), camel(&param.name)))
        .collect();
    CallbackFile {
        header,
        imports: imports(&params),
        doc: javadoc("", &summary),
        name: spelling.interface(id),
        method_doc: javadoc(INDENT, &lines),
        method: invoked(params.len()),
        params: params.join(", "),
    }
}

#[derive(Template)]
#[template(path = "java/Statics.java.j2", escape = "none")]
struct Statics<'a> {
    header: &'a Header,
    doc: String,
    name: String,
    version: u32,
    soname: String,
    dll: String,
    constants: Vec<String>,
    functions: Vec<String>,
    imports: Vec<&'static str>,
}

impl<'a> Statics<'a> {
    fn new(spelling: &Spelling, abi: &Abi, header: &'a Header) -> Self {
        let api = spelling.api;
        let constants = api
            .constants
            .iter()
            .map(|constant| {
                format!(
                    "{}{INDENT}public static final {} {} = {};",
                    javadoc(INDENT, &spelling.body(&constant.doc.body)),
                    spelling.primitive(constant.ty),
                    screaming(&constant.name),
                    java_literal(constant.ty, projection::literal(constant))
                )
            })
            .collect();
        let functions: Vec<String> = abi
            .functions
            .iter()
            .filter_map(|function| match function.origin {
                Origin::Function(id) => {
                    let call = call(spelling, function, None);
                    let (returns, tail) = result(spelling, function, &call);
                    Some(render(&Method {
                        doc: described(spelling, function),
                        head: format!("public static {returns} {}", camel(&api[id].name)),
                        params: call.params.clone(),
                        body: call.body(tail, Vec::new()),
                    }))
                }
                _ => None,
            })
            .collect();
        Self {
            header,
            doc: javadoc("", &spelling.body(&api.doc.body)),
            name: spelling.statics(),
            version: api.version.packed(),
            soname: projection::soname(api),
            dll: projection::dll(api),
            constants,
            imports: imports(&functions),
            functions,
        }
    }
}

fn java_literal(primitive: Primitive, literal: Literal) -> String {
    match (literal, primitive) {
        (Literal::Integer(value), Primitive::U8) => (value as u8 as i8).to_string(),
        (Literal::Integer(value), Primitive::U16) => (value as u16 as i16).to_string(),
        (Literal::Integer(value), Primitive::U32) => (value as u32 as i32).to_string(),
        (Literal::Integer(value), Primitive::U64 | Primitive::I64) => {
            format!("{}L", value as u64 as i64)
        }
        (Literal::Integer(value), _) => value.to_string(),
        (Literal::Float(value), Primitive::F32) => format!("{value:?}f"),
        (Literal::Float(value), _) => format!("{value:?}"),
    }
}

struct Layout {
    name: String,
    fields: String,
    offsets: Vec<String>,
    record: String,
    method: String,
    to_native: Vec<String>,
    from_native: String,
}

struct FlagSet {
    flag: String,
    method: String,
}

struct ErrorItem {
    status: String,
    success: String,
    message: String,
    exception: String,
}

#[derive(Template)]
#[template(path = "java/Native.java.j2", escape = "none")]
struct Native<'a> {
    header: &'a Header,
    statics: String,
    soname: String,
    dll: String,
    prefix: String,
    layouts: Vec<Layout>,
    handles: Vec<String>,
    flags: Vec<FlagSet>,
    error: Option<ErrorItem>,
}

fn padded_fields(spelling: &Spelling, fields: &[crate::model::Field]) -> Vec<String> {
    let mut laid = Vec::new();
    let mut offset = 0;
    let mut alignment = 1;
    let pad = |laid: &mut Vec<String>, offset: &mut u64, to: u64| {
        let gap = offset.next_multiple_of(to) - *offset;
        if gap > 0 {
            laid.push(format!("MemoryLayout.paddingLayout({gap})"));
            *offset += gap;
        }
    };
    for field in fields {
        let width = spelling.width(field.ty);
        pad(&mut laid, &mut offset, width);
        laid.push(format!(
            "{}.withName(\"{}\")",
            spelling.layout(field.ty),
            field.name
        ));
        offset += width;
        alignment = alignment.max(width);
    }
    pad(&mut laid, &mut offset, alignment);
    laid
}

impl<'a> Native<'a> {
    fn new(spelling: &Spelling, abi: &Abi, header: &'a Header) -> Self {
        let api = spelling.api;
        let layouts = api
            .structs
            .iter()
            .map(|item| {
                let layout = screaming(&item.name);
                let offset = |field: &crate::model::Field| {
                    format!("{layout}_{}", screaming(&field.name))
                };
                Layout {
                    name: layout.clone(),
                    fields: padded_fields(spelling, &item.fields).join(", "),
                    offsets: item
                        .fields
                        .iter()
                        .map(|field| {
                            format!(
                                "{} = {layout}.byteOffset(MemoryLayout.PathElement.groupElement(\"{}\"));",
                                offset(field),
                                field.name
                            )
                        })
                        .collect(),
                    record: pascal(&item.name),
                    method: camel(&item.name),
                    to_native: item
                        .fields
                        .iter()
                        .map(|field| {
                            let read = match field.ty {
                                Primitive::Usize => {
                                    format!("(long) value.{}()", camel(&field.name))
                                }
                                _ => format!("value.{}()", camel(&field.name)),
                            };
                            format!(
                                "segment.set({}, {}, {read});",
                                spelling.layout(field.ty),
                                offset(field)
                            )
                        })
                        .collect(),
                    from_native: item
                        .fields
                        .iter()
                        .map(|field| {
                            let read =
                                format!("segment.get({}, {})", spelling.layout(field.ty), offset(field));
                            match field.ty {
                                Primitive::Usize => format!("Math.toIntExact({read})"),
                                _ => read,
                            }
                        })
                        .collect::<Vec<_>>()
                        .join(", "),
                }
            })
            .collect();
        let handles = abi
            .functions
            .iter()
            .map(|function| {
                let slots: Vec<String> = function
                    .slots(api)
                    .iter()
                    .map(|slot| spelling.slot_layout(slot))
                    .collect();
                let returned = match function.returns() {
                    Return::Void => None,
                    Return::Value(value) => Some(spelling.value_layout(value)),
                    Return::Str => Some("STR".to_owned()),
                    Return::String => Some("STRING".to_owned()),
                    Return::Object(_) => Some("ADDRESS".to_owned()),
                };
                let descriptor = match returned {
                    None => format!("FunctionDescriptor.ofVoid({})", slots.join(", ")),
                    Some(layout) => {
                        let all: Vec<String> = std::iter::once(layout).chain(slots).collect();
                        format!("FunctionDescriptor.of({})", all.join(", "))
                    }
                };
                format!(
                    "{INDENT}static final MethodHandle {} = downcall(\"{}_{}\", {descriptor});",
                    spelling.handle(&function.name),
                    api.abi_prefix,
                    function.name
                )
            })
            .collect();
        let flags = Id::all(&api.flags)
            .map(|id| FlagSet {
                flag: spelling.flag(id),
                method: camel(&api[id].name),
            })
            .collect();
        let error = Id::all(&api.enums).find_map(|id| {
            let item = &api[id];
            let codes = item.error?;
            Some(ErrorItem {
                status: pascal(&item.name),
                success: screaming(&item.variants[codes.success].name),
                message: spelling.handle(&api[codes.message].name),
                exception: spelling.exception(),
            })
        });
        Self {
            header,
            statics: spelling.statics(),
            soname: projection::soname(api),
            dll: projection::dll(api),
            prefix: api.abi_prefix.clone(),
            layouts,
            handles,
            flags,
            error,
        }
    }
}

#[derive(Template)]
#[template(path = "java/Trampolines.java.j2", escape = "none")]
struct Trampolines<'a> {
    header: &'a Header,
    stubs: Vec<String>,
    methods: Vec<String>,
}

impl<'a> Trampolines<'a> {
    fn new(spelling: &Spelling, abi: &Abi, header: &'a Header) -> Self {
        let api = spelling.api;
        let trampolines = projection::trampolines(api, abi);
        Self {
            header,
            stubs: trampolines
                .iter()
                .map(|trampoline| {
                    let layouts: Vec<String> = abi::callback_slots(&api[trampoline.callback])
                        .iter()
                        .map(|slot| spelling.slot_layout(slot))
                        .collect();
                    format!(
                        "{INDENT}static final MemorySegment {} = upcall(\"{}\", FunctionDescriptor.ofVoid({}));",
                        stub(spelling, *trampoline),
                        spelling.trampoline(trampoline.callback, trampoline.position),
                        layouts.join(", ")
                    )
                })
                .collect(),
            methods: trampolines
                .into_iter()
                .map(|trampoline| render_trampoline(spelling, trampoline))
                .collect(),
        }
    }
}

fn stub(spelling: &Spelling, trampoline: Trampoline) -> String {
    screaming(&format!(
        "{}_{}",
        spelling.api[trampoline.callback].name, trampoline.position
    ))
}

fn javadoc(indent: &str, paragraphs: &[String]) -> String {
    let mut lines: Vec<String> = Vec::new();
    for paragraph in paragraphs.iter().filter(|text| !text.is_empty()) {
        match paragraph.starts_with('@') {
            true => lines.push(paragraph.clone()),
            false if lines.is_empty() => lines.push(escape(paragraph)),
            false => lines.push(format!("<p>{}", escape(paragraph))),
        }
    }
    doxygen::block(indent, &lines)
}

fn tag(tag: &str, name: Option<&str>, text: &str) -> String {
    match name {
        Some(name) => format!("@{tag} {name} {}", escape(text)),
        None => format!("@{tag} {}", escape(text)),
    }
}

fn described_param(name: &str, text: &str) -> String {
    match text.is_empty() {
        true => format!("the {}", spaced(name)),
        false => text.to_owned(),
    }
}

fn described(spelling: &Spelling, function: &Function) -> String {
    docs(spelling, projection::docs(spelling.api, function, spelling))
}

fn docs(spelling: &Spelling, docs: projection::Docs) -> String {
    let mut lines = docs.body;
    for (name, text) in &docs.params {
        lines.push(tag(
            "param",
            Some(&camel(name)),
            &described_param(name, text),
        ));
    }
    if !docs.returns.is_empty() {
        lines.push(tag("return", None, &docs.returns));
    }
    if docs.throws {
        lines.push(format!(
            "@throws {} if it fails; the exception carries the status",
            spelling.exception()
        ));
    }
    javadoc(INDENT, &lines)
}

fn lifetime_note(lifetime: Lifetime) -> &'static str {
    match lifetime {
        Lifetime::Owned => {
            "It owns its native object until it is handed over or closed; a cleaner frees an object \
             never handed over once it is unreachable."
        }
        Lifetime::Shared => {
            "It holds one reference to a native object usable from any thread; closing it, or a \
             cleaner once it is unreachable, drops the reference."
        }
        Lifetime::Local => {
            "It holds one reference to a native object usable only on the thread that made it; \
             closing it drops the reference, and a cleaner hands an unreachable one back to that \
             thread."
        }
        Lifetime::Scoped => "It is lent to a callback and valid only while the callback runs.",
    }
}

struct Method {
    doc: String,
    head: String,
    params: Vec<String>,
    body: Vec<String>,
}

fn render(method: &Method) -> String {
    let mut out = method.doc.clone();
    let single = format!("{INDENT}{}({}) {{", method.head, method.params.join(", "));
    if single.len() <= WIDTH {
        out.push_str(&single);
    } else {
        out.push_str(&format!("{INDENT}{}(\n", method.head));
        out.push_str(&format!(
            "{INDENT}{INDENT}{INDENT}{}) {{",
            method.params.join(&format!(",\n{INDENT}{INDENT}{INDENT}"))
        ));
    }
    out.push('\n');
    for line in &method.body {
        match line.is_empty() {
            true => out.push('\n'),
            false => out.push_str(&format!("{INDENT}{INDENT}{line}\n")),
        }
    }
    out.push_str(&format!("{INDENT}}}"));
    out
}

#[derive(Default)]
struct Call {
    params: Vec<String>,
    names: Vec<String>,
    checks: Vec<String>,
    late: Vec<String>,
    arguments: Vec<String>,
    arena: bool,
    allocates: bool,
    fenced: Vec<String>,
}

impl Call {
    fn param(&mut self, ty: &str, name: &str) {
        self.params.push(format!("{ty} {name}"));
        self.names.push(name.to_owned());
    }

    fn require(&mut self, name: &str) {
        self.checks
            .push(format!("Objects.requireNonNull({name}, \"{name}\");"));
    }

    fn body(&self, tail: Vec<String>, after: Vec<String>) -> Vec<String> {
        let mut lines = self.checks.clone();
        let head = match self.arena {
            true => "try (Arena arena = Arena.ofConfined()) {",
            false => "try {",
        };
        lines.push(head.to_owned());
        for line in self.late.iter().chain(&tail) {
            lines.push(format!("{INDENT}{line}"));
        }
        lines.push("} catch (Throwable failure) {".to_owned());
        lines.push(format!("{INDENT}throw Native.rethrow(failure);"));
        if self.fenced.is_empty() {
            lines.push("}".to_owned());
        } else {
            lines.push("} finally {".to_owned());
            for fenced in &self.fenced {
                lines.push(format!("{INDENT}Reference.reachabilityFence({fenced});"));
            }
            lines.push("}".to_owned());
        }
        lines.extend(after);
        lines
    }
}

enum Receiver {
    Handle(Ownership),
    Lent,
    Current,
}

fn call(spelling: &Spelling, function: &Function, receiver: Option<Receiver>) -> Call {
    let api = spelling.api;
    let mut call = Call {
        allocates: matches!(
            function.returns(),
            Return::Str | Return::String | Return::Value(Value::Struct(_))
        ),
        ..Call::default()
    };
    call.arena = call.allocates;
    let bound = match receiver {
        None => None,
        Some(Receiver::Handle(Ownership::Owned)) => {
            call.late
                .push("MemorySegment self = handle.take();".to_owned());
            call.arguments.push("self".to_owned());
            None
        }
        Some(Receiver::Handle(Ownership::Borrowed)) => {
            call.fenced.push("this".to_owned());
            Some("handle.raw()")
        }
        Some(Receiver::Lent) => Some("handle()"),
        Some(Receiver::Current) => Some("view.current(serial)"),
    };
    call.arguments.extend(bound.map(str::to_owned));
    for param in &function.signature.params {
        let name = camel(&param.name);
        match &param.ty {
            Input::Value(Value::Primitive(Primitive::Usize)) => {
                call.param("int", &name);
                call.arguments.push(format!("(long) {name}"));
            }
            Input::Value(Value::Primitive(primitive)) => {
                call.param(spelling.primitive(*primitive), &name);
                call.arguments.push(name);
            }
            Input::Value(value @ Value::Struct(id)) => {
                call.param(&spelling.value(*value), &name);
                call.require(&name);
                call.arena = true;
                call.arguments
                    .push(format!("Native.{}(arena, {name})", camel(&api[*id].name)));
            }
            Input::Value(value @ Value::Flags(id)) => {
                call.param(&spelling.value(*value), &name);
                call.require(&name);
                call.arguments
                    .push(format!("Native.{}({name})", camel(&api[*id].name)));
            }
            Input::Value(value) => {
                call.param(&spelling.value(*value), &name);
                call.require(&name);
                call.arguments.push(format!("{name}.value()"));
            }
            Input::String => {
                call.param("String", &name);
                call.require(&name);
                call.arena = true;
                call.arguments.push(format!("Native.str(arena, {name})"));
            }
            Input::Object {
                class,
                ownership: Ownership::Borrowed,
            } => {
                call.param(&spelling.class(*class), &name);
                call.require(&name);
                call.arguments.push(format!("{name}.handle.raw()"));
                call.fenced.push(name);
            }
            Input::Object {
                class,
                ownership: Ownership::Owned,
            } => {
                call.param(&spelling.class(*class), &name);
                call.require(&name);
                call.late
                    .push(format!("MemorySegment {name}Raw = {name}.handle.take();"));
                call.arguments.push(format!("{name}Raw"));
            }
            Input::Array(Element::Primitive(primitive)) => {
                call.param(&format!("{}[]", spelling.primitive(*primitive)), &name);
                call.require(&name);
                call.arena = true;
                call.arguments.push(format!(
                    "arena.allocateFrom(ValueLayout.{}, {name})",
                    spelling.layout(*primitive)
                ));
                call.arguments.push(format!("(long) {name}.length"));
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
                    call.param(&spelling.interface(part.callback), part_name);
                    call.require(part_name);
                }
                call.late.push(format!(
                    "MemorySegment userdata = Handle.retain({});",
                    names.join(", ")
                ));
                for (position, part) in closure.functions.iter().enumerate() {
                    call.arguments.push(format!(
                        "Trampolines.{}",
                        stub(
                            spelling,
                            Trampoline {
                                callback: part.callback,
                                position,
                            }
                        )
                    ));
                }
                call.arguments.push("userdata".to_owned());
                call.arguments.push("Trampolines.DESTROY".to_owned());
            }
        }
    }
    if let Some(receiver) = bound
        && !call.late.is_empty()
    {
        call.arguments[0] = "self".to_owned();
        call.late
            .insert(0, format!("MemorySegment self = {receiver};"));
    }
    call
}

fn invoke(spelling: &Spelling, function: &Function, call: &Call) -> String {
    let arguments: Vec<String> = call
        .allocates
        .then(|| "(SegmentAllocator) arena".to_owned())
        .into_iter()
        .chain(call.arguments.iter().cloned())
        .collect();
    format!(
        "Native.{}.invokeExact({})",
        spelling.handle(&function.name),
        arguments.join(", ")
    )
}

fn result(spelling: &Spelling, function: &Function, call: &Call) -> (String, Vec<String>) {
    let api = spelling.api;
    let expression = invoke(spelling, function, call);
    if function.signature.throws.is_some() {
        return (
            "void".to_owned(),
            vec![format!("Native.check((int) {expression});")],
        );
    }
    let value = |ty: String, converted: String| (ty, vec![format!("return {converted};")]);
    match (function.returns(), function.signature.returns) {
        (Return::Void, _) => ("void".to_owned(), vec![format!("{expression};")]),
        (Return::Value(Value::Primitive(Primitive::Usize)), _) => value(
            "int".to_owned(),
            format!("Math.toIntExact((long) {expression})"),
        ),
        (Return::Value(Value::Primitive(primitive)), _) => value(
            spelling.primitive(primitive).to_owned(),
            format!("({}) {expression}", spelling.carrier(primitive)),
        ),
        (Return::Value(found @ Value::Struct(id)), _) => value(
            spelling.value(found),
            format!(
                "Native.{}((MemorySegment) {expression})",
                camel(&api[id].name)
            ),
        ),
        (Return::Value(found @ Value::Flags(id)), _) => value(
            spelling.value(found),
            format!("Native.{}((int) {expression})", camel(&api[id].name)),
        ),
        (Return::Value(found @ Value::Enum(_)), _) => {
            let ty = spelling.value(found);
            value(ty.clone(), format!("{ty}.of((int) {expression})"))
        }
        (Return::Str, _) => value(
            "String".to_owned(),
            format!("Native.read((MemorySegment) {expression})"),
        ),
        (Return::String, _) => value(
            "String".to_owned(),
            format!("Native.take((MemorySegment) {expression})"),
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
                spelling.handle(&abi::name(api, Origin::Lifecycle(class, Lifecycle::Ref)));
            value(
                ty.clone(),
                format!(
                    "new {ty}((MemorySegment) Native.{reference}.invokeExact((MemorySegment) {expression}))"
                ),
            )
        }
        (Return::Object(class), _) => {
            let ty = spelling.class(class);
            value(
                ty.clone(),
                format!("new {ty}((MemorySegment) {expression})"),
            )
        }
    }
}

enum Kind {
    Handle {
        release: String,
        bound: bool,
        counted: bool,
    },
    Lent {
        end: Option<String>,
    },
}

#[derive(Template)]
#[template(path = "java/Class.java.j2", escape = "none")]
struct ClassFile<'a> {
    header: &'a Header,
    imports: Vec<&'static str>,
    doc: String,
    name: String,
    kind: Kind,
    members: Vec<String>,
}

fn class<'a>(
    spelling: &Spelling,
    id: Id<Class>,
    items: &[Item],
    declaring: bool,
    header: &'a Header,
) -> ClassFile<'a> {
    let api = spelling.api;
    let owner = &api[id];
    let name = spelling.class(id);
    let mut lines = spelling.body(&owner.doc.body);
    lines.push(lifetime_note(owner.lifetime).to_owned());
    let kind = match projection::release(owner) {
        Some(lifecycle) => Kind::Handle {
            release: spelling.handle(&abi::name(api, Origin::Lifecycle(id, lifecycle))),
            bound: owner.lifetime == Lifetime::Local,
            counted: owner.lifetime != Lifetime::Owned,
        },
        None => Kind::Lent {
            end: declaring.then(|| spelling.handle(&abi::name(api, Origin::End(id)))),
        },
    };
    let receiver = |ownership| match kind {
        Kind::Handle { .. } => Receiver::Handle(ownership),
        Kind::Lent { .. } => Receiver::Lent,
    };
    let mut members = Vec::new();
    for (index, item) in items.iter().enumerate() {
        let member = spelling.member(id, index);
        let methods: Vec<Method> = match item {
            Item::Constructor {
                function,
                constructor,
            } => {
                let call = call(spelling, function, None);
                let expression = invoke(spelling, function, &call);
                match constructor.name {
                    None => vec![
                        Method {
                            doc: described(spelling, function),
                            head: format!("public {name}"),
                            params: call.params.clone(),
                            body: vec![format!("this({CONSTRUCT}({}));", call.names.join(", "))],
                        },
                        Method {
                            doc: String::new(),
                            head: format!("private static MemorySegment {CONSTRUCT}"),
                            params: call.params.clone(),
                            body: call.body(
                                vec![format!("return (MemorySegment) {expression};")],
                                Vec::new(),
                            ),
                        },
                    ],
                    Some(_) => vec![Method {
                        doc: described(spelling, function),
                        head: format!("public static {name} {member}"),
                        params: call.params.clone(),
                        body: call.body(
                            vec![format!("return new {name}((MemorySegment) {expression});")],
                            Vec::new(),
                        ),
                    }],
                }
            }
            Item::Method {
                function,
                method,
                chained,
            } => {
                let call = call(spelling, function, Some(receiver(method.receiver)));
                let (returns, tail) = result(spelling, function, &call);
                let (returns, after) = match chained {
                    true => (name.clone(), vec!["return this;".to_owned()]),
                    false => (returns, Vec::new()),
                };
                vec![Method {
                    doc: described(spelling, function),
                    head: format!("public {returns} {member}"),
                    params: call.params.clone(),
                    body: call.body(tail, after),
                }]
            }
            Item::Property { getter, setter } => {
                let read = call(spelling, getter, Some(receiver(Ownership::Borrowed)));
                let (returns, tail) = result(spelling, getter, &read);
                let mut methods = vec![Method {
                    doc: described(spelling, getter),
                    head: format!("public {returns} {member}"),
                    params: Vec::new(),
                    body: read.body(tail, Vec::new()),
                }];
                if let Some(setter) = setter {
                    let write = call(spelling, setter, Some(receiver(Ownership::Borrowed)));
                    let (returns, tail) = result(spelling, setter, &write);
                    methods.push(Method {
                        doc: described(spelling, setter),
                        head: format!("public {returns} {}", spelling.setter(id, index)),
                        params: write.params.clone(),
                        body: write.body(tail, Vec::new()),
                    });
                }
                methods
            }
            Item::Declaration {
                function,
                declaration,
            } => {
                let mut call = call(spelling, function, Some(Receiver::Lent));
                let (_, tail) = result(spelling, function, &call);
                match declaration.kind {
                    DeclarationKind::Container => {
                        call.param("Runnable", BODY);
                        call.require(BODY);
                        vec![Method {
                            doc: docs(
                                spelling,
                                projection::docs(api, function, spelling)
                                    .with_param(BODY, "declares the children"),
                            ),
                            head: format!("public {ELEMENT} {member}"),
                            params: call.params.clone(),
                            body: call.body(tail, vec![format!("return contain({BODY});")]),
                        }]
                    }
                    DeclarationKind::Element => vec![Method {
                        doc: described(spelling, function),
                        head: format!("public {ELEMENT} {member}"),
                        params: call.params.clone(),
                        body: call.body(tail, vec!["return declared();".to_owned()]),
                    }],
                    DeclarationKind::Modifier => Vec::new(),
                }
            }
        };
        members.extend(methods.iter().map(render));
    }
    ClassFile {
        header,
        imports: imports(&members),
        doc: javadoc("", &lines),
        name,
        kind,
        members,
    }
}

#[derive(Template)]
#[template(path = "java/Element.java.j2", escape = "none")]
struct ElementFile<'a> {
    header: &'a Header,
    imports: Vec<&'static str>,
    view: String,
    modifiers: Vec<String>,
}

fn modifiers(spelling: &Spelling, class: Id<Class>, items: &[Item]) -> Vec<String> {
    let api = spelling.api;
    let mut out = Vec::new();
    for (index, function) in projection::modifiers(items) {
        let call = call(spelling, function, Some(Receiver::Current));
        let (_, tail) = result(spelling, function, &call);
        out.push(render(&Method {
            doc: docs(
                spelling,
                projection::docs(api, function, spelling).with_returns("this element"),
            ),
            head: format!("public {ELEMENT} {}", spelling.member(class, index)),
            params: call.params.clone(),
            body: call.body(tail, vec!["return this;".to_owned()]),
        }));
    }
    out
}

fn render_trampoline(spelling: &Spelling, trampoline: Trampoline) -> String {
    let api = spelling.api;
    let callback = &api[trampoline.callback];
    let params: Vec<String> = abi::callback_slots(callback)
        .iter()
        .map(|slot| format!("{} {}", spelling.slot_carrier(slot), camel(&slot.name)))
        .collect();
    let mut prelude = vec!["Handle.drain();".to_owned()];
    let mut arguments = Vec::new();
    let mut lent = Vec::new();
    for param in &callback.params {
        let name = camel(&param.name);
        let count = || camel(&abi::count_name(&param.name));
        match &param.ty {
            Input::Object { class, .. } => {
                let ty = spelling.class(*class);
                prelude.push(format!("{ty} {name}Lent = new {ty}({name});"));
                arguments.push(format!("{name}Lent"));
                lent.push(format!("{name}Lent.expire();"));
            }
            Input::Value(Value::Primitive(Primitive::Usize)) => {
                arguments.push(format!("Math.toIntExact({name})"));
            }
            Input::Value(Value::Primitive(_)) => arguments.push(name),
            Input::Value(Value::Struct(id)) => {
                arguments.push(format!("Native.{}({name})", camel(&api[*id].name)));
            }
            Input::Value(Value::Flags(id)) => {
                arguments.push(format!("Native.{}({name})", camel(&api[*id].name)));
            }
            Input::Value(value) => {
                arguments.push(format!("{}.of({name})", spelling.value(*value)));
            }
            Input::String => arguments.push(format!("Native.read({name})")),
            Input::Array(Element::String) => {
                arguments.push(format!("Native.readAll({name}, {})", count()));
            }
            Input::Array(Element::Primitive(primitive)) => arguments.push(format!(
                "{name}.reinterpret({count} * ValueLayout.{layout}.byteSize()).toArray(ValueLayout.{layout})",
                count = count(),
                layout = spelling.layout(*primitive)
            )),
            Input::Closure(_) => unreachable!("validated: callbacks receive no closures"),
        }
    }
    let mut body = prelude;
    body.push("try {".to_owned());
    body.push(format!(
        "{INDENT}Handle.<{}>retained(userdata, {}).{}({});",
        spelling.interface(trampoline.callback),
        trampoline.position,
        invoked(callback.params.len()),
        arguments.join(", ")
    ));
    body.push("} catch (Throwable failure) {".to_owned());
    body.push(format!("{INDENT}Handle.fail(failure);"));
    if lent.is_empty() {
        body.push("}".to_owned());
    } else {
        body.push("} finally {".to_owned());
        body.extend(lent.iter().map(|line| format!("{INDENT}{line}")));
        body.push("}".to_owned());
    }
    render(&Method {
        doc: String::new(),
        head: format!(
            "static void {}",
            spelling.trampoline(trampoline.callback, trampoline.position)
        ),
        params,
        body,
    })
}

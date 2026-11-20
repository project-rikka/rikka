use crate::abi::{self, Function, Origin, Return};
use crate::languages::cpp::spelling::{ELEMENT, ERROR, Spelling, configures};
use crate::languages::doxygen;
use crate::model::{
    Callback, Class, Declaration, DeclarationKind, Element, Id, Input, Lifetime, Member, Output,
    Ownership, Threading, Value,
};
use crate::names::pascal;

const RECEIVER: &str = "raw_";
const BODY: &str = "body";
const BODY_TYPE: &str = "Body";

pub(super) struct Method {
    pub(super) doc: Vec<String>,
    pub(super) template: Vec<(String, String)>,
    pub(super) specifiers: String,
    pub(super) returns: Option<String>,
    pub(super) name: String,
    pub(super) params: Vec<String>,
    pub(super) qualifier: &'static str,
    pub(super) body: Vec<String>,
}

impl Method {
    fn template_head(&self) -> Option<String> {
        if self.template.is_empty() {
            return None;
        }
        let parameters = self
            .template
            .iter()
            .map(|(name, _)| format!("class {name}"))
            .collect::<Vec<_>>()
            .join(", ");
        let constraints = self
            .template
            .iter()
            .map(|(_, constraint)| constraint.clone())
            .collect::<Vec<_>>()
            .join(" && ");
        Some(format!(
            "template <{parameters}>\n    requires {constraints}"
        ))
    }

    pub(super) fn declaration(&self) -> String {
        let mut out = doxygen::block("    ", &self.doc);
        if let Some(head) = self.template_head() {
            for line in head.lines() {
                out.push_str(&format!("    {line}\n"));
            }
        }
        let returns = self
            .returns
            .as_ref()
            .map_or(String::new(), |returns| format!("{returns} "));
        out.push_str(&format!(
            "    {}{returns}{}({}){};",
            self.specifiers,
            self.name,
            self.params.join(", "),
            self.qualifier
        ));
        out
    }

    pub(super) fn definition(&self, owner: Option<&str>) -> String {
        let mut out = String::new();
        if let Some(head) = self.template_head() {
            out.push_str(&head);
            out.push('\n');
        }
        let returns = self
            .returns
            .as_ref()
            .map_or(String::new(), |returns| format!("{returns} "));
        let (specifiers, scope) = match owner {
            Some(owner) => (String::new(), format!("{owner}::")),
            None => (self.specifiers.clone(), String::new()),
        };
        out.push_str(&format!(
            "{specifiers}inline {returns}{scope}{}({}){} {{\n",
            self.name,
            self.params.join(", "),
            self.qualifier
        ));
        for line in &self.body {
            out.push_str(&format!("    {line}\n"));
        }
        out.push('}');
        out
    }
}

#[derive(Default)]
struct Invocation {
    template: Vec<(String, String)>,
    params: Vec<String>,
    forwards: Vec<String>,
    prelude: Vec<String>,
    arguments: Vec<String>,
}

pub(super) struct Members {
    pub(super) declarations: Vec<String>,
    pub(super) definitions: Vec<String>,
}

pub(super) fn class(spelling: &Spelling, abi: &abi::Abi, class: Id<Class>) -> Members {
    let mut methods = Vec::new();
    for function in &abi.functions {
        match function.origin {
            Origin::Member(owner, index) if owner == class => {
                if let Member::Declaration(Declaration {
                    kind: DeclarationKind::Modifier,
                    ..
                }) = spelling.api.member(owner, index)
                {
                    continue;
                }
                methods.extend(member(spelling, function, owner, index));
            }
            Origin::Setter(owner, index) if owner == class => {
                methods.push(setter(spelling, function, owner, index));
            }
            _ => {}
        }
    }
    render(&spelling.class(class), methods)
}

pub(super) fn element(spelling: &Spelling, abi: &abi::Abi) -> Members {
    let mut methods = Vec::new();
    for function in &abi.functions {
        if let Origin::Member(owner, index) = function.origin
            && let Member::Declaration(declaration) = spelling.api.member(owner, index)
            && declaration.kind == DeclarationKind::Modifier
        {
            let invocation = invocation(spelling, function);
            let expression = call(spelling, function, &invocation, RECEIVER);
            let mut body = invocation.prelude;
            body.push(format!("{expression};"));
            body.push("return *this;".to_owned());
            methods.push(Method {
                doc: docs(spelling, function),
                template: invocation.template,
                specifiers: String::new(),
                returns: Some(format!("{ELEMENT}&")),
                name: spelling.member(owner, index),
                params: invocation.params,
                qualifier: "",
                body,
            });
        }
    }
    render(ELEMENT, methods)
}

fn render(owner: &str, methods: Vec<Method>) -> Members {
    Members {
        declarations: methods.iter().map(Method::declaration).collect(),
        definitions: methods
            .iter()
            .map(|method| method.definition(Some(owner)))
            .collect(),
    }
}

fn call(spelling: &Spelling, function: &Function, invocation: &Invocation, raw: &str) -> String {
    let receiver = function.receiver.map(|receiver| match receiver.ownership {
        Ownership::Owned => format!("std::exchange({raw}, nullptr)"),
        Ownership::Borrowed => raw.to_owned(),
    });
    let arguments: Vec<String> = receiver
        .into_iter()
        .chain(invocation.arguments.iter().cloned())
        .collect();
    format!(
        "{}({})",
        spelling.c.abi_function(function),
        arguments.join(", ")
    )
}

fn invocation(spelling: &Spelling, function: &Function) -> Invocation {
    let mut invocation = Invocation::default();
    for param in &function.signature.params {
        let name = spelling.ident(&param.name);
        match &param.ty {
            Input::Value(Value::Primitive(primitive)) => {
                invocation
                    .params
                    .push(format!("{} {name}", spelling.primitive(*primitive)));
                invocation.forwards.push(name.clone());
                invocation.arguments.push(name);
            }
            Input::Value(value @ Value::Struct(_)) => {
                invocation
                    .params
                    .push(format!("const {}& {name}", spelling.value(*value)));
                invocation.forwards.push(name.clone());
                invocation
                    .arguments
                    .push(format!("detail::convert({name})"));
            }
            Input::Value(value) => {
                invocation
                    .params
                    .push(format!("{} {name}", spelling.value(*value)));
                invocation.forwards.push(name.clone());
                invocation
                    .arguments
                    .push(format!("static_cast<{}>({name})", spelling.c_value(*value)));
            }
            Input::String => {
                invocation.params.push(format!("std::string_view {name}"));
                invocation.forwards.push(name.clone());
                invocation.arguments.push(format!("detail::str({name})"));
            }
            Input::Object {
                class,
                ownership: Ownership::Borrowed,
            } => {
                invocation
                    .params
                    .push(format!("const {}& {name}", spelling.class(*class)));
                invocation.forwards.push(name.clone());
                invocation.arguments.push(format!("{name}.raw()"));
            }
            Input::Object {
                class,
                ownership: Ownership::Owned,
            } => {
                invocation
                    .params
                    .push(format!("{} {name}", spelling.class(*class)));
                invocation.forwards.push(format!("std::move({name})"));
                invocation.arguments.push(format!("{name}.release()"));
            }
            Input::Array(Element::Primitive(primitive)) => {
                invocation.params.push(format!(
                    "std::span<const {}> {name}",
                    spelling.primitive(*primitive)
                ));
                invocation.forwards.push(name.clone());
                invocation.arguments.push(format!("{name}.data()"));
                invocation.arguments.push(format!("{name}.size()"));
            }
            Input::Array(Element::String) => {
                unreachable!("validated: only callbacks receive string arrays")
            }
            Input::Closure(closure) => {
                let single = closure.functions.len() == 1;
                let mut types = Vec::new();
                let mut values = Vec::new();
                for part in &closure.functions {
                    let ty = match single {
                        true => "F".to_owned(),
                        false => format!("{}Fn", pascal(&part.name)),
                    };
                    let value = spelling.ident(&part.name);
                    invocation.template.push((
                        ty.clone(),
                        constraint(spelling, &ty, &spelling.api[part.callback]),
                    ));
                    invocation.params.push(format!("{ty}&& {value}"));
                    invocation
                        .forwards
                        .push(format!("std::forward<{ty}>({value})"));
                    values.push(format!("std::forward<{ty}>({value})"));
                    types.push(format!("std::decay_t<{ty}>"));
                }
                invocation
                    .prelude
                    .push(format!("using Pack = std::tuple<{}>;", types.join(", ")));
                invocation.prelude.push(format!(
                    "auto {} = std::make_unique<Pack>({});",
                    abi::USERDATA,
                    values.join(", ")
                ));
                for (position, part) in closure.functions.iter().enumerate() {
                    invocation.arguments.push(format!(
                        "detail::{}<Pack, {position}>",
                        spelling.trampoline(part.callback)
                    ));
                }
                invocation
                    .arguments
                    .push(format!("{}.release()", abi::USERDATA));
                invocation
                    .arguments
                    .push("detail::destroy<Pack>".to_owned());
            }
        }
    }
    invocation
}

fn constraint(spelling: &Spelling, ty: &str, callback: &Callback) -> String {
    let arguments: String = callback
        .params
        .iter()
        .map(|param| format!(", {}", spelling.argument(&param.ty)))
        .collect();
    format!("std::invocable<{ty}&{arguments}>")
}

fn result(spelling: &Spelling, function: &Function, expression: String) -> (String, String) {
    if function.signature.throws.is_some() {
        return ("void".to_owned(), format!("detail::check({expression});"));
    }
    match (function.returns(), function.signature.returns) {
        (Return::Void, _) => ("void".to_owned(), format!("{expression};")),
        (Return::Value(Value::Primitive(primitive)), _) => (
            spelling.primitive(primitive).to_owned(),
            format!("return {expression};"),
        ),
        (Return::Value(value @ Value::Struct(_)), _) => (
            spelling.value(value),
            format!("return detail::convert({expression});"),
        ),
        (Return::Value(value), _) => {
            let ty = spelling.value(value);
            (
                ty.clone(),
                format!("return static_cast<{ty}>({expression});"),
            )
        }
        (Return::Str, _) => (
            "std::string_view".to_owned(),
            format!("return detail::view({expression});"),
        ),
        (Return::String, _) => (
            "std::string".to_owned(),
            format!("return detail::take({expression});"),
        ),
        (
            Return::Object(class),
            Some(Output::Object {
                ownership: Ownership::Borrowed,
                ..
            }),
        ) => {
            let ty = spelling.class(class);
            let reference = spelling.c_function(Origin::Lifecycle(class, abi::Lifecycle::Ref));
            (
                ty.clone(),
                format!("return {ty}::adopt({reference}({expression}));"),
            )
        }
        (Return::Object(class), _) => {
            let ty = spelling.class(class);
            (ty.clone(), format!("return {ty}::adopt({expression});"))
        }
    }
}

fn nodiscard(spelling: &Spelling, function: &Function) -> bool {
    function.signature.throws.is_none() && function.must_use(spelling.api)
}

fn member(spelling: &Spelling, function: &Function, class: Id<Class>, index: usize) -> Vec<Method> {
    let api = spelling.api;
    let owner = &api[class];
    let name = spelling.member(class, index);
    let invocation = invocation(spelling, function);
    let expression = call(spelling, function, &invocation, RECEIVER);
    match api.member(class, index) {
        Member::Constructor(constructor) => {
            let doc = docs(spelling, function);
            match constructor.name {
                None => {
                    let mut body = invocation.prelude;
                    body.push(format!("{RECEIVER} = {expression};"));
                    vec![Method {
                        doc,
                        template: invocation.template,
                        specifiers: "explicit ".to_owned(),
                        returns: None,
                        name,
                        params: invocation.params,
                        qualifier: "",
                        body,
                    }]
                }
                Some(_) => {
                    let ty = spelling.class(class);
                    let mut body = invocation.prelude;
                    body.push(format!("return {ty}::adopt({expression});"));
                    vec![Method {
                        doc,
                        template: invocation.template,
                        specifiers: "[[nodiscard]] static ".to_owned(),
                        returns: Some(ty),
                        name,
                        params: invocation.params,
                        qualifier: "",
                        body,
                    }]
                }
            }
        }
        Member::Method(method) => {
            let doc = docs(spelling, function);
            let (returns, last) = result(spelling, function, expression);
            let mut body = invocation.prelude;
            body.push(last);
            if configures(owner, method) {
                let ty = spelling.class(class);
                body.push("return *this;".to_owned());
                let forwarded = Method {
                    doc: Vec::new(),
                    template: invocation.template.clone(),
                    specifiers: String::new(),
                    returns: Some(format!("{ty}&&")),
                    name: name.clone(),
                    params: invocation.params.clone(),
                    qualifier: " &&",
                    body: vec![format!(
                        "return std::move(this->{name}({}));",
                        invocation.forwards.join(", ")
                    )],
                };
                let lvalue = Method {
                    doc,
                    template: invocation.template,
                    specifiers: String::new(),
                    returns: Some(format!("{ty}&")),
                    name,
                    params: invocation.params,
                    qualifier: " &",
                    body,
                };
                return vec![lvalue, forwarded];
            }
            let specifiers = match nodiscard(spelling, function) {
                true => "[[nodiscard]] ".to_owned(),
                false => String::new(),
            };
            let qualifier = match method.receiver {
                Ownership::Owned => " &&",
                Ownership::Borrowed => "",
            };
            vec![Method {
                doc,
                template: invocation.template,
                specifiers,
                returns: Some(returns),
                name,
                params: invocation.params,
                qualifier,
                body,
            }]
        }
        Member::Property(_) => {
            let (returns, last) = result(spelling, function, expression);
            vec![Method {
                doc: docs(spelling, function),
                template: Vec::new(),
                specifiers: "[[nodiscard]] ".to_owned(),
                returns: Some(returns),
                name,
                params: Vec::new(),
                qualifier: " const",
                body: vec![last],
            }]
        }
        Member::Declaration(declaration) => {
            let mut doc = docs(spelling, function);
            let mut params = invocation.params;
            let mut template = invocation.template;
            let mut body = invocation.prelude;
            match declaration.kind {
                DeclarationKind::Container => {
                    template.push((
                        BODY_TYPE.to_owned(),
                        format!("std::invocable<{BODY_TYPE}&>"),
                    ));
                    params.push(format!("{BODY_TYPE}&& {BODY}"));
                    doc.push(format!("@param {BODY} Declares the children."));
                    body.push(format!("{expression};"));
                    body.push(format!("std::invoke({BODY});"));
                    body.push(format!(
                        "{}({RECEIVER});",
                        spelling.c_function(Origin::End(class))
                    ));
                }
                DeclarationKind::Element => body.push(format!("{expression};")),
                DeclarationKind::Modifier => unreachable!("modifiers belong to the element"),
            }
            body.push(format!("return {ELEMENT}({RECEIVER});"));
            vec![Method {
                doc,
                template,
                specifiers: String::new(),
                returns: Some(ELEMENT.to_owned()),
                name,
                params,
                qualifier: "",
                body,
            }]
        }
    }
}

fn setter(spelling: &Spelling, function: &Function, class: Id<Class>, index: usize) -> Method {
    let invocation = invocation(spelling, function);
    let expression = call(spelling, function, &invocation, RECEIVER);
    Method {
        doc: docs(spelling, function),
        template: Vec::new(),
        specifiers: String::new(),
        returns: Some("void".to_owned()),
        name: spelling.setter(class, index),
        params: invocation.params,
        qualifier: "",
        body: vec![format!("{expression};")],
    }
}

fn docs(spelling: &Spelling, function: &Function) -> Vec<String> {
    let api = spelling.api;
    let mut docs = spelling.body(function.description(api));
    if function.thread_safe {
        docs.push("Thread-safe.".to_owned());
    }
    for param in &function.signature.params {
        let text = spelling.text(&param.doc);
        match &param.ty {
            Input::Closure(closure) => {
                let thread = match closure.threading {
                    Threading::AnyThread => " It may run on any thread.",
                    Threading::ThisThread => "",
                };
                let single = closure.functions.len() == 1;
                for part in &closure.functions {
                    let described = spelling
                        .body(abi::closure_description(api, param, part, single))
                        .join(" ");
                    docs.push(format!(
                        "@param {} {described}{thread}",
                        spelling.ident(&part.name)
                    ));
                }
            }
            Input::Object {
                ownership: Ownership::Owned,
                ..
            } => {
                let described = [text.as_str(), "Ownership passes to the SDK."]
                    .into_iter()
                    .filter(|part| !part.is_empty())
                    .collect::<Vec<_>>()
                    .join(" ");
                docs.push(format!(
                    "@param {} {described}",
                    spelling.ident(&param.name)
                ));
            }
            _ if !text.is_empty() => {
                docs.push(format!("@param {} {text}", spelling.ident(&param.name)));
            }
            _ => {}
        }
    }
    let returns = function
        .doc(api)
        .map_or(String::new(), |doc| spelling.text(&doc.returns));
    if !returns.is_empty() {
        docs.push(format!("@return {returns}"));
    }
    if function.signature.throws.is_some() {
        docs.push(format!(
            "@throws {}::{ERROR} with the failing status.",
            spelling.namespace()
        ));
    }
    docs
}

pub(super) fn free(spelling: &Spelling, function: &Function) -> String {
    let Origin::Function(id) = function.origin else {
        unreachable!("free functions come from functions");
    };
    let invocation = invocation(spelling, function);
    let expression = call(spelling, function, &invocation, RECEIVER);
    let (returns, last) = result(spelling, function, expression);
    let mut body = invocation.prelude;
    body.push(last);
    let doc = docs(spelling, function);
    let specifiers = match nodiscard(spelling, function) {
        true => "[[nodiscard]] ",
        false => "",
    };
    let method = Method {
        doc: Vec::new(),
        template: invocation.template,
        specifiers: specifiers.to_owned(),
        returns: Some(returns),
        name: spelling.ident(&spelling.api[id].name),
        params: invocation.params,
        qualifier: "",
        body,
    };
    format!("{}{}", doxygen::block("", &doc), method.definition(None))
}

pub(super) fn lifetime_note(lifetime: Lifetime) -> &'static str {
    match lifetime {
        Lifetime::Owned => "Move-only; destroying it releases what it owns.",
        Lifetime::Shared => "Copies share one reference-counted object, usable from any thread.",
        Lifetime::Local => {
            "Copies share one reference-counted object, usable only on the thread that made it."
        }
        Lifetime::Scoped => "Lent to a callback; valid only while the callback runs.",
    }
}

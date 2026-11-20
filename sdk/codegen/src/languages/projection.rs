use crate::abi::{self, Abi, Function, Lifecycle, Origin};
use crate::diagnostic::{Diagnostic, Span};
use crate::languages::unique;
use crate::model::{
    Api, Callback, Class, Closure, Configuration, Constant, ConstantValue, Constructor,
    Declaration, DeclarationKind, Flags, Id, Input, Lifetime, Member, Method, Ownership, Paragraph,
    Primitive, Target, Threading,
};
use crate::names::pascal;

pub(crate) trait Linker {
    fn link(&self, target: Target) -> String;

    fn text(&self, paragraph: &Paragraph) -> String {
        paragraph.render(&|target| self.link(target))
    }

    fn body(&self, paragraphs: &[Paragraph]) -> Vec<String> {
        paragraphs
            .iter()
            .map(|paragraph| self.text(paragraph))
            .collect()
    }
}

pub(crate) enum Item<'a> {
    Constructor {
        function: &'a Function,
        constructor: &'a Constructor,
    },
    Method {
        function: &'a Function,
        method: &'a Method,
        chained: bool,
    },
    Property {
        getter: &'a Function,
        setter: Option<&'a Function>,
    },
    Declaration {
        function: &'a Function,
        declaration: &'a Declaration,
    },
}

pub(crate) fn items<'a>(api: &'a Api, abi: &'a Abi, class: Id<Class>) -> Vec<Item<'a>> {
    let owner = &api[class];
    let mut members = vec![None; owner.members.len()];
    let mut setters = vec![None; owner.members.len()];
    for function in &abi.functions {
        match function.origin {
            Origin::Member(id, index) if id == class => members[index] = Some(function),
            Origin::Setter(id, index) if id == class => setters[index] = Some(function),
            _ => {}
        }
    }
    owner
        .members
        .iter()
        .zip(members.into_iter().zip(setters))
        .map(|(member, (function, setter))| {
            let function = function.expect("every member lowers");
            match member {
                Member::Constructor(constructor) => Item::Constructor {
                    function,
                    constructor,
                },
                Member::Method(method) => Item::Method {
                    function,
                    method,
                    chained: chained(owner, method),
                },
                Member::Property(_) => Item::Property {
                    getter: function,
                    setter,
                },
                Member::Declaration(declaration) => Item::Declaration {
                    function,
                    declaration,
                },
            }
        })
        .collect()
}

pub(crate) fn chained(owner: &Class, method: &Method) -> bool {
    owner.configuration == Configuration::Builder
        && method.receiver == Ownership::Borrowed
        && method.signature.returns.is_none()
        && method.signature.throws.is_none()
}

pub(crate) const SETTER: &str = "set_";

const CALLBACK_SUFFIX: &str = "_fn";

pub(crate) fn callback_name(callback: &Callback) -> String {
    let stem = callback
        .name
        .strip_suffix(CALLBACK_SUFFIX)
        .unwrap_or(&callback.name);
    format!("{}Callback", pascal(stem))
}

pub(crate) fn builder_name(method: &Method, chained: bool) -> &str {
    match (chained, method.name.strip_prefix(SETTER)) {
        (true, Some(stripped)) => stripped,
        _ => &method.name,
    }
}

pub(crate) fn declaring(api: &Api) -> Option<Id<Class>> {
    Id::all(&api.classes).find(|id| {
        api[*id]
            .members
            .iter()
            .any(|member| matches!(member, Member::Declaration(_)))
    })
}

pub(crate) fn is_modifier(api: &Api, class: Id<Class>, index: usize) -> bool {
    matches!(
        api.member(class, index),
        Member::Declaration(Declaration {
            kind: DeclarationKind::Modifier,
            ..
        })
    )
}

pub(crate) fn modifiers<'a>(items: &'a [Item<'a>]) -> impl Iterator<Item = (usize, &'a Function)> {
    items
        .iter()
        .enumerate()
        .filter_map(|(index, item)| match item {
            Item::Declaration {
                function,
                declaration,
            } if declaration.kind == DeclarationKind::Modifier => Some((index, *function)),
            _ => None,
        })
}

pub(crate) fn release(class: &Class) -> Option<Lifecycle> {
    match class.lifetime {
        Lifetime::Owned => Some(Lifecycle::Free),
        Lifetime::Shared | Lifetime::Local => Some(Lifecycle::Unref),
        Lifetime::Scoped => None,
    }
}

pub(crate) fn closure(function: &Function) -> Option<&Closure> {
    function
        .signature
        .params
        .iter()
        .find_map(|param| match &param.ty {
            Input::Closure(closure) => Some(closure),
            _ => None,
        })
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct Trampoline {
    pub(crate) callback: Id<Callback>,
    pub(crate) position: usize,
}

pub(crate) fn trampolines(api: &Api, abi: &Abi) -> Vec<Trampoline> {
    let mut used = Vec::new();
    for function in &abi.functions {
        if let Some(closure) = closure(function) {
            for (position, part) in closure.functions.iter().enumerate() {
                let trampoline = Trampoline {
                    callback: part.callback,
                    position,
                };
                if !used.contains(&trampoline) {
                    used.push(trampoline);
                }
            }
        }
    }
    let mut ordered = Vec::new();
    for callback in Id::all(&api.callbacks) {
        let mut positions: Vec<usize> = used
            .iter()
            .filter(|trampoline| trampoline.callback == callback)
            .map(|trampoline| trampoline.position)
            .collect();
        positions.sort_unstable();
        ordered.extend(
            positions
                .into_iter()
                .map(|position| Trampoline { callback, position }),
        );
    }
    ordered
}

pub(crate) struct Docs {
    pub(crate) body: Vec<String>,
    pub(crate) params: Vec<(String, String)>,
    pub(crate) returns: String,
    pub(crate) throws: bool,
}

impl Docs {
    pub(crate) fn with_paragraph(mut self, paragraph: impl Into<String>) -> Self {
        self.body.push(paragraph.into());
        self
    }

    pub(crate) fn with_param(mut self, name: impl Into<String>, text: impl Into<String>) -> Self {
        self.params.push((name.into(), text.into()));
        self
    }

    pub(crate) fn with_returns(mut self, returns: impl Into<String>) -> Self {
        self.returns = returns.into();
        self
    }
}

pub(crate) fn docs(api: &Api, function: &Function, linker: &dyn Linker) -> Docs {
    let mut body = linker.body(function.description(api));
    if function.thread_safe {
        body.push("Thread-safe.".to_owned());
    }
    let mut params = Vec::new();
    for param in &function.signature.params {
        match &param.ty {
            Input::Closure(closure) => {
                let thread = match closure.threading {
                    Threading::AnyThread => " It may run on any thread.",
                    Threading::ThisThread => "",
                };
                let single = closure.functions.len() == 1;
                for part in &closure.functions {
                    let described = linker.body(abi::closure_description(api, param, part, single));
                    params.push((
                        part.name.clone(),
                        format!("{}{thread}", described.join(" ")),
                    ));
                }
            }
            Input::Object {
                ownership: Ownership::Owned,
                ..
            } => {
                let described = [linker.text(&param.doc), "It is consumed.".to_owned()]
                    .into_iter()
                    .filter(|part| !part.is_empty())
                    .collect::<Vec<_>>()
                    .join(" ");
                params.push((param.name.clone(), described));
            }
            _ => params.push((param.name.clone(), linker.text(&param.doc))),
        }
    }
    let returns = function
        .doc(api)
        .map_or(String::new(), |doc| linker.text(&doc.returns));
    Docs {
        body,
        params,
        returns,
        throws: function.signature.throws.is_some(),
    }
}

pub(crate) fn callback_docs(callback: &Callback, linker: &dyn Linker) -> Docs {
    Docs {
        body: linker.body(&callback.doc.body),
        params: callback
            .params
            .iter()
            .map(|param| (param.name.clone(), linker.text(&param.doc)))
            .collect(),
        returns: String::new(),
        throws: false,
    }
}

pub(crate) fn summary(api: &Api, linker: &dyn Linker) -> String {
    let first = api
        .doc
        .body
        .first()
        .map(|paragraph| linker.text(paragraph))
        .unwrap_or_default();
    first
        .split(". ")
        .next()
        .unwrap_or_default()
        .trim_end_matches('.')
        .to_owned()
}

pub(crate) fn soname(api: &Api) -> String {
    format!("lib{}.so.{}", api.name, api.version.major)
}

pub(crate) fn single_error(api: &Api, mapped: &str) -> Result<(), Diagnostic> {
    let errors: Vec<_> = api
        .enums
        .iter()
        .filter(|item| item.error.is_some())
        .collect();
    match errors.as_slice() {
        [_, second, ..] => Err(Diagnostic::new(
            second.span,
            format!(
                "the binding maps one `@error` enum to `{mapped}`; `{}` is a second one",
                second.name
            ),
        )),
        _ => Ok(()),
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum Literal {
    Integer(i128),
    Float(f64),
}

pub(crate) fn literal(constant: &Constant) -> Literal {
    match (constant.value, constant.ty) {
        (ConstantValue::Maximum, Primitive::Usize) => Literal::Integer(-1),
        (ConstantValue::Maximum, primitive) => {
            Literal::Integer(primitive.integer_range().map_or(0, |(_, maximum)| maximum))
        }
        (ConstantValue::Integer(value), _) => Literal::Integer(value),
        (ConstantValue::Float(value), _) => Literal::Float(value),
    }
}

pub(crate) struct Vocabulary {
    pub(crate) language: &'static str,
    pub(crate) type_noun: &'static str,
    pub(crate) member_noun: &'static str,
    pub(crate) error: String,
    pub(crate) reserved_types: Vec<String>,
    pub(crate) reserved_members: &'static [&'static str],
}

pub(crate) trait Naming {
    fn flags(&self, flags: Id<Flags>) -> String;

    fn callback(&self, callback: Id<Callback>) -> String;

    fn members(&self, class: Id<Class>, index: usize) -> Result<Vec<String>, Diagnostic>;
}

pub(crate) fn validate(
    api: &Api,
    vocabulary: Vocabulary,
    naming: &dyn Naming,
) -> Result<(), Diagnostic> {
    let Vocabulary {
        language,
        type_noun,
        member_noun,
        error,
        reserved_types,
        reserved_members,
    } = vocabulary;
    single_error(api, &error)?;
    let declared = |name: &str, span: Span| (pascal(name), span);
    let types = reserved_types
        .into_iter()
        .map(|name| (name, Span::default()))
        .chain(
            api.structs
                .iter()
                .map(|item| declared(&item.name, item.span)),
        )
        .chain(api.enums.iter().map(|item| declared(&item.name, item.span)))
        .chain(Id::all(&api.flags).map(|id| (naming.flags(id), api[id].span)))
        .chain(
            api.classes
                .iter()
                .map(|item| declared(&item.name, item.span)),
        )
        .chain(Id::all(&api.callbacks).map(|id| (naming.callback(id), api[id].span)));
    unique(types, |name| {
        format!("this becomes the {language} {type_noun} `{name}`, which is taken")
    })?;
    for class in Id::all(&api.classes) {
        let mut names: Vec<(String, Span)> = reserved_members
            .iter()
            .map(|name| ((*name).to_owned(), Span::default()))
            .collect();
        for (index, member) in api[class].members.iter().enumerate() {
            if is_modifier(api, class, index) {
                continue;
            }
            let claimed = naming.members(class, index)?;
            names.extend(claimed.into_iter().map(|name| (name, member.span())));
        }
        unique(names, |name| {
            format!(
                "this becomes the {language} {member_noun} `{name}`, which another member also does"
            )
        })?;
    }
    Ok(())
}

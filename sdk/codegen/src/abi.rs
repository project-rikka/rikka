use std::collections::BTreeSet;

use crate::diagnostic::{Diagnostic, Result, Span};
use crate::model::{
    Access, Api, Callback, Class, ClosureFunction, Declaration, DeclarationKind, Doc, Element,
    Enum, Function as Free, Id, Input, Lifetime, Member, Output, Ownership, Paragraph, Param,
    Signature, Value,
};

pub(crate) const USERDATA: &str = "userdata";
pub(crate) const DESTROY: &str = "destroy";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Origin {
    Function(Id<Free>),
    Member(Id<Class>, usize),
    Setter(Id<Class>, usize),
    End(Id<Class>),
    Lifecycle(Id<Class>, Lifecycle),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Lifecycle {
    Free,
    Ref,
    Unref,
}

impl Lifecycle {
    pub(crate) fn suffix(self) -> &'static str {
        match self {
            Self::Free => "free",
            Self::Ref => "ref",
            Self::Unref => "unref",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Receiver {
    pub(crate) class: Id<Class>,
    pub(crate) ownership: Ownership,
}

#[derive(Clone, Debug)]
pub(crate) struct Function {
    pub(crate) name: String,
    pub(crate) origin: Origin,
    pub(crate) receiver: Option<Receiver>,
    pub(crate) signature: Signature,
    pub(crate) thread_safe: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct Slot {
    pub(crate) name: String,
    pub(crate) ty: SlotType,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SlotType {
    Value(Value),
    Str,
    Object(Id<Class>),
    Pointer(Element),
    Count,
    Callback(Id<Callback>),
    Userdata,
    Destroy,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Return {
    Void,
    Value(Value),
    Str,
    String,
    Object(Id<Class>),
}

pub(crate) struct Abi {
    pub(crate) functions: Vec<Function>,
}

impl Abi {
    pub(crate) fn new(api: &Api) -> Result<Self> {
        let mut functions = Vec::new();
        for id in Id::all(&api.functions) {
            let function = &api[id];
            functions.push(Function {
                name: function.name.clone(),
                origin: Origin::Function(id),
                receiver: None,
                signature: function.signature.clone(),
                thread_safe: function.thread_safe,
            });
        }
        for class in Id::all(&api.classes) {
            for index in 0..api[class].members.len() {
                functions.extend(members(api, class, index));
            }
            if api[class].has_containers() {
                functions.push(Function {
                    name: end_name(),
                    origin: Origin::End(class),
                    receiver: Some(borrowed(class)),
                    signature: empty(None),
                    thread_safe: false,
                });
            }
            functions.extend(lifecycle(api, class));
        }
        let abi = Self { functions };
        abi.validate(api)?;
        Ok(abi)
    }

    fn validate(&self, api: &Api) -> Result<()> {
        let mut names = BTreeSet::new();
        for function in &self.functions {
            let span = function.span(api);
            if !names.insert(function.name.as_str()) {
                return Err(Diagnostic::new(
                    span,
                    format!(
                        "this lowers to `{}`, which another declaration also does",
                        function.name
                    ),
                ));
            }
            let mut slots = BTreeSet::new();
            for slot in function.slots(api) {
                if !slots.insert(slot.name.clone()) {
                    return Err(Diagnostic::new(
                        span,
                        format!(
                            "the ABI function `{}` would have two parameters named `{}`",
                            function.name, slot.name
                        ),
                    ));
                }
            }
        }
        for callback in &api.callbacks {
            let mut slots = BTreeSet::new();
            for slot in callback_slots(callback) {
                if !slots.insert(slot.name.clone()) {
                    let span = callback
                        .params
                        .iter()
                        .find(|param| param.name == slot.name)
                        .map_or(Span::default(), |param| param.span);
                    return Err(Diagnostic::new(
                        span,
                        format!(
                            "the callback `{}` would have two parameters named `{}`",
                            callback.name, slot.name
                        ),
                    ));
                }
            }
        }
        Ok(())
    }
}

impl Function {
    pub(crate) fn span(&self, api: &Api) -> Span {
        match self.origin {
            Origin::Function(id) => api[id].span,
            Origin::Member(class, index) | Origin::Setter(class, index) => {
                api.member(class, index).span()
            }
            Origin::End(class) | Origin::Lifecycle(class, _) => api[class].span,
        }
    }

    pub(crate) fn slots(&self, api: &Api) -> Vec<Slot> {
        self.located_slots(api)
            .into_iter()
            .map(|(slot, _)| slot)
            .collect()
    }

    pub(crate) fn located_slots(&self, api: &Api) -> Vec<(Slot, Span)> {
        let receiver = self.receiver.map(|receiver| {
            let slot = Slot {
                name: receiver_name(&api[receiver.class]),
                ty: SlotType::Object(receiver.class),
            };
            (slot, self.span(api))
        });
        let params = self.signature.params.iter().flat_map(|param| {
            param_slots(param)
                .into_iter()
                .map(move |slot| (slot, param.span))
        });
        receiver.into_iter().chain(params).collect()
    }

    pub(crate) fn returns(&self) -> Return {
        match (self.signature.throws, self.signature.returns) {
            (Some(error), _) => Return::Value(Value::Enum(error)),
            (None, None) => Return::Void,
            (None, Some(Output::Value(value))) => Return::Value(value),
            (None, Some(Output::String(Ownership::Borrowed))) => Return::Str,
            (None, Some(Output::String(Ownership::Owned))) => Return::String,
            (None, Some(Output::Object { class, .. })) => Return::Object(class),
        }
    }

    pub(crate) fn returned_error(&self, api: &Api) -> Option<Id<Enum>> {
        match self.returns() {
            Return::Value(Value::Enum(id)) if api[id].error.is_some() => Some(id),
            _ => None,
        }
    }

    pub(crate) fn must_use(&self, api: &Api) -> bool {
        match (self.origin, self.signature.returns) {
            (Origin::Lifecycle(..), _) => false,
            (
                _,
                Some(
                    Output::String(Ownership::Owned)
                    | Output::Object {
                        ownership: Ownership::Owned,
                        ..
                    },
                ),
            ) => true,
            _ => self.returned_error(api).is_some(),
        }
    }

    pub(crate) fn doc<'a>(&self, api: &'a Api) -> Option<&'a Doc> {
        match self.origin {
            Origin::Function(id) => Some(&api[id].doc),
            Origin::Member(class, index) | Origin::Setter(class, index) => {
                Some(match api.member(class, index) {
                    Member::Constructor(member) => &member.doc,
                    Member::Method(member) => &member.doc,
                    Member::Property(member) => &member.doc,
                    Member::Declaration(member) => &member.doc,
                })
            }
            Origin::End(_) | Origin::Lifecycle(..) => None,
        }
    }

    pub(crate) fn description<'a>(&self, api: &'a Api) -> &'a [Paragraph] {
        match (self.origin, self.doc(api)) {
            (Origin::Setter(..), Some(doc)) if !doc.set.is_empty() => {
                std::slice::from_ref(&doc.set)
            }
            (_, Some(doc)) => &doc.body,
            (_, None) => &[],
        }
    }
}

pub(crate) fn closure_description<'a>(
    api: &'a Api,
    param: &'a Param,
    part: &ClosureFunction,
    single: bool,
) -> &'a [Paragraph] {
    match single && !param.doc.is_empty() {
        true => std::slice::from_ref(&param.doc),
        false => &api[part.callback].doc.body,
    }
}

pub(crate) fn receiver_name(class: &Class) -> String {
    class
        .name
        .rsplit('_')
        .next()
        .expect("split yields at least one part")
        .to_owned()
}

pub(crate) fn count_name(array: &str) -> String {
    format!("{array}_count")
}

pub(crate) fn param_slots(param: &Param) -> Vec<Slot> {
    let slot = |name: &str, ty| Slot {
        name: name.to_owned(),
        ty,
    };
    match &param.ty {
        Input::Value(value) => vec![slot(&param.name, SlotType::Value(*value))],
        Input::String => vec![slot(&param.name, SlotType::Str)],
        Input::Object { class, .. } => vec![slot(&param.name, SlotType::Object(*class))],
        Input::Array(element) => vec![
            slot(&param.name, SlotType::Pointer(*element)),
            slot(&count_name(&param.name), SlotType::Count),
        ],
        Input::Closure(closure) => closure
            .functions
            .iter()
            .map(|function| slot(&function.name, SlotType::Callback(function.callback)))
            .chain([
                slot(USERDATA, SlotType::Userdata),
                slot(DESTROY, SlotType::Destroy),
            ])
            .collect(),
    }
}

pub(crate) fn callback_slots(callback: &Callback) -> Vec<Slot> {
    callback
        .params
        .iter()
        .flat_map(param_slots)
        .chain([Slot {
            name: USERDATA.to_owned(),
            ty: SlotType::Userdata,
        }])
        .collect()
}

pub(crate) fn name(api: &Api, origin: Origin) -> String {
    match origin {
        Origin::Function(id) => api[id].name.clone(),
        Origin::Member(class, index) => {
            let owner = &api[class].name;
            match api.member(class, index) {
                Member::Constructor(constructor) => match &constructor.name {
                    None => format!("{owner}_new"),
                    Some(name) => format!("{owner}_new_{name}"),
                },
                Member::Method(method) => format!("{owner}_{}", method.name),
                Member::Property(property) => format!("{owner}_{}", property.name),
                Member::Declaration(Declaration {
                    kind: DeclarationKind::Container,
                    name,
                    ..
                }) => format!("{name}_begin"),
                Member::Declaration(declaration) => declaration.name.clone(),
            }
        }
        Origin::Setter(class, index) => match api.member(class, index) {
            Member::Property(property) => format!("{}_set_{}", api[class].name, property.name),
            _ => unreachable!("only properties have setters"),
        },
        Origin::End(_) => end_name(),
        Origin::Lifecycle(class, lifecycle) => {
            format!("{}_{}", api[class].name, lifecycle.suffix())
        }
    }
}

fn end_name() -> String {
    "end".to_owned()
}

fn borrowed(class: Id<Class>) -> Receiver {
    Receiver {
        class,
        ownership: Ownership::Borrowed,
    }
}

fn empty(returns: Option<Output>) -> Signature {
    Signature {
        params: Vec::new(),
        returns,
        throws: None,
    }
}

fn members(api: &Api, class: Id<Class>, index: usize) -> Vec<Function> {
    let origin = Origin::Member(class, index);
    let named = |origin| name(api, origin);
    match api.member(class, index) {
        Member::Constructor(constructor) => vec![Function {
            name: named(origin),
            origin,
            receiver: None,
            signature: Signature {
                params: constructor.params.clone(),
                returns: Some(Output::Object {
                    class,
                    ownership: Ownership::Owned,
                }),
                throws: None,
            },
            thread_safe: constructor.thread_safe,
        }],
        Member::Method(method) => vec![Function {
            name: named(origin),
            origin,
            receiver: Some(Receiver {
                class,
                ownership: method.receiver,
            }),
            signature: method.signature.clone(),
            thread_safe: method.thread_safe,
        }],
        Member::Property(property) => {
            let getter = Function {
                name: named(origin),
                origin,
                receiver: Some(borrowed(class)),
                signature: empty(Some(property.ty)),
                thread_safe: property.thread_safe,
            };
            let setter = (property.access == Access::ReadWrite).then(|| {
                let input = match property.ty {
                    Output::Value(value) => Input::Value(value),
                    Output::String(_) => Input::String,
                    Output::Object { .. } => {
                        unreachable!("validated: writable properties hold values")
                    }
                };
                Function {
                    name: named(Origin::Setter(class, index)),
                    origin: Origin::Setter(class, index),
                    receiver: Some(borrowed(class)),
                    signature: Signature {
                        params: vec![Param {
                            name: property.name.clone(),
                            span: property.span,
                            doc: Default::default(),
                            ty: input,
                        }],
                        returns: None,
                        throws: None,
                    },
                    thread_safe: property.thread_safe,
                }
            });
            std::iter::once(getter).chain(setter).collect()
        }
        Member::Declaration(declaration) => vec![Function {
            name: named(origin),
            origin,
            receiver: Some(borrowed(class)),
            signature: Signature {
                params: declaration.params.clone(),
                returns: None,
                throws: None,
            },
            thread_safe: false,
        }],
    }
}

fn lifecycle(api: &Api, class: Id<Class>) -> Vec<Function> {
    let function = |lifecycle, ownership, returns: Option<Output>, thread_safe| {
        let origin = Origin::Lifecycle(class, lifecycle);
        Function {
            name: name(api, origin),
            origin,
            receiver: Some(Receiver { class, ownership }),
            signature: empty(returns),
            thread_safe,
        }
    };
    let reference = Some(Output::Object {
        class,
        ownership: Ownership::Owned,
    });
    match api[class].lifetime {
        Lifetime::Owned => vec![function(Lifecycle::Free, Ownership::Owned, None, false)],
        Lifetime::Shared | Lifetime::Local => {
            let shared = api[class].lifetime == Lifetime::Shared;
            vec![
                function(Lifecycle::Ref, Ownership::Borrowed, reference, shared),
                function(Lifecycle::Unref, Ownership::Owned, None, shared),
            ]
        }
        Lifetime::Scoped => Vec::new(),
    }
}

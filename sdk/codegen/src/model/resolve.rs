use std::collections::BTreeSet;

use crate::diagnostic::{Diagnostic, Result, Span};
use crate::model::docs::{self, Fragment, Sections};
use crate::model::symbols::{DEFAULT_CONSTRUCTOR, STRING, Symbol, Symbols};
use crate::model::{
    Access, Api, Callback, Class, Closure, ClosureFunction, Configuration, Constant, ConstantValue,
    Constructor, Declaration, Doc, Element, Enum, ErrorCodes, Field, Flags, Function, Id, Input,
    Lifetime, Member, Method, Openness, Output, Ownership, Paragraph, Param, Primitive, Property,
    Signature, Struct, Threading, Value, Variant, Version,
};
use crate::syntax::ast::{
    self, Attribute, DocLine, ItemKind, LiteralKind, MemberKind, Name, TypeExpr,
};

pub(crate) fn resolve(file: ast::File) -> Result<Api> {
    let symbols = Symbols::build(&file)?;
    let mut lifetimes = Vec::new();
    let mut errors = Vec::new();
    for item in &file.items {
        match &item.kind {
            ItemKind::Class(class) => lifetimes.push(class.lifetime),
            ItemKind::Enum(_) => errors.push(
                item.attributes
                    .iter()
                    .any(|attribute| attribute.name.text == "error"),
            ),
            _ => {}
        }
    }
    let resolver = Resolver {
        symbols,
        lifetimes,
        errors,
    };
    resolver.file(file)
}

fn identifier(name: &Name) -> Result<()> {
    let text = name.text.as_str();
    let mut characters = text.chars();
    let valid = characters
        .next()
        .is_some_and(|first| first.is_ascii_lowercase())
        && characters.all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
        })
        && !text.ends_with('_')
        && !text.contains("__");
    match valid {
        true => Ok(()),
        false => Err(Diagnostic::new(
            name.span,
            format!("`{text}` is not a lowercase snake_case name"),
        )),
    }
}

fn unique<'a>(names: impl IntoIterator<Item = &'a Name>, what: &str) -> Result<()> {
    let mut seen = BTreeSet::new();
    for name in names {
        identifier(name)?;
        if !seen.insert(name.text.as_str()) {
            return Err(Diagnostic::new(
                name.span,
                format!("{what} `{}` is repeated", name.text),
            ));
        }
    }
    Ok(())
}

struct Attributes {
    list: Vec<Attribute>,
}

impl Attributes {
    fn new(list: Vec<Attribute>) -> Result<Self> {
        let mut seen = BTreeSet::new();
        if let Some(repeated) = list
            .iter()
            .find(|attribute| !seen.insert(attribute.name.text.as_str()))
        {
            return Err(Diagnostic::new(
                repeated.name.span,
                format!("`@{}` is repeated", repeated.name.text),
            ));
        }
        Ok(Self { list })
    }

    fn take(&mut self, name: &str) -> Option<Attribute> {
        let position = self
            .list
            .iter()
            .position(|attribute| attribute.name.text == name)?;
        Some(self.list.remove(position))
    }

    fn flag(&mut self, name: &str) -> Result<bool> {
        match self.take(name) {
            None => Ok(false),
            Some(attribute) if attribute.arguments.is_empty() => Ok(true),
            Some(attribute) => Err(Diagnostic::new(
                attribute.name.span,
                format!("`@{name}` takes no arguments"),
            )),
        }
    }

    fn single_name(&mut self, name: &str) -> Result<Option<Name>> {
        let Some(attribute) = self.take(name) else {
            return Ok(None);
        };
        match attribute.arguments.as_slice() {
            [
                ast::Argument {
                    key: None,
                    value:
                        ast::Literal {
                            kind: LiteralKind::Name(text),
                            span,
                        },
                },
            ] => {
                let value = Name {
                    text: text.clone(),
                    span: *span,
                };
                identifier(&value)?;
                Ok(Some(value))
            }
            _ => Err(Diagnostic::new(
                attribute.name.span,
                format!("`@{name}` takes one name, as in `@{name}(name)`"),
            )),
        }
    }

    fn finish(self, what: &str) -> Result<()> {
        match self.list.first() {
            Some(attribute) => Err(Diagnostic::new(
                attribute.name.span,
                format!("`@{}` does not apply to {what}", attribute.name.text),
            )),
            None => Ok(()),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Place {
    Function,
    Callback,
}

struct Tags {
    returns: bool,
    set: bool,
}

const NO_TAGS: Tags = Tags {
    returns: false,
    set: false,
};

struct Resolver {
    symbols: Symbols,
    lifetimes: Vec<Lifetime>,
    errors: Vec<bool>,
}

impl Resolver {
    fn paragraph(&self, fragments: &[Fragment]) -> Result<Paragraph> {
        docs::paragraph(fragments, &|path, span| self.symbols.target(path, span))
    }

    fn doc(&self, lines: &[DocLine], params: &mut [Param], tags: Tags) -> Result<Doc> {
        let Sections {
            body,
            params: documented,
            returns,
            set,
        } = docs::sections(lines)?;
        for (name, fragments) in documented {
            let param = params
                .iter_mut()
                .find(|param| param.name == name.text)
                .ok_or_else(|| {
                    Diagnostic::new(name.span, format!("there is no parameter `{}`", name.text))
                })?;
            param.doc = self.paragraph(&fragments)?;
        }
        let tagged = |tagged: Option<docs::Tagged>, allowed: bool, tag: &str| match tagged {
            None => Ok(Paragraph::default()),
            Some(tagged) if allowed => self.paragraph(&tagged.fragments),
            Some(tagged) => Err(Diagnostic::new(
                tagged.span,
                format!("`{tag}` does not apply here"),
            )),
        };
        Ok(Doc {
            body: body
                .iter()
                .map(|fragments| self.paragraph(fragments))
                .collect::<Result<_>>()?,
            returns: tagged(returns, tags.returns, "@returns")?,
            set: tagged(set, tags.set, "@set")?,
        })
    }

    fn file(&self, file: ast::File) -> Result<Api> {
        identifier(&file.name)?;
        identifier(&file.abi_prefix)?;
        let mut api = Api {
            name: file.name.text.clone(),
            version: version(&file.version)?,
            abi_prefix: file.abi_prefix.text.clone(),
            doc: self.doc(&file.docs, &mut [], NO_TAGS)?,
            constants: Vec::new(),
            structs: Vec::new(),
            enums: Vec::new(),
            flags: Vec::new(),
            callbacks: Vec::new(),
            classes: Vec::new(),
            functions: Vec::new(),
        };
        let mut error_codes = Vec::new();
        for item in file.items {
            identifier(item.kind.name())?;
            let mut attributes = Attributes::new(item.attributes)?;
            match item.kind {
                ItemKind::Constant(constant) => {
                    attributes.finish("a constant")?;
                    api.constants.push(self.constant(&item.docs, constant)?);
                }
                ItemKind::Struct(structure) => {
                    attributes.finish("a struct")?;
                    api.structs.push(self.structure(&item.docs, structure)?);
                }
                ItemKind::Enum(enumeration) => {
                    let (resolved, codes) =
                        self.enumeration(&item.docs, &mut attributes, enumeration)?;
                    attributes.finish("an enum")?;
                    error_codes.extend(codes.map(|codes| (api.enums.len(), codes)));
                    api.enums.push(resolved);
                }
                ItemKind::Flags(flags) => {
                    let resolved = self.flag_set(&item.docs, &mut attributes, flags)?;
                    attributes.finish("flags")?;
                    api.flags.push(resolved);
                }
                ItemKind::Callback(callback) => {
                    attributes.finish("a callback")?;
                    api.callbacks.push(self.callback(&item.docs, callback)?);
                }
                ItemKind::Class(class) => {
                    let builder = attributes.flag("builder")?;
                    attributes.finish("a class")?;
                    api.classes.push(self.class(&item.docs, builder, class)?);
                }
                ItemKind::Function(function) => {
                    let thread_safe = attributes.flag("thread_safe")?;
                    attributes.finish("a function")?;
                    api.functions
                        .push(self.function(&item.docs, thread_safe, function)?);
                }
            }
        }
        for (index, (success, message)) in error_codes {
            let function = match self.symbols.get(&message.text) {
                Some(Symbol::Function(function)) => function,
                _ => {
                    return Err(Diagnostic::new(
                        message.span,
                        format!("`{}` is not a function", message.text),
                    ));
                }
            };
            let signature = &api[function].signature;
            if !signature.params.is_empty()
                || signature.throws.is_some()
                || !matches!(signature.returns, Some(Output::String(_)))
            {
                return Err(Diagnostic::new(
                    message.span,
                    format!(
                        "`{}` must take nothing and return a string to describe errors",
                        message.text
                    ),
                ));
            }
            api.enums[index].error = Some(ErrorCodes {
                success,
                message: function,
            });
        }
        Ok(api)
    }

    fn constant(&self, docs: &[DocLine], constant: ast::Constant) -> Result<Constant> {
        let ty = Primitive::named(&constant.ty.text)
            .filter(|ty| *ty != Primitive::Bool)
            .ok_or_else(|| {
                Diagnostic::new(constant.ty.span, "a constant has an integer or float type")
            })?;
        let invalid = || {
            Diagnostic::new(
                constant.value.span,
                format!("`{}` cannot hold this value", constant.ty.text),
            )
        };
        let value = match (constant.value.kind, ty.integer_range()) {
            (LiteralKind::Integer(value), Some((least, most))) => {
                match (least..=most).contains(&value) {
                    true => ConstantValue::Integer(value),
                    false => return Err(invalid()),
                }
            }
            (LiteralKind::Name(name), Some(_)) if name == "max" => ConstantValue::Maximum,
            (LiteralKind::Float(value), None) => ConstantValue::Float(value),
            (LiteralKind::Integer(value), None) => ConstantValue::Float(value as f64),
            _ => return Err(invalid()),
        };
        if let ConstantValue::Float(value) = value
            && ty == Primitive::F32
            && value.abs() > f64::from(f32::MAX)
        {
            return Err(invalid());
        }
        Ok(Constant {
            span: constant.name.span,
            name: constant.name.text,
            doc: self.doc(docs, &mut [], NO_TAGS)?,
            ty,
            value,
        })
    }

    fn structure(&self, docs: &[DocLine], structure: ast::Struct) -> Result<Struct> {
        unique(structure.fields.iter().map(|field| &field.name), "field")?;
        let fields = structure
            .fields
            .into_iter()
            .map(|field| {
                let ty = Primitive::named(&field.ty.text).ok_or_else(|| {
                    Diagnostic::new(field.ty.span, "a struct field has a primitive type")
                })?;
                Ok(Field {
                    name: field.name.text,
                    span: field.name.span,
                    doc: self.doc(&field.docs, &mut [], NO_TAGS)?,
                    ty,
                })
            })
            .collect::<Result<_>>()?;
        Ok(Struct {
            name: structure.name.text,
            span: structure.name.span,
            doc: self.doc(docs, &mut [], NO_TAGS)?,
            fields,
        })
    }

    fn variants(&self, owner: &ast::Enum, explicit: bool) -> Result<Vec<Variant>> {
        unique(owner.variants.iter().map(|variant| &variant.name), "value")?;
        if owner.variants.is_empty() {
            return Err(Diagnostic::new(
                owner.name.span,
                format!("`{}` has no values", owner.name.text),
            ));
        }
        let mut numbers = BTreeSet::new();
        let mut next = 0i128;
        let mut resolved = Vec::new();
        for variant in &owner.variants {
            let value = match (&variant.value, explicit) {
                (
                    Some(ast::Literal {
                        kind: LiteralKind::Integer(value),
                        ..
                    }),
                    _,
                ) => *value,
                (Some(literal), _) => {
                    return Err(Diagnostic::new(literal.span, "expected an integer value"));
                }
                (None, true) => {
                    return Err(Diagnostic::new(
                        variant.name.span,
                        "every flag needs an explicit value",
                    ));
                }
                (None, false) => next,
            };
            let span = variant
                .value
                .as_ref()
                .map_or(variant.name.span, |literal| literal.span);
            let value = u32::try_from(value)
                .ok()
                .filter(|value| *value <= i32::MAX as u32)
                .ok_or_else(|| Diagnostic::new(span, "a value lies from 0 to 0x7FFFFFFF"))?;
            if !numbers.insert(value) {
                return Err(Diagnostic::new(span, format!("value {value} is repeated")));
            }
            next = i128::from(value) + 1;
            resolved.push(Variant {
                name: variant.name.text.clone(),
                span: variant.name.span,
                doc: self.doc(&variant.docs, &mut [], NO_TAGS)?,
                value,
            });
        }
        Ok(resolved)
    }

    fn enumeration(
        &self,
        docs: &[DocLine],
        attributes: &mut Attributes,
        enumeration: ast::Enum,
    ) -> Result<(Enum, Option<(usize, Name)>)> {
        let openness = match attributes.flag("open")? {
            true => Openness::Open,
            false => Openness::Closed,
        };
        let value_prefix = attributes
            .single_name("value_prefix")?
            .map_or(enumeration.name.text.clone(), |name| name.text);
        let variants = self.variants(&enumeration, false)?;
        let codes = match attributes.take("error") {
            None => None,
            Some(attribute) => Some(error_codes(&attribute, &variants)?),
        };
        Ok((
            Enum {
                name: enumeration.name.text,
                span: enumeration.name.span,
                doc: self.doc(docs, &mut [], NO_TAGS)?,
                openness,
                error: None,
                value_prefix,
                variants,
            },
            codes,
        ))
    }

    fn flag_set(
        &self,
        docs: &[DocLine],
        attributes: &mut Attributes,
        flags: ast::Enum,
    ) -> Result<Flags> {
        let value_prefix = attributes
            .single_name("value_prefix")?
            .map_or(flags.name.text.clone(), |name| name.text);
        let resolved = self.variants(&flags, true)?;
        if let Some(flag) = resolved.iter().find(|flag| !flag.value.is_power_of_two()) {
            return Err(Diagnostic::new(
                flag.span,
                format!("flag `{}` is not a single bit", flag.name),
            ));
        }
        Ok(Flags {
            name: flags.name.text,
            span: flags.name.span,
            doc: self.doc(docs, &mut [], NO_TAGS)?,
            value_prefix,
            flags: resolved,
        })
    }

    fn callback(&self, docs: &[DocLine], callback: ast::Callback) -> Result<Callback> {
        let mut params = self.params(callback.params, Place::Callback)?;
        let doc = self.doc(docs, &mut params, NO_TAGS)?;
        Ok(Callback {
            name: callback.name.text,
            span: callback.name.span,
            doc,
            params,
        })
    }

    fn function(
        &self,
        docs: &[DocLine],
        thread_safe: bool,
        function: ast::Function,
    ) -> Result<Function> {
        if let Some(span) = function.consumes {
            return Err(Diagnostic::new(
                span,
                "only a method can consume its receiver",
            ));
        }
        let mut signature = self.signature(function.params, function.returns, function.throws)?;
        let tags = tags_for(&signature);
        let doc = self.doc(docs, &mut signature.params, tags)?;
        Ok(Function {
            name: function.name.text,
            span: function.name.span,
            doc,
            signature,
            thread_safe,
        })
    }

    fn signature(
        &self,
        params: Vec<ast::Param>,
        returns: Option<TypeExpr>,
        throws: Option<Name>,
    ) -> Result<Signature> {
        let params = self.params(params, Place::Function)?;
        let output = returns.as_ref().map(|ty| self.output(ty)).transpose()?;
        let error = throws
            .as_ref()
            .map(|name| match self.symbols.get(&name.text) {
                Some(Symbol::Enum(id)) if self.errors[id.index] => Ok(id),
                _ => Err(Diagnostic::new(
                    name.span,
                    format!("`{}` is not an enum marked `@error`", name.text),
                )),
            })
            .transpose()?;
        if let (Some(_), Some(name)) = (&output, &throws) {
            return Err(Diagnostic::new(
                name.span,
                "a function that throws returns nothing else",
            ));
        }
        Ok(Signature {
            params,
            returns: output,
            throws: error,
        })
    }

    fn class(&self, docs: &[DocLine], builder: bool, class: ast::Class) -> Result<Class> {
        let lifetime = class.lifetime;
        if builder && lifetime != Lifetime::Owned {
            return Err(Diagnostic::new(
                class.name.span,
                "only an owned class can be a `@builder`",
            ));
        }
        let names: Vec<Name> = class
            .members
            .iter()
            .map(|member| match &member.kind {
                MemberKind::Constructor { keyword, name, .. } => {
                    name.clone().unwrap_or_else(|| Name {
                        text: DEFAULT_CONSTRUCTOR.to_owned(),
                        span: keyword.span,
                    })
                }
                MemberKind::Function(function) => function.name.clone(),
                MemberKind::Property { name, .. } | MemberKind::Declaration { name, .. } => {
                    name.clone()
                }
            })
            .collect();
        unique(&names, "member")?;
        let members = class
            .members
            .into_iter()
            .map(|member| self.member(lifetime, member))
            .collect::<Result<_>>()?;
        Ok(Class {
            name: class.name.text,
            span: class.name.span,
            doc: self.doc(docs, &mut [], NO_TAGS)?,
            lifetime,
            configuration: match builder {
                true => Configuration::Builder,
                false => Configuration::Plain,
            },
            members,
        })
    }

    fn member(&self, lifetime: Lifetime, member: ast::Member) -> Result<Member> {
        let mut attributes = Attributes::new(member.attributes)?;
        let resolved = match member.kind {
            MemberKind::Constructor {
                keyword,
                name,
                params,
            } => {
                if lifetime == Lifetime::Scoped {
                    return Err(Diagnostic::new(
                        keyword.span,
                        "a scoped class is only ever lent by the SDK and has no constructor",
                    ));
                }
                let thread_safe = attributes.flag("thread_safe")?;
                attributes.finish("a constructor")?;
                let mut params = self.params(params, Place::Function)?;
                let doc = self.doc(&member.docs, &mut params, NO_TAGS)?;
                Member::Constructor(Constructor {
                    span: name.as_ref().map_or(keyword.span, |name| name.span),
                    name: name.map(|name| name.text),
                    doc,
                    params,
                    thread_safe,
                })
            }
            MemberKind::Function(function) => {
                let thread_safe = attributes.flag("thread_safe")?;
                attributes.finish("a method")?;
                let receiver = match function.consumes {
                    None => Ownership::Borrowed,
                    Some(_) if lifetime == Lifetime::Owned => Ownership::Owned,
                    Some(span) => {
                        return Err(Diagnostic::new(
                            span,
                            "only a method of an owned class can consume its receiver",
                        ));
                    }
                };
                let mut signature =
                    self.signature(function.params, function.returns, function.throws)?;
                let tags = tags_for(&signature);
                let doc = self.doc(&member.docs, &mut signature.params, tags)?;
                Member::Method(Method {
                    name: function.name.text,
                    span: function.name.span,
                    doc,
                    receiver,
                    signature,
                    thread_safe,
                })
            }
            MemberKind::Property { name, ty, readonly } => {
                let thread_safe = attributes.flag("thread_safe")?;
                attributes.finish("a property")?;
                let output = self.output(&ty)?;
                let access = match readonly {
                    true => Access::ReadOnly,
                    false => Access::ReadWrite,
                };
                if access == Access::ReadWrite
                    && !matches!(output, Output::Value(_) | Output::String(Ownership::Owned))
                {
                    return Err(Diagnostic::new(
                        ty.span(),
                        "a writable property holds a value or a string; mark it `readonly`",
                    ));
                }
                let doc = self.doc(
                    &member.docs,
                    &mut [],
                    Tags {
                        returns: false,
                        set: access == Access::ReadWrite,
                    },
                )?;
                Member::Property(Property {
                    name: name.text,
                    span: name.span,
                    doc,
                    ty: output,
                    access,
                    thread_safe,
                })
            }
            MemberKind::Declaration { kind, name, params } => {
                attributes.finish("a declaration")?;
                if lifetime != Lifetime::Scoped {
                    return Err(Diagnostic::new(
                        name.span,
                        "only a scoped class declares view content",
                    ));
                }
                let mut params = self.params(params, Place::Function)?;
                let doc = self.doc(&member.docs, &mut params, NO_TAGS)?;
                Member::Declaration(Declaration {
                    kind,
                    name: name.text,
                    span: name.span,
                    doc,
                    params,
                })
            }
        };
        Ok(resolved)
    }

    fn params(&self, params: Vec<ast::Param>, place: Place) -> Result<Vec<Param>> {
        unique(params.iter().map(|param| &param.name), "parameter")?;
        let mut closures = 0;
        let mut resolved = Vec::new();
        for param in params {
            let mut attributes = Attributes::new(param.attributes)?;
            let threading = match attributes.flag("any_thread")? {
                true => Threading::AnyThread,
                false => Threading::ThisThread,
            };
            attributes.finish("a parameter")?;
            let ty = self.input(&param.name, &param.ty, place, threading)?;
            if threading == Threading::AnyThread && !matches!(ty, Input::Closure(_)) {
                return Err(Diagnostic::new(
                    param.name.span,
                    "only a closure can be `@any_thread`",
                ));
            }
            if matches!(ty, Input::Closure(_)) {
                closures += 1;
                if closures > 1 {
                    return Err(Diagnostic::new(
                        param.name.span,
                        "a signature takes at most one closure",
                    ));
                }
            }
            resolved.push(Param {
                name: param.name.text.clone(),
                span: param.name.span,
                doc: Paragraph::default(),
                ty,
            });
        }
        Ok(resolved)
    }

    fn value(&self, name: &Name) -> Option<Value> {
        if let Some(primitive) = Primitive::named(&name.text) {
            return Some(Value::Primitive(primitive));
        }
        match self.symbols.get(&name.text)? {
            Symbol::Struct(id) => Some(Value::Struct(id)),
            Symbol::Enum(id) => Some(Value::Enum(id)),
            Symbol::Flags(id) => Some(Value::Flags(id)),
            _ => None,
        }
    }

    fn class_of(&self, name: &Name) -> Option<(Id<Class>, Lifetime)> {
        match self.symbols.get(&name.text)? {
            Symbol::Class(id) => Some((id, self.lifetimes[id.index])),
            _ => None,
        }
    }

    fn callback_of(&self, name: &Name) -> Option<Id<Callback>> {
        match self.symbols.get(&name.text)? {
            Symbol::Callback(id) => Some(id),
            _ => None,
        }
    }

    fn unknown(name: &Name) -> Diagnostic {
        Diagnostic::new(name.span, format!("unknown type `{}`", name.text))
    }

    fn input(
        &self,
        param: &Name,
        ty: &TypeExpr,
        place: Place,
        threading: Threading,
    ) -> Result<Input> {
        let in_callback = |span: Span, what: &str| match place {
            Place::Callback => Err(Diagnostic::new(
                span,
                format!("a callback cannot receive {what}"),
            )),
            Place::Function => Ok(()),
        };
        match ty {
            TypeExpr::Named(name) if name.text == STRING => Ok(Input::String),
            TypeExpr::Named(name) => {
                if let Some(value) = self.value(name) {
                    return Ok(Input::Value(value));
                }
                if let Some((class, lifetime)) = self.class_of(name) {
                    if place == Place::Callback && lifetime != Lifetime::Scoped {
                        return Err(Diagnostic::new(
                            name.span,
                            "a callback receives only scoped classes",
                        ));
                    }
                    return Ok(Input::Object {
                        class,
                        ownership: Ownership::Borrowed,
                    });
                }
                if let Some(callback) = self.callback_of(name) {
                    in_callback(name.span, "a closure")?;
                    return Ok(Input::Closure(Closure {
                        functions: vec![ClosureFunction {
                            name: param.text.clone(),
                            callback,
                        }],
                        threading,
                    }));
                }
                Err(Self::unknown(name))
            }
            TypeExpr::Owned(inner, span) => {
                in_callback(*span, "ownership")?;
                match inner.as_ref() {
                    TypeExpr::Named(name) => match self.class_of(name) {
                        Some((class, Lifetime::Owned)) => Ok(Input::Object {
                            class,
                            ownership: Ownership::Owned,
                        }),
                        _ => Err(Diagnostic::new(
                            *span,
                            "only an owned class can be handed over",
                        )),
                    },
                    _ => Err(Diagnostic::new(
                        *span,
                        "only an owned class can be handed over",
                    )),
                }
            }
            TypeExpr::Borrowed(_, span) => Err(Diagnostic::new(
                *span,
                "parameters are borrowed unless marked `owned`",
            )),
            TypeExpr::Array(inner, span) => {
                let TypeExpr::Named(name) = inner.as_ref() else {
                    return Err(Diagnostic::new(
                        *span,
                        "an array holds primitives or strings",
                    ));
                };
                match (Primitive::named(&name.text), name.text == STRING) {
                    (Some(primitive), _) => Ok(Input::Array(Element::Primitive(primitive))),
                    (None, true) if place == Place::Callback => Ok(Input::Array(Element::String)),
                    (None, true) => Err(Diagnostic::new(
                        name.span,
                        "only a callback receives an array of strings",
                    )),
                    (None, false) => Err(Diagnostic::new(
                        *span,
                        "an array holds primitives or strings",
                    )),
                }
            }
            TypeExpr::Closure(parts, span) => {
                in_callback(*span, "a closure")?;
                if parts.len() < 2 {
                    return Err(Diagnostic::new(
                        *span,
                        "a closure group holds two or more functions; name a callback type for one",
                    ));
                }
                unique(parts.iter().map(|part| &part.name), "closure function")?;
                let functions = parts
                    .iter()
                    .map(|part| {
                        let callback = self.callback_of(&part.callback).ok_or_else(|| {
                            Diagnostic::new(
                                part.callback.span,
                                format!("`{}` is not a callback", part.callback.text),
                            )
                        })?;
                        Ok(ClosureFunction {
                            name: part.name.text.clone(),
                            callback,
                        })
                    })
                    .collect::<Result<_>>()?;
                Ok(Input::Closure(Closure {
                    functions,
                    threading,
                }))
            }
        }
    }

    fn output(&self, ty: &TypeExpr) -> Result<Output> {
        match ty {
            TypeExpr::Named(name) if name.text == STRING => Ok(Output::String(Ownership::Owned)),
            TypeExpr::Named(name) => {
                if let Some(value) = self.value(name) {
                    return Ok(Output::Value(value));
                }
                match self.class_of(name) {
                    Some((_, Lifetime::Scoped)) => Err(Diagnostic::new(
                        name.span,
                        "a scoped class is only lent to callbacks and never returned",
                    )),
                    Some((class, _)) => Ok(Output::Object {
                        class,
                        ownership: Ownership::Owned,
                    }),
                    None => Err(Self::unknown(name)),
                }
            }
            TypeExpr::Borrowed(inner, span) => match inner.as_ref() {
                TypeExpr::Named(name) if name.text == STRING => {
                    Ok(Output::String(Ownership::Borrowed))
                }
                TypeExpr::Named(name) => match self.class_of(name) {
                    Some((class, Lifetime::Shared)) => Ok(Output::Object {
                        class,
                        ownership: Ownership::Borrowed,
                    }),
                    _ => Err(Diagnostic::new(
                        *span,
                        "only a string or a shared class can be returned borrowed",
                    )),
                },
                _ => Err(Diagnostic::new(
                    *span,
                    "only a string or a shared class can be returned borrowed",
                )),
            },
            TypeExpr::Owned(_, span) => Err(Diagnostic::new(
                *span,
                "results are owned unless marked `borrowed`",
            )),
            TypeExpr::Array(_, span) | TypeExpr::Closure(_, span) => Err(Diagnostic::new(
                *span,
                "a result is a value, a string or a class",
            )),
        }
    }
}

fn tags_for(signature: &Signature) -> Tags {
    Tags {
        returns: signature.returns.is_some(),
        set: false,
    }
}

fn version(literal: &ast::Literal) -> Result<Version> {
    let invalid = || {
        Diagnostic::new(
            literal.span,
            "a version is a string \"major.minor.patch\" within 10, 10 and 12 bits",
        )
    };
    let LiteralKind::String(text) = &literal.kind else {
        return Err(invalid());
    };
    let parts: Vec<u32> = text
        .split('.')
        .map(str::parse)
        .collect::<std::result::Result<_, _>>()
        .map_err(|_| invalid())?;
    match parts.as_slice() {
        [major, minor, patch] if *major < 1024 && *minor < 1024 && *patch < 4096 => Ok(Version {
            major: *major,
            minor: *minor,
            patch: *patch,
        }),
        _ => Err(invalid()),
    }
}

fn error_codes(attribute: &Attribute, variants: &[Variant]) -> Result<(usize, Name)> {
    let invalid = || {
        Diagnostic::new(
            attribute.name.span,
            "`@error` takes `success = <value>, message = <function>`",
        )
    };
    let named = |key: &str| {
        attribute.arguments.iter().find_map(|argument| {
            match (&argument.key, &argument.value.kind) {
                (Some(name), LiteralKind::Name(value)) if name.text == key => Some(Name {
                    text: value.clone(),
                    span: argument.value.span,
                }),
                _ => None,
            }
        })
    };
    if attribute.arguments.len() != 2 {
        return Err(invalid());
    }
    let (Some(success), Some(message)) = (named("success"), named("message")) else {
        return Err(invalid());
    };
    let index = variants
        .iter()
        .position(|variant| variant.name == success.text)
        .ok_or_else(|| {
            Diagnostic::new(
                success.span,
                format!("there is no value `{}`", success.text),
            )
        })?;
    Ok((index, message))
}

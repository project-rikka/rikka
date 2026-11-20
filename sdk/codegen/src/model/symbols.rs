use std::collections::{BTreeMap, HashMap};
use std::mem::discriminant;

use crate::diagnostic::{Diagnostic, Result, Span};
use crate::model::{
    Callback, Class, Constant, Enum, Flags, Function, Id, Primitive, Struct, Target,
};
use crate::syntax::ast::{File, ItemKind, MemberKind};

pub(super) const STRING: &str = "string";
pub(super) const DEFAULT_CONSTRUCTOR: &str = "new";

#[derive(Clone, Copy, Debug)]
pub(super) enum Symbol {
    Constant(Id<Constant>),
    Struct(Id<Struct>),
    Enum(Id<Enum>),
    Flags(Id<Flags>),
    Callback(Id<Callback>),
    Class(Id<Class>),
    Function(Id<Function>),
}

pub(super) struct Symbols {
    top: BTreeMap<String, Symbol>,
    children: BTreeMap<String, Vec<String>>,
}

impl Symbols {
    pub(super) fn build(file: &File) -> Result<Self> {
        let mut symbols = Self {
            top: BTreeMap::new(),
            children: BTreeMap::new(),
        };
        let mut counts = HashMap::new();
        for item in &file.items {
            let name = item.kind.name();
            if name.text == STRING || Primitive::named(&name.text).is_some() {
                return Err(Diagnostic::new(
                    name.span,
                    format!("`{}` is a builtin type", name.text),
                ));
            }
            let count = counts.entry(discriminant(&item.kind)).or_insert(0);
            let index = *count;
            *count += 1;
            let symbol = match &item.kind {
                ItemKind::Constant(_) => Symbol::Constant(Id::new(index)),
                ItemKind::Struct(_) => Symbol::Struct(Id::new(index)),
                ItemKind::Enum(_) => Symbol::Enum(Id::new(index)),
                ItemKind::Flags(_) => Symbol::Flags(Id::new(index)),
                ItemKind::Callback(_) => Symbol::Callback(Id::new(index)),
                ItemKind::Class(_) => Symbol::Class(Id::new(index)),
                ItemKind::Function(_) => Symbol::Function(Id::new(index)),
            };
            if symbols.top.insert(name.text.clone(), symbol).is_some() {
                return Err(Diagnostic::new(
                    name.span,
                    format!("`{}` is declared twice", name.text),
                ));
            }
            let children: Vec<String> = match &item.kind {
                ItemKind::Struct(item) => item
                    .fields
                    .iter()
                    .map(|field| field.name.text.clone())
                    .collect(),
                ItemKind::Enum(item) | ItemKind::Flags(item) => item
                    .variants
                    .iter()
                    .map(|variant| variant.name.text.clone())
                    .collect(),
                ItemKind::Class(item) => item
                    .members
                    .iter()
                    .map(|member| match &member.kind {
                        MemberKind::Constructor { name, .. } => name
                            .as_ref()
                            .map_or(DEFAULT_CONSTRUCTOR.to_owned(), |name| name.text.clone()),
                        MemberKind::Function(function) => function.name.text.clone(),
                        MemberKind::Property { name, .. }
                        | MemberKind::Declaration { name, .. } => name.text.clone(),
                    })
                    .collect(),
                _ => continue,
            };
            symbols.children.insert(name.text.clone(), children);
        }
        Ok(symbols)
    }

    pub(super) fn get(&self, name: &str) -> Option<Symbol> {
        self.top.get(name).copied()
    }

    pub(super) fn target(&self, path: &str, span: Span) -> Result<Target> {
        let unknown = || Diagnostic::new(span, format!("`[{path}]` refers to nothing"));
        let (owner, child) = match path.split_once('.') {
            Some((owner, child)) => (owner, Some(child)),
            None => (path, None),
        };
        let symbol = self.get(owner).ok_or_else(unknown)?;
        let Some(child) = child else {
            return Ok(match symbol {
                Symbol::Constant(id) => Target::Constant(id),
                Symbol::Struct(id) => Target::Struct(id),
                Symbol::Enum(id) => Target::Enum(id),
                Symbol::Flags(id) => Target::Flags(id),
                Symbol::Callback(id) => Target::Callback(id),
                Symbol::Class(id) => Target::Class(id),
                Symbol::Function(id) => Target::Function(id),
            });
        };
        let position = self
            .children
            .get(owner)
            .and_then(|children| children.iter().position(|name| name == child))
            .ok_or_else(unknown)?;
        match symbol {
            Symbol::Struct(id) => Ok(Target::Field(id, position)),
            Symbol::Enum(id) => Ok(Target::Variant(id, position)),
            Symbol::Flags(id) => Ok(Target::Flag(id, position)),
            Symbol::Class(id) => Ok(Target::Member(id, position)),
            _ => Err(unknown()),
        }
    }
}

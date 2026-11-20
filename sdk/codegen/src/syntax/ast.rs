use crate::diagnostic::Span;

#[derive(Clone, Debug)]
pub(crate) struct Name {
    pub(crate) text: String,
    pub(crate) span: Span,
}

#[derive(Clone, Debug)]
pub(crate) struct DocLine {
    pub(crate) text: String,
    pub(crate) span: Span,
}

#[derive(Debug)]
pub(crate) struct Attribute {
    pub(crate) name: Name,
    pub(crate) arguments: Vec<Argument>,
}

#[derive(Debug)]
pub(crate) struct Argument {
    pub(crate) key: Option<Name>,
    pub(crate) value: Literal,
}

#[derive(Debug)]
pub(crate) struct Literal {
    pub(crate) kind: LiteralKind,
    pub(crate) span: Span,
}

#[derive(Debug)]
pub(crate) enum LiteralKind {
    Name(String),
    Integer(i128),
    Float(f64),
    String(String),
}

#[derive(Debug)]
pub(crate) struct File {
    pub(crate) docs: Vec<DocLine>,
    pub(crate) name: Name,
    pub(crate) version: Literal,
    pub(crate) abi_prefix: Name,
    pub(crate) items: Vec<Item>,
}

#[derive(Debug)]
pub(crate) struct Item {
    pub(crate) docs: Vec<DocLine>,
    pub(crate) attributes: Vec<Attribute>,
    pub(crate) kind: ItemKind,
}

#[derive(Debug)]
pub(crate) enum ItemKind {
    Constant(Constant),
    Struct(Struct),
    Enum(Enum),
    Flags(Enum),
    Callback(Callback),
    Class(Class),
    Function(Function),
}

impl ItemKind {
    pub(crate) fn name(&self) -> &Name {
        match self {
            Self::Constant(item) => &item.name,
            Self::Struct(item) => &item.name,
            Self::Enum(item) | Self::Flags(item) => &item.name,
            Self::Callback(item) => &item.name,
            Self::Class(item) => &item.name,
            Self::Function(item) => &item.name,
        }
    }
}

#[derive(Debug)]
pub(crate) struct Constant {
    pub(crate) name: Name,
    pub(crate) ty: Name,
    pub(crate) value: Literal,
}

#[derive(Debug)]
pub(crate) struct Struct {
    pub(crate) name: Name,
    pub(crate) fields: Vec<Field>,
}

#[derive(Debug)]
pub(crate) struct Field {
    pub(crate) docs: Vec<DocLine>,
    pub(crate) name: Name,
    pub(crate) ty: Name,
}

#[derive(Debug)]
pub(crate) struct Enum {
    pub(crate) name: Name,
    pub(crate) variants: Vec<Variant>,
}

#[derive(Debug)]
pub(crate) struct Variant {
    pub(crate) docs: Vec<DocLine>,
    pub(crate) name: Name,
    pub(crate) value: Option<Literal>,
}

#[derive(Debug)]
pub(crate) struct Callback {
    pub(crate) name: Name,
    pub(crate) params: Vec<Param>,
}

#[derive(Debug)]
pub(crate) struct Class {
    pub(crate) lifetime: Lifetime,
    pub(crate) name: Name,
    pub(crate) members: Vec<Member>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Lifetime {
    Owned,
    Shared,
    Local,
    Scoped,
}

impl Lifetime {
    pub(crate) fn named(text: &str) -> Option<Self> {
        Some(match text {
            "owned" => Self::Owned,
            "shared" => Self::Shared,
            "local" => Self::Local,
            "scoped" => Self::Scoped,
            _ => return None,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DeclarationKind {
    Container,
    Element,
    Modifier,
}

impl DeclarationKind {
    pub(crate) fn named(text: &str) -> Option<Self> {
        Some(match text {
            "container" => Self::Container,
            "element" => Self::Element,
            "modifier" => Self::Modifier,
            _ => return None,
        })
    }
}

#[derive(Debug)]
pub(crate) struct Member {
    pub(crate) docs: Vec<DocLine>,
    pub(crate) attributes: Vec<Attribute>,
    pub(crate) kind: MemberKind,
}

#[derive(Debug)]
pub(crate) enum MemberKind {
    Constructor {
        keyword: Name,
        name: Option<Name>,
        params: Vec<Param>,
    },
    Function(Function),
    Property {
        name: Name,
        ty: TypeExpr,
        readonly: bool,
    },
    Declaration {
        kind: DeclarationKind,
        name: Name,
        params: Vec<Param>,
    },
}

#[derive(Debug)]
pub(crate) struct Function {
    pub(crate) name: Name,
    pub(crate) consumes: Option<Span>,
    pub(crate) params: Vec<Param>,
    pub(crate) returns: Option<TypeExpr>,
    pub(crate) throws: Option<Name>,
}

#[derive(Debug)]
pub(crate) struct Param {
    pub(crate) attributes: Vec<Attribute>,
    pub(crate) name: Name,
    pub(crate) ty: TypeExpr,
}

#[derive(Debug)]
pub(crate) enum TypeExpr {
    Named(Name),
    Owned(Box<TypeExpr>, Span),
    Borrowed(Box<TypeExpr>, Span),
    Array(Box<TypeExpr>, Span),
    Closure(Vec<ClosurePart>, Span),
}

impl TypeExpr {
    pub(crate) fn span(&self) -> Span {
        match self {
            Self::Named(name) => name.span,
            Self::Owned(_, span)
            | Self::Borrowed(_, span)
            | Self::Array(_, span)
            | Self::Closure(_, span) => *span,
        }
    }
}

#[derive(Debug)]
pub(crate) struct ClosurePart {
    pub(crate) name: Name,
    pub(crate) callback: Name,
}

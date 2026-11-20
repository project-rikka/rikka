use crate::context::Context;
use crate::error::{Error, Result};
use crate::model::{Arg, ArgKind, Interface, Message};
use crate::names;
use crate::side::Side;

pub(crate) struct Field {
    pub(crate) name: String,
    pub(crate) kind: String,
    pub(crate) decode: String,
}

pub(crate) struct Param {
    pub(crate) name: String,
    pub(crate) kind: String,
    pub(crate) encode: String,
}

#[derive(Clone, Copy)]
pub(crate) enum Source {
    Args,
    Word(usize),
}

impl Source {
    fn read(self, accessor: &str, fallible: bool) -> String {
        match self {
            Self::Args => format!("args.{accessor}()?"),
            Self::Word(index) if fallible => format!("words[{index}].{accessor}()?"),
            Self::Word(index) => format!("words[{index}].{accessor}()"),
        }
    }
}

pub(crate) fn signature(arg: &Arg) -> &'static str {
    match (arg.kind, arg.nullable, arg.interface.is_some()) {
        (ArgKind::Int, ..) => "wire::ArgType::Int",
        (ArgKind::Uint, ..) => "wire::ArgType::Uint",
        (ArgKind::Fixed, ..) => "wire::ArgType::Fixed",
        (ArgKind::String, false, _) => "wire::ArgType::String",
        (ArgKind::String, true, _) => "wire::ArgType::NullableString",
        (ArgKind::Object, false, _) => "wire::ArgType::Object",
        (ArgKind::Object, true, _) => "wire::ArgType::NullableObject",
        (ArgKind::NewId, _, true) => "wire::ArgType::NewId",
        (ArgKind::NewId, _, false) => "wire::ArgType::UntypedNewId",
        (ArgKind::Array, ..) => "wire::ArgType::Array",
        (ArgKind::Fd, ..) => "wire::ArgType::Fd",
    }
}

pub(crate) fn borrows(arg: &Arg) -> bool {
    match arg.kind {
        ArgKind::String | ArgKind::Array => true,
        ArgKind::NewId => arg.interface.is_none(),
        _ => false,
    }
}

pub(crate) fn incoming_type(messages: &[Message], side: &Side, lifetime: &str) -> String {
    let borrows = messages
        .iter()
        .flat_map(|message| &message.args)
        .any(borrows);
    if borrows && !side.owned {
        format!("{}<{lifetime}>", side.incoming)
    } else {
        side.incoming.to_owned()
    }
}

pub(crate) fn words(message: &Message) -> bool {
    !message.args.is_empty()
        && message.args.iter().all(|arg| match arg.kind {
            ArgKind::Int | ArgKind::Uint | ArgKind::Fixed | ArgKind::Object => true,
            ArgKind::NewId => arg.interface.is_some(),
            ArgKind::String | ArgKind::Array | ArgKind::Fd => false,
        })
}

pub(crate) fn field<'p>(
    context: &Context<'p>,
    interface: &'p Interface,
    arg: &Arg,
    source: Source,
    side: &Side,
) -> Result<Field> {
    let name = names::snake(&arg.name);
    let (kind, decode) = match (arg.kind, &arg.interface) {
        (ArgKind::Int | ArgKind::Uint, _) if arg.enumeration.is_some() => {
            numbered_field(context, interface, arg, source)?
        }
        (ArgKind::Int, _) => ("i32".to_owned(), source.read("int", false)),
        (ArgKind::Uint, _) => ("u32".to_owned(), source.read("uint", false)),
        (ArgKind::Fixed, _) => ("f64".to_owned(), source.read("fixed", false)),
        (ArgKind::String, _) if arg.nullable && side.owned => (
            "Option<String>".to_owned(),
            "args.nullable_string()?.map(str::to_owned)".to_owned(),
        ),
        (ArgKind::String, _) if arg.nullable => (
            "Option<&'m str>".to_owned(),
            "args.nullable_string()?".to_owned(),
        ),
        (ArgKind::String, _) if side.owned => {
            ("String".to_owned(), "args.string()?.to_owned()".to_owned())
        }
        (ArgKind::String, _) => ("&'m str".to_owned(), "args.string()?".to_owned()),
        (ArgKind::Object, Some(target)) => {
            let handle = context.handle(interface, target);
            if arg.nullable {
                (
                    format!("Option<{handle}>"),
                    format!(
                        "objects.nullable::<{handle}>({})?",
                        source.read("nullable_object", false)
                    ),
                )
            } else {
                (
                    handle.clone(),
                    format!("objects.get::<{handle}>({})?", source.read("object", true)),
                )
            }
        }
        (ArgKind::Object, None) if arg.nullable => (
            format!("Option<{}>", side.untyped),
            format!(
                "objects.nullable_any({})?",
                source.read("nullable_object", false)
            ),
        ),
        (ArgKind::Object, None) => (
            side.untyped.to_owned(),
            format!("objects.any({})?", source.read("object", true)),
        ),
        (ArgKind::NewId, Some(target)) => {
            let handle = context.handle(interface, target);
            (
                handle.clone(),
                format!(
                    "objects.create::<{handle}>({}, parent.version())?",
                    source.read("new_id", true)
                ),
            )
        }
        (ArgKind::NewId, None) => (
            "wire::NewId<'m>".to_owned(),
            "args.untyped_new_id()?".to_owned(),
        ),
        (ArgKind::Array, _) if side.owned => {
            ("Vec<u8>".to_owned(), "args.array()?.to_vec()".to_owned())
        }
        (ArgKind::Array, _) => ("&'m [u8]".to_owned(), "args.array()?".to_owned()),
        (ArgKind::Fd, _) => ("std::os::fd::OwnedFd".to_owned(), "args.fd()?".to_owned()),
    };
    Ok(Field { name, kind, decode })
}

pub(crate) fn param<'p>(
    context: &Context<'p>,
    interface: &'p Interface,
    event: &str,
    arg: &Arg,
    side: &Side,
) -> Result<Param> {
    let untyped = format!("{}::{}", side.typed, side.as_untyped);
    let name = names::snake(&arg.name);
    let (kind, encode) = match (arg.kind, &arg.interface) {
        (ArgKind::Int | ArgKind::Uint, _) if arg.enumeration.is_some() => {
            numbered_param(context, interface, arg, &name)?
        }
        (ArgKind::Int, _) => ("i32".to_owned(), format!("int({name})")),
        (ArgKind::Uint, _) => ("u32".to_owned(), format!("uint({name})")),
        (ArgKind::Fixed, _) => ("f64".to_owned(), format!("fixed({name})")),
        (ArgKind::String, _) if arg.nullable => (
            "Option<&str>".to_owned(),
            format!("nullable_string({name})"),
        ),
        (ArgKind::String, _) => ("&str".to_owned(), format!("string({name})")),
        (ArgKind::Object, Some(target)) => {
            let handle = context.handle(interface, target);
            if arg.nullable {
                (
                    format!("Option<&{handle}>"),
                    format!("nullable_object({name}.map({untyped}))"),
                )
            } else {
                (format!("&{handle}"), format!("object({untyped}({name}))"))
            }
        }
        (ArgKind::Object, None) if arg.nullable => (
            format!("Option<&{}>", side.untyped),
            format!("nullable_object({name})"),
        ),
        (ArgKind::Object, None) => (format!("&{}", side.untyped), format!("object({name})")),
        (ArgKind::NewId, Some(target)) => (
            format!("&{}", context.handle(interface, target)),
            format!("new_id({untyped}({name}))"),
        ),
        (ArgKind::NewId, None) => {
            return Err(Error::Inconsistent(format!(
                "{}.{event} creates an object of unknown interface",
                interface.name
            )));
        }
        (ArgKind::Array, _) => ("&[u8]".to_owned(), format!("array({name})")),
        (ArgKind::Fd, _) => ("impl Into<wire::Fd<'f>>".to_owned(), format!("fd({name})")),
    };
    Ok(Param { name, kind, encode })
}

fn numbered_field<'p>(
    context: &Context<'p>,
    interface: &'p Interface,
    arg: &Arg,
    source: Source,
) -> Result<(String, String)> {
    let reference = arg.enumeration.as_deref().unwrap_or_default();
    let resolved = context.enum_type(interface, reference)?;
    let raw = match arg.kind {
        ArgKind::Int => format!("{} as u32", source.read("int", false)),
        _ => source.read("uint", false),
    };
    Ok(if resolved.definition.bitfield {
        (
            resolved.path.clone(),
            format!("{}::from_bits_retain({raw})", resolved.path),
        )
    } else {
        (
            format!("Result<{}, wire::UnknownValue>", resolved.path),
            format!("{}::try_from({raw})", resolved.path),
        )
    })
}

fn numbered_param<'p>(
    context: &Context<'p>,
    interface: &'p Interface,
    arg: &Arg,
    name: &str,
) -> Result<(String, String)> {
    let reference = arg.enumeration.as_deref().unwrap_or_default();
    let resolved = context.enum_type(interface, reference)?;
    let raw = if resolved.definition.bitfield {
        format!("{name}.bits()")
    } else {
        format!("u32::from({name})")
    };
    let encode = match arg.kind {
        ArgKind::Int => format!("int({raw} as i32)"),
        _ => format!("uint({raw})"),
    };
    Ok((resolved.path, encode))
}

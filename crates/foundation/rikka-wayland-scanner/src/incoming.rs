use crate::code::Code;
use crate::context::Context;
use crate::error::{Error, Result};
use crate::model::{ArgKind, Interface, Message};
use crate::names;
use crate::side::Side;
use crate::types::{self, Field, Source};

struct Decoded {
    variant: String,
    words: Option<usize>,
    fields: Vec<Field>,
}

pub(crate) fn emit<'p>(
    code: &mut Code,
    context: &Context<'p>,
    interface: &'p Interface,
    messages: &'p [Message],
    side: &Side,
) -> Result<()> {
    if let Some(message) = messages.iter().find(|message| untyped_new_id(message))
        && !side.binds
    {
        return Err(Error::Inconsistent(format!(
            "{}.{} creates an object of unknown interface",
            interface.name, message.name
        )));
    }
    let requests = messages
        .iter()
        .map(|request| {
            let words = types::words(request).then_some(request.args.len());
            let fields = request
                .args
                .iter()
                .enumerate()
                .map(|(index, arg)| {
                    let source = match words {
                        Some(_) => Source::Word(index),
                        None => Source::Args,
                    };
                    types::field(context, interface, arg, source, side)
                })
                .collect::<Result<Vec<_>>>()?;
            Ok(Decoded {
                variant: names::camel(&request.name),
                words,
                fields,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    let incoming = side.incoming;
    let request_type = types::incoming_type(messages, side, "'m");
    code.line("#[derive(Debug)]");
    if side.owned {
        code.line("#[non_exhaustive]");
    }
    if requests.is_empty() {
        code.line(format!("pub enum {incoming} {{}}")).blank();
    } else {
        code.open(format!("pub enum {request_type} {{"));
        for request in &requests {
            if request.fields.is_empty() {
                code.line(format!("{},", request.variant));
            } else {
                code.open(format!("{} {{", request.variant));
                for field in &request.fields {
                    code.line(format!("{}: {},", field.name, field.kind));
                }
                code.close("},");
            }
        }
        code.close("}").blank();
    }

    resource(code, context, interface, messages, side, &requests);
    Ok(())
}

fn untyped_new_id(message: &Message) -> bool {
    message
        .args
        .iter()
        .any(|arg| arg.kind == ArgKind::NewId && arg.interface.is_none())
}

fn resource(
    code: &mut Code,
    context: &Context<'_>,
    interface: &Interface,
    messages: &[Message],
    side: &Side,
    requests: &[Decoded],
) {
    let handle = names::camel(&interface.name);
    let Side {
        incoming,
        typed,
        untyped,
        from_untyped,
        as_untyped,
        objects,
        binds,
        owned,
    } = *side;
    let args = messages.iter().flat_map(|request| &request.args);
    let uses_args = args.clone().next().is_some();
    let uses_objects = args.clone().any(|arg| {
        matches!(arg.kind, ArgKind::Object)
            || (arg.kind == ArgKind::NewId && arg.interface.is_some())
    });
    let uses_parent = args
        .clone()
        .any(|arg| arg.kind == ArgKind::NewId && arg.interface.is_some());
    let unused = |used: bool, name: &str| {
        if used {
            name.to_owned()
        } else {
            format!("_{name}")
        }
    };
    let request_type = types::incoming_type(messages, side, "'m");

    code.open(format!("impl {typed} for {handle} {{"))
        .line(match owned {
            true => format!("type {incoming} = {request_type};"),
            false => format!("type {incoming}<'m> = {request_type};"),
        })
        .blank()
        .line("const INTERFACE: &'static wire::Interface = &INTERFACE;")
        .blank()
        .open(format!(
            "fn {from_untyped}({as_untyped}: {untyped}) -> Option<Self> {{"
        ))
        .open(format!("if {as_untyped}.interface().is(&INTERFACE) {{"))
        .line(format!("Some(Self({as_untyped}))"))
        .reopen("} else {")
        .line("None")
        .close("}")
        .close("}")
        .blank()
        .open(format!("fn {as_untyped}(&self) -> &{untyped} {{"))
        .line("&self.0")
        .close("}")
        .blank()
        .open(match owned {
            true => "fn decode(",
            false => "fn decode<'m>(",
        })
        .line(format!("{}: &{untyped},", unused(uses_parent, "parent")))
        .line("opcode: u16,")
        .line(format!(
            "{}: &mut wire::Args<{}>,",
            unused(uses_args, "args"),
            if owned { "'_" } else { "'m" }
        ))
        .line(format!(
            "{}: &mut {objects},",
            unused(uses_objects, "objects")
        ))
        .reopen(format!(") -> wire::Result<{request_type}> {{"));
    if requests.is_empty() {
        code.line("Err(wire::WireError::UnknownOpcode(opcode))");
    } else {
        code.open("match opcode {");
        for (opcode, request) in requests.iter().enumerate() {
            let variant = &request.variant;
            if request.fields.is_empty() {
                code.line(format!("{opcode} => Ok({incoming}::{variant}),"));
                continue;
            }
            match request.words {
                Some(count) => {
                    code.open(format!("{opcode} => {{"))
                        .line(format!("let words = args.words::<{count}>()?;"))
                        .open(format!("Ok({incoming}::{variant} {{"));
                }
                None => {
                    code.open(format!("{opcode} => Ok({incoming}::{variant} {{"));
                }
            }
            for field in &request.fields {
                code.line(format!("{}: {},", field.name, field.decode));
            }
            match request.words {
                Some(_) => code.close("})").close("}"),
                None => code.close("}),"),
            };
        }
        code.line("_ => Err(wire::WireError::UnknownOpcode(opcode)),")
            .close("}");
    }
    code.close("}").close("}").blank();

    if binds && context.bindable(interface) {
        code.line(format!("impl wire::Bindable for {handle} {{}}"))
            .blank();
    }
}

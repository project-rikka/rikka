use crate::code::Code;
use crate::context::Context;
use crate::error::{Error, Result};
use crate::model::{ArgKind, Interface, Message};
use crate::names;
use crate::side::CLIENT;
use crate::types::{self, Param};

const ACCESSORS: &[&str] = &[
    "id", "version", "alive", "set_key", "key", "take_key", "with_key", "via",
];

enum Created {
    Nothing,
    Typed(String),
    Untyped,
}

pub(crate) fn emit<'p>(
    code: &mut Code,
    context: &Context<'p>,
    interface: &'p Interface,
) -> Result<()> {
    let proxy = names::camel(&interface.name);
    code.line("#[derive(Debug, Clone, PartialEq, Eq, Hash)]")
        .line(format!("pub struct {proxy}(wire::client::Handle);"))
        .blank()
        .open(format!("impl {proxy} {{"));
    accessors(code);
    for (opcode, request) in interface.requests.iter().enumerate() {
        method(code, context, interface, opcode, request)?;
    }
    code.close("}").blank();
    Ok(())
}

fn method<'p>(
    code: &mut Code,
    context: &Context<'p>,
    interface: &'p Interface,
    opcode: usize,
    request: &'p Message,
) -> Result<()> {
    let name = match names::snake(&request.name) {
        name if ACCESSORS.contains(&name.as_str()) => format!("send_{name}"),
        name => name,
    };
    let created = created(context, interface, request)?;
    let mut params = Vec::new();
    let mut encodes: Vec<String> = Vec::new();
    for arg in &request.args {
        if arg.kind == ArgKind::NewId {
            match &created {
                Created::Untyped => {
                    params.push(Param {
                        name: "version".to_owned(),
                        kind: "u32".to_owned(),
                        encode: String::new(),
                    });
                    encodes.push("untyped_new_id(P::INTERFACE, version)".to_owned());
                }
                _ => encodes.push("new_id()".to_owned()),
            }
            continue;
        }
        let param = types::param(context, interface, &request.name, arg, &CLIENT)?;
        encodes.push(param.encode.clone());
        params.push(param);
    }
    let signature: String = params
        .iter()
        .map(|param| format!(", {}: {}", param.name, param.kind))
        .collect();
    let lifetime = request.args.iter().any(|arg| arg.kind == ArgKind::Fd);
    let generics = match (&created, lifetime) {
        (Created::Untyped, true) => "<'f, P: wire::client::Proxy>",
        (Created::Untyped, false) => "<P: wire::client::Proxy>",
        (_, true) => "<'f>",
        (_, false) => "",
    };
    let returned = match &created {
        Created::Nothing => String::new(),
        Created::Typed(proxy) => format!(" -> {proxy}"),
        Created::Untyped => " -> P".to_owned(),
    };
    let writer = ["request", "writer"]
        .into_iter()
        .find(|candidate| params.iter().all(|param| param.name != *candidate))
        .unwrap_or("request_writer");
    code.blank().open(format!(
        "pub fn {name}{generics}(&self{signature}){returned} {{"
    ));
    if request.destructor {
        encodes.push("destroys()".to_owned());
    }
    let binding = if encodes.is_empty() { "let" } else { "let mut" };
    code.line(format!("{binding} {writer} = self.0.request({opcode});"));
    for encode in &encodes {
        code.line(format!("{writer}.{encode};"));
    }
    match created {
        Created::Nothing => code.line(format!("{writer}.send();")),
        Created::Typed(_) | Created::Untyped => code.line(format!("{writer}.send_constructor()")),
    };
    code.close("}");
    Ok(())
}

fn created<'p>(
    context: &Context<'p>,
    interface: &'p Interface,
    request: &'p Message,
) -> Result<Created> {
    let mut new_ids = request.args.iter().filter(|arg| arg.kind == ArgKind::NewId);
    let Some(first) = new_ids.next() else {
        return Ok(Created::Nothing);
    };
    if new_ids.next().is_some() {
        return Err(Error::Inconsistent(format!(
            "{}.{} creates more than one object",
            interface.name, request.name
        )));
    }
    Ok(match &first.interface {
        Some(target) => Created::Typed(context.handle(interface, target)),
        None => Created::Untyped,
    })
}

fn accessors(code: &mut Code) {
    code.open("pub fn id(&self) -> u32 {")
        .line("self.0.id()")
        .close("}")
        .blank()
        .open("pub fn version(&self) -> u32 {")
        .line("self.0.version()")
        .close("}")
        .blank()
        .open("pub fn alive(&self) -> bool {")
        .line("self.0.alive()")
        .close("}")
        .blank()
        .open("pub fn set_key<K: wire::Key>(&self, key: K) {")
        .line("self.0.set_key(key);")
        .close("}")
        .blank()
        .open("pub fn key<K: wire::Key>(&self) -> Option<K> {")
        .line("self.0.key()")
        .close("}")
        .blank()
        .open("pub fn take_key<K: wire::Key>(&self) -> Option<K> {")
        .line("self.0.take_key()")
        .close("}")
        .blank()
        .open("pub fn with_key<K: wire::Key>(self, key: K) -> Self {")
        .line("self.0.set_key(key);")
        .line("self")
        .close("}")
        .blank()
        .line("#[must_use]")
        .open("pub fn via(&self, route: &wire::client::Route) -> Self {")
        .line("Self(self.0.via(route))")
        .close("}");
}

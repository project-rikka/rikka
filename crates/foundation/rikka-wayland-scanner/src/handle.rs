use crate::code::Code;
use crate::context::Context;
use crate::error::Result;
use crate::model::{ArgKind, Interface};
use crate::names;
use crate::side::SERVER;
use crate::types;

const ACCESSORS: &[&str] = &[
    "id",
    "version",
    "client",
    "alive",
    "set_key",
    "key",
    "take_key",
    "post_error",
];

pub(crate) fn emit<'p>(
    code: &mut Code,
    context: &Context<'p>,
    interface: &'p Interface,
) -> Result<()> {
    let handle = names::camel(&interface.name);
    code.line("#[derive(Debug, Clone, PartialEq, Eq, Hash)]")
        .line(format!("pub struct {handle}(wire::Object);"))
        .blank()
        .open(format!("impl {handle} {{"));
    accessors(code, interface);
    for (opcode, event) in interface.events.iter().enumerate() {
        let method = match names::snake(&event.name) {
            name if ACCESSORS.contains(&name.as_str()) => format!("send_{name}"),
            name => name,
        };
        let params = event
            .args
            .iter()
            .map(|arg| types::param(context, interface, &event.name, arg, &SERVER))
            .collect::<Result<Vec<_>>>()?;
        let signature: String = params
            .iter()
            .map(|param| format!(", {}: {}", param.name, param.kind))
            .collect();
        let generics = if event.args.iter().any(|arg| arg.kind == ArgKind::Fd) {
            "<'f>"
        } else {
            ""
        };
        let writer = ["event", "writer"]
            .into_iter()
            .find(|candidate| params.iter().all(|param| param.name != *candidate))
            .unwrap_or("event_writer");
        code.blank()
            .open(format!("pub fn {method}{generics}(&self{signature}) {{"));
        if params.is_empty() {
            code.open(format!("if let Some({writer}) = self.0.event({opcode}) {{"));
        } else {
            code.open(format!(
                "if let Some(mut {writer}) = self.0.event({opcode}) {{"
            ));
            for param in &params {
                code.line(format!("{writer}.{};", param.encode));
            }
        }
        code.line(format!("{writer}.finish();"))
            .close("}")
            .close("}");
    }
    code.close("}").blank();
    Ok(())
}

fn accessors(code: &mut Code, interface: &Interface) {
    code.open("pub fn id(&self) -> u32 {")
        .line("self.0.id()")
        .close("}")
        .blank()
        .open("pub fn version(&self) -> u32 {")
        .line("self.0.version()")
        .close("}")
        .blank()
        .open("pub fn client(&self) -> wire::ClientId {")
        .line("self.0.client()")
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
        .close("}");
    let has_error = interface
        .enums
        .iter()
        .any(|definition| definition.name == "error" && !definition.bitfield);
    if has_error {
        code.blank()
            .open("pub fn post_error(&self, error: Error) {")
            .line("self.0.post_error(error.code(), error.summary());")
            .close("}");
    }
}

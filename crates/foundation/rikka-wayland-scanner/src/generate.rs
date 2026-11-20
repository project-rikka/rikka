use crate::code::Code;
use crate::context::Context;
use crate::error::{Error, Result};
use crate::model::{Interface, Protocol};
use crate::side::{CLIENT, SERVER};
use crate::{enums, handle, incoming, interface, proxy};

const ALLOWED: &[&str] = &[
    "clippy::too_many_arguments",
    "clippy::large_enum_variant",
    "clippy::enum_variant_names",
];

#[derive(Debug, Clone, Copy)]
pub(crate) enum Emission<'a> {
    Shared,
    Server { shared: &'a str },
    Client { shared: &'a str },
}

pub(crate) fn emit(
    protocols: &[Protocol],
    runtime: &str,
    emission: Emission<'_>,
) -> Result<String> {
    let context = Context::new(protocols, runtime)?;
    let mut code = Code::default();
    let interfaces = protocols.iter().flat_map(|protocol| &protocol.interfaces);
    for (index, interface) in interfaces.enumerate() {
        if u16::try_from(index).is_err() {
            return Err(Error::Inconsistent(format!(
                "interface {} is past the last index a u16 holds ({})",
                interface.name,
                u16::MAX
            )));
        }
        if index > 0 {
            code.blank();
        }
        module(&mut code, &context, interface, index, emission)?;
    }
    Ok(code.finish())
}

fn module<'p>(
    code: &mut Code,
    context: &Context<'p>,
    interface: &'p Interface,
    index: usize,
    emission: Emission<'_>,
) -> Result<()> {
    code.line(format!("#[allow({})]", ALLOWED.join(", ")));
    code.open(format!("pub mod {} {{", interface.name))
        .line(format!("use {} as wire;", context.runtime()));
    match emission {
        Emission::Shared => {
            code.blank();
            for definition in &interface.enums {
                enums::emit(code, definition);
            }
            interface::emit(code, context, interface, index);
        }
        Emission::Server { shared } => {
            reexport(code, shared, interface);
            handle::emit(code, context, interface)?;
            incoming::emit(code, context, interface, &interface.requests, &SERVER)?;
        }
        Emission::Client { shared } => {
            reexport(code, shared, interface);
            proxy::emit(code, context, interface)?;
            incoming::emit(code, context, interface, &interface.events, &CLIENT)?;
        }
    }
    code.close("}");
    Ok(())
}

fn reexport(code: &mut Code, shared: &str, interface: &Interface) {
    code.line(format!("pub use {shared}::{}::*;", interface.name))
        .blank();
}

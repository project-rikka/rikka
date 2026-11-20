use crate::code::{Code, quoted};
use crate::context::Context;
use crate::model::{ArgKind, Interface, Message};
use crate::types;

pub(crate) fn emit(code: &mut Code, context: &Context<'_>, interface: &Interface, index: usize) {
    code.open("pub static INTERFACE: wire::Interface = wire::Interface {")
        .line(format!("index: {index},"))
        .line(format!("name: {},", quoted(&interface.name)))
        .line(format!("version: {},", interface.version));
    messages(code, context, interface, "requests", &interface.requests);
    messages(code, context, interface, "events", &interface.events);
    code.close("};");
}

fn messages(
    code: &mut Code,
    context: &Context<'_>,
    interface: &Interface,
    field: &str,
    messages: &[Message],
) {
    if messages.is_empty() {
        code.line(format!("{field}: &[],"));
        return;
    }
    code.open(format!("{field}: &["));
    for message in messages {
        let signature = message
            .args
            .iter()
            .map(types::signature)
            .collect::<Vec<_>>()
            .join(", ");
        code.open("wire::MessageSpec {")
            .line(format!("name: {},", quoted(&message.name)))
            .line(format!("since: {},", message.since))
            .line(format!("destructor: {},", message.destructor))
            .line(format!("signature: &[{signature}],"))
            .line(format!(
                "children: &[{}],",
                children(context, interface, message)
            ))
            .close("},");
    }
    code.close("],");
}

fn children(context: &Context<'_>, interface: &Interface, message: &Message) -> String {
    message
        .args
        .iter()
        .filter(|arg| arg.kind == ArgKind::NewId)
        .filter_map(|arg| arg.interface.as_deref())
        .map(|target| format!("&{}", context.metadata(interface, target)))
        .collect::<Vec<_>>()
        .join(", ")
}

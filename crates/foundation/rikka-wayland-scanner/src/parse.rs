use std::path::Path;

use rikka_xml::Element;

use crate::error::{Error, Result};
use crate::model::{Arg, ArgKind, Entry, Enum, Interface, Message, Protocol};

pub(crate) fn protocol(path: &Path) -> Result<Protocol> {
    let text = std::fs::read_to_string(path).map_err(|source| Error::Read {
        path: path.to_owned(),
        source,
    })?;
    let root = rikka_xml::parse(&text).map_err(|source| Error::Xml {
        path: path.to_owned(),
        source,
    })?;
    let invalid = |message: String| Error::Invalid {
        path: path.to_owned(),
        message,
    };
    if root.name() != "protocol" {
        return Err(invalid(format!(
            "root element is <{}>, expected <protocol>",
            root.name()
        )));
    }
    let interfaces = root
        .children_named("interface")
        .map(|node| interface(node).map_err(invalid))
        .collect::<Result<_>>()?;
    Ok(Protocol { interfaces })
}

fn interface(node: &Element<'_>) -> Result<Interface, String> {
    let name = required(node, "name")?;
    let version = number(node, "version")?;
    let messages = |tag: &str| -> Result<Vec<Message>, String> {
        node.children_named(tag)
            .map(|child| message(child).map_err(|error| format!("{name}.{error}")))
            .collect()
    };
    Ok(Interface {
        requests: messages("request")?,
        events: messages("event")?,
        enums: node
            .children_named("enum")
            .map(|child| enumeration(child).map_err(|error| format!("{name}.{error}")))
            .collect::<Result<_, _>>()?,
        name,
        frozen: node.attribute("frozen") == Some("true"),
        version,
    })
}

fn message(node: &Element<'_>) -> Result<Message, String> {
    let name = required(node, "name")?;
    let since = match node.attribute("since") {
        Some(_) => number(node, "since").map_err(|error| format!("{name}: {error}"))?,
        None => 1,
    };
    let destructor = match node.attribute("type") {
        None => false,
        Some("destructor") => true,
        Some(other) => return Err(format!("{name} has unknown type {other:?}")),
    };
    let args = node
        .children_named("arg")
        .map(|child| arg(child).map_err(|error| format!("{name}: {error}")))
        .collect::<Result<_, _>>()?;
    Ok(Message {
        name,
        since,
        destructor,
        args,
    })
}

fn arg(node: &Element<'_>) -> Result<Arg, String> {
    let name = required(node, "name")?;
    let kind = match required(node, "type")?.as_str() {
        "int" => ArgKind::Int,
        "uint" => ArgKind::Uint,
        "fixed" => ArgKind::Fixed,
        "string" => ArgKind::String,
        "object" => ArgKind::Object,
        "new_id" => ArgKind::NewId,
        "array" => ArgKind::Array,
        "fd" => ArgKind::Fd,
        other => return Err(format!("argument {name} has unknown type {other:?}")),
    };
    let nullable = node.attribute("allow-null") == Some("true");
    if nullable && !matches!(kind, ArgKind::String | ArgKind::Object) {
        return Err(format!(
            "argument {name} is nullable but not a string or object"
        ));
    }
    let enumeration = node.attribute("enum").map(str::to_owned);
    if enumeration.is_some() && !matches!(kind, ArgKind::Int | ArgKind::Uint) {
        return Err(format!(
            "argument {name} names an enum but is not an integer"
        ));
    }
    Ok(Arg {
        name,
        kind,
        interface: node.attribute("interface").map(str::to_owned),
        nullable,
        enumeration,
    })
}

fn enumeration(node: &Element<'_>) -> Result<Enum, String> {
    let name = required(node, "name")?;
    let entries = node
        .children_named("entry")
        .map(|child| {
            Ok(Entry {
                name: required(child, "name")?,
                value: number(child, "value")?,
                summary: child
                    .attribute("summary")
                    .map(str::to_owned)
                    .or_else(|| description(child)),
            })
        })
        .collect::<Result<_, String>>()
        .map_err(|error| format!("{name}: {error}"))?;
    Ok(Enum {
        bitfield: node.attribute("bitfield") == Some("true"),
        name,
        entries,
    })
}

fn description(node: &Element<'_>) -> Option<String> {
    node.children_named("description")
        .find_map(|child| child.attribute("summary"))
        .map(str::to_owned)
}

fn required(node: &Element<'_>, attribute: &str) -> Result<String, String> {
    node.attribute(attribute)
        .map(str::to_owned)
        .ok_or_else(|| format!("<{}> has no {attribute} attribute", node.name()))
}

fn number(node: &Element<'_>, attribute: &str) -> Result<u32, String> {
    let text = required(node, attribute)?;
    let parsed = match text.strip_prefix("0x") {
        Some(hex) => u32::from_str_radix(hex, 16),
        None => text.parse(),
    };
    parsed.map_err(|_| format!("{attribute}={text:?} is not a number"))
}

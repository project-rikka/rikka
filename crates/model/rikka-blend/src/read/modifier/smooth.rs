use rikka_mesh_modifiers::SmoothByAngle;

use crate::file::Record;
use crate::read::idprop;

const PANEL_DEPTH: usize = 16;

struct Socket {
    name: String,
    identifier: String,
    default: f64,
}

pub(super) fn read(modifier: &Record<'_>) -> Option<SmoothByAngle> {
    let group = modifier.id("node_group")?;
    let title = group.id_name();
    if !(title.starts_with("Smooth by Angle") || title.starts_with("Auto Smooth")) {
        return None;
    }
    let sockets = interface(&group);
    let value = |name: &str| {
        sockets
            .iter()
            .find(|socket| socket.name == name)
            .map(|socket| input(modifier, &socket.identifier).unwrap_or(socket.default))
    };
    let angle = value("Angle").unwrap_or(std::f64::consts::FRAC_PI_6) as f32;
    let ignore = value("Ignore Sharpness").unwrap_or(0.0) != 0.0;
    Some(SmoothByAngle {
        angle,
        ignore_sharpness: ignore,
    })
}

pub(crate) fn socket_identifier(group: &Record<'_>, name: &str) -> Option<String> {
    interface(group)
        .into_iter()
        .find(|socket| socket.name == name)
        .map(|socket| socket.identifier)
}

fn interface(group: &Record<'_>) -> Vec<Socket> {
    let mut sockets = Vec::new();
    if let Some(root) = group
        .record("tree_interface")
        .and_then(|interface| interface.record("root_panel"))
    {
        panel(&root, &mut sockets, 0);
    }
    for socket in group.list("inputs") {
        let default = socket
            .deref("default_value")
            .and_then(|value| value.float("value"))
            .map_or(0.0, f64::from);
        sockets.push(Socket {
            name: socket.text("name").unwrap_or_default(),
            identifier: socket.text("identifier").unwrap_or_default(),
            default,
        });
    }
    sockets
}

fn panel(panel: &Record<'_>, sockets: &mut Vec<Socket>, depth: usize) {
    if depth > PANEL_DEPTH {
        return;
    }
    let count = panel.int("items_num").unwrap_or(0).max(0) as usize;
    for address in panel.pointers("items_array").into_iter().take(count) {
        let Some(item) = panel.data_at(address) else {
            continue;
        };
        match item.kind() {
            "bNodeTreeInterfaceSocket" => {
                let input = item.int("flag").unwrap_or(0) & 1 != 0;
                if !input {
                    continue;
                }
                let default = item
                    .deref("socket_data")
                    .and_then(|data| data.float("value"))
                    .map_or(0.0, f64::from);
                sockets.push(Socket {
                    name: item.string("name").unwrap_or_default(),
                    identifier: item.string("identifier").unwrap_or_default(),
                    default,
                });
            }
            "bNodeTreeInterfacePanel" => self::panel(&item, sockets, depth + 1),
            _ => {}
        }
    }
}

fn input(modifier: &Record<'_>, identifier: &str) -> Option<f64> {
    let base = modifier.record("modifier")?;
    if let Some(system) = base.deref("system_properties")
        && let Some(inputs) = idprop::child(&system, "inputs")
        && let Some(entry) = idprop::child(&inputs, identifier)
        && let Some(value) = idprop::child(&entry, "value")
    {
        return idprop::number(&value);
    }
    let properties = modifier
        .record("settings")
        .or_else(|| modifier.record("settings_legacy"))
        .and_then(|settings| settings.deref("properties"))?;
    let entry = idprop::child(&properties, identifier)?;
    idprop::number(&entry)
}

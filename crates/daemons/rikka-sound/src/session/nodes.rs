use std::cell::RefCell;
use std::rc::Weak;

use libspa::node::command::{NodeCommand, NodeCommandId};
use libspa::param::{ParamInfoFlags, ParamType};
use libspa::pod::Pod;
use libspa::utils::dict::DictRef;
use pipewire::keys;
use pipewire::node::{Node as NodeProxy, NodeChangeMask, NodeListener, NodeState};
use pipewire::registry::GlobalObject;

use crate::config::DeviceTuning;
use crate::node::{Node, NodeKind};
use crate::spectrum;
use crate::volume::Volume;

use super::alsa::{self, Pins};
use super::format_query::FormatQuery;
use super::{Session, meters, pods, with};

pub(super) struct WatchedNode {
    pub(super) proxy: NodeProxy,
    _listener: NodeListener,
    pub(super) node: Node,
    alsa: bool,
    listing: Option<ParamInfoFlags>,
    pub(super) pins: Option<Pins>,
    pub(super) format_query: Option<FormatQuery>,
}

impl Session {
    pub(super) fn add_node(
        &mut self,
        global: &GlobalObject<&DictRef>,
        watcher: &Weak<RefCell<Session>>,
    ) {
        let Some(props) = global.props else {
            return;
        };
        let Some(kind) = props
            .get(*keys::MEDIA_CLASS)
            .and_then(NodeKind::from_media_class)
        else {
            return;
        };
        if matches!(
            props.get(*keys::NODE_NAME),
            Some(meters::METER | spectrum::NAME)
        ) {
            return;
        }
        let Ok(proxy) = self.registry.bind::<NodeProxy, _>(global) else {
            return;
        };

        let id = global.id;
        let informed = watcher.clone();
        let parametered = watcher.clone();
        let listener = proxy
            .add_listener_local()
            .info(move |info| {
                with(&informed, |session| {
                    let Some(watched) = session.nodes.get_mut(&id) else {
                        return;
                    };
                    let watched_node = watched.node.clone();
                    let props = info
                        .change_mask()
                        .contains(NodeChangeMask::PROPS)
                        .then(|| info.props())
                        .flatten();
                    if let Some(props) = props {
                        describe(&mut watched.node, props);
                        if !watched.alsa && alsa_card(watched.node.kind, props) {
                            watched.alsa = true;
                            watched.node.hardware = Some(session.learnt.get(&watched.node.name));
                        }
                    }
                    watched.node.running = matches!(info.state(), NodeState::Running);
                    let listing = info
                        .params()
                        .iter()
                        .find(|param| param.id() == ParamType::EnumFormat)
                        .map(|param| param.flags());
                    let relisted = watched.alsa && listing.is_some() && listing != watched.listing;
                    watched.listing = listing;
                    let moved = watched_node != session.nodes[&id].node;
                    session.publish_if(moved);
                    if relisted {
                        session.query_formats(id);
                    }
                });
            })
            .param(move |_, param, _, _, pod| {
                with(&parametered, |session| session.node_param(id, param, pod));
            })
            .register();
        proxy.subscribe_params(&[ParamType::Props, ParamType::Format]);

        let mut node = Node {
            id,
            serial: 0,
            kind,
            name: String::new(),
            title: String::new(),
            detail: None,
            icon: None,
            device: None,
            route: None,
            volume: Volume::default(),
            format: None,
            quantum: None,
            target: None,
            latency_offset: 0,
            running: false,
            hardware: None,
        };
        describe(&mut node, props);
        self.nodes.insert(
            id,
            WatchedNode {
                proxy,
                _listener: listener,
                node,
                alsa: false,
                listing: None,
                pins: None,
                format_query: None,
            },
        );
        self.publish();
    }

    fn node_param(&mut self, id: u32, param: ParamType, pod: Option<&Pod>) {
        let Some(pod) = pod else {
            return;
        };
        if param == ParamType::EnumFormat {
            self.record_format(id, pod);
            return;
        }
        let Some(watched) = self.nodes.get_mut(&id) else {
            return;
        };
        let mut moved = false;
        match param {
            ParamType::Props => {
                if let Some(pins) = alsa::pins(pod) {
                    watched.pins = Some(pins);
                }
                if let Some(volume) = pods::volume(pod) {
                    moved |= watched.node.volume != volume;
                    watched.node.volume = volume;
                }
                if let Some(offset) = pods::latency_offset(pod) {
                    moved |= watched.node.latency_offset != offset;
                    watched.node.latency_offset = offset;
                }
            }
            ParamType::Format => {
                let format = pods::format(pod);
                moved = watched.node.format != format;
                watched.node.format = format;
            }
            _ => return,
        }
        self.publish_if(moved);
    }

    pub(super) fn set_node_param(&self, node: u32, param: ParamType, pod: &[u8]) {
        let (Some(watched), Some(pod)) = (self.nodes.get(&node), Pod::from_bytes(pod)) else {
            return;
        };
        watched.proxy.set_param(param, 0, pod);
    }

    pub(super) fn tune(&self, node: u32, tuning: &DeviceTuning) {
        let Some(watched) = self.nodes.get(&node).filter(|watched| watched.alsa) else {
            return;
        };
        self.set_node_param(node, ParamType::Props, &alsa::tuning_param(tuning));
        watched
            .proxy
            .send_command(&NodeCommand::new(NodeCommandId::SUSPEND));
    }
}

fn alsa_card(kind: NodeKind, props: &DictRef) -> bool {
    matches!(kind, NodeKind::Sink | NodeKind::Source)
        && props.get(*keys::DEVICE_API) == Some("alsa")
}

fn describe(node: &mut Node, props: &DictRef) {
    if let Some(serial) = props.get(*keys::OBJECT_SERIAL).and_then(|s| s.parse().ok()) {
        node.serial = serial;
    }
    if let Some(name) = props.get(*keys::NODE_NAME) {
        node.name = name.to_owned();
    }
    let title = props
        .get(*keys::NODE_DESCRIPTION)
        .or_else(|| props.get(*keys::NODE_NICK))
        .or_else(|| props.get(*keys::APP_NAME))
        .or_else(|| props.get(*keys::MEDIA_NAME));
    if let Some(title) = title {
        node.title = title.to_owned();
    }
    if node.title.is_empty() {
        node.title = node.name.clone();
    }
    node.detail = node
        .kind
        .is_stream()
        .then(|| props.get(*keys::MEDIA_NAME).map(str::to_owned))
        .flatten()
        .filter(|detail| *detail != node.title);
    node.icon = props
        .get(*keys::APP_ICON_NAME)
        .or_else(|| props.get(*keys::DEVICE_ICON_NAME))
        .map(str::to_owned);
    node.device = props.get(*keys::DEVICE_ID).and_then(|id| id.parse().ok());
    node.route = props
        .get("card.profile.device")
        .and_then(|index| index.parse().ok());
    node.target = props.get(*keys::TARGET_OBJECT).map(str::to_owned);
    node.quantum = props.get(*keys::NODE_LATENCY).and_then(quantum);
}

fn quantum(latency: &str) -> Option<(u32, u32)> {
    let (quantum, rate) = latency.split_once('/')?;
    Some((quantum.parse().ok()?, rate.parse().ok()?))
}

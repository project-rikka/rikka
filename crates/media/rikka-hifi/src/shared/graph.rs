use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::{Rc, Weak};

use libspa::param::ParamType;
use libspa::pod::Pod;
use libspa::utils::dict::DictRef;
use pipewire::keys;
use pipewire::metadata::{Metadata, MetadataListener};
use pipewire::node::{Node, NodeChangeMask, NodeListener};
use pipewire::permissions::PermissionFlags;
use pipewire::registry::{GlobalObject, RegistryRc};
use pipewire::types::ObjectType;
use rikka_hifi_core::{SharedPath, SinkPath};

use crate::exclusive::probe;
use crate::notice::{Notice, Notifier};
use crate::shared::pods;

pub(super) struct Graph {
    registry: RegistryRc,
    nodes: HashMap<u32, u32>,
    links: HashMap<u32, (u32, u32)>,
    stream: Option<u32>,
    sink: Option<Sink>,
    settings: Option<Settings>,
    path: SharedPath,
    card_rates: bool,
    probed: Option<(u32, u32)>,
    last_notified: Option<SharedPath>,
    notifier: Notifier,
}

struct Sink {
    id: u32,
    _proxy: Node,
    _listener: NodeListener,
}

struct Settings {
    id: u32,
    _proxy: Metadata,
    _listener: MetadataListener,
}

impl Graph {
    pub(super) fn new(
        registry: RegistryRc,
        rate: u32,
        channels: u16,
        volume: f32,
        notifier: Notifier,
    ) -> Rc<RefCell<Graph>> {
        Rc::new(RefCell::new(Graph {
            registry,
            nodes: HashMap::new(),
            links: HashMap::new(),
            stream: None,
            sink: None,
            settings: None,
            path: SharedPath {
                stream_rate: rate,
                stream_channels: channels,
                stream_volume: volume,
                sink: None,
                sink_volume: None,
                clock_rate: None,
                forced_rate: None,
                allowed_rates: Vec::new(),
            },
            card_rates: false,
            probed: None,
            last_notified: None,
            notifier,
        }))
    }

    pub(super) fn added(
        &mut self,
        global: &GlobalObject<&DictRef>,
        watcher: &Weak<RefCell<Graph>>,
    ) {
        match global.type_ {
            ObjectType::Node => {
                self.nodes.insert(global.id, global.version);
                self.relink(watcher);
            }
            ObjectType::Link => {
                let Some(props) = global.props else {
                    return;
                };
                let end = |key: &str| props.get(key).and_then(|id| id.parse().ok());
                if let (Some(output), Some(input)) =
                    (end("link.output.node"), end("link.input.node"))
                {
                    self.links.insert(global.id, (output, input));
                    self.relink(watcher);
                }
            }
            ObjectType::Metadata
                if global.props.and_then(|props| props.get("metadata.name"))
                    == Some("settings") =>
            {
                self.watch_settings(global, watcher);
            }
            _ => {}
        }
    }

    pub(super) fn removed(&mut self, id: u32, watcher: &Weak<RefCell<Graph>>) {
        self.nodes.remove(&id);
        self.links.remove(&id);
        if self
            .settings
            .as_ref()
            .is_some_and(|settings| settings.id == id)
        {
            self.settings = None;
        }
        self.relink(watcher);
    }

    pub(super) fn streaming_as(&mut self, node: u32, watcher: &Weak<RefCell<Graph>>) {
        if self.stream != Some(node) {
            self.stream = Some(node);
            self.relink(watcher);
        }
    }

    pub(super) fn set_volume(&mut self, volume: f32) {
        self.path.stream_volume = volume;
        self.notify();
    }

    fn linked_sink(&self) -> Option<u32> {
        let stream = self.stream?;
        self.links
            .values()
            .find(|(output, _)| *output == stream)
            .map(|(_, input)| *input)
            .filter(|input| self.nodes.contains_key(input))
    }

    fn relink(&mut self, watcher: &Weak<RefCell<Graph>>) {
        let linked = self.linked_sink();
        if linked == self.sink.as_ref().map(|sink| sink.id) {
            self.notify();
            return;
        }
        self.sink = None;
        self.path.sink = None;
        self.path.sink_volume = None;
        self.card_rates = false;
        self.probed = None;
        if let Some(id) = linked {
            self.sink = self.watch_sink(id, watcher);
        }
        self.notify();
    }

    fn watch_sink(&self, id: u32, watcher: &Weak<RefCell<Graph>>) -> Option<Sink> {
        let global = GlobalObject::<&DictRef> {
            id,
            permissions: PermissionFlags::empty(),
            type_: ObjectType::Node,
            version: *self.nodes.get(&id)?,
            props: None,
        };
        let proxy = self.registry.bind::<Node, _>(&global).ok()?;
        let informed = watcher.clone();
        let parametered = watcher.clone();
        let listener = proxy
            .add_listener_local()
            .info(move |info| {
                let Some(props) = info
                    .change_mask()
                    .contains(NodeChangeMask::PROPS)
                    .then(|| info.props())
                    .flatten()
                else {
                    return;
                };
                with(&informed, |graph| graph.described(id, props));
            })
            .param(move |_, param, _, _, pod| {
                with(&parametered, |graph| graph.parametered(id, param, pod));
            })
            .register();
        proxy.subscribe_params(&[ParamType::Format, ParamType::Props]);
        proxy.enum_params(0, Some(ParamType::EnumFormat), 0, u32::MAX);
        Some(Sink {
            id,
            _proxy: proxy,
            _listener: listener,
        })
    }

    fn sink_path(&mut self) -> &mut SinkPath {
        self.path.sink.get_or_insert_with(|| SinkPath {
            name: String::new(),
            description: String::new(),
            rate: None,
            format: None,
            channels: None,
            bluetooth: None,
            card: None,
            hardware_rates: Vec::new(),
        })
    }

    fn described(&mut self, id: u32, props: &DictRef) {
        if self.sink.as_ref().map(|sink| sink.id) != Some(id) {
            return;
        }
        let sink = self.sink_path();
        if let Some(name) = props.get(*keys::NODE_NAME) {
            name.clone_into(&mut sink.name);
        }
        if let Some(description) = props
            .get(*keys::NODE_DESCRIPTION)
            .or_else(|| props.get(*keys::NODE_NICK))
        {
            description.clone_into(&mut sink.description);
        }
        sink.bluetooth = props.get("api.bluez5.codec").map(str::to_owned);
        sink.card = props
            .get("api.alsa.pcm.card")
            .or_else(|| props.get("alsa.card"))
            .and_then(|card| card.parse().ok());
        let device = props
            .get("alsa.device")
            .and_then(|device| device.parse().ok());
        if let Some(card) = sink.card
            && let Some(device) = device
        {
            self.probe(card, device);
        }
        self.notify();
    }

    fn probe(&mut self, card: u32, device: u32) {
        if self.probed == Some((card, device)) {
            return;
        }
        self.probed = Some((card, device));
        if let Some(rates) = probe::card_rates(card, device) {
            self.sink_path().hardware_rates = rates;
            self.card_rates = true;
        }
    }

    fn parametered(&mut self, id: u32, param: ParamType, pod: Option<&Pod>) {
        if self.sink.as_ref().map(|sink| sink.id) != Some(id) {
            return;
        }
        let Some(pod) = pod else {
            return;
        };
        match param {
            ParamType::Format => {
                let negotiated = pods::negotiated(pod);
                let sink = self.sink_path();
                sink.rate = negotiated.map(|format| format.rate);
                sink.channels = negotiated.map(|format| format.channels);
                sink.format = negotiated.and_then(|format| format.format);
            }
            ParamType::Props => {
                if let Some(level) = pods::soft_volume(pod) {
                    self.path.sink_volume = Some(level);
                }
            }
            ParamType::EnumFormat => {
                if self.card_rates {
                    return;
                }
                let offered = pods::offered_rates(pod);
                let sink = self.sink_path();
                for rate in offered {
                    if !sink.hardware_rates.contains(&rate) {
                        sink.hardware_rates.push(rate);
                    }
                }
                sink.hardware_rates.sort_unstable();
            }
            _ => return,
        }
        self.notify();
    }

    fn watch_settings(&mut self, global: &GlobalObject<&DictRef>, watcher: &Weak<RefCell<Graph>>) {
        let Ok(proxy) = self.registry.bind::<Metadata, _>(global) else {
            return;
        };
        let graph_ref = watcher.clone();
        let listener = proxy
            .add_listener_local()
            .property(move |subject, key, _, value| {
                if subject == 0 {
                    with(&graph_ref, |graph| graph.apply_clock_setting(key, value));
                }
                0
            })
            .register();
        self.settings = Some(Settings {
            id: global.id,
            _proxy: proxy,
            _listener: listener,
        });
    }

    fn apply_clock_setting(&mut self, key: Option<&str>, value: Option<&str>) {
        let number = || {
            value
                .and_then(rikka_pipewire::number)
                .filter(|rate| *rate > 0)
        };
        match key {
            Some("clock.rate") => self.path.clock_rate = number(),
            Some("clock.force-rate") => self.path.forced_rate = number(),
            Some("clock.allowed-rates") => {
                self.path.allowed_rates = value.map(rikka_pipewire::numbers).unwrap_or_default();
            }
            _ => return,
        }
        self.notify();
    }

    fn notify(&mut self) {
        if self.last_notified.as_ref() == Some(&self.path) {
            return;
        }
        self.last_notified = Some(self.path.clone());
        self.notifier.send(Notice::Graph(self.path.clone()));
    }
}

pub(super) fn with(graph: &Weak<RefCell<Graph>>, act: impl FnOnce(&mut Graph)) {
    if let Some(graph) = graph.upgrade() {
        act(&mut graph.borrow_mut());
    }
}

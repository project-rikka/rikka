use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use pipewire::metadata::Metadata;
use pipewire::spa::utils::dict::DictRef;
use pipewire::types::ObjectType;
use rikka_pipewire::Connection;

use crate::device_watch::DeviceDirection;
use crate::error::{Error, Result};

const SETTLE_TIMEOUT: Duration = Duration::from_secs(3);
const SETTLE_ROUNDS: usize = 2;
const SOURCE_CLASS: &str = "Audio/Source";
const SINK_CLASS: &str = "Audio/Sink";
const DUPLEX_CLASS: &str = "Audio/Duplex";
const DEFAULTS: &str = "default";
const DEFAULT_SINK: &str = "default.audio.sink";
const DEFAULT_SOURCE: &str = "default.audio.source";
const CLOCK_RATE: &str = "default.clock.rate";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Device {
    pub(crate) node: String,
    pub(crate) label: String,
    pub(crate) direction: DeviceDirection,
}

impl Device {
    pub(crate) fn read(props: &DictRef) -> Option<Device> {
        let direction = match props.get("media.class") {
            Some(SOURCE_CLASS) => DeviceDirection::Input,
            Some(SINK_CLASS) => DeviceDirection::Output,
            Some(DUPLEX_CLASS) => DeviceDirection::Duplex,
            _ => return None,
        };
        let node = props.get("node.name").unwrap_or_default();
        Some(Device {
            node: node.to_owned(),
            label: props.get("node.description").unwrap_or(node).to_owned(),
            direction,
        })
    }

    pub(crate) fn plays(&self) -> bool {
        self.direction != DeviceDirection::Input
    }

    pub(crate) fn records(&self) -> bool {
        self.direction != DeviceDirection::Output
    }
}

#[derive(Debug, Default)]
pub(crate) struct Graph {
    pub(crate) devices: Vec<Device>,
    pub(crate) clock_rate: Option<u32>,
    default_sink: Option<String>,
    default_source: Option<String>,
}

impl Graph {
    pub(crate) fn read(connection: &Connection) -> Result<Graph> {
        let graph = Rc::new(RefCell::new(Graph::default()));

        let clocked = Rc::clone(&graph);
        let _core = connection
            .core()
            .add_listener_local()
            .info(move |info| {
                let rate = info
                    .props()
                    .and_then(|props| props.get(CLOCK_RATE))
                    .and_then(|rate| rate.parse().ok());
                if rate.is_some() {
                    clocked.borrow_mut().clock_rate = rate;
                }
            })
            .register();

        let defaults = Rc::new(RefCell::new(Vec::new()));
        let held = Rc::clone(&defaults);
        let listed = Rc::clone(&graph);
        let registry = connection.registry().clone();
        let _registry = connection
            .registry()
            .add_listener_local()
            .global(move |global| {
                let Some(props) = global.props else {
                    return;
                };
                match global.type_ {
                    ObjectType::Node => listed.borrow_mut().devices.extend(Device::read(props)),
                    ObjectType::Metadata if props.get("metadata.name") == Some(DEFAULTS) => {
                        let Ok(proxy) = registry.bind::<Metadata, _>(global) else {
                            return;
                        };
                        let noted = Rc::clone(&listed);
                        let listener = proxy
                            .add_listener_local()
                            .property(move |subject, key, _, value| {
                                if subject == 0 {
                                    noted.borrow_mut().note_default(key, value);
                                }
                                0
                            })
                            .register();
                        held.borrow_mut().push((proxy, listener));
                    }
                    _ => {}
                }
            })
            .register();

        for _ in 0..SETTLE_ROUNDS {
            if !connection.settle(SETTLE_TIMEOUT)? {
                return Err(Error::Unanswered);
            }
        }
        Ok(graph.take())
    }

    fn note_default(&mut self, key: Option<&str>, value: Option<&str>) {
        match key {
            Some(DEFAULT_SINK) => self.default_sink = value.and_then(named),
            Some(DEFAULT_SOURCE) => self.default_source = value.and_then(named),
            _ => {}
        }
    }

    pub(crate) fn outputs(&self) -> impl Iterator<Item = &Device> {
        self.devices.iter().filter(|device| device.plays())
    }

    pub(crate) fn inputs(&self) -> impl Iterator<Item = &Device> {
        self.devices.iter().filter(|device| device.records())
    }

    pub(crate) fn find(&self, name: &str) -> Option<&Device> {
        self.devices
            .iter()
            .find(|device| device.label == name)
            .or_else(|| self.devices.iter().find(|device| device.node == name))
    }

    pub(crate) fn is_default(&self, device: &Device) -> bool {
        let default = match device.plays() {
            true => &self.default_sink,
            false => &self.default_source,
        };
        default.as_deref() == Some(device.node.as_str())
    }
}

fn named(value: &str) -> Option<String> {
    let after = value.split_once("\"name\"")?.1;
    let quoted = after.trim_start().strip_prefix(':')?.trim_start();
    let (name, _) = quoted.strip_prefix('"')?.split_once('"')?;
    Some(name.to_owned())
}

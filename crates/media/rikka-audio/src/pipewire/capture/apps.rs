use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::rc::{Rc, Weak};
use std::time::Duration;

use pipewire::core::CoreRc;
use pipewire::keys;
use pipewire::link::Link;
use pipewire::properties::properties;
use pipewire::registry::GlobalObject;
use pipewire::spa::utils::dict::DictRef;
use pipewire::types::ObjectType;
use rikka_pipewire::Connection;

use crate::capture::AudioApp;
use crate::error::{Error, Result};

const PLAYBACK: &str = "Stream/Output/Audio";
const LIST_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Linked {
    Others,
    Only(Vec<String>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Channel {
    Left,
    Right,
    Both,
}

impl Channel {
    fn named(name: Option<&str>) -> Channel {
        match name {
            Some("FL" | "RL" | "SL" | "FLC" | "RLC" | "TFL" | "TRL") => Channel::Left,
            Some("FR" | "RR" | "SR" | "FRC" | "RRC" | "TFR" | "TRR") => Channel::Right,
            _ => Channel::Both,
        }
    }
}

type Props = HashMap<String, String>;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Port {
    node: u32,
    output: bool,
    channel: Channel,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Graph {
    nodes: HashMap<u32, Props>,
    ports: HashMap<u32, Port>,
    clients: HashMap<u32, Props>,
}

fn prop<'a>(props: &'a Props, key: &str) -> Option<&'a str> {
    props
        .get(key)
        .map(String::as_str)
        .filter(|value| !value.is_empty())
}

impl Graph {
    fn track(&mut self, global: &GlobalObject<&DictRef>) -> bool {
        let Some(props) = global.props else {
            return false;
        };
        match global.type_ {
            ObjectType::Node => {
                self.nodes.insert(global.id, collect(props));
                true
            }
            ObjectType::Port => {
                let Some(node) = props.get(*keys::NODE_ID).and_then(|node| node.parse().ok())
                else {
                    return false;
                };
                self.ports.insert(
                    global.id,
                    Port {
                        node,
                        output: props.get(*keys::PORT_DIRECTION) == Some("out"),
                        channel: Channel::named(props.get(*keys::AUDIO_CHANNEL)),
                    },
                );
                true
            }
            ObjectType::Client => {
                self.clients.insert(global.id, collect(props));
                true
            }
            _ => false,
        }
    }

    fn forget(&mut self, id: u32) -> bool {
        self.nodes.remove(&id).is_some()
            | self.ports.remove(&id).is_some()
            | self.clients.remove(&id).is_some()
    }

    fn playing_nodes(&self) -> impl Iterator<Item = (u32, &Props)> {
        self.nodes
            .iter()
            .filter(|(_, props)| prop(props, "media.class") == Some(PLAYBACK))
            .map(|(&id, props)| (id, props))
    }

    fn pid(&self, props: &Props) -> Option<u32> {
        prop(props, "application.process.id")
            .or_else(|| {
                let client = prop(props, "client.id")?.parse::<u32>().ok()?;
                let client = self.clients.get(&client)?;
                prop(client, "application.process.id").or_else(|| prop(client, "pipewire.sec.pid"))
            })
            .and_then(|pid| pid.parse().ok())
    }

    fn key(props: &Props) -> Option<String> {
        prop(props, "application.process.binary")
            .or_else(|| prop(props, "application.name"))
            .or_else(|| prop(props, "node.name"))
            .map(str::to_lowercase)
    }

    fn foreign_playing(&self, own_pid: u32) -> impl Iterator<Item = (u32, &Props)> {
        self.playing_nodes()
            .filter(move |(_, props)| self.pid(props) != Some(own_pid))
    }

    fn apps(&self, own_pid: u32) -> Vec<AudioApp> {
        let mut found: BTreeMap<String, AudioApp> = BTreeMap::new();
        for (_, props) in self.foreign_playing(own_pid) {
            let Some(key) = Graph::key(props) else {
                continue;
            };
            let name = prop(props, "application.name")
                .or_else(|| prop(props, "application.process.binary"))
                .or_else(|| prop(props, "node.description"))
                .unwrap_or(key.as_str())
                .to_owned();
            found.entry(key.clone()).or_insert(AudioApp {
                key,
                name,
                icon: prop(props, "application.icon-name").map(str::to_owned),
            });
        }
        found.into_values().collect()
    }

    fn plan(&self, capture_node: u32, linked: &Linked, own_pid: u32) -> BTreeSet<(u32, u32)> {
        let inputs: Vec<(u32, Channel)> = self
            .ports
            .iter()
            .filter(|(_, port)| port.node == capture_node && !port.output)
            .map(|(&id, port)| (id, port.channel))
            .collect();
        let chosen: BTreeSet<u32> = self
            .foreign_playing(own_pid)
            .filter(|(_, props)| match linked {
                Linked::Others => true,
                Linked::Only(keys) => Graph::key(props).is_some_and(|key| keys.contains(&key)),
            })
            .map(|(id, _)| id)
            .collect();
        let mut links = BTreeSet::new();
        for (&output, port) in &self.ports {
            if !port.output || !chosen.contains(&port.node) {
                continue;
            }
            for &(input, channel) in &inputs {
                let joined = port.channel == Channel::Both
                    || channel == Channel::Both
                    || port.channel == channel;
                if joined {
                    links.insert((output, input));
                }
            }
        }
        links
    }
}

pub(crate) fn list() -> Result<Vec<AudioApp>> {
    let connection = Connection::open()?;
    let graph = Rc::new(RefCell::new(Graph::default()));
    let tracked = Rc::downgrade(&graph);
    let _registry = connection
        .registry()
        .add_listener_local()
        .global(move |global| {
            if let Some(graph) = tracked.upgrade() {
                graph.borrow_mut().track(global);
            }
        })
        .register();
    if !connection.settle(LIST_TIMEOUT)? {
        return Err(Error::Unanswered);
    }
    let apps = graph.borrow().apps(std::process::id());
    Ok(apps)
}

pub(super) struct Linker {
    core: CoreRc,
    graph: Graph,
    linked: Linked,
    capture_node: Option<u32>,
    links: HashMap<(u32, u32), Link>,
    own_pid: u32,
}

impl Linker {
    pub(super) fn new(core: CoreRc, linked: Linked) -> Rc<RefCell<Linker>> {
        Rc::new(RefCell::new(Linker {
            core,
            graph: Graph::default(),
            linked,
            capture_node: None,
            links: HashMap::new(),
            own_pid: std::process::id(),
        }))
    }

    pub(super) fn watch(
        linker: &Rc<RefCell<Linker>>,
        connection: &Connection,
    ) -> impl Sized + use<> {
        let appeared = Rc::downgrade(linker);
        let vanished = Rc::downgrade(linker);
        connection
            .registry()
            .add_listener_local()
            .global(move |global| {
                with(&appeared, |linker| {
                    if linker.graph.track(global) {
                        linker.update();
                    }
                });
            })
            .global_remove(move |id| {
                with(&vanished, |linker| {
                    if linker.graph.forget(id) {
                        linker.update();
                    }
                });
            })
            .register()
    }

    pub(super) fn capture_into(linker: &Weak<RefCell<Linker>>, node: u32) {
        with(linker, |linker| {
            if linker.capture_node != Some(node) {
                linker.capture_node = Some(node);
                linker.update();
            }
        });
    }

    pub(super) fn unlink(&mut self) {
        self.links.clear();
    }

    fn update(&mut self) {
        let Some(capture_node) = self.capture_node else {
            return;
        };
        let planned = self.graph.plan(capture_node, &self.linked, self.own_pid);
        self.links.retain(|pair, _| planned.contains(pair));
        for (output, input) in planned {
            if self.links.contains_key(&(output, input)) {
                continue;
            }
            let Some(from) = self.graph.ports.get(&output).map(|port| port.node) else {
                continue;
            };
            let linked = self.core.create_object::<Link>(
                "link-factory",
                &properties! {
                    "link.output.node" => from.to_string(),
                    "link.output.port" => output.to_string(),
                    "link.input.node" => capture_node.to_string(),
                    "link.input.port" => input.to_string(),
                    "object.linger" => "false",
                },
            );
            match linked {
                Ok(link) => {
                    self.links.insert((output, input), link);
                }
                Err(error) => tracing::debug!(%error, output, input, "failed to link audio ports"),
            }
        }
    }
}

fn with(linker: &Weak<RefCell<Linker>>, act: impl FnOnce(&mut Linker)) {
    if let Some(linker) = linker.upgrade() {
        act(&mut linker.borrow_mut());
    }
}

fn collect(props: &DictRef) -> Props {
    props
        .iter()
        .map(|(key, value)| (key.to_owned(), value.to_owned()))
        .collect()
}

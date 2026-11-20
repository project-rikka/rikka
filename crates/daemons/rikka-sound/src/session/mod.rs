mod alsa;
mod defaults;
mod devices;
mod format_query;
mod metadata;
mod meters;
mod nodes;
mod pods;

use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::ControlFlow;
use std::rc::{Rc, Weak};

use libspa::param::ParamType;
use libspa::utils::dict::DictRef;
use pipewire::core::CoreRc;
use pipewire::registry::{GlobalObject, RegistryRc};
use pipewire::types::ObjectType;
use rikka_pipewire::Connection;

use crate::command::Command;
use crate::config::drop_in;
use crate::device::Device;
use crate::error::{Error, Result};
use crate::graph::Graph;
use crate::learnt::Learnt;
use crate::node::Node;
use crate::peaks::Peaks;
use crate::snapshot::Snapshot;

use defaults::Defaults;
use devices::WatchedDevice;
use metadata::WatchedMetadata;
use meters::Meter;
use nodes::WatchedNode;

struct Session {
    core: CoreRc,
    registry: RegistryRc,
    nodes: HashMap<u32, WatchedNode>,
    devices: HashMap<u32, WatchedDevice>,
    knobs: Option<WatchedMetadata>,
    unsaid: Vec<(&'static str, String)>,
    default_metadata: Option<WatchedMetadata>,
    defaults: Defaults,
    graph: Graph,
    meters: HashMap<u32, Meter>,
    peaks: Peaks,
    learnt: Learnt,
    report: Box<dyn Fn(Snapshot)>,
    reported: Snapshot,
}

pub(crate) fn open(
    report: Box<dyn Fn(Snapshot) + Send>,
    commands: pipewire::channel::Receiver<Command>,
    peaks: Peaks,
) -> Result<impl FnOnce()> {
    let connection = Connection::open().map_err(unreachable)?;
    Ok(move || run(&connection, report, commands, peaks))
}

fn run(
    connection: &Connection,
    report: Box<dyn Fn(Snapshot) + Send>,
    commands: pipewire::channel::Receiver<Command>,
    peaks: Peaks,
) {
    let main_loop = connection.main_loop();
    let core = connection.core();
    let registry = connection.registry();
    let session = Rc::new(RefCell::new(Session {
        core: core.clone(),
        registry: registry.clone(),
        nodes: HashMap::new(),
        devices: HashMap::new(),
        knobs: None,
        unsaid: Vec::new(),
        default_metadata: None,
        defaults: Defaults::default(),
        graph: Graph::default(),
        meters: HashMap::new(),
        peaks,
        learnt: Learnt::load(),
        report,
        reported: Snapshot::default(),
    }));

    let dying = main_loop.downgrade();
    let session_ref = Rc::downgrade(&session);
    let _core_listener = core
        .add_listener_local()
        .done(move |id, pending| {
            if id == pipewire::core::PW_ID_CORE {
                with(&session_ref, |session| session.finish_format_query(pending));
            }
        })
        .error(move |id, _, _, _| {
            if id == 0
                && let Some(main_loop) = dying.upgrade()
            {
                main_loop.quit();
            }
        })
        .register();

    let appearing = Rc::downgrade(&session);
    let disappearing = Rc::downgrade(&session);
    let _registry_listener = registry
        .add_listener_local()
        .global(move |global| {
            with(&appearing, |session| session.added(global, &appearing));
        })
        .global_remove(move |id| {
            with(&disappearing, |session| session.removed(id));
        })
        .register();

    let commanded = Rc::downgrade(&session);
    connection.run_with(commands, move |command| {
        if command == Command::Stop {
            return ControlFlow::Break(());
        }
        with(&commanded, |session| session.act(command));
        ControlFlow::Continue(())
    });
}

pub(crate) fn unreachable(error: pipewire::Error) -> Error {
    Error::Unreachable(error.to_string())
}

fn with(session: &Weak<RefCell<Session>>, act: impl FnOnce(&mut Session)) {
    let Some(session) = session.upgrade() else {
        return;
    };
    act(&mut session.borrow_mut());
}

impl Session {
    fn added(&mut self, global: &GlobalObject<&DictRef>, watcher: &Weak<RefCell<Session>>) {
        match global.type_ {
            ObjectType::Node => self.add_node(global, watcher),
            ObjectType::Device => self.add_device(global, watcher),
            ObjectType::Metadata => self.add_metadata(global, watcher),
            _ => {}
        }
    }

    fn removed(&mut self, id: u32) {
        let node = self.nodes.remove(&id).is_some();
        if node {
            self.defaults.forget(id);
        }
        let known = node | self.devices.remove(&id).is_some() | self.forget_metadata(id);
        self.meters.remove(&id);
        if known {
            self.publish();
        }
    }

    fn act(&mut self, command: Command) {
        match command {
            Command::Volume { node, volume } => {
                self.set_node_param(node, ParamType::Props, &pods::volume_param(&volume));
            }
            Command::LatencyOffset { node, nanoseconds } => {
                let param = pods::latency_offset_param(nanoseconds);
                self.set_node_param(node, ParamType::Props, &param);
            }
            Command::Default { kind, name } => self.set_default(kind, &name),
            Command::Target { node, sink } => self.set_target(node, sink.as_deref()),
            Command::Profile { device, index } => {
                self.set_device_param(device, ParamType::Profile, &pods::profile_param(index));
            }
            Command::Route {
                device,
                index,
                port,
            } => {
                let param = pods::route_param(index, port);
                self.set_device_param(device, ParamType::Route, &param);
            }
            Command::ForceRate(rate) => self.tell_knob("clock.force-rate", &rate),
            Command::ForceQuantum(quantum) => {
                self.tell_knob("clock.force-quantum", &quantum);
            }
            Command::AllowRates(rates) => {
                self.tell_knob("clock.allowed-rates", &drop_in::format_list(&rates));
            }
            Command::Tune { node, tuning } => self.tune(node, &tuning),
            Command::Meter(nodes) => self.meter(&nodes),
            Command::Stop => {}
        }
    }

    fn meter(&mut self, nodes: &[u32]) {
        self.meters.retain(|node, _| nodes.contains(node));
        self.peaks.only(nodes);
        for node in nodes {
            if self.meters.contains_key(node) {
                continue;
            }
            let Some(watched) = self.nodes.get(node) else {
                continue;
            };
            let level = self.peaks.watch(*node);
            if let Some(meter) = Meter::watch(&self.core, &watched.node, level) {
                self.meters.insert(*node, meter);
            }
        }
    }

    pub(super) fn publish_if(&mut self, moved: bool) {
        if moved {
            self.publish();
        }
    }

    pub(super) fn publish(&mut self) {
        let mut nodes: Vec<Node> = self
            .nodes
            .values()
            .map(|watched| {
                let mut node = watched.node.clone();
                if let Some(target) = self.defaults.target(node.id) {
                    node.target = Some(target.to_owned());
                }
                node
            })
            .collect();
        nodes.sort_by(|one, other| one.title.cmp(&other.title).then(one.id.cmp(&other.id)));

        let mut devices: Vec<Device> = self
            .devices
            .values()
            .map(|watched| watched.device.clone())
            .collect();
        devices.sort_by(|one, other| one.title.cmp(&other.title).then(one.id.cmp(&other.id)));

        let snapshot = Snapshot {
            nodes,
            devices,
            default_sink: self.defaults.sink.clone(),
            default_source: self.defaults.source.clone(),
            graph: self.graph.clone(),
        };
        if snapshot == self.reported {
            return;
        }
        self.reported = snapshot.clone();
        (self.report)(snapshot);
    }
}

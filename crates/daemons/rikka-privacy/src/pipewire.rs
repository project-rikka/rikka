use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::{Rc, Weak};

use libspa::utils::dict::DictRef;
use pipewire::keys;
use pipewire::node::{Node as NodeProxy, NodeChangeMask, NodeListener, NodeState};
use pipewire::registry::{GlobalObject, RegistryRc};
use pipewire::types::ObjectType;
use rikka_pipewire::{Connection, LoopWorker};

use crate::graph::{Graph, Kind, Link, Node, Props, Users};

const NAME: &str = "rikka-privacy";

type EventSink = Box<dyn Fn(Users) + Send>;

pub(crate) fn start(report: EventSink) -> Result<LoopWorker, String> {
    rikka_pipewire::spawn(NAME, move |quitting| {
        let connection =
            Connection::open().map_err(|error| format!("PipeWire cannot be reached: {error}"))?;
        Ok(move || run(&connection, report, quitting))
    })
    .map_err(|error: rikka_worker::Error<String>| error.to_string())
}

struct Bound {
    _proxy: NodeProxy,
    _listener: NodeListener,
}

struct Watch {
    registry: RegistryRc,
    graph: Graph,
    bound: HashMap<u32, Bound>,
    report: EventSink,
    reported: Option<Users>,
}

fn run(connection: &Connection, report: EventSink, quitting: pipewire::channel::Receiver<()>) {
    let main_loop = connection.main_loop();
    let registry = connection.registry();
    let watch = Rc::new(RefCell::new(Watch {
        registry: registry.clone(),
        graph: Graph::default(),
        bound: HashMap::new(),
        report,
        reported: None,
    }));

    let dying = main_loop.downgrade();
    let _core_listener = connection
        .core()
        .add_listener_local()
        .error(move |id, _, _, _| {
            if id == 0
                && let Some(main_loop) = dying.upgrade()
            {
                main_loop.quit();
            }
        })
        .register();

    let appearing = Rc::downgrade(&watch);
    let disappearing = Rc::downgrade(&watch);
    let _registry_listener = registry
        .add_listener_local()
        .global(move |global| with(&appearing, |watch| watch.added(global, &appearing)))
        .global_remove(move |id| with(&disappearing, |watch| watch.removed(id)))
        .register();

    connection.run_until_stopped(quitting);
}

fn with(watch: &Weak<RefCell<Watch>>, act: impl FnOnce(&mut Watch)) {
    if let Some(watch) = watch.upgrade() {
        act(&mut watch.borrow_mut());
    }
}

impl Watch {
    fn added(&mut self, global: &GlobalObject<&DictRef>, watcher: &Weak<RefCell<Watch>>) {
        let Some(props) = global.props else {
            return;
        };
        match global.type_ {
            ObjectType::Node => self.add_node(global, props, watcher),
            ObjectType::Link => {
                let end = |key: &str| props.get(key).and_then(|id| id.parse().ok());
                let (Some(output), Some(input)) =
                    (end(*keys::LINK_OUTPUT_NODE), end(*keys::LINK_INPUT_NODE))
                else {
                    return;
                };
                self.graph.links.insert(global.id, Link { output, input });
                self.publish();
            }
            ObjectType::Client => {
                self.graph.clients.insert(global.id, gathered(props));
                self.publish();
            }
            _ => {}
        }
    }

    fn add_node(
        &mut self,
        global: &GlobalObject<&DictRef>,
        props: &DictRef,
        watcher: &Weak<RefCell<Watch>>,
    ) {
        let Some(kind) = props
            .get(*keys::MEDIA_CLASS)
            .and_then(Kind::from_media_class)
        else {
            return;
        };
        let Ok(proxy) = self.registry.bind::<NodeProxy, _>(global) else {
            return;
        };
        let id = global.id;
        let informed = watcher.clone();
        let listener = proxy
            .add_listener_local()
            .info(move |info| {
                with(&informed, |watch| {
                    let Some(node) = watch.graph.nodes.get_mut(&id) else {
                        return;
                    };
                    let props = info
                        .change_mask()
                        .contains(NodeChangeMask::PROPS)
                        .then(|| info.props().map(gathered))
                        .flatten();
                    node.informed(props, matches!(info.state(), NodeState::Running));
                    watch.publish();
                });
            })
            .register();
        self.graph.nodes.insert(
            id,
            Node {
                kind,
                running: false,
                props: gathered(props),
            },
        );
        self.bound.insert(
            id,
            Bound {
                _proxy: proxy,
                _listener: listener,
            },
        );
    }

    fn removed(&mut self, id: u32) {
        let known = self.graph.nodes.remove(&id).is_some()
            | self.graph.links.remove(&id).is_some()
            | self.graph.clients.remove(&id).is_some();
        self.bound.remove(&id);
        if known {
            self.publish();
        }
    }

    fn publish(&mut self) {
        let users = self.graph.users();
        if self.reported.as_ref() == Some(&users) {
            return;
        }
        self.reported = Some(users.clone());
        (self.report)(users);
    }
}

fn gathered(props: &DictRef) -> Props {
    Props(
        props
            .iter()
            .map(|(key, value)| (key.to_owned(), value.to_owned()))
            .collect(),
    )
}

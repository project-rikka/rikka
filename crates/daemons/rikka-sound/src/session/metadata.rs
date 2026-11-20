use std::cell::RefCell;
use std::rc::Weak;

use libspa::utils::dict::DictRef;
use pipewire::metadata::{Metadata, MetadataListener};
use pipewire::registry::GlobalObject;

use crate::node::NodeKind;

use super::defaults::Defaults;
use super::{Session, with};

pub(super) struct WatchedMetadata {
    id: u32,
    proxy: Metadata,
    _listener: MetadataListener,
}

type PropertyHandler = fn(&mut Session, u32, Option<&str>, Option<&str>);

impl Session {
    pub(super) fn add_metadata(
        &mut self,
        global: &GlobalObject<&DictRef>,
        watcher: &Weak<RefCell<Session>>,
    ) {
        match global.props.and_then(|props| props.get("metadata.name")) {
            Some("default") => {
                self.defaults = Defaults::default();
                self.default_metadata = self.watch_metadata(global, watcher, Session::told_default);
            }
            Some("settings") => {
                self.knobs = self.watch_metadata(global, watcher, Session::told_knob);
                for (key, value) in std::mem::take(&mut self.unsaid) {
                    self.tell_knob(key, &value);
                }
            }
            _ => {}
        }
    }

    fn watch_metadata(
        &self,
        global: &GlobalObject<&DictRef>,
        watcher: &Weak<RefCell<Session>>,
        handler: PropertyHandler,
    ) -> Option<WatchedMetadata> {
        let proxy = self.registry.bind::<Metadata, _>(global).ok()?;
        let session = watcher.clone();
        let listener = proxy
            .add_listener_local()
            .property(move |subject, key, _, value| {
                with(&session, |session| handler(session, subject, key, value));
                0
            })
            .register();
        Some(WatchedMetadata {
            id: global.id,
            proxy,
            _listener: listener,
        })
    }

    pub(super) fn forget_metadata(&mut self, id: u32) -> bool {
        if self.knobs.as_ref().is_some_and(|knobs| knobs.id == id) {
            self.knobs = None;
            return true;
        }
        if self
            .default_metadata
            .as_ref()
            .is_some_and(|node| node.id == id)
        {
            self.default_metadata = None;
            self.defaults = Defaults::default();
            return true;
        }
        false
    }

    fn told_default(&mut self, subject: u32, key: Option<&str>, value: Option<&str>) {
        self.defaults.apply(subject, key, value);
        self.publish();
    }

    fn told_knob(&mut self, subject: u32, key: Option<&str>, value: Option<&str>) {
        if subject != 0 {
            return;
        }
        match key {
            Some("clock.rate") => self.graph.rate = number(value),
            Some("clock.quantum") => self.graph.quantum = number(value),
            Some("clock.min-quantum") => self.graph.min_quantum = number(value),
            Some("clock.max-quantum") => self.graph.max_quantum = number(value),
            Some("clock.force-rate") => self.graph.forced_rate = number(value),
            Some("clock.force-quantum") => self.graph.forced_quantum = number(value),
            Some("clock.allowed-rates") => self.graph.allowed_rates = numbers(value),
            _ => return,
        }
        self.publish();
    }

    pub(super) fn set_default(&mut self, kind: NodeKind, name: &str) {
        let key = match kind {
            NodeKind::Sink => "default.configured.audio.sink",
            NodeKind::Source => "default.configured.audio.source",
            NodeKind::Playback | NodeKind::Recording => return,
        };
        let Some(node) = &self.default_metadata else {
            return;
        };
        let value = format!("{{\"name\":\"{name}\"}}");
        node.proxy
            .set_property(0, key, Some("Spa:String:JSON"), Some(&value));
        self.defaults.choose(kind, name);
        self.publish();
    }

    pub(super) fn set_target(&mut self, node: u32, sink: Option<&str>) {
        let serial = sink.and_then(|name| {
            self.nodes
                .values()
                .find(|watched| watched.node.name == name)
                .map(|watched| watched.node.serial.to_string())
        });
        let Some(default_node) = &self.default_metadata else {
            return;
        };
        default_node
            .proxy
            .set_property(node, "target.object", None, serial.as_deref());
        self.defaults.aim(node, serial);
        self.publish();
    }

    pub(super) fn tell_knob(&mut self, key: &'static str, value: &impl std::fmt::Display) {
        let value = value.to_string();
        match &self.knobs {
            Some(knobs) => knobs.proxy.set_property(0, key, None, Some(&value)),
            None => {
                self.unsaid.retain(|(pending_key, _)| *pending_key != key);
                self.unsaid.push((key, value));
            }
        }
    }
}

fn number(value: Option<&str>) -> u32 {
    value.and_then(rikka_pipewire::number).unwrap_or_default()
}

fn numbers(value: Option<&str>) -> Vec<u32> {
    value.map(rikka_pipewire::numbers).unwrap_or_default()
}

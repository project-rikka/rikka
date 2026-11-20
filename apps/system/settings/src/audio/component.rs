use std::collections::HashMap;
use std::time::Duration;

use rikka_app::prelude::*;
use rikka_widgets::{column, row};

use rikka_sound::config::{DeviceTunings, GraphTuning};
use rikka_sound::{Command, Node, NodeKind, Peaks, Snapshot, Sound, Volume};

use rikka_sound::config::DeviceTuning;

use crate::amount::amount;
use crate::audio::meter::{meter, quantize};
use crate::audio::{advanced, readout};
use crate::notice::error_notice;
use crate::page::Page;
use crate::pending::Pending;
use crate::picker::{default_picker, picker};
use crate::session::Session;
use crate::spot_reveal::SpotReveal;

const POLL: Duration = Duration::from_millis(60);

const POLLING: &str = "settings.audio.polling";
const FOLLOWING: &str = "settings.audio.following";
const BOOST: f32 = 1.15;

#[derive(State)]
pub(crate) struct Audio {
    sound: Tracked<Option<Sound>>,
    #[untracked]
    graph: Snapshot,
    shape: Revision,
    #[untracked]
    volumes: HashMap<u32, Tracked<Volume>>,
    #[untracked]
    metered: Vec<u32>,
    #[untracked]
    shown: Vec<u16>,
    #[untracked]
    pub(super) meters: Watchers,
    pub(super) tuning: Tracked<GraphTuning>,
    pub(super) tunings: Tracked<DeviceTunings>,
    pub(super) advanced: Tracked<bool>,
    error: Tracked<Option<String>>,
    reveal: Tracked<SpotReveal<NodeKind>>,
}

impl Page for Audio {
    fn opened(&mut self, _: &Pending, _: &Session) {
        if self.sound.is_none() {
            let sound = Sound::start();
            stream_keyed(
                FOLLOWING,
                sound.changes(),
                |audio: &mut Audio, graph: Snapshot| audio.graph_changed(graph),
            );
            let graph = sound.state();
            *self.sound = Some(sound);
            self.graph_changed(graph);
        }
        self.meter();
        every_keyed(POLLING, POLL, |audio: &mut Audio| audio.tick());
    }

    fn closed(&mut self) {
        self.metered.clear();
        self.shown.clear();
        self.send(Command::Meter(Vec::new()));
        cancel(POLLING);
        if !self.reveal.is_idle() {
            self.reveal.clear();
        }
    }
}

impl Audio {
    pub(crate) fn new() -> Self {
        Self {
            sound: Tracked::new(None),
            graph: Snapshot::default(),
            shape: Revision::new(),
            volumes: HashMap::new(),
            metered: Vec::new(),
            shown: Vec::new(),
            meters: Watchers::new(),
            tuning: Tracked::new(GraphTuning::read()),
            tunings: Tracked::new(DeviceTunings::read()),
            advanced: Tracked::new(false),
            error: Tracked::new(None),
            reveal: Tracked::new(SpotReveal::new()),
        }
    }

    pub(crate) fn reveal(&mut self, kind: NodeKind) {
        self.reveal.request(kind);
        self.settle_reveal();
    }

    fn settle_reveal(&mut self) {
        let Some(kind) = self.reveal.wanted() else {
            return;
        };
        let unavailable = self.sound.as_ref().and_then(Sound::unavailable).is_some();
        let defaults_known =
            self.graph.default_sink.is_some() || self.graph.default_source.is_some();
        if unavailable || defaults_known || self.graph.default_node(kind).is_some() {
            self.reveal.settle();
        }
    }

    fn graph_changed(&mut self, graph: Snapshot) {
        let reshaped = shaped(&self.graph) != shaped(&graph);
        for node in &graph.nodes {
            match self.volumes.get_mut(&node.id) {
                Some(volume) => volume.set(node.volume.clone()),
                None => {
                    self.volumes
                        .insert(node.id, Tracked::new(node.volume.clone()));
                }
            }
        }
        self.volumes.retain(|id, _| graph.node(*id).is_some());
        self.graph = graph;
        if reshaped {
            self.shape.changed();
        }
        self.meter();
        self.settle_reveal();
    }

    pub(super) fn volume_at(&self, node: u32) -> Volume {
        self.volumes
            .get(&node)
            .map(|volume| (**volume).clone())
            .unwrap_or_default()
    }

    fn tick(&mut self) {
        let drawn = self.drawn_peaks();
        if drawn != self.shown {
            self.shown = drawn;
            self.meters.repaint();
            skip_view();
            return;
        }
        skip_view();
    }

    fn meter(&mut self) {
        let node_ids: Vec<u32> = self
            .graph
            .sink()
            .into_iter()
            .chain(self.graph.source())
            .map(|node| node.id)
            .chain(
                self.graph
                    .nodes
                    .iter()
                    .filter(|node| node.kind.is_stream())
                    .map(|node| node.id),
            )
            .collect();
        if node_ids == self.metered {
            return;
        }
        self.metered = node_ids.clone();
        self.send(Command::Meter(node_ids));
    }

    pub(super) fn send(&self, command: Command) {
        if let Some(sound) = &*self.sound {
            sound.send(command);
        }
    }

    fn peaks(&self) -> Option<&Peaks> {
        self.sound.as_ref()?.peaks()
    }

    fn drawn_peaks(&self) -> Vec<u16> {
        let peaks = self.peaks();
        self.metered
            .iter()
            .map(|&node| quantize(peaks.map_or(0.0, |peaks| peaks.level(node))))
            .collect()
    }

    pub(super) fn graph(&self) -> &Snapshot {
        self.shape.read();
        &self.graph
    }

    fn pinned(&mut self, result: rikka_sound::Result<()>) {
        self.error.set(result.err().map(|error| error.to_string()));
    }

    fn sinks(&self) -> Vec<&Node> {
        self.graph().nodes_of_kind(NodeKind::Sink).collect()
    }

    fn notice(&self) -> Option<Container> {
        let unreachable = self.sound.as_ref().and_then(Sound::unavailable);
        error_notice(
            unreachable.as_deref(),
            self.error.as_deref(),
            ("settings.audio.unavailable", "settings.audio.refused"),
        )
    }

    fn volume(&mut self, node: u32, change: impl FnOnce(Volume) -> Volume) {
        if let Some(volume) = self.volume_of(node) {
            self.set_volume(node, change(volume));
        }
    }

    fn choose_default(&mut self, kind: NodeKind, index: usize) {
        if let Some(node) = self.graph.nodes_of_kind(kind).nth(index) {
            let name = node.name.clone();
            self.send(Command::Default { kind, name });
        }
    }

    fn target(&mut self, node: u32, index: Option<usize>) {
        let sink = index.and_then(|index| self.sinks().get(index).map(|sink| sink.name.clone()));
        self.send(Command::Target { node, sink });
    }

    fn profile(&mut self, device: u32, index: usize) {
        let profile = self.graph.device(device).and_then(|device| {
            device
                .profiles
                .iter()
                .filter(|profile| profile.available)
                .nth(index)
                .map(|profile| profile.index)
        });
        if let Some(index) = profile {
            self.send(Command::Profile { device, index });
        }
    }

    fn port(&mut self, device: u32, index: usize, output: bool) {
        let on = if output {
            self.graph.sink()
        } else {
            self.graph.source()
        }
        .and_then(|node| node.route);
        let port = self.graph.device(device).and_then(|device| {
            device
                .ports(output)
                .nth(index)
                .map(|port| (port.index, on.unwrap_or(port.device)))
        });
        if let Some((index, port)) = port {
            self.send(Command::Route {
                device,
                index,
                port,
            });
        }
    }

    fn set_volume(&mut self, node: u32, volume: Volume) {
        if let Some(entry) = self.graph.nodes.iter_mut().find(|entry| entry.id == node) {
            entry.volume = volume.clone();
        }
        if let Some(shown) = self.volumes.get_mut(&node) {
            shown.set(volume.clone());
        }
        self.send(Command::Volume { node, volume });
    }

    fn volume_of(&self, node: u32) -> Option<Volume> {
        self.graph.node(node).map(|node| node.volume.clone())
    }

    pub(super) fn pin_graph(&mut self) {
        let result = self.tuning.write();
        self.pinned(result);
    }

    pub(super) fn tune(&mut self, change: impl FnOnce(&mut DeviceTuning)) {
        let Some((node, name)) = self.graph.sink().map(|sink| (sink.id, sink.name.clone())) else {
            return;
        };
        let mut tuning = self.tunings.get(&name).cloned().unwrap_or_default();
        change(&mut tuning);
        DeviceTunings::set(&mut self.tunings, &name, tuning.clone());
        let result = self.tunings.write();
        if result.is_ok() {
            self.send(Command::Tune { node, tuning });
        }
        self.pinned(result);
    }
}

fn shaped(graph: &Snapshot) -> Snapshot {
    let mut shaped = graph.clone();
    for node in &mut shaped.nodes {
        node.volume = Volume::default();
    }
    shaped
}

pub(super) fn option_at<T: Copy>(offered: &[T], index: Option<usize>) -> Option<T> {
    offered.get(index?).copied()
}

pub(crate) fn view(audio: &Audio) -> impl View {
    column! {
        audio.notice();
        audio.reveal.section(
            NodeKind::Sink,
            section(
                tr!("settings.audio.output"),
                side(audio, NodeKind::Sink, tr!("settings.audio.no-output")),
            ),
        );
        audio.reveal.section(
            NodeKind::Source,
            section(
                tr!("settings.audio.input"),
                side(audio, NodeKind::Source, tr!("settings.audio.no-input")),
            ),
        );
        section(tr!("settings.audio.applications"), applications(audio));
        advanced::view(audio);
    }
    .gap(Space::ROOMY)
}

fn side(audio: &Audio, kind: NodeKind, empty: Label) -> Element {
    let devices: Vec<&Node> = audio.graph().nodes_of_kind(kind).collect();
    let Some(default_node) = audio.graph().default_node(kind) else {
        return hint(empty).build();
    };
    let output = kind == NodeKind::Sink;
    column! {
        field(
            tr!("settings.audio.device"),
            picker(
                devices.iter().position(|node| node.id == default_node.id),
                devices.iter().map(|node| Label::from(node.title.as_str())),
                move |audio: &mut Audio, index: usize| audio.choose_default(kind, index),
            ),
        );
        fader(default_node);
        if output {
            balance(default_node);
        }
        for row in card(audio, default_node, output) {
            row;
        }
    }
    .gap(Space::SNUG)
    .build()
}

fn applications(audio: &Audio) -> Element {
    let streams: Vec<&Node> = audio
        .graph()
        .nodes
        .iter()
        .filter(|node| node.kind.is_stream())
        .collect();
    if streams.is_empty() {
        return hint(tr!("settings.audio.nothing-playing")).build();
    }
    let sinks = audio.sinks();
    column_of(streams.into_iter().map(|node| stream(node, &sinks)))
        .gap(Space::ROOMY)
        .build()
}

fn stream(node: &Node, sinks: &[&Node]) -> impl View {
    let palette = palette();
    container(
        column! {
            row! {
                icon(if node.kind == NodeKind::Recording {
                    MaterialSymbol::MIC
                } else {
                    MaterialSymbol::GRAPHIC_EQ
                })
                .extent(Length::px(18.0))
                .color(palette.text_muted);
                column! {
                    text(node.title.as_str()).label().semibold().no_wrap();
                }
                .gap(Space::HAIR);
            }
            .gap(Space::SNUG)
            .align_items(Align::Center);
            fader(node);
            node.detail
                .as_ref()
                .map(|detail| text(detail.as_str()).caption().muted().no_wrap());
            if node.kind == NodeKind::Playback {
                destination(node, sinks);
            }
            readout::format_summary(node).map(|carrying| text(carrying).caption().muted());
        }
        .gap(Space::NORMAL),
    )
    .background(palette.surface_variant)
    .radius(Corner::Panel)
    .padding(Space::NORMAL)
    .fill_width()
}

fn destination(node: &Node, sinks: &[&Node]) -> impl View {
    let target_index = node.target.as_ref().and_then(|target| {
        let serial: Option<u64> = target.parse().ok();
        sinks
            .iter()
            .position(|sink| Some(sink.serial) == serial || sink.name == *target)
    });
    let id = node.id;
    field(
        tr!("settings.audio.plays-through"),
        default_picker(
            tr!("settings.audio.follow-default"),
            sinks.iter().map(|sink| Label::from(sink.title.as_str())),
            target_index,
            move |audio: &mut Audio, index: Option<usize>| audio.target(id, index),
        ),
    )
}

fn fader(node: &Node) -> Element {
    component_with(|audio: &Audio| audio, node.id, fader_part)
}

fn fader_part(audio: &Audio, id: &u32) -> impl View {
    let id = *id;
    let volume = audio.volume_at(id);
    let muted = volume.muted();
    let palette = palette();
    field(
        tr!("settings.audio.volume"),
        column! {
            row! {
                button(
                    icon(if muted {
                        MaterialSymbol::VOLUME_OFF
                    } else {
                        MaterialSymbol::VOLUME_UP
                    })
                    .extent(Length::px(18.0)),
                )
                .ghost()
                .icon_only()
                .on_press(move |audio: &mut Audio| audio.volume(id, |volume| volume.muted_to(!muted)));
                amount(
                    volume.level(),
                    0.0..=BOOST,
                    0.005,
                    readout::percent(&volume),
                    move |audio: &mut Audio, level: f32| {
                        audio.volume(id, |volume| volume.at_level(level));
                    },
                );
            }
            .gap(Space::SNUG)
            .align_items(Align::Center);
            meter(
                &audio.meters,
                audio.peaks(),
                id,
                if muted {
                    palette.text_muted
                } else {
                    palette.accent
                },
            );
        }
        .gap(Space::SNUG),
    )
}

fn balance(node: &Node) -> Element {
    component_with(|audio: &Audio| audio, node.id, balance_part)
}

fn balance_part(audio: &Audio, id: &u32) -> Option<impl View> {
    let id = *id;
    let volume = audio.volume_at(id);
    volume.stereo()?;
    let balance = volume.balance();
    Some(field(
        tr!("settings.audio.balance"),
        amount(
            balance,
            -1.0..=1.0,
            0.01,
            readout::balance(balance),
            move |audio: &mut Audio, moved: f32| {
                audio.volume(id, |volume| volume.at_balance(moved));
            },
        ),
    ))
}

fn card(audio: &Audio, node: &Node, output: bool) -> Vec<Element> {
    let Some(device) = node.device.and_then(|id| audio.graph().device(id)) else {
        return Vec::new();
    };
    let mut rows = Vec::new();

    let ports: Vec<_> = device.ports(output).collect();
    if ports.len() > 1 {
        let card = device.id;
        rows.push(
            field(
                tr!("settings.audio.port"),
                picker(
                    ports.iter().position(|port| port.active),
                    ports.iter().map(|port| {
                        if port.available == Some(false) {
                            tr!("settings.audio.unplugged", port = &port.description)
                        } else {
                            Label::from(port.description.as_str())
                        }
                    }),
                    move |audio: &mut Audio, index: usize| audio.port(card, index, output),
                ),
            )
            .build(),
        );
    }

    let profiles: Vec<_> = device
        .profiles
        .iter()
        .filter(|profile| profile.available)
        .collect();
    if profiles.len() > 1 {
        let card = device.id;
        rows.push(
            field(
                tr!("settings.audio.profile"),
                picker(
                    profiles
                        .iter()
                        .position(|profile| Some(profile.index) == device.profile),
                    profiles
                        .iter()
                        .map(|profile| Label::from(profile.description.as_str())),
                    move |audio: &mut Audio, index: usize| audio.profile(card, index),
                ),
            )
            .build(),
        );
    }
    rows
}

pub(super) fn hint(message: Label) -> Text {
    text(message).caption().muted()
}

impl std::fmt::Debug for Audio {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Audio")
            .field("nodes", &self.graph.nodes.len())
            .field("advanced", &self.advanced)
            .finish_non_exhaustive()
    }
}

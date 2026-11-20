use rikka_sound::{Node, NodeKind, Snapshot, Volume};
use rikka_ui::{Element, Inset, Label, MaterialSymbol, Styled, TextStyled, View, text, tr};
use rikka_widgets::{Flex, column, slider};
use shell_feeds::symbols;

use crate::centre::detail::Detail;
use crate::centre::state::Centre;
use crate::centre::view::quick::{Quick, quick};
use crate::centre::view::tile::{self, DETAIL, ICON, INSET, ROW};

struct Kind {
    device: NodeKind,
    stream: NodeKind,
    devices: &'static str,
    streams: &'static str,
    page: Detail,
    symbol: fn(Option<&Volume>) -> MaterialSymbol,
}

const OUT: Kind = Kind {
    device: NodeKind::Sink,
    stream: NodeKind::Playback,
    devices: "shell.centre.output",
    streams: "shell.centre.playing",
    page: Detail::Sound,
    symbol: symbols::sound,
};

const IN: Kind = Kind {
    device: NodeKind::Source,
    stream: NodeKind::Recording,
    devices: "shell.centre.input",
    streams: "shell.centre.recording",
    page: Detail::Microphone,
    symbol: symbols::microphone,
};

pub(super) fn speaker(graph: &Snapshot) -> Quick {
    tile(graph, &OUT)
}

pub(crate) fn microphone(graph: &Snapshot) -> Quick {
    tile(graph, &IN)
}

pub(crate) fn playing(graph: &Snapshot) -> Vec<Element> {
    page(graph, &OUT)
}

pub(crate) fn recording(graph: &Snapshot) -> Vec<Element> {
    page(graph, &IN)
}

fn page(graph: &Snapshot, kind: &Kind) -> Vec<Element> {
    let Some(node) = graph.default_node(kind.device) else {
        return vec![tile::caption(tr!("shell.centre.unavailable")).build()];
    };

    let mut rows = vec![
        controls(kind, node).build(),
        tile::caption(tr!(kind.devices)).build(),
    ];
    rows.extend(devices(graph, kind, node));

    let streams = streams(graph, kind);
    if !streams.is_empty() {
        rows.push(tile::caption(tr!(kind.streams)).build());
        rows.extend(streams);
    }
    rows
}

fn controls(kind: &Kind, node: &Node) -> Flex {
    let id = node.id;
    tile::strip(vec![
        tile::symbol_button(
            (kind.symbol)(Some(&node.volume)),
            move |centre: &mut Centre| {
                centre.mute(id);
            },
        )
        .build(),
        level(node).build(),
    ])
}

fn level(node: &Node) -> impl View {
    let id = node.id;
    slider(node.volume.level())
        .range(0.0..=1.0)
        .enabled(!node.volume.muted())
        .on_change(move |centre: &mut Centre, level| centre.daemons().sound().set_level(id, level))
        .grow(1.0)
}

fn tile(graph: &Snapshot, kind: &Kind) -> Quick {
    let node = graph.default_node(kind.device);
    let volume = node.map(|node| &node.volume);
    let mut tile = quick((kind.symbol)(volume), kind.page.title())
        .on(volume.is_some_and(|volume| !volume.muted()))
        .here(node.is_some())
        .opens(kind.page);
    if let Some(node) = node {
        let id = node.id;
        tile = tile
            .taskbar_visibility(taskbar_visibility(node))
            .toggle(move |centre: &mut Centre| centre.mute(id));
    }
    tile
}

fn taskbar_visibility(node: &Node) -> Label {
    match node.volume.muted() {
        true => tr!("shell.centre.muted"),
        false => Label::from(node.title.as_str()),
    }
}

fn devices(graph: &Snapshot, kind: &Kind, chosen: &Node) -> Vec<Element> {
    let device = kind.device;
    graph
        .nodes_of_kind(device)
        .map(|node| {
            let name = node.name.clone();
            tile::choice(
                device_symbol(device),
                Label::from(node.title.as_str()),
                None,
                node.id == chosen.id,
                move |centre: &mut Centre| centre.daemons().sound().use_device(device, &name),
            )
        })
        .collect()
}

fn device_symbol(kind: NodeKind) -> MaterialSymbol {
    match kind {
        NodeKind::Source | NodeKind::Recording => MaterialSymbol::MIC,
        NodeKind::Sink | NodeKind::Playback => MaterialSymbol::SPEAKER,
    }
}

fn streams(graph: &Snapshot, kind: &Kind) -> Vec<Element> {
    graph
        .nodes_of_kind(kind.stream)
        .map(|node| {
            column! {
                text(node.title.as_str())
                    .text_size(DETAIL)
                    .muted()
                    .no_wrap()
                    .margin(Inset::edges(2.0, INSET, 0.0, INSET));
                controls(kind, node).height(ROW - ICON / 2.0)
            }
            .build()
        })
        .collect()
}

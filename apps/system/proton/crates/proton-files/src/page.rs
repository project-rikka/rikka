use rikka_proton_drive::{LinkState, Node};

use crate::entry::Entry;
use crate::order::Order;

#[derive(Debug, Default)]
pub struct Page {
    pub nodes: Vec<Node>,
    pub entries: Vec<Entry>,
}

impl Page {
    #[must_use]
    pub fn from_nodes(nodes: Vec<Node>, order: Order) -> Page {
        let nodes: Vec<Node> = nodes
            .into_iter()
            .filter(|node| node.link().state() != LinkState::Trashed)
            .collect();
        let mut entries: Vec<Entry> = nodes.iter().map(Entry::from_node).collect();
        order.arrange(&mut entries);

        Page { nodes, entries }
    }
}

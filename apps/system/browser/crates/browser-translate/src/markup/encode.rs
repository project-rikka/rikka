use std::fmt::Write;

use rikka_html::Escaped;

use crate::markup::dialect::{self, Dialect, ELEMENT_TAG, KEEP_TAG, PRE_TAG};
use crate::markup::plan::{Kind, Piece, Plan};
use crate::protect::protected;
use crate::unit::{Node, Unit};

#[derive(Debug)]
pub(crate) struct Encoded {
    pub(crate) html: String,
    pub(crate) plan: Plan,
}

pub(crate) fn encode(unit: &Unit, dialect: &Dialect, protect: bool) -> Encoded {
    let mut encoder = Encoder {
        dialect,
        protect,
        html: String::new(),
        plan: Plan::default(),
        next_token: highest_id(&unit.nodes).map_or(0, |id| id + 1),
    };
    if unit.pre {
        encoder.html.push('<');
        encoder.html.push_str(PRE_TAG);
        encoder.html.push('>');
    }
    let (root, _) = encoder.nodes(&unit.nodes, None);
    if unit.pre {
        dialect::close(&mut encoder.html, PRE_TAG);
    }
    encoder.plan.root = root;
    Encoded {
        html: encoder.html,
        plan: encoder.plan,
    }
}

fn highest_id(nodes: &[Node]) -> Option<u32> {
    nodes
        .iter()
        .filter_map(|node| match node {
            Node::Text(_) => None,
            Node::Keep { id, .. } => Some(*id),
            Node::Element { id, nodes } => {
                Some(highest_id(nodes).map_or(*id, |inner| inner.max(*id)))
            }
        })
        .max()
}

struct Encoder<'a> {
    dialect: &'a Dialect,
    protect: bool,
    html: String,
    plan: Plan,
    next_token: u32,
}

impl Encoder<'_> {
    fn nodes(&mut self, nodes: &[Node], parent: Option<usize>) -> (Vec<Piece>, bool) {
        let mut pieces = Vec::new();
        let mut previous = None;
        let mut has_text = false;
        for node in nodes {
            match node {
                Node::Text(text) => {
                    has_text |= !text.trim().is_empty();
                    self.text(text, parent, &mut previous, &mut pieces);
                }
                Node::Keep { id, text } => {
                    let slot = self.plan.add(*id, Kind::Keep, parent, previous);
                    self.push_kept_slot(*id, text);
                    pieces.push(Piece::Slot(slot));
                    previous = Some(slot);
                }
                Node::Element { id, nodes } => {
                    let slot = self.plan.add(*id, Kind::Element, parent, previous);
                    self.dialect.open_element(&mut self.html, *id);
                    let (children, inner_text) = self.nodes(nodes, Some(slot));
                    dialect::close(&mut self.html, ELEMENT_TAG);
                    self.plan.slots[slot].children = children;
                    self.plan.slots[slot].has_text = inner_text;
                    has_text |= inner_text;
                    pieces.push(Piece::Slot(slot));
                    previous = Some(slot);
                }
            }
        }
        (pieces, has_text)
    }

    fn text(
        &mut self,
        text: &str,
        parent: Option<usize>,
        previous: &mut Option<usize>,
        pieces: &mut Vec<Piece>,
    ) {
        let tokens = if self.protect {
            protected(text)
        } else {
            Vec::new()
        };
        let mut from = 0;
        for token in tokens {
            self.plain(&text[from..token.start], pieces);
            let id = self.next_token;
            self.next_token += 1;
            let word = &text[token.clone()];
            let slot = self
                .plan
                .add(id, Kind::Token(word.to_owned()), parent, *previous);
            self.push_kept_slot(id, word);
            pieces.push(Piece::Slot(slot));
            *previous = Some(slot);
            from = token.end;
        }
        self.plain(&text[from..], pieces);
    }

    fn plain(&mut self, text: &str, pieces: &mut Vec<Piece>) {
        if text.is_empty() {
            return;
        }
        self.escaped(text);
        pieces.push(Piece::Text(text.to_owned()));
    }

    fn push_kept_slot(&mut self, id: u32, text: &str) {
        self.dialect.open_keep(&mut self.html, id);
        self.escaped(text);
        dialect::close(&mut self.html, KEEP_TAG);
    }

    fn escaped(&mut self, text: &str) {
        write!(self.html, "{}", Escaped(text)).expect("writing into a String cannot fail");
    }
}

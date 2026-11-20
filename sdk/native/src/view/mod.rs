pub(crate) mod drop;
mod lower;
pub(crate) mod menu;
pub(crate) mod node;

use std::rc::Rc;

use rikka_ui::{Element, Length, Styled, View as _, hash_of};
use rikka_widgets::{ListItem, column_of};

use crate::abi::violation;
use crate::view::node::{Kind, Node, Root, Target};

pub(crate) const STAGE: &str = "rikka-sdk-stage";

pub(crate) fn key_of(key: &str) -> u64 {
    hash_of(("rikka-sdk-key", key))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Focus {
    Nothing,
    Open,
    Last,
}

pub(crate) struct View {
    frames: Vec<Node>,
    focus: Focus,
}

impl View {
    pub(crate) fn new() -> Self {
        Self::rooted(Root::Window)
    }

    pub(crate) fn fragment() -> Self {
        Self::rooted(Root::Fragment)
    }

    pub(crate) fn list_item() -> Self {
        Self::rooted(Root::ListItem)
    }

    fn rooted(root: Root) -> Self {
        Self {
            frames: vec![Node::new(Kind::Root(root))],
            focus: Focus::Nothing,
        }
    }

    fn top(&mut self) -> &mut Node {
        self.frames.last_mut().expect("the root frame stays open")
    }

    fn admit(&mut self, kind: &Kind) {
        let parent = &self.top().kind;
        let holds = parent.holds();
        let fits = match kind {
            Kind::ContextMenu { .. } => true,
            _ => holds == kind.belongs_in(),
        };
        if !fits {
            violation(format!("{} cannot hold {}", parent.name(), kind.name()));
        }
    }

    pub(crate) fn leaf(&mut self, kind: Kind) {
        self.admit(&kind);
        self.top().children.push(Node::new(kind));
        self.focus = Focus::Last;
    }

    pub(crate) fn begin(&mut self, kind: Kind) {
        self.admit(&kind);
        self.frames.push(Node::new(kind));
        self.focus = Focus::Open;
    }

    pub(crate) fn begin_context_menu(&mut self) {
        let target = match self.focus {
            Focus::Nothing => violation("a context menu needs a current element to attach to"),
            Focus::Open => Target::Parent,
            Focus::Last => Target::Last,
        };
        let attached = match target {
            Target::Parent => self.top(),
            Target::Last => self.current(),
        };
        if !attached.kind.is_element() {
            violation(format!(
                "{} cannot have a context menu",
                attached.kind.name()
            ));
        }
        if attached.common.menu.is_some() {
            violation(format!(
                "{} already has a context menu",
                attached.kind.name()
            ));
        }
        self.begin(Kind::ContextMenu { target });
    }

    pub(crate) fn end(&mut self) {
        if self.frames.len() == 1 {
            violation("nothing was begun");
        }
        let node = self.frames.pop().expect("more than the root frame is open");
        let expected = match node.kind {
            Kind::Split { .. } => Some(2),
            Kind::PageStack { .. } | Kind::Field { .. } => Some(1),
            _ => None,
        };
        if let Some(expected) = expected
            && node.children.len() != expected
        {
            violation(format!(
                "{} holds {} children instead of {expected}",
                node.kind.name(),
                node.children.len()
            ));
        }
        if let Kind::ContextMenu { target } = node.kind {
            let entries = Rc::new(lower::entries(node.children));
            let parent = self.top();
            let attached = match target {
                Target::Parent => parent,
                Target::Last => parent
                    .children
                    .last_mut()
                    .expect("the target was the last child"),
            };
            attached.common.menu = Some(entries);
            self.focus = match target {
                Target::Parent => Focus::Open,
                Target::Last => Focus::Last,
            };
            return;
        }
        self.top().children.push(node);
        self.focus = Focus::Last;
    }

    pub(crate) fn current(&mut self) -> &mut Node {
        match self.focus {
            Focus::Nothing => violation("there is no current element; declare one first"),
            Focus::Open => self.top(),
            Focus::Last => self
                .top()
                .children
                .last_mut()
                .expect("the focused child exists"),
        }
    }

    pub(crate) fn current_element(&mut self) -> &mut Node {
        let node = self.current();
        if !node.kind.is_element() {
            violation(format!(
                "{} is not an element and takes no layout or decoration",
                node.kind.name()
            ));
        }
        node
    }

    fn close(mut self) -> Vec<Node> {
        if self.frames.len() > 1 {
            let open = self.top().kind.name();
            violation(format!(
                "{} begun elements were never ended, the innermost {open}",
                self.frames.len() - 1
            ));
        }
        self.frames.pop().expect("the root frame exists").children
    }

    pub(crate) fn finish_window(self, overlays: Vec<Element>) -> Element {
        let mut elements = lower::elements(self.close());
        elements.extend(overlays);
        column_of(elements)
            .build()
            .keyed(STAGE)
            .width(Length::FILL)
            .height(Length::FILL)
    }

    pub(crate) fn finish_fragment(self) -> Element {
        let mut elements = lower::elements(self.close());
        match elements.len() {
            1 => elements.pop().expect("one element"),
            _ => column_of(elements).build().width(Length::FILL),
        }
    }

    pub(crate) fn finish_list_item(self) -> ListItem {
        let mut nodes = self.close();
        if nodes.len() != 1 {
            violation(format!(
                "the item callback declared {} list items instead of 1",
                nodes.len()
            ));
        }
        let node = nodes.pop().expect("one node");
        match node.kind {
            Kind::ListItem(item) => item.finish(|| unreachable!("list items are ready")),
            _ => unreachable!("a list item root only admits list items"),
        }
    }
}

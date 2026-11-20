use std::ops::Range;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

pub(crate) const CHUNK_CAPACITY: usize = 1024;

pub(crate) const CHUNK_FILL: usize = 512;

const LINE_BREAK: u8 = b'\n';

static PLANTED: AtomicU64 = AtomicU64::new(0);

pub(crate) type Link = Option<Arc<Node>>;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Extent {
    pub(crate) bytes: usize,
    pub(crate) chars: usize,
    pub(crate) breaks: usize,
    pub(crate) chunks: usize,
}

impl Extent {
    pub(crate) fn of(text: &str) -> Extent {
        Extent {
            bytes: text.len(),
            chars: text.chars().count(),
            breaks: breaks_in(text.as_bytes()),
            chunks: 1,
        }
    }

    fn joined(self, other: Extent) -> Extent {
        Extent {
            bytes: self.bytes + other.bytes,
            chars: self.chars + other.chars,
            breaks: self.breaks + other.breaks,
            chunks: self.chunks + other.chunks,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Node {
    pub(crate) left: Link,
    pub(crate) right: Link,
    pub(crate) chunk: String,
    priority: u64,
    own: Extent,
    whole: Extent,
}

impl Node {
    fn planted(chunk: String) -> Node {
        let own = Extent::of(&chunk);
        Node {
            left: None,
            right: None,
            chunk,
            priority: priority(),
            own,
            whole: own,
        }
    }

    pub(crate) fn own(&self) -> Extent {
        self.own
    }

    fn measure(&mut self) {
        self.own = Extent::of(&self.chunk);
    }

    fn total(&mut self) {
        self.whole = extent(&self.left)
            .joined(self.own)
            .joined(extent(&self.right));
    }

    fn take_in(&mut self, at: usize, text: &str) -> bool {
        if self.chunk.len() + text.len() > CHUNK_CAPACITY {
            return false;
        }
        self.chunk.insert_str(at, text);
        self.measure();
        true
    }
}

pub(crate) fn extent(link: &Link) -> Extent {
    link.as_ref()
        .map_or_else(Extent::default, |node| node.whole)
}

pub(crate) fn breaks_in(text: &[u8]) -> usize {
    text.iter().filter(|byte| **byte == LINE_BREAK).count()
}

pub(crate) fn after_break(text: &[u8], nth: usize) -> usize {
    text.iter()
        .enumerate()
        .filter(|(_, byte)| **byte == LINE_BREAK)
        .nth(nth)
        .map_or(text.len(), |(at, _)| at + 1)
}

pub(crate) fn planted(text: &str) -> Link {
    let mut tree = None;
    let mut rest = text;
    while !rest.is_empty() {
        let (chunk, later) = rest.split_at(rest.floor_char_boundary(CHUNK_FILL));
        tree = merge(tree, Some(Arc::new(Node::planted(chunk.to_owned()))));
        rest = later;
    }
    tree
}

pub(crate) fn split(link: Link, at: usize) -> (Link, Link) {
    let Some(shared) = link else {
        return (None, None);
    };
    let mut node = Arc::unwrap_or_clone(shared);
    let before = extent(&node.left).bytes;
    let end = before + node.chunk.len();
    if at <= before {
        let (head, tail) = split(node.left.take(), at);
        node.left = tail;
        node.total();
        return (head, Some(Arc::new(node)));
    }
    if at >= end {
        let (head, tail) = split(node.right.take(), at - end);
        node.right = head;
        node.total();
        return (Some(Arc::new(node)), tail);
    }
    let later = Node::planted(node.chunk.split_off(at - before));
    let tail = merge(Some(Arc::new(later)), node.right.take());
    node.measure();
    node.total();
    (Some(Arc::new(node)), tail)
}

pub(crate) fn merge(head: Link, tail: Link) -> Link {
    let (head, tail) = match (head, tail) {
        (None, rest) | (rest, None) => return rest,
        (Some(head), Some(tail)) => (head, tail),
    };
    if head.priority >= tail.priority {
        let mut node = Arc::unwrap_or_clone(head);
        node.right = merge(node.right.take(), Some(tail));
        node.total();
        Some(Arc::new(node))
    } else {
        let mut node = Arc::unwrap_or_clone(tail);
        node.left = merge(Some(head), node.left.take());
        node.total();
        Some(Arc::new(node))
    }
}

pub(crate) fn grow(link: &mut Link, at: usize, text: &str) -> bool {
    let Some(shared) = link else {
        return false;
    };
    let before = extent(&shared.left).bytes;
    let end = before + shared.chunk.len();
    let node = Arc::make_mut(shared);
    let grown = (at <= before && grow(&mut node.left, at, text))
        || (before <= at && at <= end && node.take_in(at - before, text))
        || (end <= at && grow(&mut node.right, at - end, text));
    if grown {
        node.total();
    }
    grown
}

pub(crate) fn shrink(link: &mut Link, range: Range<usize>) -> bool {
    let Some(shared) = link else {
        return false;
    };
    let before = extent(&shared.left).bytes;
    let end = before + shared.chunk.len();
    let within = before <= range.start && range.end <= end && range.len() < shared.chunk.len();
    if !within && range.start < end && before < range.end {
        return false;
    }
    let node = Arc::make_mut(shared);
    let shrunk = if within {
        node.chunk
            .replace_range(range.start - before..range.end - before, "");
        node.measure();
        true
    } else if range.end <= before {
        shrink(&mut node.left, range)
    } else {
        shrink(&mut node.right, range.start - end..range.end - end)
    };
    if shrunk {
        node.total();
    }
    shrunk
}

fn priority() -> u64 {
    let mut mixed = PLANTED
        .fetch_add(1, Ordering::Relaxed)
        .wrapping_add(1)
        .wrapping_mul(0x9e37_79b9_7f4a_7c15);
    mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    mixed ^ (mixed >> 31)
}

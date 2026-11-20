use crate::runtime::layout::Node;

const NONE: u32 = u32::MAX;

const SPAN: u64 = 1 << 44;

const STRIDE: u64 = 1 << 24;

const PRIM_BITS: u32 = 20;

const PRIM_LAST: u64 = (1 << PRIM_BITS) - 1;

#[derive(Debug, Default)]
pub(crate) struct Labels {
    label: Vec<u64>,
    prev: Vec<u32>,
    next: Vec<u32>,
    attached: Vec<bool>,
    head: Option<u32>,
    relabeled: Vec<Node>,
}

impl Labels {
    pub(crate) fn prim(&self, node: Node, index: usize) -> u64 {
        (self.label[node.index()] << PRIM_BITS) | (index as u64).min(PRIM_LAST)
    }

    pub(crate) fn label(&self, node: Node) -> u64 {
        self.label[node.index()]
    }

    pub(crate) fn is_attached(&self, node: Node) -> bool {
        self.attached.get(node.index()).copied().unwrap_or(false)
    }

    pub(crate) fn follows(&self, node: Node, anchor: Node) -> bool {
        self.is_attached(node) && self.prev[node.index()] == anchor.slot()
    }

    pub(crate) fn relabeled(&self) -> &[Node] {
        &self.relabeled
    }

    pub(crate) fn settle_relabeled(&mut self) {
        self.relabeled.clear();
    }

    fn grow(&mut self, node: Node) {
        let needed_len = node.index() + 1;
        if self.label.len() < needed_len {
            self.label.resize(needed_len, 0);
            self.prev.resize(needed_len, NONE);
            self.next.resize(needed_len, NONE);
            self.attached.resize(needed_len, false);
        }
    }

    pub(crate) fn join(&mut self, last: Node, then: Node) {
        self.grow(then);
        self.grow(last);
        self.next[last.index()] = then.slot();
        self.prev[then.index()] = last.slot();
    }

    pub(crate) fn single(&mut self, node: Node) {
        self.grow(node);
        self.prev[node.index()] = NONE;
        self.next[node.index()] = NONE;
        self.attached[node.index()] = false;
    }

    pub(crate) fn detach(&mut self, first: Node, last: Node) {
        if !self.is_attached(first) {
            return;
        }
        let before = self.prev[first.index()];
        let after = self.next[last.index()];
        match before {
            NONE => self.head = (after != NONE).then_some(after),
            before => self.next[before as usize] = after,
        }
        if after != NONE {
            self.prev[after as usize] = before;
        }
        self.prev[first.index()] = NONE;
        self.next[last.index()] = NONE;
        let mut at = first.slot();
        loop {
            self.attached[at as usize] = false;
            if at == last.slot() {
                break;
            }
            at = self.next[at as usize];
        }
    }

    pub(crate) fn attach_after(&mut self, anchor: Option<Node>, first: Node, last: Node) {
        self.grow(first);
        self.grow(last);
        let after = match anchor {
            Some(anchor) => self.next[anchor.index()],
            None => self.head.unwrap_or(NONE),
        };
        match anchor {
            Some(anchor) => {
                self.next[anchor.index()] = first.slot();
                self.prev[first.index()] = anchor.slot();
            }
            None => {
                self.head = Some(first.slot());
                self.prev[first.index()] = NONE;
            }
        }
        self.next[last.index()] = after;
        if after != NONE {
            self.prev[after as usize] = last.slot();
        }
        let mut count = 0usize;
        let mut at = first.slot();
        loop {
            self.attached[at as usize] = true;
            count += 1;
            if at == last.slot() {
                break;
            }
            at = self.next[at as usize];
        }
        self.spread(first.slot(), last.slot(), count);
    }

    fn bound_below(&self, slot: u32) -> u64 {
        match self.prev[slot as usize] {
            NONE => 0,
            before => self.label[before as usize],
        }
    }

    fn bound_above(&self, slot: u32) -> u64 {
        match self.next[slot as usize] {
            NONE => SPAN,
            after => self.label[after as usize],
        }
    }

    fn spread(&mut self, first: u32, last: u32, count: usize) {
        let (mut first, mut last, mut count) = (first, last, count);
        let mut reach = 1usize;
        loop {
            let low = self.bound_below(first);
            let high = self.bound_above(last);
            let room = high.saturating_sub(low);
            let needed_len = (count as u64 + 1) * (count as u64).max(2);
            let whole = self.prev[first as usize] == NONE && self.next[last as usize] == NONE;
            if room >= needed_len || (whole && room > count as u64) {
                self.assign(first, last, count, low, room);
                return;
            }
            for _ in 0..reach {
                if self.prev[first as usize] != NONE {
                    first = self.prev[first as usize];
                    count += 1;
                }
                if self.next[last as usize] != NONE {
                    last = self.next[last as usize];
                    count += 1;
                }
            }
            reach *= 2;
        }
    }

    fn assign(&mut self, first: u32, last: u32, count: usize, low: u64, room: u64) {
        let step = (room / (count as u64 + 1)).min(STRIDE);
        let mut at = first;
        let mut label = low;
        loop {
            label += step;
            if self.label[at as usize] != label {
                self.label[at as usize] = label;
                self.relabeled.push(Node::from_slot(at));
            }
            if at == last {
                break;
            }
            at = self.next[at as usize];
        }
    }
}

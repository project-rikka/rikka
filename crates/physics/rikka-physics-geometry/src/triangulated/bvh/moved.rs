#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct MovedMarks {
    nodes: Vec<u8>,
    positions: Vec<u64>,
    marked_nodes: Vec<u32>,
    marked_words: Vec<u32>,
    pub(super) pending: Vec<u32>,
}

impl MovedMarks {
    pub(crate) fn clear(&mut self) {
        for &node in &self.marked_nodes {
            self.nodes[node as usize] = 0;
        }
        for &word in &self.marked_words {
            self.positions[word as usize] = 0;
        }
        self.marked_nodes.clear();
        self.marked_words.clear();
        self.pending.clear();
    }

    pub(crate) fn fit(&mut self, nodes: usize, positions: usize) {
        if self.nodes.len() < nodes {
            self.nodes.resize(nodes, 0);
        }
        let words = positions.div_ceil(64);
        if self.positions.len() < words {
            self.positions.resize(words, 0);
        }
    }

    #[inline]
    pub(crate) fn mark_position(&mut self, position: u32) {
        let word = &mut self.positions[position as usize >> 6];
        if *word == 0 {
            self.marked_words.push(position >> 6);
        }
        let bit = 1 << (position & 63);
        if *word & bit == 0 {
            *word |= bit;
            self.pending.push(position);
        }
    }

    #[inline]
    pub(crate) fn mark_slot(&mut self, node: u32, slot: u32) -> bool {
        let mask = &mut self.nodes[node as usize];
        if *mask == 0 {
            self.marked_nodes.push(node);
        }
        let fresh = *mask & (1 << slot) == 0;
        *mask |= 1 << slot;
        fresh
    }

    #[inline]
    pub(crate) fn set_slots(&mut self, node: u32, slots: u32) {
        if slots == 0 {
            return;
        }
        let mask = &mut self.nodes[node as usize];
        if *mask == 0 {
            self.marked_nodes.push(node);
        }
        *mask |= slots as u8;
    }

    #[inline]
    pub(crate) fn slots(&self, node: u32) -> u32 {
        self.nodes
            .get(node as usize)
            .map_or(0, |&mask| u32::from(mask))
    }

    #[inline]
    pub(crate) fn positions(&self, start: u32, count: u32) -> u32 {
        let word = start as usize >> 6;
        let shift = start & 63;
        let low = self.positions.get(word).map_or(0, |&bits| bits >> shift);
        let high = match shift {
            0..=60 => 0,
            _ => self
                .positions
                .get(word + 1)
                .map_or(0, |&bits| bits << (64 - shift)),
        };
        (low | high) as u32 & ((1u32 << count.min(4)) - 1)
    }

    #[inline]
    pub(crate) fn any_position(&self, start: u32, count: u32) -> bool {
        let (first, last) = (start as usize, (start + count - 1) as usize);
        let (low, high) = (first >> 6, last >> 6);
        let head = u64::MAX << (first & 63);
        let tail = u64::MAX >> (63 - (last & 63));
        if low == high {
            return self.positions[low] & head & tail != 0;
        }
        self.positions[low] & head != 0
            || self.positions[low + 1..high].iter().any(|&word| word != 0)
            || self.positions[high] & tail != 0
    }
}

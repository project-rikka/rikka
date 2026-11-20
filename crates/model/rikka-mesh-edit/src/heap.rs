#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct HeapNode(u32);

#[derive(Clone, Debug)]
struct HeapEntry<T> {
    value: f32,
    position: u32,
    item: T,
}

#[derive(Clone, Debug)]
pub struct Heap<T> {
    tree: Vec<u32>,
    entries: Vec<HeapEntry<T>>,
    free: Vec<u32>,
}

impl<T: Copy> Default for Heap<T> {
    fn default() -> Heap<T> {
        Heap::new()
    }
}

impl<T: Copy> Heap<T> {
    #[must_use]
    pub fn new() -> Heap<T> {
        Heap::with_capacity(0)
    }

    #[must_use]
    pub fn with_capacity(capacity: usize) -> Heap<T> {
        Heap {
            tree: Vec::with_capacity(capacity),
            entries: Vec::with_capacity(capacity),
            free: Vec::new(),
        }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.tree.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tree.is_empty()
    }

    pub fn clear(&mut self) {
        self.tree.clear();
        self.entries.clear();
        self.free.clear();
    }

    fn value_at(&self, position: usize) -> f32 {
        self.entries[self.tree[position] as usize].value
    }

    fn swap(&mut self, a: usize, b: usize) {
        self.tree.swap(a, b);
        self.entries[self.tree[a] as usize].position = a as u32;
        self.entries[self.tree[b] as usize].position = b as u32;
    }

    fn sift_down(&mut self, mut position: usize) {
        let size = self.tree.len();
        loop {
            let left = position * 2 + 1;
            let right = position * 2 + 2;
            let mut smallest = position;
            if left < size && self.value_at(left) < self.value_at(smallest) {
                smallest = left;
            }
            if right < size && self.value_at(right) < self.value_at(smallest) {
                smallest = right;
            }
            if smallest == position {
                break;
            }
            self.swap(position, smallest);
            position = smallest;
        }
    }

    fn sift_up(&mut self, mut position: usize) {
        while position > 0 {
            let parent = (position - 1) >> 1;
            if self.value_at(parent) < self.value_at(position) {
                break;
            }
            self.swap(parent, position);
            position = parent;
        }
    }

    pub fn insert(&mut self, value: f32, item: T) -> HeapNode {
        let position = self.tree.len() as u32;
        let entry = HeapEntry {
            value,
            position,
            item,
        };
        let node = match self.free.pop() {
            Some(slot) => {
                self.entries[slot as usize] = entry;
                slot
            }
            None => {
                self.entries.push(entry);
                (self.entries.len() - 1) as u32
            }
        };
        self.tree.push(node);
        self.sift_up(position as usize);
        HeapNode(node)
    }

    pub fn insert_or_update(&mut self, slot: &mut Option<HeapNode>, value: f32, item: T) {
        match *slot {
            Some(node) => self.update_item(node, value, item),
            None => *slot = Some(self.insert(value, item)),
        }
    }

    #[must_use]
    pub fn top(&self) -> Option<(HeapNode, f32, T)> {
        let &node = self.tree.first()?;
        let entry = &self.entries[node as usize];
        Some((HeapNode(node), entry.value, entry.item))
    }

    #[must_use]
    pub fn top_value(&self) -> Option<f32> {
        self.tree
            .first()
            .map(|&node| self.entries[node as usize].value)
    }

    pub fn pop(&mut self) -> Option<(f32, T)> {
        let &node = self.tree.first()?;
        let entry = &self.entries[node as usize];
        let popped = (entry.value, entry.item);
        self.free.push(node);
        let last = self.tree.len() - 1;
        if last > 0 {
            self.swap(0, last);
            self.tree.pop();
            self.sift_down(0);
        } else {
            self.tree.pop();
        }
        Some(popped)
    }

    pub fn pop_min(&mut self) -> Option<T> {
        self.pop().map(|(_, item)| item)
    }

    pub fn remove(&mut self, node: HeapNode) {
        let mut position = self.entries[node.0 as usize].position as usize;
        while position > 0 {
            let parent = (position - 1) >> 1;
            self.swap(parent, position);
            position = parent;
        }
        self.pop();
    }

    #[must_use]
    pub fn value(&self, node: HeapNode) -> f32 {
        self.entries[node.0 as usize].value
    }

    #[must_use]
    pub fn item(&self, node: HeapNode) -> T {
        self.entries[node.0 as usize].item
    }

    pub fn update(&mut self, node: HeapNode, value: f32) {
        let entry = &mut self.entries[node.0 as usize];
        let position = entry.position as usize;
        if value < entry.value {
            entry.value = value;
            self.sift_up(position);
        } else if value > entry.value {
            entry.value = value;
            self.sift_down(position);
        }
    }

    pub fn update_item(&mut self, node: HeapNode, value: f32, item: T) {
        self.entries[node.0 as usize].item = item;
        self.update(node, value);
    }
}

#[derive(Clone, Debug)]
pub struct SimpleHeap<T> {
    nodes: Vec<(f32, T)>,
}

impl<T: Copy> Default for SimpleHeap<T> {
    fn default() -> SimpleHeap<T> {
        SimpleHeap::new()
    }
}

impl<T: Copy> SimpleHeap<T> {
    #[must_use]
    pub fn new() -> SimpleHeap<T> {
        SimpleHeap { nodes: Vec::new() }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn clear(&mut self) {
        self.nodes.clear();
    }

    #[must_use]
    pub fn top_value(&self) -> Option<f32> {
        self.nodes.first().map(|node| node.0)
    }

    pub fn insert(&mut self, value: f32, item: T) {
        self.nodes.push((value, item));
        let mut index = self.nodes.len() - 1;
        while index > 0 {
            let parent = (index - 1) >> 1;
            if value >= self.nodes[parent].0 {
                break;
            }
            self.nodes[index] = self.nodes[parent];
            index = parent;
        }
        self.nodes[index] = (value, item);
    }

    pub fn pop(&mut self) -> Option<(f32, T)> {
        let top = *self.nodes.first()?;
        let last = self.nodes.pop()?;
        if self.nodes.is_empty() {
            return Some(top);
        }
        let size = self.nodes.len();
        let mut index = 0;
        loop {
            let left = 2 * index + 1;
            let right = left + 1;
            let mut smallest = index;
            let mut smallest_value = last.0;
            if left < size && self.nodes[left].0 < smallest_value {
                smallest = left;
                smallest_value = self.nodes[left].0;
            }
            if right < size && self.nodes[right].0 < smallest_value {
                smallest = right;
            }
            if smallest == index {
                break;
            }
            self.nodes[index] = self.nodes[smallest];
            index = smallest;
        }
        self.nodes[index] = last;
        Some(top)
    }

    pub fn pop_min(&mut self) -> Option<T> {
        self.pop().map(|(_, item)| item)
    }
}

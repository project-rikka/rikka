use std::cmp::Ordering;
use std::collections::BinaryHeap;

#[derive(Clone, Debug)]
pub(crate) struct Graph {
    starts: Vec<u32>,
    targets: Vec<u32>,
    lengths: Vec<f32>,
}

#[derive(Clone, Debug)]
pub(crate) struct Reached {
    pub(crate) distance: Vec<f32>,
    pub(crate) source: Vec<u32>,
    pub(crate) previous: Vec<u32>,
    pub(crate) order: Vec<u32>,
}

#[derive(Clone, Copy, PartialEq)]
struct Visit {
    distance: f32,
    node: u32,
}

impl Eq for Visit {}

impl Ord for Visit {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .distance
            .total_cmp(&self.distance)
            .then_with(|| other.node.cmp(&self.node))
    }
}

impl PartialOrd for Visit {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Graph {
    pub(crate) fn new(nodes: usize, edges: impl IntoIterator<Item = (u32, u32, f32)>) -> Graph {
        let mut both: Vec<(u32, u32, f32)> = Vec::new();
        for (a, b, length) in edges {
            if a == b || a as usize >= nodes || b as usize >= nodes {
                continue;
            }
            both.push((a, b, length));
            both.push((b, a, length));
        }
        both.sort_by(|x, y| x.0.cmp(&y.0).then(x.1.cmp(&y.1)).then(x.2.total_cmp(&y.2)));
        both.dedup_by(|later, earlier| later.0 == earlier.0 && later.1 == earlier.1);
        let mut starts = vec![0u32; nodes + 1];
        for (a, _, _) in &both {
            starts[*a as usize + 1] += 1;
        }
        for index in 0..nodes {
            starts[index + 1] += starts[index];
        }
        Graph {
            starts,
            targets: both.iter().map(|edge| edge.1).collect(),
            lengths: both.iter().map(|edge| edge.2).collect(),
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.starts.len() - 1
    }

    pub(crate) fn neighbours(&self, node: u32) -> impl Iterator<Item = (u32, f32)> + '_ {
        let range = self.starts[node as usize] as usize..self.starts[node as usize + 1] as usize;
        self.targets[range.clone()]
            .iter()
            .copied()
            .zip(self.lengths[range].iter().copied())
    }

    pub(crate) fn reach(&self, sources: impl IntoIterator<Item = u32>, limit: f32) -> Reached {
        let count = self.len();
        let mut reached = Reached {
            distance: vec![f32::INFINITY; count],
            source: vec![u32::MAX; count],
            previous: vec![u32::MAX; count],
            order: Vec::new(),
        };
        let mut heap = BinaryHeap::new();
        for source in sources {
            if (source as usize) < count && reached.distance[source as usize] > 0.0 {
                reached.distance[source as usize] = 0.0;
                reached.source[source as usize] = source;
                heap.push(Visit {
                    distance: 0.0,
                    node: source,
                });
            }
        }
        let mut done = vec![false; count];
        while let Some(Visit { distance, node }) = heap.pop() {
            if done[node as usize] {
                continue;
            }
            done[node as usize] = true;
            reached.order.push(node);
            for (next, length) in self.neighbours(node) {
                let through = distance + length;
                if through > limit {
                    continue;
                }
                let slot = next as usize;
                let better = through < reached.distance[slot]
                    || (through == reached.distance[slot]
                        && reached.source[node as usize] < reached.source[slot]);
                if !done[slot] && better {
                    reached.distance[slot] = through;
                    reached.source[slot] = reached.source[node as usize];
                    reached.previous[slot] = node;
                    heap.push(Visit {
                        distance: through,
                        node: next,
                    });
                }
            }
        }
        reached
    }

    pub(crate) fn components(&self) -> (Vec<u32>, usize) {
        let mut label = vec![u32::MAX; self.len()];
        let mut count = 0u32;
        let mut stack = Vec::new();
        for start in 0..self.len() as u32 {
            if label[start as usize] != u32::MAX {
                continue;
            }
            label[start as usize] = count;
            stack.push(start);
            while let Some(node) = stack.pop() {
                for (next, _) in self.neighbours(node) {
                    if label[next as usize] == u32::MAX {
                        label[next as usize] = count;
                        stack.push(next);
                    }
                }
            }
            count += 1;
        }
        (label, count as usize)
    }
}

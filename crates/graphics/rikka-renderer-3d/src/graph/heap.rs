use crate::graph::compile::Lifetime;

#[derive(Clone, Copy, Debug)]
pub(super) struct Demand {
    pub size: u64,
    pub alignment: u64,
    pub lifetime: Lifetime,
}

#[derive(Clone, Debug, Default)]
pub(super) struct Placement {
    pub offsets: Vec<u64>,
    pub size: u64,
    pub alignment: u64,
}

pub(super) fn place(demands: &[Demand]) -> Placement {
    let mut order: Vec<usize> = (0..demands.len()).collect();
    order.sort_by(|&a, &b| {
        demands[b]
            .size
            .cmp(&demands[a].size)
            .then(demands[a].lifetime.first.cmp(&demands[b].lifetime.first))
    });
    let mut offsets = vec![0; demands.len()];
    let mut placed: Vec<usize> = Vec::with_capacity(demands.len());
    let mut size = 0;
    let mut alignment = 1;
    for index in order {
        let demand = demands[index];
        alignment = alignment.max(demand.alignment);
        let mut busy: Vec<(u64, u64)> = placed
            .iter()
            .filter(|&&other| demands[other].lifetime.overlaps(demand.lifetime))
            .map(|&other| (offsets[other], offsets[other] + demands[other].size))
            .collect();
        busy.sort_unstable();
        let mut offset: u64 = 0;
        for (start, end) in busy {
            if offset.next_multiple_of(demand.alignment) + demand.size <= start {
                break;
            }
            offset = offset.max(end);
        }
        let offset = offset.next_multiple_of(demand.alignment);
        offsets[index] = offset;
        size = size.max(offset + demand.size);
        placed.push(index);
    }
    Placement {
        offsets,
        size,
        alignment,
    }
}

pub(super) fn overlap(a: (u64, u64), b: (u64, u64)) -> bool {
    a.0 < b.0 + b.1 && b.0 < a.0 + a.1
}

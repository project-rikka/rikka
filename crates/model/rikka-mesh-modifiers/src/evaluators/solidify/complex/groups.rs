use rikka_polymesh::Polymesh;

use super::{EdgeGroup, GroupId, Settings, Topology, other_side};

pub(super) fn build(topology: &mut Topology, mesh: &Polymesh, settings: &Settings) {
    let mut new_verts = 0u32;
    for vert in 0..mesh.point_count() {
        if topology.vert_edges[vert].len() < 2 {
            continue;
        }
        let (mut groups, long) = sorted_groups(topology, mesh, vert as u32);
        if long {
            topology.has_singularities |= split_long_groups(topology, &mut groups);
        }
        for (index, group) in groups.iter_mut().enumerate() {
            for &edge in &group.edges {
                let new_edge = &mut topology.edges[edge as usize];
                let [_, b] = mesh.edges[new_edge.old_edge as usize];
                let flip = usize::from(topology.vm[b as usize] == vert as u32);
                new_edge.groups[flip] = Some(GroupId {
                    vert: vert as u32,
                    index: index as u32,
                });
            }
            if settings.shell || (settings.rim && !group.is_orig_closed) {
                group.new_vert = new_verts;
                new_verts += 1;
            }
        }
        topology.groups[vert] = groups;
    }
}

fn sorted_groups(topology: &Topology, mesh: &Polymesh, vert: u32) -> (Vec<EdgeGroup>, bool) {
    let mut unassigned: Vec<Option<u32>> = Vec::new();
    for &edge in &topology.vert_edges[vert as usize] {
        if let Some(list) = topology.edge_list[edge as usize] {
            unassigned.extend(topology.lists[list as usize].iter().map(|&id| Some(id)));
        }
    }
    let total = unassigned.len();
    let mut groups: Vec<EdgeGroup> = Vec::new();
    let mut assigned = 0;
    let mut topo_groups = 0u32;
    let mut track: [Option<u32>; 2] = [None, None];
    let mut last_open: Option<u32> = None;
    let mut long = false;
    while assigned < total {
        let mut found: Option<(usize, u32)> = None;
        let mut at_start = false;
        let current = groups.len().checked_sub(1);
        if let Some(current) = current
            && groups[current].edges.is_empty()
        {
            let mut slot = 0;
            let mut edge = None;
            while edge.is_none() && slot < total {
                edge = unassigned[slot];
                slot += 1;
                if let (Some(id), Some(open)) = (edge, last_open) {
                    let candidate = &topology.edges[id as usize];
                    if candidate.faces[0] != Some(open) || candidate.faces[1].is_some() {
                        edge = None;
                    }
                }
            }
            if edge.is_none() && last_open.is_some() {
                topo_groups += 1;
                last_open = None;
                groups[current].topo_group += 1;
                slot = 0;
                while edge.is_none() && slot < total {
                    edge = unassigned[slot];
                    slot += 1;
                }
            } else if last_open.is_none() && current > 0 {
                topo_groups += 1;
                groups[current].topo_group += 1;
            }
            let Some(id) = edge else {
                break;
            };
            found = Some((slot - 1, id));
            let candidate = &topology.edges[id as usize];
            let [a, _] = mesh.edges[candidate.old_edge as usize];
            if last_open.is_none() && topology.vm[a as usize] == vert {
                track = candidate.faces;
                if candidate.faces[1].is_none() {
                    last_open = candidate.faces[0].map(other_side);
                }
            } else {
                track = [candidate.faces[1], candidate.faces[0]];
            }
        } else if let Some(current) = current {
            for (slot, entry) in unassigned.iter().enumerate() {
                let Some(id) = *entry else {
                    continue;
                };
                let candidate = &topology.edges[id as usize];
                if candidate.faces[0] == track[1] {
                    at_start = false;
                    track[1] = candidate.faces[1];
                    found = Some((slot, id));
                    if candidate.faces[1].is_none() {
                        groups[current].is_orig_closed = false;
                        last_open = candidate.faces[0].map(other_side);
                    }
                    break;
                }
                if candidate.faces[0] == track[0] {
                    at_start = true;
                    track[0] = candidate.faces[1];
                    found = Some((slot, id));
                    if candidate.faces[1].is_none() {
                        groups[current].is_orig_closed = false;
                    }
                    break;
                }
                if candidate.faces[1].is_some() {
                    if candidate.faces[1] == track[1] {
                        at_start = false;
                        track[1] = candidate.faces[0];
                        found = Some((slot, id));
                        break;
                    }
                    if candidate.faces[1] == track[0] {
                        at_start = true;
                        track[0] = candidate.faces[0];
                        found = Some((slot, id));
                        break;
                    }
                }
            }
        }
        if let (Some((slot, id)), Some(group)) = (found, groups.last_mut()) {
            unassigned[slot] = None;
            assigned += 1;
            match at_start {
                true => group.edges.insert(0, id),
                false => group.edges.push(id),
            }
            if let Some(&last) = group.edges.last()
                && topology.edges[last as usize].faces[1].is_some()
            {
                last_open = None;
            }
            if group.edges.len() > 3 {
                long = true;
            }
        } else {
            groups.push(EdgeGroup::new(Vec::new(), topo_groups));
            track = [None, None];
        }
    }
    (groups, long)
}

fn split_long_groups(topology: &Topology, groups: &mut Vec<EdgeGroup>) -> bool {
    let mut singularities = false;
    let mut splits = 0u32;
    let mut add_index = 0usize;
    let count = groups.len();
    for j in 0..count {
        let g = groups[j + add_index].clone();
        let len = g.edges.len();
        if len <= 3 {
            continue;
        }
        let mut doubles = vec![false; len];
        let mut has_doubles = false;
        for k in 0..len {
            for l in k + 1..len {
                if topology.edges[g.edges[k] as usize].old_edge
                    == topology.edges[g.edges[l] as usize].old_edge
                {
                    doubles[k] = true;
                    doubles[l] = true;
                    has_doubles = true;
                }
            }
        }
        if !has_doubles {
            continue;
        }
        let prior_splits = splits;
        let prior_index = add_index;
        let mut unique_start: Option<usize> = None;
        let mut first_unique_end: Option<usize> = None;
        let mut last_split: Option<usize> = None;
        let mut first_split: Option<usize> = None;
        let mut first_even_split = false;
        let mut real_k = 0usize;
        let closed_bound = |first_unique_end: Option<usize>| first_unique_end.unwrap_or(0) + len;
        while real_k < len
            || (g.is_orig_closed
                && (real_k <= closed_bound(first_unique_end) || first_split != last_split))
        {
            let k = real_k % len;
            if !doubles[k] {
                if first_unique_end.is_some() && unique_start.is_none() {
                    unique_start = Some(real_k);
                }
            } else if first_unique_end.is_none() {
                first_unique_end = Some(k);
            } else if let Some(start) = unique_start {
                let split = (start + real_k).div_ceil(2) % len;
                let is_even_split = (start + real_k) & 1 == 1;
                if let Some(last) = last_split {
                    if prior_splits != splits {
                        groups.insert(j + add_index + 1, g.clone());
                        add_index += 1;
                    }
                    let edges = match last > split {
                        true => [&g.edges[last..], &g.edges[..split]].concat(),
                        false => g.edges[last..split].to_vec(),
                    };
                    groups[j + add_index] = EdgeGroup {
                        is_orig_closed: g.is_orig_closed,
                        is_even_split,
                        split: (add_index - prior_index) as u32 + 1 + u32::from(!g.is_orig_closed),
                        ..EdgeGroup::new(edges, g.topo_group)
                    };
                    splits += 1;
                }
                last_split = Some(split);
                if first_split.is_none() {
                    first_split = Some(split);
                    first_even_split = is_even_split;
                }
                unique_start = None;
            }
            real_k += 1;
        }
        if let (Some(first), Some(last)) = (first_split, last_split)
            && !g.is_orig_closed
        {
            let opening = EdgeGroup {
                is_orig_closed: false,
                is_even_split: first_even_split,
                split: 1,
                ..EdgeGroup::new(g.edges[..first].to_vec(), g.topo_group)
            };
            match prior_splits != splits {
                true => {
                    groups.insert(j + prior_index, opening);
                    add_index += 1;
                }
                false => groups[j + add_index] = opening,
            }
            add_index += 1;
            splits += 1;
            let closing = EdgeGroup {
                is_orig_closed: false,
                split: (add_index - prior_index) as u32 + 1,
                ..EdgeGroup::new(g.edges[last..].to_vec(), g.topo_group)
            };
            groups.insert(j + add_index, closing);
        }
        if first_unique_end.is_some() && prior_splits == splits {
            singularities = true;
            groups[j + add_index].is_singularity = true;
        }
    }
    singularities
}

pub(super) fn is_open_boundary(group: &EdgeGroup, settings: &Settings) -> bool {
    (settings.rim && !group.is_orig_closed) || (settings.shell && group.split != 0)
}

pub(super) fn ends_topology(groups: &[EdgeGroup], index: usize) -> bool {
    groups
        .get(index + 1)
        .is_none_or(|next| next.topo_group != groups[index].topo_group)
}

use crate::ids::VertId;
use crate::mesh::EditMesh;
use crate::sets::IdMap;

impl EditMesh {
    #[must_use]
    pub fn topology_mirror(&self) -> IdMap<VertId, VertId> {
        let verts: Vec<VertId> = self.verts().collect();
        let mut dense = vec![u32::MAX; self.vert_capacity()];
        for (index, vert) in verts.iter().enumerate() {
            dense[vert.index()] = index as u32;
        }
        let edges: Vec<[u32; 2]> = self
            .edges()
            .map(|edge| self.edge_verts(edge).map(|vert| dense[vert.index()]))
            .collect();
        let table = topology_mirror_table(verts.len(), &edges);
        let mut mirror = IdMap::with_capacity(self.vert_capacity());
        for (index, other) in table.into_iter().enumerate() {
            if let Some(other) = other {
                mirror.insert(verts[index], verts[other as usize]);
            }
        }
        mirror
    }
}

#[must_use]
pub fn topology_mirror_table(vert_count: usize, edges: &[[u32; 2]]) -> Vec<Option<u32>> {
    let mut hash = vec![0u32; vert_count];
    for &[a, b] in edges {
        hash[a as usize] = hash[a as usize].wrapping_add(1);
        hash[b as usize] = hash[b as usize].wrapping_add(1);
    }
    let mut previous = hash.clone();
    let mut unique_previous: i64 = -1;
    let mut unique_edges_previous: i64 = -1;
    let mut pass: u32 = 1;
    loop {
        let mut unique_edges: i64 = 0;
        for &[a, b] in edges {
            let (a, b) = (a as usize, b as usize);
            hash[a] = hash[a].wrapping_add(previous[b].wrapping_mul(pass));
            hash[b] = hash[b].wrapping_add(previous[a].wrapping_mul(pass));
            unique_edges += i64::from(hash[a] != hash[b]);
        }
        previous.copy_from_slice(&hash);
        previous.sort_unstable();
        let mut unique: i64 = 1;
        for pair in previous.windows(2) {
            if pair[0] != pair[1] {
                unique += 1;
            }
        }
        if unique <= unique_previous && unique_edges <= unique_edges_previous {
            break;
        }
        unique_previous = unique;
        unique_edges_previous = unique_edges;
        previous.copy_from_slice(&hash);
        pass = pass.wrapping_add(1);
    }
    let mut pairs: Vec<(u32, u32)> = hash
        .iter()
        .enumerate()
        .map(|(vert, &value)| (value, vert as u32))
        .collect();
    pairs.sort_by_key(|pair| pair.0);
    let mut lookup = vec![None; vert_count];
    let mut last = 0;
    for index in 1..=vert_count {
        if index == vert_count || pairs[index - 1].0 != pairs[index].0 {
            match index - last {
                2 => {
                    let (first, second) = (pairs[index - 1].1, pairs[index - 2].1);
                    lookup[first as usize] = Some(second);
                    lookup[second as usize] = Some(first);
                }
                1 => {
                    let center = pairs[index - 1].1;
                    lookup[center as usize] = Some(center);
                }
                _ => {}
            }
            last = index;
        }
    }
    lookup
}

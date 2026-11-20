use crate::data::KEY_INDEX_NONE;
use crate::error::{Error, Result};
use crate::ids::{EdgeId, LoopId, NONE, VertId};
use crate::mesh::{EditMesh, Flags};
use crate::sets::IdSet;

impl EditMesh {
    fn new_vert_like(&mut self, vert: u32, copy_select: bool) -> u32 {
        let created = self.alloc_vert(self.positions[vert as usize]);
        self.copy_vert_attrs(vert, created);
        self.vert_data.key_index[created as usize] = KEY_INDEX_NONE;
        if copy_select {
            let selected = self.verts.records[vert as usize]
                .flags
                .contains(Flags::SELECT);
            self.verts.records[created as usize]
                .flags
                .set(Flags::SELECT, selected);
        }
        created
    }

    fn edge_supports_separate(&self, edge: u32) -> bool {
        let lp = self.edges.records[edge as usize].first_loop;
        lp != NONE && self.loops.records[lp as usize].radial_next != lp
    }

    pub fn separate_edge(&mut self, edge: EdgeId, lp: LoopId, copy_select: bool) -> Result<EdgeId> {
        self.check_edge(edge)?;
        self.check_loop(lp)?;
        if self.loop_edge(lp) != edge {
            return Err(Error::EdgeNotInFace(edge, self.loop_face(lp)));
        }
        if !self.edge_supports_separate(edge.0) {
            return Ok(edge);
        }
        let created = self.separate_edge_raw(edge.0, lp.0, copy_select);
        self.touch_topology();
        Ok(EdgeId(created))
    }

    fn separate_edge_raw(&mut self, edge: u32, lp: u32, copy_select: bool) -> u32 {
        let [a, b] = self.edges.records[edge as usize].verts;
        let created = self.alloc_edge(a, b);
        self.copy_edge_attrs(edge, created);
        if copy_select {
            let selected = self.edges.records[edge as usize]
                .flags
                .contains(Flags::SELECT);
            self.edges.records[created as usize]
                .flags
                .set(Flags::SELECT, selected);
        }
        self.radial_remove(edge, lp);
        self.radial_append(created, lp);
        created
    }

    fn separate_vert_fans(&mut self, vert: u32, copy_select: bool) -> Vec<VertId> {
        let mut created = vec![VertId(vert)];
        let total = self.vert_edge_count(VertId(vert));
        if total == 0 {
            return created;
        }
        let mut pending: IdSet<EdgeId> = self.vert_edges(VertId(vert)).collect();
        let mut found = 0;
        loop {
            let start = self.verts.records[vert as usize].edge;
            pending.remove(EdgeId(start));
            let mut fan = Vec::new();
            let mut search = vec![start];
            while let Some(edge) = search.pop() {
                fan.push(edge);
                found += 1;
                for lp in self.edge_loops(EdgeId(edge)) {
                    let adjacent = match self.loops.records[lp.index()].vert == vert {
                        true => self.loops.records[lp.index()].prev,
                        false => self.loops.records[lp.index()].next,
                    };
                    let adjacent_edge = self.loops.records[adjacent as usize].edge;
                    if pending.remove(EdgeId(adjacent_edge)) {
                        search.push(adjacent_edge);
                    }
                }
            }
            if found == total {
                break;
            }
            let new_vert = self.new_vert_like(vert, copy_select);
            for edge in fan.into_iter().rev() {
                self.edge_vert_swap(edge, new_vert, vert);
            }
            created.push(VertId(new_vert));
        }
        created
    }

    pub fn separate_vert(
        &mut self,
        vert: VertId,
        edges: &[EdgeId],
        copy_select: bool,
    ) -> Result<Vec<VertId>> {
        self.check_vert(vert)?;
        for &edge in edges {
            self.check_edge(edge)?;
            if !self.edge_has_vert(edge, vert) {
                return Err(Error::VertNotInEdge(vert, edge));
            }
        }
        let mut separated: Vec<Vec<u32>> = Vec::new();
        for &edge in edges {
            if !self.edge_supports_separate(edge.0) {
                continue;
            }
            let mut group = Vec::new();
            while self.edge_supports_separate(edge.0) {
                let lp = self.edges.records[edge.index()].first_loop;
                let created = self.separate_edge_raw(edge.0, lp, copy_select);
                group.push(created);
            }
            group.reverse();
            group.insert(0, edge.0);
            separated.push(group);
        }
        let verts = self.separate_vert_fans(vert.0, copy_select);
        for group in separated.iter().rev() {
            self.rejoin_separated(group);
        }
        self.touch_topology();
        Ok(verts)
    }

    fn rejoin_separated(&mut self, group: &[u32]) {
        let mut remaining: Vec<u32> = group.to_vec();
        let mut index = 0;
        while index < remaining.len() {
            let original = remaining[index];
            let mut probe = index + 1;
            while probe < remaining.len() {
                let candidate = remaining[probe];
                let same = self.edges.is_alive(original)
                    && self.edges.is_alive(candidate)
                    && self.edges.records[candidate as usize].verts
                        == self.edges.records[original as usize].verts;
                if same {
                    self.splice_edges_raw(original, candidate);
                    remaining.remove(probe);
                } else {
                    probe += 1;
                }
            }
            index += 1;
        }
    }

    pub fn separate_vert_fan_regions(
        &mut self,
        vert: VertId,
        copy_select: bool,
    ) -> Result<Vec<VertId>> {
        self.check_vert(vert)?;
        let verts = self.separate_vert_fans(vert.0, copy_select);
        self.touch_topology();
        Ok(verts)
    }

    pub fn separate_loop_region(&mut self, lp: LoopId) -> Result<VertId> {
        self.check_loop(lp)?;
        let vert = self.loop_vert(lp);
        let edge = self.loop_edge(lp);
        if !self.is_edge_boundary(edge) {
            self.separate_edge_raw(edge.0, lp.0, false);
        }
        let previous = self.loop_prev(lp);
        let previous_edge = self.loop_edge(previous);
        if !self.is_edge_boundary(previous_edge) {
            self.separate_edge_raw(previous_edge.0, previous.0, false);
        }
        self.touch_topology();
        let mine = [self.loop_edge(lp), self.loop_edge(self.loop_prev(lp))];
        let start = EdgeId(self.verts.records[vert.index()].edge);
        let mut edge = start;
        while !mine.contains(&edge) {
            edge = self.disk_next(edge, vert);
            if edge == start {
                return Ok(vert);
            }
        }
        self.verts.records[vert.index()].edge = mine[0].0;
        let created = self.new_vert_like(vert.0, false);
        for edge in mine {
            self.edge_vert_swap(edge.0, created, vert.0);
        }
        Ok(VertId(created))
    }

    pub fn separate_loop_region_multi(&mut self, loops: &[LoopId]) -> Result<VertId> {
        let Some(&first) = loops.first() else {
            return Err(Error::DeadLoop(LoopId(NONE)));
        };
        for &lp in loops {
            self.check_loop(lp)?;
        }
        let vert = self.loop_vert(first);
        let mut visited_loops: IdSet<LoopId> = IdSet::new();
        let mut visited_edges: IdSet<EdgeId> = IdSet::new();
        for &lp in loops {
            visited_loops.insert(lp);
            visited_loops.insert(self.loop_next(lp));
            visited_loops.insert(self.loop_prev(lp));
            visited_edges.insert(self.loop_edge(lp));
            visited_edges.insert(self.loop_edge(self.loop_prev(lp)));
        }
        let mut edges = Vec::new();
        let mut mixed_edge = false;
        let mut mixed_loop = false;
        for edge in self.vert_edges(vert).collect::<Vec<_>>() {
            if visited_edges.contains(edge) {
                let outside = self
                    .edge_loops(edge)
                    .find(|&lp| !visited_loops.contains(lp));
                if let Some(outside) = outside {
                    self.edges.records[edge.index()].first_loop = outside.0;
                    mixed_loop = true;
                }
                edges.push(edge);
            } else {
                mixed_edge = true;
            }
        }
        let new_vert = match mixed_loop || mixed_edge {
            false => vert,
            true => {
                let created = VertId(self.new_vert_like(vert.0, false));
                for edge in edges {
                    let first = self.edges.records[edge.index()].first_loop;
                    if first != NONE && !visited_loops.contains(LoopId(first)) {
                        let [a, b] = self.edge_verts(edge);
                        let pair = match a == vert {
                            true => (created, b),
                            false => (a, created),
                        };
                        let split = self.alloc_edge(pair.0.0, pair.1.0);
                        self.copy_edge_attrs(edge.0, split);
                        let radial: Vec<LoopId> = self.edge_loops(edge).collect();
                        for lp in radial {
                            if visited_loops.contains(lp) {
                                self.radial_remove(edge.0, lp.0);
                                self.radial_append(split, lp.0);
                            }
                        }
                    } else {
                        self.disk_vert_replace(edge.0, created.0, vert.0);
                    }
                }
                created
            }
        };
        for &lp in loops {
            self.loops.records[lp.index()].vert = new_vert.0;
        }
        self.touch_topology();
        Ok(new_vert)
    }

    pub fn separate_loop_region_isolated(&mut self, lp: LoopId) -> Result<VertId> {
        self.check_loop(lp)?;
        let vert = self.loop_vert(lp);
        let created = self.new_vert_like(vert.0, false);
        let mut pending = vec![self.loop_edge(lp).0];
        while let Some(edge) = pending.pop() {
            if !self.edges.records[edge as usize].verts.contains(&vert.0) {
                continue;
            }
            self.disk_vert_replace(edge, created, vert.0);
            for radial in self.edge_loops(EdgeId(edge)).collect::<Vec<_>>() {
                let record = self.loops.records[radial.index()];
                if record.vert == vert.0 {
                    self.loops.records[radial.index()].vert = created;
                    let previous_edge = self.loops.records[record.prev as usize].edge;
                    if self.edges.records[previous_edge as usize]
                        .verts
                        .contains(&vert.0)
                    {
                        pending.push(previous_edge);
                    }
                } else if self.loops.records[record.next as usize].vert == vert.0 {
                    self.loops.records[record.next as usize].vert = created;
                    let next_edge = self.loops.records[record.next as usize].edge;
                    if self.edges.records[next_edge as usize]
                        .verts
                        .contains(&vert.0)
                    {
                        pending.push(next_edge);
                    }
                }
            }
        }
        self.touch_topology();
        Ok(VertId(created))
    }
}

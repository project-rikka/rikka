use crate::ids::{EdgeId, FaceId, VertId};
use crate::mesh::EditMesh;
use crate::sets::{IdMap, IdSet};

const PRIME_VERT_INIT: u64 = 100_003;

#[derive(Default)]
struct FaceStepItem {
    uid: u64,
    faces: Vec<FaceId>,
}

#[derive(Default)]
struct FaceStep {
    faces: Vec<FaceId>,
    items: Vec<FaceStepItem>,
}

#[derive(Default)]
struct UidWalk {
    steps: Vec<FaceStep>,
    verts: IdMap<VertId, u64>,
    faces: IdMap<FaceId, u64>,
    isolate: bool,
    pass: u64,
}

impl UidWalk {
    fn new() -> UidWalk {
        UidWalk {
            pass: 1,
            ..UidWalk::default()
        }
    }

    fn face_test(&self, region: &Region, face: FaceId) -> bool {
        !self.isolate || region.faces.contains(face)
    }

    fn vert_uid(&self, mesh: &EditMesh, vert: VertId) -> u64 {
        let mut uid = self.pass.wrapping_mul(1031);
        let mut total = 0u32;
        for edge in mesh.vert_edges(vert) {
            let other = mesh.edge_other_vert(edge, vert).unwrap_or(vert);
            if let Some(&other_uid) = self.verts.get(other) {
                uid ^= other_uid.wrapping_mul(7);
                total += 1;
            }
        }
        uid ^= u64::from(total.wrapping_mul(43));
        let mut total = 0u32;
        for face in mesh.vert_faces(vert) {
            if let Some(&other_uid) = self.faces.get(face) {
                uid ^= other_uid.wrapping_mul(13);
                total += 1;
            }
        }
        uid ^= u64::from(total.wrapping_mul(53));
        uid
    }

    fn face_uid(&self, mesh: &EditMesh, face: FaceId) -> u64 {
        let mut uid = self
            .pass
            .wrapping_mul(mesh.face_len(face) as u64)
            .wrapping_mul(1013);
        for vert in mesh.face_verts(face) {
            if let Some(&other_uid) = self.verts.get(vert) {
                uid ^= other_uid.wrapping_mul(11);
            }
        }
        for lp in mesh.face_loops(face) {
            let mut radial = mesh.loop_radial_next(lp);
            while radial != lp {
                if let Some(&other_uid) = self.faces.get(mesh.loop_face(radial)) {
                    uid ^= other_uid.wrapping_mul(17);
                }
                radial = mesh.loop_radial_next(radial);
            }
        }
        uid
    }

    fn rehash(&mut self, mesh: &EditMesh) {
        let verts: Vec<(VertId, u64)> = self
            .verts
            .keys()
            .map(|vert| (vert, self.vert_uid(mesh, vert)))
            .collect();
        for (vert, uid) in verts {
            self.verts.insert(vert, uid);
        }
        let faces: Vec<(FaceId, u64)> = self
            .faces
            .keys()
            .map(|face| (face, self.face_uid(mesh, face)))
            .collect();
        for (face, uid) in faces {
            self.faces.insert(face, uid);
        }
    }

    fn rehash_faces(&mut self, mesh: &EditMesh, faces: &[FaceId]) {
        let uids: Vec<u64> = faces
            .iter()
            .map(|&face| self.face_uid(mesh, face))
            .collect();
        for (&face, uid) in faces.iter().zip(uids) {
            self.faces.insert(face, uid);
        }
    }

    fn is_uid_connected(&self, mesh: &EditMesh, vert: VertId) -> bool {
        mesh.vert_edges(vert).any(|edge| {
            mesh.edge_other_vert(edge, vert)
                .is_some_and(|other| self.verts.contains(other))
        })
    }

    fn pass_add(&mut self, mesh: &EditMesh, region: &Region, faces: &[FaceId]) {
        self.rehash_faces(mesh, faces);
        let mut verts_pass: IdMap<VertId, u64> = IdMap::new();
        let mut faces_next: IdSet<FaceId> = IdSet::new();
        let mut step = FaceStep::default();
        for &face in faces {
            for lp in mesh.face_loops(face) {
                let vert = mesh.loop_vert(lp);
                if !self.verts.contains(vert) && !verts_pass.contains(vert) {
                    let uid = match self.is_uid_connected(mesh, vert) {
                        true => self.vert_uid(mesh, vert),
                        false => 0,
                    };
                    verts_pass.insert(vert, uid);
                }
                let mut radial = mesh.loop_radial_next(lp);
                while radial != lp {
                    let other = mesh.loop_face(radial);
                    if !self.faces.contains(other)
                        && !faces_next.contains(other)
                        && self.face_test(region, other)
                    {
                        faces_next.insert(other);
                        step.faces.push(other);
                    }
                    radial = mesh.loop_radial_next(radial);
                }
            }
        }
        self.steps.insert(0, step);
        for (vert, &uid) in verts_pass.iter() {
            self.verts.insert(vert, uid);
        }
        self.rehash_faces(mesh, faces);
        self.pass += 1;
    }

    fn init_from_edge(&mut self, mesh: &EditMesh, region: &Region, edge: EdgeId) -> usize {
        let mut faces: Vec<FaceId> = mesh
            .edge_faces(edge)
            .filter(|&face| self.face_test(region, face))
            .collect();
        faces.sort_by_key(|&face| mesh.face_len(face));
        for vert in mesh.edge_verts(edge) {
            self.verts.insert(vert, PRIME_VERT_INIT);
        }
        let mut steps = 0;
        for group in faces.chunk_by(|&a, &b| mesh.face_len(a) == mesh.face_len(b)) {
            self.pass_add(mesh, region, group);
            steps += 1;
        }
        steps
    }

    fn step_begin(&mut self, mesh: &EditMesh, index: usize) -> bool {
        let faces = std::mem::take(&mut self.steps[index].faces);
        let mut kept = Vec::with_capacity(faces.len());
        let mut items: Vec<FaceStepItem> = Vec::new();
        for face in faces {
            if self.faces.contains(face) {
                continue;
            }
            let uid = self.face_uid(mesh, face);
            match items.iter_mut().find(|item| item.uid == uid) {
                Some(item) => item.faces.push(face),
                None => items.push(FaceStepItem {
                    uid,
                    faces: vec![face],
                }),
            }
            kept.push(face);
        }
        items.sort_by_key(|item| item.uid);
        let ok = !kept.is_empty();
        let step = &mut self.steps[index];
        step.faces = kept;
        step.items = items;
        ok
    }
}

struct Region {
    faces: IdSet<FaceId>,
    edges: IdSet<EdgeId>,
    verts: IdSet<VertId>,
}

impl Region {
    fn visit(&mut self, mesh: &EditMesh, faces: &[FaceId], visit_faces: bool) -> usize {
        let mut verts = 0;
        for &face in faces {
            for lp in mesh.face_loops(face) {
                self.edges.insert(mesh.loop_edge(lp));
                if self.verts.insert(mesh.loop_vert(lp)) {
                    verts += 1;
                }
            }
            if visit_faces {
                self.faces.insert(face);
            }
        }
        verts
    }

    fn is_edge_boundary(&self, mesh: &EditMesh, edge: EdgeId) -> bool {
        let Some(first) = mesh.edge_first_loop(edge) else {
            return true;
        };
        if mesh.loop_radial_next(first) == first {
            return true;
        }
        mesh.edge_loops(edge)
            .any(|lp| !self.faces.contains(mesh.loop_face(lp)))
    }

    fn vert_boundary_id(&self, mesh: &EditMesh, vert: VertId) -> i64 {
        let mut id: i64 = 103;
        let mut total: i32 = 0;
        for lp in mesh.vert_loops(vert) {
            let boundary = self.is_edge_boundary(mesh, mesh.loop_edge(lp))
                || self.is_edge_boundary(mesh, mesh.loop_edge(mesh.loop_prev(lp)));
            let prime = if boundary { 7 } else { 13 };
            id ^= i64::from((mesh.face_len(mesh.loop_face(lp)) as i32).wrapping_mul(prime));
            total += 1;
        }
        id ^= i64::from(total.wrapping_mul(131));
        match id {
            0 => 1,
            id => id.wrapping_abs(),
        }
    }

    fn vert_pass_id(&self, mesh: &EditMesh, ids: &IdMap<VertId, i64>, vert: VertId) -> i64 {
        let mut total: i64 = 0;
        let mut face_len_sum: i64 = 0;
        let mut id_sum: i64 = 0;
        for edge in mesh.vert_edges(vert) {
            if !self.edges.contains(edge) {
                continue;
            }
            let other = mesh.edge_other_vert(edge, vert).unwrap_or(vert);
            if !self.verts.contains(other) {
                continue;
            }
            let other_id = ids.get(other).copied().unwrap_or(0);
            if other_id > 0 {
                id_sum = id_sum.wrapping_add(other_id);
                total += 1;
                for lp in mesh.edge_loops(edge) {
                    let face = mesh.loop_face(lp);
                    if self.faces.contains(face) {
                        face_len_sum += mesh.face_len(face) as i64;
                    }
                }
            }
        }
        let mut id = total.wrapping_mul(23);
        id ^= face_len_sum.wrapping_mul(31);
        id ^= id_sum;
        let id_min = i64::MIN + 1;
        let id = if id == 0 {
            1
        } else if id < id_min {
            id_min
        } else {
            id
        };
        id.wrapping_abs()
    }

    fn pivot_use_best(
        ids: &IdMap<VertId, i64>,
        mesh: &EditMesh,
        edge: EdgeId,
        best: &mut Option<EdgeId>,
        best_id: &mut [i64; 2],
    ) {
        let [a, b] = mesh.edge_verts(edge);
        let mut test = [
            ids.get(a).copied().unwrap_or(0),
            ids.get(b).copied().unwrap_or(0),
        ];
        if test[0] > test[1] {
            test.swap(0, 1);
        }
        let better = match best_id[0] != test[0] {
            true => best_id[0] < test[0],
            false => best_id[1] < test[1],
        };
        if best.is_none() || better {
            *best_id = test;
            *best = Some(edge);
        }
    }

    fn pivot_edge(
        &mut self,
        mesh: &EditMesh,
        faces: &[FaceId],
        verts_len: usize,
    ) -> Option<(EdgeId, usize)> {
        let mut ids: IdMap<VertId, i64> = IdMap::new();
        let mut queue_prev: Vec<VertId> = Vec::new();
        let mut queue_next: Vec<VertId> = Vec::new();
        let mut fallback = None;
        let mut pass: usize = 0;
        let mut used = 0usize;
        for &face in faces {
            for lp in mesh.face_loops(face) {
                let edge = mesh.loop_edge(lp);
                if self.is_edge_boundary(mesh, edge) {
                    for vert in mesh.edge_verts(edge) {
                        if !ids.contains(vert) {
                            ids.insert(vert, self.vert_boundary_id(mesh, vert));
                            queue_prev.push(vert);
                            used += 1;
                        }
                    }
                } else {
                    fallback = Some(edge);
                }
            }
        }
        while !queue_prev.is_empty() {
            while let Some(vert) = queue_prev.pop() {
                for edge in mesh.vert_edges(vert) {
                    if !self.edges.contains(edge) {
                        continue;
                    }
                    let other = mesh.edge_other_vert(edge, vert).unwrap_or(vert);
                    if self.verts.contains(other) && !ids.contains(other) {
                        let id = -self.vert_pass_id(mesh, &ids, other);
                        ids.insert(other, id);
                        queue_next.push(other);
                        used += 1;
                    }
                }
            }
            for &vert in &queue_next {
                if let Some(id) = ids.get_mut(vert) {
                    *id = -*id;
                }
            }
            std::mem::swap(&mut queue_prev, &mut queue_next);
            pass += 1;
            if used == verts_len {
                break;
            }
        }
        let mut pivot = None;
        if queue_prev.len() >= 2 {
            let mut best = None;
            let mut best_id = [0i64; 2];
            for &vert in queue_prev.iter().rev() {
                self.verts.remove(vert);
            }
            for &vert in queue_prev.iter().rev() {
                self.verts.insert(vert);
                for edge in mesh.vert_edges(vert) {
                    if !self.edges.contains(edge) {
                        continue;
                    }
                    let other = mesh.edge_other_vert(edge, vert).unwrap_or(vert);
                    if !self.verts.contains(other) {
                        Region::pivot_use_best(&ids, mesh, edge, &mut best, &mut best_id);
                    }
                }
            }
            pivot = best;
        }
        if pivot.is_none() && !queue_prev.is_empty() {
            let mut best = None;
            let mut best_id = [0i64; 2];
            pass = pass.saturating_sub(1);
            for &vert in queue_prev.iter().rev() {
                for edge in mesh.vert_edges(vert) {
                    if !self.edges.contains(edge) {
                        continue;
                    }
                    let other = mesh.edge_other_vert(edge, vert).unwrap_or(vert);
                    if self.verts.contains(other) {
                        Region::pivot_use_best(&ids, mesh, edge, &mut best, &mut best_id);
                    }
                }
            }
            pivot = best;
        }
        match pivot {
            Some(edge) => Some((edge, pass)),
            None => fallback.map(|edge| (edge, 0)),
        }
    }
}

fn fasthash_single(mesh: &EditMesh, vert: VertId) -> u64 {
    let (mut edges, mut faces, mut loops) = (0u64, 0u64, 0u64);
    for edge in mesh.vert_edges(vert) {
        if mesh.is_edge_wire(edge) {
            continue;
        }
        edges += 1;
        for lp in mesh.edge_loops(edge) {
            faces += 1;
            loops += mesh.face_len(mesh.loop_face(lp)) as u64;
        }
    }
    edges.wrapping_mul(7) ^ faces.wrapping_mul(31).wrapping_mul(loops.wrapping_mul(61))
}

fn fasthash(mesh: &EditMesh, depth: usize) -> Vec<u64> {
    let mut previous = vec![0u64; mesh.vert_capacity()];
    for vert in mesh.verts() {
        previous[vert.index()] = fasthash_single(mesh, vert);
    }
    let mut current = previous.clone();
    for _ in 0..depth {
        current.copy_from_slice(&previous);
        for edge in mesh.edges() {
            if mesh.is_edge_wire(edge) {
                continue;
            }
            let [a, b] = mesh.edge_verts(edge);
            current[a.index()] = current[a.index()].wrapping_add(previous[b.index()]);
            current[b.index()] = current[b.index()].wrapping_add(previous[a.index()]);
        }
    }
    current
}

fn fasthash_edge(hashes: &[u64], mesh: &EditMesh, edge: EdgeId) -> [u64; 2] {
    let [a, b] = mesh.edge_verts(edge);
    let mut pair = [hashes[a.index()], hashes[b.index()]];
    if pair[0] > pair[1] {
        pair.swap(0, 1);
    }
    pair
}

fn match_pair(
    mesh: &EditMesh,
    region: &Region,
    source_edge: EdgeId,
    target_edge: EdgeId,
    faces_len: usize,
) -> Option<Vec<FaceId>> {
    let mut source = UidWalk::new();
    let mut target = UidWalk::new();
    source.isolate = true;
    if source.init_from_edge(mesh, region, source_edge)
        != target.init_from_edge(mesh, region, target_edge)
    {
        return None;
    }
    loop {
        let mut ok = false;
        let mut index = 0;
        while index < source.steps.len() {
            if source.steps[index].faces.is_empty() {
                source.steps.remove(index);
                target.steps.remove(index);
                continue;
            }
            let mut added = 0;
            if source.step_begin(mesh, index) && target.step_begin(mesh, index) {
                let source_items = std::mem::take(&mut source.steps[index].items);
                let target_items = std::mem::take(&mut target.steps[index].items);
                let mut target_cursor = 0;
                for source_item in &source_items {
                    if target_cursor >= target_items.len() {
                        break;
                    }
                    while target_cursor < target_items.len()
                        && target_items[target_cursor].uid < source_item.uid
                    {
                        target_cursor += 1;
                    }
                    let Some(target_item) = target_items.get(target_cursor) else {
                        ok = false;
                        break;
                    };
                    if source_item.uid != target_item.uid
                        || source_item.faces.len() > target_item.faces.len()
                    {
                        ok = false;
                        break;
                    }
                    if source_item.faces.len() == target_item.faces.len() {
                        source.pass_add(mesh, region, &source_item.faces);
                        target.pass_add(mesh, region, &target_item.faces);
                        added += 1;
                        ok = true;
                    }
                    target_cursor += 1;
                }
            }
            index += added;
            source.steps[index].items.clear();
            target.steps[index].items.clear();
            index += 1;
        }
        if !ok {
            return None;
        }
        if target.faces.len() == faces_len {
            return Some(target.faces.keys().collect());
        }
        source.rehash(mesh);
        target.rehash(mesh);
    }
}

impl EditMesh {
    #[must_use]
    pub fn region_match(&self, faces: &[FaceId]) -> Vec<Vec<FaceId>> {
        let mut region = Region {
            faces: IdSet::with_capacity(self.face_capacity()),
            edges: IdSet::with_capacity(self.edge_capacity()),
            verts: IdSet::with_capacity(self.vert_capacity()),
        };
        let verts_len = region.visit(self, faces, true);
        let Some((source_edge, depth)) = region.pivot_edge(self, faces, verts_len) else {
            return Vec::new();
        };
        let hashes = (depth > 0).then(|| fasthash(self, depth));
        let mut regions = Vec::new();
        let edges: Vec<EdgeId> = self.edges().collect();
        for target_edge in edges {
            if region.edges.contains(target_edge) || self.is_edge_wire(target_edge) {
                continue;
            }
            if let Some(hashes) = &hashes
                && fasthash_edge(hashes, self, source_edge)
                    != fasthash_edge(hashes, self, target_edge)
            {
                continue;
            }
            if let Some(found) = match_pair(self, &region, source_edge, target_edge, faces.len()) {
                region.visit(self, &found, false);
                regions.push(found);
            }
        }
        regions
    }
}

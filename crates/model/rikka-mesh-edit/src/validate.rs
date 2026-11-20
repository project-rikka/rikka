use rikka_polymesh::Domain;

use crate::error::{Error, Result};
use crate::ids::{Element, NONE};
use crate::mesh::EditMesh;

fn corrupt(message: String) -> Error {
    Error::Corrupt(message)
}

impl EditMesh {
    pub fn validate(&self) -> Result<()> {
        self.validate_storage()?;
        self.validate_verts()?;
        self.validate_edges()?;
        self.validate_faces()?;
        self.validate_selection()
    }

    fn validate_storage(&self) -> Result<()> {
        let sizes = [
            ("positions", self.positions.len(), self.verts.capacity()),
            (
                "vertex normals",
                self.vert_normals.len(),
                self.verts.capacity(),
            ),
            (
                "face normals",
                self.face_normals.len(),
                self.faces.capacity(),
            ),
            (
                "key indices",
                self.vert_data.key_index.len(),
                self.verts.capacity(),
            ),
        ];
        for (name, len, expected) in sizes {
            if len != expected {
                return Err(corrupt(format!(
                    "{name} hold {len} rows for {expected} slots"
                )));
            }
        }
        for domain in Domain::ALL {
            let expected = self.capacity(domain);
            let table = self.table(domain);
            for attribute in table.attributes.iter() {
                if attribute.values.len() != expected || attribute.domain != domain {
                    return Err(corrupt(format!(
                        "attribute {} holds {} rows for {expected} {} slots",
                        attribute.name,
                        attribute.values.len(),
                        domain.name()
                    )));
                }
            }
        }
        if let Some(weights) = &self.vert_data.weights
            && weights.len() != self.verts.capacity()
        {
            return Err(corrupt(
                "vertex weights do not match the vertex slots".into(),
            ));
        }
        if self.vert_data.keys.len() != self.layout.key_blocks.len() {
            return Err(corrupt(
                "shape key layers do not match the key blocks".into(),
            ));
        }
        for key in &self.vert_data.keys {
            if key.len() != self.verts.capacity() {
                return Err(corrupt(
                    "shape key layer does not match the vertex slots".into(),
                ));
            }
        }
        Ok(())
    }

    fn validate_verts(&self) -> Result<()> {
        let mut disk_counts = vec![0usize; self.verts.capacity()];
        for edge in self.edges.indices() {
            for vert in self.edges.records[edge as usize].verts {
                if !self.verts.is_alive(vert) {
                    return Err(corrupt(format!("edge {edge} uses dead vertex {vert}")));
                }
                disk_counts[vert as usize] += 1;
            }
        }
        for vert in self.verts.indices() {
            let first = self.verts.records[vert as usize].edge;
            if first == NONE {
                if disk_counts[vert as usize] != 0 {
                    return Err(corrupt(format!("vertex {vert} has edges but no disk")));
                }
                continue;
            }
            let mut edge = first;
            let mut count = 0;
            loop {
                if !self.edges.is_alive(edge) {
                    return Err(corrupt(format!(
                        "disk of vertex {vert} holds dead edge {edge}"
                    )));
                }
                let record = self.edges.records[edge as usize];
                if !record.verts.contains(&vert) {
                    return Err(corrupt(format!(
                        "disk of vertex {vert} holds foreign edge {edge}"
                    )));
                }
                let side = usize::from(record.verts[0] != vert);
                let next = record.disk[side].next;
                if next == NONE || !self.edges.is_alive(next) {
                    return Err(corrupt(format!("disk link of edge {edge} is broken")));
                }
                let next_record = self.edges.records[next as usize];
                if !next_record.verts.contains(&vert) {
                    return Err(corrupt(format!("disk of vertex {vert} leaves the vertex")));
                }
                let next_side = usize::from(next_record.verts[0] != vert);
                if next_record.disk[next_side].prev != edge {
                    return Err(corrupt(format!(
                        "disk of vertex {vert} is not doubly linked"
                    )));
                }
                count += 1;
                if count > disk_counts[vert as usize] {
                    return Err(corrupt(format!("disk of vertex {vert} does not close")));
                }
                edge = next;
                if edge == first {
                    break;
                }
            }
            if count != disk_counts[vert as usize] {
                return Err(corrupt(format!(
                    "disk of vertex {vert} holds {count} of its {} edges",
                    disk_counts[vert as usize]
                )));
            }
        }
        Ok(())
    }

    fn validate_edges(&self) -> Result<()> {
        let mut radial_counts = vec![0usize; self.edges.capacity()];
        for lp in self.loops.indices() {
            let edge = self.loops.records[lp as usize].edge;
            if edge == NONE || !self.edges.is_alive(edge) {
                return Err(corrupt(format!("loop {lp} uses a dead edge")));
            }
            radial_counts[edge as usize] += 1;
        }
        for edge in self.edges.indices() {
            let record = self.edges.records[edge as usize];
            if record.verts[0] == record.verts[1] {
                return Err(corrupt(format!("edge {edge} joins a vertex with itself")));
            }
            let first = record.first_loop;
            if first == NONE {
                if radial_counts[edge as usize] != 0 {
                    return Err(corrupt(format!(
                        "edge {edge} has loops but no radial cycle"
                    )));
                }
                continue;
            }
            let mut lp = first;
            let mut count = 0;
            loop {
                if !self.loops.is_alive(lp) {
                    return Err(corrupt(format!(
                        "radial cycle of edge {edge} holds dead loop {lp}"
                    )));
                }
                let corner = self.loops.records[lp as usize];
                if corner.edge != edge {
                    return Err(corrupt(format!(
                        "radial cycle of edge {edge} holds foreign loop {lp}"
                    )));
                }
                let next_vert = self.loops.records[corner.next as usize].vert;
                let pair = [corner.vert, next_vert];
                if !(pair == record.verts || pair == [record.verts[1], record.verts[0]]) {
                    return Err(corrupt(format!("loop {lp} does not run along edge {edge}")));
                }
                let next = corner.radial_next;
                if next == NONE
                    || !self.loops.is_alive(next)
                    || self.loops.records[next as usize].radial_prev != lp
                {
                    return Err(corrupt(format!("radial cycle of edge {edge} is broken")));
                }
                count += 1;
                if count > radial_counts[edge as usize] {
                    return Err(corrupt(format!(
                        "radial cycle of edge {edge} does not close"
                    )));
                }
                lp = next;
                if lp == first {
                    break;
                }
            }
            if count != radial_counts[edge as usize] {
                return Err(corrupt(format!(
                    "radial cycle of edge {edge} holds {count} of its {} loops",
                    radial_counts[edge as usize]
                )));
            }
        }
        Ok(())
    }

    fn validate_faces(&self) -> Result<()> {
        let mut owned = 0usize;
        for face in self.faces.indices() {
            let record = self.faces.records[face as usize];
            if record.len < 3 {
                return Err(corrupt(format!("face {face} has {} corners", record.len)));
            }
            let first = record.first;
            let mut lp = first;
            let mut count = 0u32;
            loop {
                if lp == NONE || !self.loops.is_alive(lp) {
                    return Err(corrupt(format!("face {face} holds a dead loop")));
                }
                let corner = self.loops.records[lp as usize];
                if corner.face != face {
                    return Err(corrupt(format!(
                        "loop {lp} of face {face} names face {}",
                        corner.face
                    )));
                }
                if !self.verts.is_alive(corner.vert) {
                    return Err(corrupt(format!("loop {lp} uses a dead vertex")));
                }
                if !self.loops.is_alive(corner.next)
                    || self.loops.records[corner.next as usize].prev != lp
                {
                    return Err(corrupt(format!("loop cycle of face {face} is broken")));
                }
                count += 1;
                if count > record.len {
                    return Err(corrupt(format!(
                        "loop cycle of face {face} exceeds its length"
                    )));
                }
                lp = corner.next;
                if lp == first {
                    break;
                }
            }
            if count != record.len {
                return Err(corrupt(format!(
                    "face {face} holds {count} loops but records {}",
                    record.len
                )));
            }
            owned += count as usize;
        }
        if owned != self.loops.live() {
            return Err(corrupt(format!(
                "faces own {owned} loops of {} live loops",
                self.loops.live()
            )));
        }
        Ok(())
    }

    fn validate_selection(&self) -> Result<()> {
        for &element in &self.history {
            let alive = match element {
                Element::Vert(vert) => self.contains_vert(vert),
                Element::Edge(edge) => self.contains_edge(edge),
                Element::Face(face) => self.contains_face(face),
            };
            if !alive {
                return Err(corrupt(format!("selection history holds dead {element:?}")));
            }
        }
        if let Some(face) = self.active_face
            && !self.faces.is_alive(face)
        {
            return Err(corrupt(format!("active face {face} is dead")));
        }
        Ok(())
    }
}

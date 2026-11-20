use crate::domain::Domain;
use crate::error::{Error, Result};
use crate::names::UV_PIN_PREFIX;
use crate::polymesh::Polymesh;
use crate::values::{DataType, Values};
use crate::well_known::WELL_KNOWN;

#[must_use]
pub fn ordered([a, b]: [u32; 2]) -> [u32; 2] {
    match a <= b {
        true => [a, b],
        false => [b, a],
    }
}

fn invalid<T>(reason: String) -> Result<T> {
    Err(Error::Invalid(reason))
}

impl Polymesh {
    pub fn validate(&self) -> Result<()> {
        let points = self.point_count() as u64;
        let corners = self.corner_count();
        if self.face_offsets.first() != Some(&0) {
            return invalid("face offsets do not start at 0".to_owned());
        }
        if let Some(face) = self
            .face_offsets
            .windows(2)
            .position(|pair| u64::from(pair[1]) < u64::from(pair[0]) + 3)
        {
            return invalid(format!("face {face} has fewer than 3 corners"));
        }
        if self.face_offsets.last().map(|&last| last as usize) != Some(corners) {
            return invalid(format!("face offsets do not end at {corners} corners"));
        }
        if self.corner_edges.len() != corners {
            return invalid(format!(
                "{} corner edges for {corners} corners",
                self.corner_edges.len()
            ));
        }
        if let Some(corner) = self
            .corner_verts
            .iter()
            .position(|&point| u64::from(point) >= points)
        {
            return invalid(format!("corner {corner} names a missing point"));
        }
        if let Some(edge) = self
            .edges
            .iter()
            .position(|edge| edge.iter().any(|&point| u64::from(point) >= points))
        {
            return invalid(format!("edge {edge} names a missing point"));
        }
        if let Some(corner) = self.inconsistent_corner_edge() {
            return invalid(format!(
                "corner {corner} names an edge that does not join its points"
            ));
        }
        self.validate_data()
    }

    fn validate_data(&self) -> Result<()> {
        let mut names: Vec<&str> = self
            .attributes
            .iter()
            .map(|attribute| attribute.name.as_str())
            .collect();
        names.sort_unstable();
        if let Some(pair) = names.windows(2).find(|pair| pair[0] == pair[1]) {
            return invalid(format!("attribute \"{}\" repeats", pair[0]));
        }
        for attribute in self.attributes.iter() {
            let expected = self.domain_len(attribute.domain);
            if attribute.values.len() != expected {
                return invalid(format!(
                    "attribute \"{}\" holds {} values for {expected} {} elements",
                    attribute.name,
                    attribute.values.len(),
                    attribute.domain.name()
                ));
            }
            if let Some((_, domain, data_type)) = WELL_KNOWN
                .iter()
                .find(|(name, _, _)| *name == attribute.name)
                && (attribute.domain != *domain || attribute.data_type() != *data_type)
            {
                return invalid(format!(
                    "attribute \"{}\" must be {} on the {} domain",
                    attribute.name,
                    data_type.name(),
                    domain.name()
                ));
            }
            if attribute.name.starts_with(UV_PIN_PREFIX)
                && (attribute.domain != Domain::Corner || attribute.data_type() != DataType::Bool)
            {
                return invalid(format!(
                    "UV pin attribute \"{}\" must be bool on the corner domain",
                    attribute.name
                ));
            }
        }
        for name in &self.uv_maps.names {
            if self.uv_map(name).is_none() {
                return invalid(format!("UV map \"{name}\" is not a corner vec2 attribute"));
            }
        }
        for name in &self.color_attributes.names {
            if self.color_attribute(name).is_none() {
                return invalid(format!(
                    "color attribute \"{name}\" is not a point or corner color attribute"
                ));
            }
        }
        for (what, layers) in [
            ("UV map", &self.uv_maps),
            ("color attribute", &self.color_attributes),
        ] {
            if [layers.active, layers.render]
                .into_iter()
                .flatten()
                .any(|index| index >= layers.len())
            {
                return invalid(format!("active or render {what} is out of range"));
            }
        }
        self.validate_deform()
    }

    fn validate_deform(&self) -> Result<()> {
        let points = self.point_count();
        let groups = &self.vertex_groups;
        if groups.has_weights() && groups.weights.len() != points {
            return invalid(format!(
                "{} vertex weight lists for {points} points",
                groups.weights.len()
            ));
        }
        if groups
            .weights
            .iter()
            .flatten()
            .any(|influence| influence.group as usize >= groups.groups.len())
        {
            return invalid("a vertex weight names a missing vertex group".to_owned());
        }
        if groups
            .active
            .is_some_and(|active| active >= groups.groups.len())
        {
            return invalid("active vertex group is out of range".to_owned());
        }
        let keys = &self.shape_keys;
        for key in &keys.keys {
            if key.positions.len() != points {
                return invalid(format!(
                    "shape key \"{}\" holds {} positions for {points} points",
                    key.name,
                    key.positions.len()
                ));
            }
            if key.relative_to as usize >= keys.len() {
                return invalid(format!(
                    "shape key \"{}\" is relative to a missing key",
                    key.name
                ));
            }
        }
        if keys.active.is_some_and(|active| active >= keys.len()) {
            return invalid("active shape key is out of range".to_owned());
        }
        Ok(())
    }

    pub fn sanitize(&mut self) -> bool {
        let mut repaired = self.sanitize_faces();
        repaired |= self.sanitize_edges();
        self.ensure_edges();
        repaired |= self.sanitize_data();
        repaired |= self.sanitize_deform();
        repaired
    }

    fn sanitize_faces(&mut self) -> bool {
        let points = self.point_count() as u64;
        let corners = self.corner_count();
        if self.face_offsets.is_empty() {
            self.face_offsets.push(0);
        }
        let sound = |start: u32, end: u32| {
            let (start, end) = (start as usize, end as usize);
            start + 3 <= end
                && end <= corners
                && self.corner_verts[start..end]
                    .iter()
                    .all(|&point| u64::from(point) < points)
        };
        let all_sound = self.face_offsets[0] == 0
            && self.face_offsets.last().map(|&last| last as usize) == Some(corners)
            && self
                .face_offsets
                .windows(2)
                .all(|pair| sound(pair[0], pair[1]));
        if all_sound {
            return false;
        }
        let kept: Vec<(u32, Vec<u32>)> = self
            .face_offsets
            .windows(2)
            .enumerate()
            .filter(|(_, pair)| sound(pair[0], pair[1]))
            .map(|(face, pair)| (face as u32, (pair[0]..pair[1]).collect()))
            .collect();
        let faces = self.face_count();
        for attribute in self.attributes.iter_mut() {
            let len = match attribute.domain {
                Domain::Face => faces,
                Domain::Corner => corners,
                _ => continue,
            };
            if attribute.values.len() != len {
                attribute.values.resize(len);
            }
        }
        if self.corner_edges.len() != corners {
            self.corner_edges.resize(corners, 0);
        }
        self.rebuild_faces(
            kept.iter()
                .map(|(face, corners)| (*face, corners.as_slice())),
        );
        true
    }

    fn sanitize_edges(&mut self) -> bool {
        let points = self.point_count() as u64;
        if self
            .edges
            .iter()
            .flatten()
            .all(|&point| u64::from(point) < points)
        {
            return false;
        }
        let kept: Vec<u32> = (0..self.edge_count() as u32)
            .filter(|&edge| {
                self.edges[edge as usize]
                    .iter()
                    .all(|&point| u64::from(point) < points)
            })
            .collect();
        let edges = self.edge_count();
        for attribute in self.attributes.iter_mut() {
            if attribute.domain == Domain::Edge && attribute.values.len() != edges {
                attribute.values.resize(edges);
            }
        }
        self.gather(Domain::Edge, &kept);
        self.corner_edges.clear();
        true
    }

    fn sanitize_data(&mut self) -> bool {
        let mut repaired = false;
        let mut seen: Vec<String> = Vec::new();
        self.attributes.retain(|attribute| {
            let fresh = !seen.contains(&attribute.name);
            if fresh {
                seen.push(attribute.name.clone());
            }
            repaired |= !fresh;
            fresh
        });
        let sizes = self.sizes();
        for attribute in self.attributes.iter_mut() {
            let expected = sizes.get(attribute.domain);
            if attribute.values.len() != expected {
                attribute.values.resize(expected);
                repaired = true;
            }
            let required = WELL_KNOWN
                .iter()
                .find(|(name, _, _)| *name == attribute.name)
                .map(|(_, domain, data_type)| (*domain, *data_type))
                .or_else(|| {
                    attribute
                        .name
                        .starts_with(UV_PIN_PREFIX)
                        .then_some((Domain::Corner, DataType::Bool))
                });
            if let Some((domain, data_type)) = required {
                if attribute.domain != domain {
                    attribute.domain = domain;
                    attribute.values = Values::new(data_type, sizes.get(domain));
                    repaired = true;
                } else if attribute.data_type() != data_type {
                    attribute.values = attribute.values.convert(data_type);
                    repaired = true;
                }
            }
        }
        let attributes = &self.attributes;
        let uv_sound = |name: &str| {
            attributes
                .values::<glam::Vec2>(name, Domain::Corner)
                .is_some()
        };
        let before = self.uv_maps.len() + self.color_attributes.len();
        self.uv_maps.retain(uv_sound);
        self.color_attributes.retain(|name| {
            attributes.get(name).is_some_and(|attribute| {
                matches!(attribute.domain, Domain::Point | Domain::Corner)
                    && matches!(attribute.values, Values::Color(_))
            })
        });
        repaired |= before != self.uv_maps.len() + self.color_attributes.len();
        repaired |= self.uv_maps.clamp();
        repaired |= self.color_attributes.clamp();
        repaired
    }

    fn sanitize_deform(&mut self) -> bool {
        let points = self.point_count();
        let mut repaired = false;
        let groups = &mut self.vertex_groups;
        if groups.has_weights() && groups.weights.len() != points {
            groups.weights.resize(points, Vec::new());
            repaired = true;
        }
        let group_count = groups.groups.len();
        for influences in &mut groups.weights {
            let before = influences.len();
            influences.retain(|influence| (influence.group as usize) < group_count);
            repaired |= before != influences.len();
        }
        if groups.active.is_some_and(|active| active >= group_count) {
            groups.active = (group_count > 0).then_some(0);
            repaired = true;
        }
        let keys = &mut self.shape_keys;
        let key_count = keys.keys.len();
        let positions = &self.positions;
        for key in &mut keys.keys {
            if key.positions.len() != points {
                let kept = key.positions.len().min(points);
                key.positions.truncate(kept);
                key.positions.extend_from_slice(&positions[kept..]);
                repaired = true;
            }
            if key.relative_to as usize >= key_count {
                key.relative_to = 0;
                repaired = true;
            }
        }
        if keys.active.is_some_and(|active| active >= key_count) {
            keys.active = (key_count > 0).then_some(0);
            repaired = true;
        }
        repaired
    }

    pub fn ensure_edges(&mut self) {
        let corners = self.corner_count();
        if self.corner_edges.len() != corners {
            self.corner_edges.resize(corners, u32::MAX);
        }
        let Some(first) = self.inconsistent_corner_edge() else {
            return;
        };
        let added_from = self.edge_count();
        self.resolve_corner_edges(first);
        let edges = self.edge_count();
        if edges != added_from {
            self.attributes.resize(Domain::Edge, edges);
        }
    }

    fn inconsistent_corner_edge(&self) -> Option<usize> {
        if self.corner_edges.len() != self.corner_count() {
            return Some(0);
        }
        for face in 0..self.face_count() {
            let range = self.face(face);
            for corner in range.clone() {
                let next = match corner + 1 == range.end {
                    true => range.start,
                    false => corner + 1,
                };
                let key = ordered([self.corner_verts[corner], self.corner_verts[next]]);
                let joined = self
                    .edges
                    .get(self.corner_edges[corner] as usize)
                    .is_some_and(|edge| ordered(*edge) == key);
                if !joined {
                    return Some(corner);
                }
            }
        }
        None
    }
}

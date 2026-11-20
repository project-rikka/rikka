use glam::Vec3;
use rikka_polymesh::{
    Attribute, Attributes, Domain, HIDE_EDGE, HIDE_POLY, HIDE_VERT, MATERIAL_INDEX, Polymesh,
    SELECT_EDGE, SELECT_POLY, SELECT_VERT, SHARP_EDGE, SHARP_FACE, ShapeKey, ShapeKeys, UV_SEAM,
    Values, VertexGroups,
};

use crate::arena::Arena;
use crate::data::{KEY_INDEX_NONE, each_values};
use crate::error::Result;
use crate::ids::{FaceId, NONE};
use crate::mesh::{EditMesh, Flags, KeyBlock};

const FLAG_LAYERS: &[&str] = &[
    SELECT_VERT,
    HIDE_VERT,
    SELECT_EDGE,
    HIDE_EDGE,
    UV_SEAM,
    SHARP_EDGE,
    SELECT_POLY,
    HIDE_POLY,
    SHARP_FACE,
    MATERIAL_INDEX,
];

fn bools<'a>(mesh: &'a Polymesh, name: &str, domain: Domain) -> Option<&'a [bool]> {
    mesh.attributes.values(name, domain)
}

fn flag_at(values: Option<&[bool]>, index: usize) -> bool {
    values
        .and_then(|values| values.get(index).copied())
        .unwrap_or(false)
}

impl EditMesh {
    pub fn from_polymesh(source: &Polymesh) -> Result<EditMesh> {
        source.validate()?;
        let mut mesh = EditMesh {
            verts: Arena::with_capacity(source.point_count()),
            edges: Arena::with_capacity(source.edge_count()),
            loops: Arena::with_capacity(source.corner_count()),
            faces: Arena::with_capacity(source.face_count()),
            ..EditMesh::default()
        };
        mesh.positions.reserve(source.point_count());
        mesh.vert_normals.reserve(source.point_count());
        mesh.face_normals.reserve(source.face_count());

        let keys = &source.shape_keys;
        let active_key = keys
            .active
            .filter(|&active| active < keys.len())
            .filter(|&active| keys.keys[active].positions.len() == source.point_count());
        let shown = match active_key {
            Some(active) => &keys.keys[active].positions,
            None => &source.positions,
        };
        let select = bools(source, SELECT_VERT, Domain::Point);
        let hide = bools(source, HIDE_VERT, Domain::Point);
        for (index, &position) in shown.iter().enumerate() {
            let vert = mesh.alloc_vert(position);
            let flags = &mut mesh.verts.records[vert as usize].flags;
            flags.set(Flags::SELECT, flag_at(select, index));
            flags.set(Flags::HIDDEN, flag_at(hide, index));
        }

        let select = bools(source, SELECT_EDGE, Domain::Edge);
        let hide = bools(source, HIDE_EDGE, Domain::Edge);
        let seam = bools(source, UV_SEAM, Domain::Edge);
        let sharp = bools(source, SHARP_EDGE, Domain::Edge);
        for (index, &[a, b]) in source.edges.iter().enumerate() {
            let edge = mesh.alloc_edge(a, b);
            let flags = &mut mesh.edges.records[edge as usize].flags;
            flags.set(Flags::SELECT, flag_at(select, index));
            flags.set(Flags::HIDDEN, flag_at(hide, index));
            flags.set(Flags::SEAM, flag_at(seam, index));
            flags.set(Flags::SHARP, flag_at(sharp, index));
        }

        let select = bools(source, SELECT_POLY, Domain::Face);
        let hide = bools(source, HIDE_POLY, Domain::Face);
        let flat = bools(source, SHARP_FACE, Domain::Face);
        let materials = source.material_indices();
        for face_index in 0..source.face_count() {
            let range = source.face(face_index);
            let face = mesh.alloc_face();
            mesh.link_face_loops(
                face,
                &source.corner_verts[range.clone()],
                &source.corner_edges[range],
            );
            let record = &mut mesh.faces.records[face as usize];
            record.flags.set(Flags::SELECT, flag_at(select, face_index));
            record.flags.set(Flags::HIDDEN, flag_at(hide, face_index));
            record.flags.set(Flags::SMOOTH, !flag_at(flat, face_index));
            record.material = materials.map_or(0, |materials| materials[face_index]);
        }

        for attribute in source.attributes.iter() {
            if FLAG_LAYERS.contains(&attribute.name.as_str()) {
                continue;
            }
            mesh.table_mut(attribute.domain)
                .attributes
                .insert(attribute.clone());
        }
        mesh.layout.attribute_order = source
            .attributes
            .iter()
            .map(|attribute| attribute.name.clone())
            .collect();
        mesh.layout.uv_maps = source.uv_maps.clone();
        mesh.layout.color_attributes = source.color_attributes.clone();
        mesh.layout.vertex_groups = source.vertex_groups.groups.clone();
        mesh.layout.active_vertex_group = source.vertex_groups.active;
        if source.vertex_groups.has_weights() {
            mesh.vert_data.weights = Some(source.vertex_groups.weights.clone());
        }
        mesh.layout.key_blocks = keys
            .keys
            .iter()
            .map(|key| KeyBlock {
                name: key.name.clone(),
                value: key.value,
                min: key.min,
                max: key.max,
                relative_to: key.relative_to,
                vertex_group: key.vertex_group.clone(),
                muted: key.muted,
            })
            .collect();
        mesh.layout.active_key_block = keys.active.filter(|&active| active < keys.len());
        mesh.vert_data.keys = keys.keys.iter().map(|key| key.positions.clone()).collect();
        mesh.vert_data.key_index = (0..source.point_count() as i32).collect();
        mesh.update_normals();
        Ok(mesh)
    }

    #[must_use]
    pub fn to_polymesh(&self) -> Polymesh {
        let verts: Vec<u32> = self.verts.indices().collect();
        let edges: Vec<u32> = self.edges.indices().collect();
        let faces: Vec<u32> = self.faces.indices().collect();
        let mut vert_map = vec![NONE; self.verts.capacity()];
        for (new, &old) in verts.iter().enumerate() {
            vert_map[old as usize] = new as u32;
        }
        let mut edge_map = vec![NONE; self.edges.capacity()];
        for (new, &old) in edges.iter().enumerate() {
            edge_map[old as usize] = new as u32;
        }
        let mut corners: Vec<u32> = Vec::with_capacity(self.loops.live());
        let mut face_offsets = Vec::with_capacity(faces.len() + 1);
        face_offsets.push(0u32);
        for &face in &faces {
            corners.extend(self.face_loops(FaceId(face)).map(|lp| lp.0));
            face_offsets.push(corners.len() as u32);
        }

        let mut mesh = Polymesh {
            positions: verts
                .iter()
                .map(|&vert| self.positions[vert as usize])
                .collect(),
            edges: edges
                .iter()
                .map(|&edge| {
                    self.edges.records[edge as usize]
                        .verts
                        .map(|vert| vert_map[vert as usize])
                })
                .collect(),
            face_offsets,
            corner_verts: corners
                .iter()
                .map(|&lp| vert_map[self.loops.records[lp as usize].vert as usize])
                .collect(),
            corner_edges: corners
                .iter()
                .map(|&lp| edge_map[self.loops.records[lp as usize].edge as usize])
                .collect(),
            attributes: Attributes::new(),
            uv_maps: self.layout.uv_maps.clone(),
            color_attributes: self.layout.color_attributes.clone(),
            vertex_groups: VertexGroups {
                groups: self.layout.vertex_groups.clone(),
                weights: Vec::new(),
                active: self.layout.active_vertex_group,
            },
            shape_keys: ShapeKeys::default(),
        };

        let mut attributes: Vec<Attribute> = Vec::new();
        for (domain, order) in [
            (Domain::Point, &verts),
            (Domain::Edge, &edges),
            (Domain::Face, &faces),
            (Domain::Corner, &corners),
        ] {
            for attribute in self.table(domain).attributes.iter() {
                let values = each_values!(&attribute.values, list => {
                    gathered(list, order)
                });
                attributes.push(Attribute::new(attribute.name.clone(), domain, values));
            }
        }
        let vert_flag = |flag: Flags| -> Vec<bool> {
            verts
                .iter()
                .map(|&vert| self.verts.records[vert as usize].flags.contains(flag))
                .collect()
        };
        let edge_flag = |flag: Flags| -> Vec<bool> {
            edges
                .iter()
                .map(|&edge| self.edges.records[edge as usize].flags.contains(flag))
                .collect()
        };
        let face_flag = |flag: Flags, expected: bool| -> Vec<bool> {
            faces
                .iter()
                .map(|&face| self.faces.records[face as usize].flags.contains(flag) == expected)
                .collect()
        };
        let mut push_bools = |name: &str, domain: Domain, values: Vec<bool>| {
            if values.contains(&true) {
                attributes.push(Attribute::new(name, domain, Values::Bool(values)));
            }
        };
        push_bools(SELECT_VERT, Domain::Point, vert_flag(Flags::SELECT));
        push_bools(HIDE_VERT, Domain::Point, vert_flag(Flags::HIDDEN));
        push_bools(SELECT_EDGE, Domain::Edge, edge_flag(Flags::SELECT));
        push_bools(HIDE_EDGE, Domain::Edge, edge_flag(Flags::HIDDEN));
        push_bools(UV_SEAM, Domain::Edge, edge_flag(Flags::SEAM));
        push_bools(SHARP_EDGE, Domain::Edge, edge_flag(Flags::SHARP));
        push_bools(SELECT_POLY, Domain::Face, face_flag(Flags::SELECT, true));
        push_bools(HIDE_POLY, Domain::Face, face_flag(Flags::HIDDEN, true));
        push_bools(SHARP_FACE, Domain::Face, face_flag(Flags::SMOOTH, false));
        let materials: Vec<i32> = faces
            .iter()
            .map(|&face| self.faces.records[face as usize].material)
            .collect();
        if materials.iter().any(|&material| material != 0) {
            attributes.push(Attribute::new(
                MATERIAL_INDEX,
                Domain::Face,
                Values::Int(materials),
            ));
        }
        let rank = |name: &str| {
            self.layout
                .attribute_order
                .iter()
                .position(|known| known == name)
                .unwrap_or(usize::MAX)
        };
        attributes.sort_by_key(|attribute| rank(&attribute.name));
        for attribute in attributes {
            mesh.attributes.insert(attribute);
        }

        if let Some(weights) = &self.vert_data.weights {
            mesh.vertex_groups.weights = verts
                .iter()
                .map(|&vert| weights[vert as usize].clone())
                .collect();
        }
        self.write_shape_keys(&verts, &mut mesh);
        mesh
    }

    fn write_shape_keys(&self, verts: &[u32], mesh: &mut Polymesh) {
        let blocks = &self.layout.key_blocks;
        if blocks.is_empty() {
            return;
        }
        let active = self.layout.active_key_block;
        let layers = &self.vert_data.keys;
        let original = |vert: u32| self.vert_data.key_index[vert as usize] != KEY_INDEX_NONE;
        let dependents = active.and_then(|active| dependent_keys(blocks, active));
        let offsets: Option<Vec<Vec3>> = match (active, &dependents) {
            (Some(active), Some(_)) if verts.iter().all(|&vert| original(vert)) => Some(
                verts
                    .iter()
                    .map(|&vert| self.positions[vert as usize] - layers[active][vert as usize])
                    .collect(),
            ),
            _ => None,
        };
        let from_reference = active.is_some_and(|active| active != 0);
        let mut written: Vec<Vec<Vec3>> = Vec::with_capacity(blocks.len());
        for (key, block) in blocks.iter().enumerate() {
            let apply_offset = offsets.is_some()
                && Some(key) != active
                && dependents
                    .as_ref()
                    .is_some_and(|dependents| dependents[key]);
            let data: Vec<Vec3> = verts
                .iter()
                .enumerate()
                .map(|(index, &vert)| {
                    let mut value = match Some(key) == active {
                        true => self.positions[vert as usize],
                        false => layers[key][vert as usize],
                    };
                    if apply_offset && let Some(offsets) = &offsets {
                        value += offsets[index];
                    }
                    value
                })
                .collect();
            if Some(key) == active && from_reference {
                for (index, &vert) in verts.iter().enumerate() {
                    if original(vert) {
                        mesh.positions[index] = match written.first() {
                            Some(reference) => reference[index],
                            None => layers[0][vert as usize],
                        };
                    }
                }
            }
            written.push(data);
            let _ = block;
        }
        mesh.shape_keys = ShapeKeys {
            keys: blocks
                .iter()
                .zip(written)
                .map(|(block, positions)| ShapeKey {
                    name: block.name.clone(),
                    positions,
                    value: block.value,
                    min: block.min,
                    max: block.max,
                    relative_to: block.relative_to,
                    vertex_group: block.vertex_group.clone(),
                    muted: block.muted,
                })
                .collect(),
            active,
        };
    }
}

fn gathered<T: rikka_polymesh::AttributeValue>(list: &[T], order: &[u32]) -> Values {
    T::wrap(order.iter().map(|&index| list[index as usize]).collect())
}

fn dependent_keys(blocks: &[KeyBlock], index: usize) -> Option<Vec<bool>> {
    let count = blocks.len();
    if index >= count {
        return None;
    }
    let mut marked = vec![false; count];
    marked[index] = true;
    let mut found = false;
    loop {
        let mut updated = false;
        for (key, block) in blocks.iter().enumerate() {
            let relative = block.relative_to as usize;
            if !marked[key] && relative < count && marked[relative] {
                marked[key] = true;
                updated = true;
                found = true;
            }
        }
        if !updated {
            break;
        }
    }
    if !found {
        return None;
    }
    marked[index] = false;
    Some(marked)
}

use glam::{Vec2, Vec3, Vec4};
use rikka_polymesh::{
    Attribute, AttributeValue, BEVEL_WEIGHT_EDGE, BEVEL_WEIGHT_VERT, CREASE_EDGE, CREASE_VERT,
    CUSTOM_NORMAL, DataType, Domain, HIDE_EDGE, HIDE_POLY, HIDE_VERT, Influence, Layers,
    MATERIAL_INDEX, SELECT_EDGE, SELECT_POLY, SELECT_VERT, SHARP_EDGE, SHARP_FACE, UV_SEAM,
    UV_SELECT_EDGE, UV_SELECT_FACE, UV_SELECT_VERT, Values, VertexGroup, uv_pin_name,
};

use crate::error::{Error, Result};
use crate::ids::{EdgeId, ElementKind, FaceId, LoopId, VertId};
use crate::mesh::{EditMesh, Flags, KeyBlock};

const RESERVED: &[&str] = &[
    "position",
    MATERIAL_INDEX,
    SHARP_FACE,
    SHARP_EDGE,
    UV_SEAM,
    SELECT_VERT,
    SELECT_EDGE,
    SELECT_POLY,
    HIDE_VERT,
    HIDE_EDGE,
    HIDE_POLY,
];

macro_rules! well_known {
    ($($all:ident, $all_mut:ident, $name:ident, $domain:ident, $ty:ty;)*) => {
        impl EditMesh {
            $(
                #[must_use]
                pub fn $all(&self) -> Option<&[$ty]> {
                    self.attribute::<$ty>($name, Domain::$domain)
                }

                pub fn $all_mut(&mut self) -> &mut [$ty] {
                    self.ensure_attribute::<$ty>($name, Domain::$domain)
                }
            )*
        }
    };
}

well_known! {
    edge_creases, edge_creases_mut, CREASE_EDGE, Edge, f32;
    vert_creases, vert_creases_mut, CREASE_VERT, Point, f32;
    edge_bevel_weights, edge_bevel_weights_mut, BEVEL_WEIGHT_EDGE, Edge, f32;
    vert_bevel_weights, vert_bevel_weights_mut, BEVEL_WEIGHT_VERT, Point, f32;
    custom_normals, custom_normals_mut, CUSTOM_NORMAL, Corner, Vec3;
    uv_selected_verts, uv_selected_verts_mut, UV_SELECT_VERT, Corner, bool;
    uv_selected_edges, uv_selected_edges_mut, UV_SELECT_EDGE, Corner, bool;
    uv_selected_faces, uv_selected_faces_mut, UV_SELECT_FACE, Face, bool;
}

fn flag_revision(flag: Flags) -> FlagRevision {
    if flag.intersects(Flags::SELECT | Flags::HIDDEN) {
        FlagRevision::Selection
    } else if flag.intersects(Flags::SEAM | Flags::SHARP | Flags::SMOOTH) {
        FlagRevision::Data
    } else {
        FlagRevision::None
    }
}

enum FlagRevision {
    Selection,
    Data,
    None,
}

impl EditMesh {
    fn touch_flag(&mut self, flag: Flags) {
        match flag_revision(flag) {
            FlagRevision::Selection => self.touch_selection(),
            FlagRevision::Data => self.touch_data(),
            FlagRevision::None => {}
        }
    }

    #[must_use]
    pub fn position(&self, vert: VertId) -> Vec3 {
        self.positions[vert.index()]
    }

    pub fn set_position(&mut self, vert: VertId, position: Vec3) {
        self.positions[vert.index()] = position;
        self.log_moved(vert);
        self.touch_moved();
    }

    #[must_use]
    pub fn positions(&self) -> &[Vec3] {
        &self.positions
    }

    pub fn positions_mut(&mut self) -> &mut [Vec3] {
        self.touch_shape();
        &mut self.positions
    }

    #[must_use]
    pub fn vert_flags(&self, vert: VertId) -> Flags {
        self.verts.records[vert.index()].flags
    }

    #[must_use]
    pub fn edge_flags(&self, edge: EdgeId) -> Flags {
        self.edges.records[edge.index()].flags
    }

    #[must_use]
    pub fn face_flags(&self, face: FaceId) -> Flags {
        self.faces.records[face.index()].flags
    }

    #[must_use]
    pub fn loop_flags(&self, lp: LoopId) -> Flags {
        self.loops.records[lp.index()].flags
    }

    pub fn set_vert_flag(&mut self, vert: VertId, flag: Flags, enabled: bool) {
        self.mark_vert(vert, flag, enabled);
        self.touch_flag(flag);
    }

    pub fn set_edge_flag(&mut self, edge: EdgeId, flag: Flags, enabled: bool) {
        self.mark_edge(edge, flag, enabled);
        self.touch_flag(flag);
    }

    pub fn set_face_flag(&mut self, face: FaceId, flag: Flags, enabled: bool) {
        self.mark_face(face, flag, enabled);
        self.touch_flag(flag);
    }

    pub fn set_loop_flag(&mut self, lp: LoopId, flag: Flags, enabled: bool) {
        self.loops.records[lp.index()].flags.set(flag, enabled);
        self.touch_flag(flag);
    }

    pub fn set_vert_flags(&mut self, vert: VertId, flags: Flags) {
        let changed = self.vert_flags(vert) ^ flags;
        self.put_vert_flags(vert, flags);
        self.touch_flags(changed);
    }

    pub fn set_edge_flags(&mut self, edge: EdgeId, flags: Flags) {
        let changed = self.edge_flags(edge) ^ flags;
        self.put_edge_flags(edge, flags);
        self.touch_flags(changed);
    }

    pub fn set_face_flags(&mut self, face: FaceId, flags: Flags) {
        let changed = self.face_flags(face) ^ flags;
        self.put_face_flags(face, flags);
        self.touch_flags(changed);
    }

    pub fn merge_vert_flags(&mut self, a: VertId, b: VertId) {
        let merged = self.vert_flags(a).merge_hidden(self.vert_flags(b));
        self.set_vert_flags(a, merged);
        self.set_vert_flags(b, merged);
    }

    pub fn merge_edge_flags(&mut self, a: EdgeId, b: EdgeId) {
        let merged = self.edge_flags(a).merge_hidden(self.edge_flags(b));
        self.set_edge_flags(a, merged);
        self.set_edge_flags(b, merged);
    }

    pub fn merge_face_flags(&mut self, a: FaceId, b: FaceId) {
        let merged = self.face_flags(a).merge_hidden(self.face_flags(b));
        self.set_face_flags(a, merged);
        self.set_face_flags(b, merged);
    }

    fn touch_flags(&mut self, changed: Flags) {
        if changed.intersects(Flags::SELECT | Flags::HIDDEN) {
            self.touch_selection();
        }
        if changed.intersects(Flags::SEAM | Flags::SHARP | Flags::SMOOTH) {
            self.touch_data();
        }
    }

    #[must_use]
    pub fn is_vert_selected(&self, vert: VertId) -> bool {
        self.vert_flags(vert).contains(Flags::SELECT)
    }

    #[must_use]
    pub fn is_edge_selected(&self, edge: EdgeId) -> bool {
        self.edge_flags(edge).contains(Flags::SELECT)
    }

    #[must_use]
    pub fn is_face_selected(&self, face: FaceId) -> bool {
        self.face_flags(face).contains(Flags::SELECT)
    }

    #[must_use]
    pub fn is_vert_hidden(&self, vert: VertId) -> bool {
        self.vert_flags(vert).contains(Flags::HIDDEN)
    }

    #[must_use]
    pub fn is_edge_hidden(&self, edge: EdgeId) -> bool {
        self.edge_flags(edge).contains(Flags::HIDDEN)
    }

    #[must_use]
    pub fn is_face_hidden(&self, face: FaceId) -> bool {
        self.face_flags(face).contains(Flags::HIDDEN)
    }

    #[must_use]
    pub fn is_edge_seam(&self, edge: EdgeId) -> bool {
        self.edge_flags(edge).contains(Flags::SEAM)
    }

    pub fn set_edge_seam(&mut self, edge: EdgeId, seam: bool) {
        self.set_edge_flag(edge, Flags::SEAM, seam);
    }

    #[must_use]
    pub fn is_edge_sharp(&self, edge: EdgeId) -> bool {
        self.edge_flags(edge).contains(Flags::SHARP)
    }

    pub fn set_edge_sharp(&mut self, edge: EdgeId, sharp: bool) {
        self.set_edge_flag(edge, Flags::SHARP, sharp);
    }

    #[must_use]
    pub fn is_face_smooth(&self, face: FaceId) -> bool {
        self.face_flags(face).contains(Flags::SMOOTH)
    }

    pub fn set_face_smooth(&mut self, face: FaceId, smooth: bool) {
        self.set_face_flag(face, Flags::SMOOTH, smooth);
    }

    #[must_use]
    pub fn is_vert_tagged(&self, vert: VertId) -> bool {
        self.vert_flags(vert).contains(Flags::TAG)
    }

    #[must_use]
    pub fn is_edge_tagged(&self, edge: EdgeId) -> bool {
        self.edge_flags(edge).contains(Flags::TAG)
    }

    #[must_use]
    pub fn is_face_tagged(&self, face: FaceId) -> bool {
        self.face_flags(face).contains(Flags::TAG)
    }

    #[must_use]
    pub fn is_loop_tagged(&self, lp: LoopId) -> bool {
        self.loop_flags(lp).contains(Flags::TAG)
    }

    pub fn set_vert_tag(&mut self, vert: VertId, tag: bool) {
        self.verts.records[vert.index()].flags.set(Flags::TAG, tag);
    }

    pub fn set_edge_tag(&mut self, edge: EdgeId, tag: bool) {
        self.edges.records[edge.index()].flags.set(Flags::TAG, tag);
    }

    pub fn set_face_tag(&mut self, face: FaceId, tag: bool) {
        self.faces.records[face.index()].flags.set(Flags::TAG, tag);
    }

    pub fn set_loop_tag(&mut self, lp: LoopId, tag: bool) {
        self.loops.records[lp.index()].flags.set(Flags::TAG, tag);
    }

    pub fn clear_tags(&mut self, kind: ElementKind) {
        match kind {
            ElementKind::Vert => self
                .verts
                .records
                .iter_mut()
                .for_each(|record| record.flags.remove(Flags::TAG)),
            ElementKind::Edge => self
                .edges
                .records
                .iter_mut()
                .for_each(|record| record.flags.remove(Flags::TAG)),
            ElementKind::Face => self
                .faces
                .records
                .iter_mut()
                .for_each(|record| record.flags.remove(Flags::TAG)),
        }
    }

    pub fn clear_loop_tags(&mut self) {
        self.loops
            .records
            .iter_mut()
            .for_each(|record| record.flags.remove(Flags::TAG));
    }

    #[must_use]
    pub fn face_material(&self, face: FaceId) -> i32 {
        self.faces.records[face.index()].material
    }

    pub fn set_face_material(&mut self, face: FaceId, material: i32) {
        self.faces.records[face.index()].material = material;
        self.touch_data();
    }

    pub fn attributes(&self, domain: Domain) -> impl Iterator<Item = &Attribute> {
        self.table(domain).attributes.iter()
    }

    #[must_use]
    pub fn attribute<T: AttributeValue>(&self, name: &str, domain: Domain) -> Option<&[T]> {
        self.table(domain).attributes.values(name, domain)
    }

    pub fn attribute_mut<T: AttributeValue>(
        &mut self,
        name: &str,
        domain: Domain,
    ) -> Option<&mut [T]> {
        self.touch_data();
        self.table_mut(domain).attributes.values_mut(name, domain)
    }

    #[must_use]
    pub fn attribute_domain(&self, name: &str) -> Option<Domain> {
        rikka_polymesh::Domain::ALL
            .into_iter()
            .find(|&domain| self.table(domain).attributes.contains(name))
    }

    pub fn ensure_attribute<T: AttributeValue>(&mut self, name: &str, domain: Domain) -> &mut [T] {
        self.touch_data();
        if let Some(existing) = self.attribute_domain(name)
            && existing != domain
        {
            self.table_mut(existing).attributes.remove(name);
        }
        let len = self.capacity(domain);
        self.table_mut(domain).attributes.ensure(name, domain, len)
    }

    pub fn add_attribute(&mut self, name: &str, domain: Domain, data_type: DataType) -> Result<()> {
        if RESERVED.contains(&name) {
            return Err(Error::ReservedAttribute(name.to_owned()));
        }
        if self.attribute_domain(name).is_some() {
            return Err(Error::AttributeExists(name.to_owned()));
        }
        let len = self.capacity(domain);
        self.table_mut(domain).attributes.insert(Attribute::new(
            name,
            domain,
            Values::new(data_type, len),
        ));
        self.touch_data();
        Ok(())
    }

    pub fn remove_attribute(&mut self, name: &str) -> bool {
        let Some(domain) = self.attribute_domain(name) else {
            return false;
        };
        self.table_mut(domain).attributes.remove(name);
        if self.layout.uv_maps.remove(name) {
            self.table_mut(Domain::Corner)
                .attributes
                .remove(&uv_pin_name(name));
        }
        self.layout.color_attributes.remove(name);
        self.touch_data();
        true
    }

    pub fn rename_attribute(&mut self, from: &str, to: &str) -> Result<()> {
        let domain = self
            .attribute_domain(from)
            .ok_or_else(|| Error::UnknownAttribute(from.to_owned()))?;
        if RESERVED.contains(&to) {
            return Err(Error::ReservedAttribute(to.to_owned()));
        }
        if from != to && self.attribute_domain(to).is_some() {
            return Err(Error::AttributeExists(to.to_owned()));
        }
        self.table_mut(domain).attributes.rename(from, to);
        if self.layout.uv_maps.contains(from) {
            self.layout.uv_maps.rename(from, to);
            self.table_mut(Domain::Corner)
                .attributes
                .rename(&uv_pin_name(from), &uv_pin_name(to));
        }
        self.layout.color_attributes.rename(from, to);
        self.touch_data();
        Ok(())
    }

    #[must_use]
    pub fn uv_maps(&self) -> &Layers {
        &self.layout.uv_maps
    }

    pub fn add_uv_map(&mut self, name: &str) -> Result<()> {
        self.add_attribute(name, Domain::Corner, DataType::Vec2)?;
        self.layout.uv_maps.push(name);
        Ok(())
    }

    pub fn remove_uv_map(&mut self, name: &str) -> bool {
        self.layout.uv_maps.contains(name) && self.remove_attribute(name)
    }

    pub fn set_active_uv_map(&mut self, name: &str) -> bool {
        self.touch_data();
        self.layout.uv_maps.set_active(name)
    }

    pub fn set_render_uv_map(&mut self, name: &str) -> bool {
        self.touch_data();
        self.layout.uv_maps.set_render(name)
    }

    #[must_use]
    pub fn loop_uvs(&self, uv_map: &str) -> Option<&[Vec2]> {
        match self.layout.uv_maps.contains(uv_map) {
            true => self.attribute(uv_map, Domain::Corner),
            false => None,
        }
    }

    pub fn loop_uvs_mut(&mut self, uv_map: &str) -> Option<&mut [Vec2]> {
        match self.layout.uv_maps.contains(uv_map) {
            true => self.attribute_mut(uv_map, Domain::Corner),
            false => None,
        }
    }

    #[must_use]
    pub fn active_loop_uvs(&self) -> Option<&[Vec2]> {
        self.loop_uvs(self.layout.uv_maps.active_name()?)
    }

    #[must_use]
    pub fn uv_pins(&self, uv_map: &str) -> Option<&[bool]> {
        self.attribute(&uv_pin_name(uv_map), Domain::Corner)
    }

    pub fn uv_pins_mut(&mut self, uv_map: &str) -> Option<&mut [bool]> {
        match self.layout.uv_maps.contains(uv_map) {
            true => Some(self.ensure_attribute(&uv_pin_name(uv_map), Domain::Corner)),
            false => None,
        }
    }

    #[must_use]
    pub fn color_attributes(&self) -> &Layers {
        &self.layout.color_attributes
    }

    pub fn add_color_attribute(&mut self, name: &str, domain: Domain) -> Result<()> {
        self.add_attribute(name, domain, DataType::Color)?;
        self.layout.color_attributes.push(name);
        Ok(())
    }

    pub fn remove_color_attribute(&mut self, name: &str) -> bool {
        self.layout.color_attributes.contains(name) && self.remove_attribute(name)
    }

    pub fn set_active_color_attribute(&mut self, name: &str) -> bool {
        self.touch_data();
        self.layout.color_attributes.set_active(name)
    }

    pub fn set_render_color_attribute(&mut self, name: &str) -> bool {
        self.touch_data();
        self.layout.color_attributes.set_render(name)
    }

    #[must_use]
    pub fn colors(&self, name: &str) -> Option<(Domain, &[Vec4])> {
        if !self.layout.color_attributes.contains(name) {
            return None;
        }
        let domain = self.attribute_domain(name)?;
        Some((domain, self.attribute(name, domain)?))
    }

    #[must_use]
    pub fn vertex_groups(&self) -> &[VertexGroup] {
        &self.layout.vertex_groups
    }

    pub fn vertex_groups_mut(&mut self) -> &mut [VertexGroup] {
        self.touch_data();
        &mut self.layout.vertex_groups
    }

    #[must_use]
    pub fn active_vertex_group(&self) -> Option<usize> {
        self.layout.active_vertex_group
    }

    pub fn set_active_vertex_group(&mut self, group: Option<usize>) {
        self.layout.active_vertex_group =
            group.filter(|&group| group < self.layout.vertex_groups.len());
        self.touch_data();
    }

    pub fn add_vertex_group(&mut self, name: &str) -> u32 {
        if let Some(index) = self
            .layout
            .vertex_groups
            .iter()
            .position(|group| group.name == name)
        {
            return index as u32;
        }
        self.layout.vertex_groups.push(VertexGroup::new(name));
        let index = self.layout.vertex_groups.len() - 1;
        self.layout.active_vertex_group.get_or_insert(index);
        self.ensure_weights();
        self.touch_data();
        index as u32
    }

    pub fn remove_vertex_group(&mut self, group: u32) {
        let groups = &mut self.layout.vertex_groups;
        if group as usize >= groups.len() {
            return;
        }
        groups.remove(group as usize);
        let remaining = groups.len();
        if let Some(weights) = &mut self.vert_data.weights {
            for influences in weights.iter_mut() {
                influences.retain(|influence| influence.group != group);
                for influence in influences.iter_mut() {
                    if influence.group > group {
                        influence.group -= 1;
                    }
                }
            }
        }
        self.layout.active_vertex_group = match self.layout.active_vertex_group {
            Some(active) if active == group as usize => {
                (remaining > 0).then(|| active.min(remaining - 1))
            }
            Some(active) if active > group as usize => Some(active - 1),
            other => other,
        };
        self.touch_data();
    }

    fn ensure_weights(&mut self) {
        if self.vert_data.weights.is_none() {
            self.vert_data.weights = Some(vec![Vec::new(); self.verts.capacity()]);
        }
    }

    #[must_use]
    pub fn vert_influences(&self, vert: VertId) -> &[Influence] {
        self.vert_data
            .weights
            .as_ref()
            .map_or(&[], |weights| weights[vert.index()].as_slice())
    }

    #[must_use]
    pub fn vert_weight(&self, vert: VertId, group: u32) -> f32 {
        self.vert_influences(vert)
            .iter()
            .find(|influence| influence.group == group)
            .map_or(0.0, |influence| influence.weight)
    }

    pub fn set_vert_weight(&mut self, vert: VertId, group: u32, weight: f32) {
        self.ensure_weights();
        let influences = &mut self
            .vert_data
            .weights
            .as_mut()
            .expect("weights were ensured")[vert.index()];
        match influences
            .iter_mut()
            .find(|influence| influence.group == group)
        {
            Some(influence) => influence.weight = weight,
            None => influences.push(Influence { group, weight }),
        }
        self.touch_data();
    }

    pub fn remove_vert_weight(&mut self, vert: VertId, group: u32) {
        if let Some(weights) = &mut self.vert_data.weights {
            weights[vert.index()].retain(|influence| influence.group != group);
            self.touch_data();
        }
    }

    #[must_use]
    pub fn key_blocks(&self) -> &[KeyBlock] {
        &self.layout.key_blocks
    }

    pub fn key_blocks_mut(&mut self) -> &mut [KeyBlock] {
        self.touch_keys();
        &mut self.layout.key_blocks
    }

    #[must_use]
    pub fn active_key_block(&self) -> Option<usize> {
        self.layout.active_key_block
    }

    pub fn set_active_key_block(&mut self, key: Option<usize>) {
        self.layout.active_key_block = key.filter(|&key| key < self.layout.key_blocks.len());
        self.touch_keys();
    }

    #[must_use]
    pub fn key_positions(&self, key: usize) -> &[Vec3] {
        &self.vert_data.keys[key]
    }

    pub fn key_positions_mut(&mut self, key: usize) -> &mut [Vec3] {
        self.touch_keys();
        &mut self.vert_data.keys[key]
    }

    pub fn add_key_block(&mut self, block: KeyBlock) -> usize {
        self.layout.key_blocks.push(block);
        self.vert_data.keys.push(self.positions.clone());
        let index = self.layout.key_blocks.len() - 1;
        self.layout.active_key_block.get_or_insert(index);
        self.touch_keys();
        index
    }

    pub fn remove_key_block(&mut self, key: usize) {
        if key >= self.layout.key_blocks.len() {
            return;
        }
        self.layout.key_blocks.remove(key);
        self.vert_data.keys.remove(key);
        for block in &mut self.layout.key_blocks {
            if block.relative_to as usize == key {
                block.relative_to = 0;
            } else if block.relative_to as usize > key {
                block.relative_to -= 1;
            }
        }
        let remaining = self.layout.key_blocks.len();
        self.layout.active_key_block = match self.layout.active_key_block {
            Some(active) if active == key => (remaining > 0).then(|| active.min(remaining - 1)),
            Some(active) if active > key => Some(active - 1),
            other => other,
        };
        self.touch_keys();
    }

    #[must_use]
    pub fn is_vert_original(&self, vert: VertId) -> bool {
        self.vert_data.key_index[vert.index()] != crate::data::KEY_INDEX_NONE
    }

    pub fn set_vert_original(&mut self, vert: VertId, original: bool) {
        self.vert_data.key_index[vert.index()] = match original {
            true => vert.index() as i32,
            false => crate::data::KEY_INDEX_NONE,
        };
        self.touch_keys();
    }
}

use std::f64::consts::PI;

use glam::Vec3;
use rikka_polymesh::{CREASE_VERT, Domain, Polymesh};

use super::groups::{ends_topology, is_open_boundary};
use super::{EMPTY, EdgeGroup, NewEdge, Settings, Topology};
use crate::evaluators::offset_shape_keys;
use crate::evaluators::solidify::mark_group;
use crate::kinds::solidify::Solidify;
use crate::note::SkipReason;
use crate::origins;

#[derive(Default)]
struct Output {
    point_sources: Vec<u32>,
    positions: Vec<Vec3>,
    edges: Vec<[u32; 2]>,
    edge_sources: Vec<u32>,
    creases: Option<Vec<f32>>,
    bevel_weights: Option<Vec<f32>>,
    face_sources: Vec<u32>,
    face_materials: Vec<i32>,
    face_offsets: Vec<u32>,
    corner_sources: Vec<u32>,
    corner_verts: Vec<u32>,
    corner_edges: Vec<u32>,
    shell_points: Vec<u32>,
    rim_points: Vec<u32>,
}

impl Output {
    fn corner(&mut self, source: usize, vert: u32, edge: u32) {
        self.corner_sources.push(source as u32);
        self.corner_verts.push(vert);
        self.corner_edges.push(edge);
    }

    fn face(&mut self, source: usize, material: i32) {
        self.face_sources.push(source as u32);
        self.face_materials.push(material);
        self.face_offsets.push(self.corner_verts.len() as u32);
    }

    fn edge(&self, index: u32) -> Option<[u32; 2]> {
        self.edges.get(index as usize).copied()
    }
}

struct Assembly<'a> {
    solidify: &'a Solidify,
    topology: &'a Topology,
    mesh: &'a Polymesh,
    settings: &'a Settings,
    edge_creases: Option<&'a [f32]>,
    edge_bevel_weights: Option<&'a [f32]>,
    vertex_creases: Option<&'a [f32]>,
    vertex_bevel_weights: Option<&'a [f32]>,
}

fn group_of<'a>(
    topology: &'a Topology,
    edge: &NewEdge,
    end: usize,
) -> Result<&'a EdgeGroup, SkipReason> {
    topology.group(edge.groups[end])
}

pub(super) fn assemble(
    solidify: &Solidify,
    topology: &mut Topology,
    mesh: &Polymesh,
    settings: &Settings,
) -> Result<Polymesh, SkipReason> {
    let singularities = singularity_edges(topology, settings)?;
    let mut out = Output::default();
    let assembly_creases = mesh.edge_creases().is_some() || mesh.vertex_creases().is_some();
    let assembly_bevel = mesh.edge_bevel_weights().is_some()
        || solidify.bevel_convex != 0.0
        || mesh.vertex_bevel_weights().is_some();
    if assembly_creases {
        out.creases = Some(Vec::new());
    }
    if assembly_bevel {
        out.bevel_weights = Some(Vec::new());
    }
    for (vert, groups) in topology.groups.iter().enumerate() {
        for group in groups {
            if group.new_vert != EMPTY {
                out.point_sources.push(vert as u32);
                out.positions.push(group.co);
            }
        }
    }
    shell_edges(topology, mesh, settings, solidify, &singularities, &mut out)?;
    let shell_edge_count = out.edges.len();
    let mut open_face_edges: Vec<Vec<u32>> = topology
        .groups
        .iter()
        .map(|groups| vec![EMPTY; groups.len()])
        .collect();
    Assembly::new(solidify, topology, mesh, settings).boundaries(&mut out, &mut open_face_edges)?;
    let fill_faces = out.face_sources.len();
    for (groups, edges) in topology.groups.iter_mut().zip(open_face_edges) {
        for (group, edge) in groups.iter_mut().zip(edges) {
            group.open_face_edge = edge;
        }
    }
    let assembly = Assembly::new(solidify, topology, mesh, settings);
    if settings.rim {
        assembly.rims(&mut out)?;
    }
    if settings.shell {
        assembly.shells(&mut out)?;
    }
    Ok(assembly.finish(out, shell_edge_count, fill_faces))
}

fn singularity_edges(
    topology: &Topology,
    settings: &Settings,
) -> Result<Vec<[u32; 2]>, SkipReason> {
    let mut pairs: Vec<[u32; 2]> = Vec::new();
    if !topology.has_singularities {
        return Ok(pairs);
    }
    for edge in 0..topology.adj_len.len() {
        if !(settings.shell || topology.adj_len[edge] == 1) {
            continue;
        }
        let Some(list) = topology.list_owner(edge) else {
            continue;
        };
        for &id in list {
            let new_edge = &topology.edges[id as usize];
            let first = group_of(topology, new_edge, 0)?;
            let second = group_of(topology, new_edge, 1)?;
            if first.is_singularity && second.is_singularity {
                let pair = [first.new_vert, second.new_vert];
                let exists = pairs
                    .iter()
                    .any(|known| *known == pair || *known == [pair[1], pair[0]]);
                if !exists {
                    pairs.push(pair);
                }
            }
        }
    }
    Ok(pairs)
}

fn shell_edges(
    topology: &mut Topology,
    mesh: &Polymesh,
    settings: &Settings,
    solidify: &Solidify,
    singularities: &[[u32; 2]],
    out: &mut Output,
) -> Result<(), SkipReason> {
    let edge_creases = mesh.edge_creases();
    let edge_bevel_weights = mesh.edge_bevel_weights();
    let convex = solidify.bevel_convex;
    out.edges = vec![[0, 0]; singularities.len()];
    out.edge_sources = vec![0; singularities.len()];
    if let Some(creases) = out.creases.as_mut() {
        creases.resize(singularities.len(), 0.0);
    }
    if let Some(weights) = out.bevel_weights.as_mut() {
        weights.resize(singularities.len(), 0.0);
    }
    for edge in 0..mesh.edge_count() {
        if !(settings.shell || topology.adj_len[edge] == 1) {
            continue;
        }
        let Some(list) = topology.list_owner(edge).map(<[u32]>::to_vec) else {
            continue;
        };
        for id in list {
            let new_edge = topology.edges[id as usize].clone();
            if new_edge.new_edge == EMPTY {
                continue;
            }
            let first = group_of(topology, &new_edge, 0)?;
            let second = group_of(topology, &new_edge, 1)?;
            let pair = [first.new_vert, second.new_vert];
            let singular =
                topology.has_singularities && first.is_singularity && second.is_singularity;
            let index = match singular {
                true => singularities
                    .iter()
                    .position(|known| *known == pair || *known == [pair[1], pair[0]])
                    .ok_or(SkipReason::InvalidMesh)?,
                false => {
                    out.edges.push([0, 0]);
                    out.edge_sources.push(0);
                    if let Some(creases) = out.creases.as_mut() {
                        creases.push(0.0);
                    }
                    if let Some(weights) = out.bevel_weights.as_mut() {
                        weights.push(0.0);
                    }
                    out.edges.len() - 1
                }
            };
            out.edges[index] = pair;
            out.edge_sources[index] = edge as u32;
            let old = new_edge.old_edge as usize;
            if let Some(creases) = out.creases.as_mut() {
                creases[index] = edge_creases.map_or(0.0, |values| values[old]);
            }
            if let Some(weights) = out.bevel_weights.as_mut() {
                let mut weight = edge_bevel_weights.map_or(0.0, |values| values[old]);
                if convex != 0.0 && new_edge.faces[1].is_some() {
                    let angle = f64::from(new_edge.angle);
                    let tolerance = f64::from(f32::EPSILON);
                    let change = if angle > PI + tolerance {
                        convex.clamp(0.0, 1.0)
                    } else if angle < PI - tolerance {
                        convex.clamp(-1.0, 0.0)
                    } else {
                        0.0
                    };
                    weight = (weight + change).clamp(0.0, 1.0);
                }
                weights[index] = weight;
            }
            topology.edges[id as usize].new_edge = index as u32;
        }
    }
    Ok(())
}

impl<'a> Assembly<'a> {
    fn new(
        solidify: &'a Solidify,
        topology: &'a Topology,
        mesh: &'a Polymesh,
        settings: &'a Settings,
    ) -> Assembly<'a> {
        Assembly {
            solidify,
            topology,
            mesh,
            settings,
            edge_creases: mesh.edge_creases(),
            edge_bevel_weights: mesh.edge_bevel_weights(),
            vertex_creases: mesh.vertex_creases(),
            vertex_bevel_weights: mesh.vertex_bevel_weights(),
        }
    }

    fn material(&self, face: usize) -> i32 {
        self.mesh.material_index(face)
    }

    fn corner_at(&self, face: usize, vert: usize) -> Option<usize> {
        let topology = self.topology;
        self.mesh
            .face(face)
            .find(|&corner| topology.vm[self.mesh.corner_verts[corner] as usize] as usize == vert)
    }

    fn boundaries(&self, out: &mut Output, open: &mut [Vec<u32>]) -> Result<(), SkipReason> {
        let topology = self.topology;
        let settings = self.settings;
        let has_creases = out.creases.is_some();
        for (vert, groups) in topology.groups.iter().enumerate() {
            if groups.is_empty() {
                continue;
            }
            let vertex_crease = self.vertex_creases.map_or(0.0, |values| values[vert]);
            let vertex_bevel = self.vertex_bevel_weights.map_or(0.0, |values| values[vert]);
            let mut fill_start = 0usize;
            let mut last: Option<usize> = None;
            let mut first: Option<usize> = None;
            let (mut last_crease, mut first_crease) = (0.0f32, 0.0f32);
            let (mut last_bevel, mut first_bevel) = (0.0f32, 0.0f32);
            let mut count = 0usize;
            for (index, group) in groups.iter().enumerate() {
                if is_open_boundary(group, settings) {
                    let mut max_crease = 0.0f32;
                    let mut max_bevel = 0.0f32;
                    let edges = &group.edges;
                    let old = |slot: usize| topology.edges[edges[slot] as usize].old_edge as usize;
                    if edges.len() == 2 {
                        if has_creases {
                            max_crease = self
                                .edge_creases
                                .map_or(0.0, |values| values[old(0)].min(values[old(1)]));
                        }
                    } else {
                        for slot in 1..edges.len().saturating_sub(1) {
                            if has_creases
                                && let Some(values) = self.edge_creases
                                && values[old(slot)] > max_crease
                            {
                                max_crease = values[old(slot)];
                            }
                            let new_edge = topology.edges[edges[slot] as usize].new_edge;
                            if new_edge != EMPTY
                                && let Some(weights) = out.bevel_weights.as_ref()
                            {
                                max_bevel = weights[new_edge as usize].max(max_bevel);
                            }
                        }
                    }
                    let open_bevel = self.edge_bevel_weights.map_or(0.0, |values| {
                        values[old(0)].min(values[old(edges.len() - 1)])
                    });
                    if open_bevel > 0.0 {
                        max_bevel = open_bevel.min(max_bevel);
                    } else if self.solidify.bevel_convex < 0.0 {
                        max_bevel = 0.0;
                    }
                    match (first, last) {
                        (None, _) => {
                            first = Some(index);
                            first_crease = max_crease;
                            first_bevel = max_bevel;
                        }
                        (Some(_), Some(previous)) => {
                            open[vert][previous] = out.edges.len() as u32;
                            self.boundary_edge(
                                out,
                                &groups[previous],
                                group.new_vert,
                                vertex_crease.max(last_crease.min(max_crease)),
                                vertex_bevel.max(last_bevel.min(max_bevel)),
                            );
                        }
                        (Some(_), None) => {}
                    }
                    last = Some(index);
                    last_crease = max_crease;
                    last_bevel = max_bevel;
                    count += 1;
                }
                if ends_topology(groups, index) {
                    if let (Some(previous), Some(start)) = (last, first) {
                        if count == 2 {
                            open[vert][previous] = out.edges.len() as u32 - 1;
                        }
                        if count > 2 {
                            open[vert][previous] = out.edges.len() as u32;
                            self.boundary_edge(
                                out,
                                &groups[previous],
                                groups[start].new_vert,
                                vertex_crease.max(last_crease.min(first_crease)),
                                vertex_bevel.max(last_bevel.min(first_bevel)),
                            );
                            fill_start =
                                self.fill_face(out, groups, group, vert, fill_start, count)?;
                        }
                    }
                    count = 0;
                    last = None;
                    first = None;
                    last_crease = 0.0;
                    first_crease = 0.0;
                    last_bevel = 0.0;
                    first_bevel = 0.0;
                }
            }
        }
        Ok(())
    }

    fn boundary_edge(&self, out: &mut Output, from: &EdgeGroup, to: u32, crease: f32, bevel: f32) {
        let source = self.topology.edges[from.edges[0] as usize].old_edge;
        out.edges.push([from.new_vert, to]);
        out.edge_sources.push(source);
        if let Some(creases) = out.creases.as_mut() {
            creases.push(crease);
        }
        if let Some(weights) = out.bevel_weights.as_mut() {
            weights.push(bevel);
        }
    }

    fn fill_face(
        &self,
        out: &mut Output,
        groups: &[EdgeGroup],
        current: &EdgeGroup,
        vert: usize,
        start: usize,
        count: usize,
    ) -> Result<usize, SkipReason> {
        let topology = self.topology;
        let settings = self.settings;
        let side_face = |side: Option<u32>| side.map(|side| side as usize / 2);
        let material_count = settings.materials.largest() + 1;
        let mut best = (0i32, 0usize, 0usize);
        for material in 0..material_count {
            let mut found = 0usize;
            let mut face = 0usize;
            let mut seen = 0;
            for group in groups[start..].iter() {
                if seen >= count {
                    break;
                }
                if !is_open_boundary(group, settings) {
                    continue;
                }
                let first_edge = &topology.edges[group.edges[0] as usize];
                if let Some(first_face) = side_face(first_edge.faces[0])
                    && self.material(first_face) == material
                {
                    face = first_face;
                    found += 1;
                }
                let last_edge = &topology.edges[group.edges[group.edges.len() - 1] as usize];
                match side_face(last_edge.faces[1]) {
                    Some(last_face) => {
                        if self.material(last_face) == material {
                            face = last_face;
                            found += 1;
                        }
                    }
                    None => {
                        if let Some(last_face) = side_face(last_edge.faces[0])
                            && self.material(last_face) == material
                        {
                            face = last_face;
                            found += 1;
                        }
                    }
                }
                seen += 1;
            }
            if found > best.2 {
                best = (material, face, found);
            }
        }
        let mut corners = Vec::with_capacity(count);
        let mut next = start;
        while next < groups.len() && corners.len() < count {
            let group = &groups[next];
            if is_open_boundary(group, settings) {
                let face = topology.edges[group.edges[0] as usize].faces[0]
                    .map(|side| side as usize / 2)
                    .ok_or(SkipReason::InvalidMesh)?;
                corners.push(self.corner_at(face, vert).ok_or(SkipReason::InvalidMesh)?);
            }
            next += 1;
        }
        let material = settings
            .materials
            .complex_rim(best.0, settings.rim && !current.is_orig_closed);
        out.face(best.1, material);
        let total = out.edges.len();
        match settings.flip {
            false => {
                for (k, &corner) in corners.iter().enumerate() {
                    let edge = total - count + k;
                    out.corner(corner, out.edges[edge][0], edge as u32);
                }
            }
            true => {
                for k in 1..=count {
                    let edge = total - k;
                    out.corner(corners[count - k], out.edges[edge][1], edge as u32);
                }
            }
        }
        Ok(next)
    }

    fn rims(&self, out: &mut Output) -> Result<(), SkipReason> {
        let topology = self.topology;
        let mesh = self.mesh;
        for edge in 0..mesh.edge_count() {
            if topology.adj_len[edge] != 1 {
                continue;
            }
            let Some(list) = topology.list_owner(edge) else {
                continue;
            };
            let (Some(&first), Some(&second)) = (list.first(), list.get(1)) else {
                continue;
            };
            let edge1 = &topology.edges[first as usize];
            let edge2 = &topology.edges[second as usize];
            let v1_singular = group_of(topology, edge1, 0)?.is_singularity
                && group_of(topology, edge2, 0)?.is_singularity;
            let v2_singular = group_of(topology, edge1, 1)?.is_singularity
                && group_of(topology, edge2, 1)?.is_singularity;
            if v1_singular && v2_singular {
                continue;
            }
            let face = edge1.faces[0].ok_or(SkipReason::InvalidMesh)? as usize / 2;
            out.face(
                face,
                self.settings
                    .materials
                    .complex_rim(self.material(face), true),
            );
            let [a, b] = mesh.edges[edge1.old_edge as usize];
            let (old_v1, old_v2) = (topology.vm[a as usize], topology.vm[b as usize]);
            let mut loop1 = None;
            let mut loop2 = None;
            for corner in mesh.face(face) {
                let vert = topology.vm[mesh.corner_verts[corner] as usize];
                if vert == old_v1 {
                    loop1 = Some(corner);
                } else if vert == old_v2 {
                    loop2 = Some(corner);
                }
            }
            let (Some(loop1), Some(loop2)) = (loop1, loop2) else {
                return Err(SkipReason::InvalidMesh);
            };
            let e1 = out.edge(edge1.new_edge).ok_or(SkipReason::InvalidMesh)?;
            let e2 = out.edge(edge2.new_edge).ok_or(SkipReason::InvalidMesh)?;
            let open_edge =
                |near: &NewEdge, far: &NewEdge, end: usize, vert: u32| -> Result<u32, SkipReason> {
                    let candidate = group_of(topology, near, end)?.open_face_edge;
                    match out.edge(candidate).is_some_and(|pair| pair.contains(&vert)) {
                        true => Ok(candidate),
                        false => Ok(group_of(topology, far, end)?.open_face_edge),
                    }
                };
            let mut corners: Vec<(usize, u32, u32)> = Vec::with_capacity(4);
            match self.settings.flip {
                false => {
                    corners.push((loop1, e1[0], edge1.new_edge));
                    if !v2_singular {
                        corners.push((loop2, e1[1], open_edge(edge1, edge2, 1, e2[1])?));
                    }
                    corners.push((loop2, e2[1], edge2.new_edge));
                    if !v1_singular {
                        corners.push((loop1, e2[0], open_edge(edge2, edge1, 0, e1[0])?));
                    }
                }
                true => {
                    if !v1_singular {
                        corners.push((loop1, e1[0], open_edge(edge1, edge2, 0, e2[0])?));
                    }
                    corners.push((loop1, e2[0], edge2.new_edge));
                    if !v2_singular {
                        corners.push((loop2, e2[1], open_edge(edge2, edge1, 1, e1[1])?));
                    }
                    corners.push((loop2, e1[1], edge1.new_edge));
                }
            }
            for (source, vert, edge) in corners {
                out.rim_points.push(vert);
                out.corner(source, vert, edge);
            }
        }
        Ok(())
    }

    fn shells(&self, out: &mut Output) -> Result<(), SkipReason> {
        let topology = self.topology;
        let mesh = self.mesh;
        for side in 0..mesh.face_count() * 2 {
            let face = side / 2;
            let reversed = side & 1 == 1;
            let range = mesh.face(face);
            let start = range.start;
            let links = &topology.side_links[side];
            let valid = |link: Option<u32>| {
                link.filter(|&id| topology.edges[id as usize].new_edge != EMPTY)
            };
            let mut total = range.len();
            while total > 0 && valid(links[total - 1]).is_none() {
                total -= 1;
            }
            if total == 0 {
                continue;
            }
            let vm = |corner: usize| topology.vm[mesh.corner_verts[corner] as usize];
            let old_ends = |id: u32| {
                let [a, b] = mesh.edges[topology.edges[id as usize].old_edge as usize];
                [topology.vm[a as usize], topology.vm[b as usize]]
            };
            let Some(mut prior) = links[total - 1] else {
                continue;
            };
            let mut prior_flip = usize::from(old_ends(prior)[0] == vm(start + total - 1));
            let mut loops: Vec<usize> = Vec::with_capacity(total * 2);
            let mut verts: Vec<u32> = Vec::with_capacity(total * 2);
            let mut edges: Vec<u32> = Vec::with_capacity(total * 2);
            let mut valid_edges = 0;
            for (j, &link) in links.iter().enumerate().take(total) {
                let Some(id) = valid(link) else {
                    continue;
                };
                valid_edges += 1;
                let new_edge = &topology.edges[id as usize];
                let flip = usize::from(old_ends(id)[1] == vm(start + j));
                let v1 = group_of(topology, new_edge, flip)?.new_vert;
                let v2 = group_of(topology, new_edge, 1 - flip)?.new_vert;
                if verts.last() != Some(&v1) {
                    loops.push(start + j);
                    edges.push(match reversed {
                        true => {
                            group_of(topology, &topology.edges[prior as usize], prior_flip)?
                                .open_face_edge
                        }
                        false => group_of(topology, new_edge, flip)?.open_face_edge,
                    });
                    verts.push(v1);
                }
                prior = id;
                prior_flip = 1 - flip;
                if j < total - 1 || verts.first() != Some(&v2) {
                    loops.push(start + (j + 1) % total);
                    edges.push(new_edge.new_edge);
                    verts.push(v2);
                } else {
                    edges[0] = new_edge.new_edge;
                }
            }
            let k = verts.len();
            if k <= 2 || valid_edges <= 2 {
                continue;
            }
            let inverted = reversed != self.settings.flip;
            out.face(
                face,
                self.settings
                    .materials
                    .complex_shell(self.material(face), inverted),
            );
            match inverted {
                true => {
                    for l in (0..k).rev() {
                        out.shell_points.push(verts[l]);
                        out.corner(loops[l], verts[l], edges[l]);
                    }
                }
                false => {
                    let mut l = k - 1;
                    for (next, &edge) in edges.iter().enumerate() {
                        out.corner(loops[l], verts[l], edge);
                        l = next;
                    }
                }
            }
        }
        Ok(())
    }

    fn finish(&self, mut out: Output, shell_edge_count: usize, fill_faces: usize) -> Polymesh {
        let mesh = self.mesh;
        let mut result = mesh.clone();
        let before: Vec<Vec3> = out
            .point_sources
            .iter()
            .map(|&source| mesh.positions[source as usize])
            .collect();
        result.gather(Domain::Point, &out.point_sources);
        offset_shape_keys(&mut result, &before, &out.positions);
        result.positions = std::mem::take(&mut out.positions);
        result.gather(Domain::Edge, &out.edge_sources);
        result.edges = std::mem::take(&mut out.edges);
        result.gather(Domain::Face, &out.face_sources);
        result.gather(Domain::Corner, &out.corner_sources);
        out.face_offsets.push(out.corner_verts.len() as u32);
        result.face_offsets = std::mem::take(&mut out.face_offsets);
        result.corner_verts = std::mem::take(&mut out.corner_verts);
        let edge_count = result.edges.len() as u32;
        let broken = out.corner_edges.iter().any(|&edge| edge >= edge_count);
        result.corner_edges = std::mem::take(&mut out.corner_edges);
        if broken {
            result.ensure_edges();
        }
        if mesh.material_indices().is_some()
            || out.face_materials.iter().any(|&material| material != 0)
        {
            result
                .material_indices_mut()
                .copy_from_slice(&out.face_materials);
        }
        if let Some(creases) = out.creases.take() {
            result.edge_creases_mut()[..creases.len()].copy_from_slice(&creases);
            if self.settings.rim {
                result.attributes.remove(CREASE_VERT);
            }
        }
        if let Some(weights) = out.bevel_weights.take() {
            result.edge_bevel_weights_mut()[..weights.len()].copy_from_slice(&weights);
        }
        origins::clear(
            &mut result,
            Domain::Edge,
            shell_edge_count..edge_count as usize,
        );
        origins::unmark_control_range(&mut result, shell_edge_count..edge_count as usize);
        origins::clear(&mut result, Domain::Face, 0..fill_faces);
        mark_group(
            &mut result,
            &self.solidify.rim_vertex_group,
            &out.rim_points,
        );
        mark_group(
            &mut result,
            &self.solidify.shell_vertex_group,
            &out.shell_points,
        );
        result
    }
}

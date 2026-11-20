use rikka_mesh_edit::{EdgeId, EditMesh, ElementKind, FaceId, Geometry, IdSet, VertId};

use crate::error::{Error, Result};
use crate::input::BevelInput;
use crate::options::{Affect, BevelOptions};
use crate::params::Params;
use crate::state::Bevel;

pub fn bevel(mesh: &mut EditMesh, input: &BevelInput, options: &BevelOptions) -> Result<Geometry> {
    options.validate()?;
    if let Some(group) = input.vertex_group
        && group as usize >= mesh.vertex_groups().len()
    {
        return Err(Error::UnknownVertexGroup(group));
    }
    for vert in input.verts.iter() {
        if !mesh.contains_vert(vert) {
            return Err(rikka_mesh_edit::Error::DeadVert(vert).into());
        }
    }
    for edge in input.edges.iter() {
        if !mesh.contains_edge(edge) {
            return Err(rikka_mesh_edit::Error::DeadEdge(edge).into());
        }
    }
    if options.width <= 0.0 {
        return Ok(Geometry::new());
    }
    for kind in [ElementKind::Vert, ElementKind::Edge, ElementKind::Face] {
        mesh.clear_tags(kind);
    }
    mesh.update_normals();
    for vert in input.verts.iter() {
        mesh.set_vert_tag(vert, true);
    }
    for edge in input.edges.iter() {
        if mesh.is_edge_manifold(edge) {
            mesh.set_edge_tag(edge, true);
            for vert in mesh.edge_verts(edge) {
                mesh.set_vert_tag(vert, true);
            }
        }
    }
    let params = Params::new(mesh, input, options);
    let mut state = Bevel::new(mesh, params);
    let run = state.run();
    let output = state.output();
    for kind in [ElementKind::Vert, ElementKind::Edge, ElementKind::Face] {
        mesh.clear_tags(kind);
    }
    run?;
    mesh.update_normals();
    Ok(output)
}

impl Bevel<'_> {
    fn tagged_verts(&self) -> Vec<VertId> {
        self.mesh
            .verts()
            .filter(|&vert| self.mesh.is_vert_tagged(vert))
            .collect()
    }

    fn run(&mut self) -> Result<()> {
        self.init_uv_state();
        let candidates = self.tagged_verts();
        for &vert in &candidates {
            if let Some(bev) = self.construct(vert)
                && !self.params.limit_offset
            {
                self.build_boundary(bev, true);
                self.determine_uv_connectivity(vert);
            }
        }
        let tagged = self.tagged_verts();
        if self.params.limit_offset {
            self.limit_offset(&tagged);
            for &vert in &tagged {
                if let Some(bev) = self.bev_of(vert) {
                    self.build_boundary(bev, true);
                    self.determine_uv_connectivity(vert);
                }
            }
        }
        if self.params.offset_adjust {
            self.adjust_offsets(&tagged);
        }
        if self.params.is_custom() {
            let edges: Vec<EdgeId> = self
                .mesh
                .edges()
                .filter(|&edge| self.mesh.is_edge_tagged(edge))
                .collect();
            for edge in edges {
                self.regularize_profile_orientation(edge);
            }
        }
        for &vert in &tagged {
            if let Some(bev) = self.bev_of(vert) {
                self.build_vmesh(bev)?;
            }
        }
        if self.params.affect != Affect::Vertices {
            let edges: Vec<EdgeId> = self
                .mesh
                .edges()
                .filter(|&edge| self.mesh.is_edge_tagged(edge))
                .collect();
            for edge in edges {
                self.build_edge_polygons(edge)?;
            }
        }
        for &vert in &tagged {
            if let Some(bev) = self.bev_of(vert) {
                self.extend_edge_data(bev);
            }
        }
        let mut rebuilt = IdSet::new();
        let mut order: Vec<FaceId> = Vec::new();
        for &vert in &tagged {
            self.rebuild_existing_polygons(vert, &mut rebuilt, &mut order)?;
            self.reattach_wires(vert)?;
        }
        for face in order {
            self.mesh.kill_face(face)?;
        }
        for &vert in &tagged {
            if self.mesh.contains_vert(vert) {
                self.forget_uv_vert(vert);
                self.mesh.kill_vert(vert)?;
            }
        }
        self.merge_uvs();
        if self.params.harden_normals {
            self.harden_normals();
        }
        self.set_face_strength();
        Ok(())
    }

    fn output(&mut self) -> Geometry {
        for vert in self.out_verts.iter() {
            if self.mesh.contains_vert(vert) {
                self.mesh.set_vert_tag(vert, true);
            }
        }
        for edge in self.out_edges.iter() {
            if self.mesh.contains_edge(edge) {
                self.mesh.set_edge_tag(edge, true);
            }
        }
        let mut output = Geometry::new();
        output.verts.extend(
            self.mesh
                .verts()
                .filter(|&vert| self.mesh.is_vert_tagged(vert)),
        );
        output.edges.extend(
            self.mesh
                .edges()
                .filter(|&edge| self.mesh.is_edge_tagged(edge)),
        );
        output.faces.extend(
            self.mesh
                .faces()
                .filter(|&face| self.mesh.is_face_tagged(face)),
        );
        output
    }
}

use glam::Vec3;

use crate::domain::Domain;
use crate::names::{
    BEVEL_WEIGHT_EDGE, BEVEL_WEIGHT_VERT, CREASE_EDGE, CREASE_VERT, CUSTOM_NORMAL, HIDE_EDGE,
    HIDE_POLY, HIDE_VERT, MATERIAL_INDEX, SELECT_EDGE, SELECT_POLY, SELECT_VERT, SHARP_EDGE,
    SHARP_FACE, UV_SEAM, UV_SELECT_EDGE, UV_SELECT_FACE, UV_SELECT_VERT,
};
use crate::polymesh::Polymesh;
use crate::values::{AttributeValue, DataType};

macro_rules! well_known {
    ($($all:ident, $all_mut:ident, $one:ident, $name:ident, $domain:ident, $ty:ty;)*) => {
        pub(crate) const WELL_KNOWN: &[(&str, Domain, DataType)] = &[
            $(($name, Domain::$domain, <$ty as AttributeValue>::DATA_TYPE),)*
        ];

        impl Polymesh {
            $(
                #[must_use]
                pub fn $all(&self) -> Option<&[$ty]> {
                    self.attributes.values($name, Domain::$domain)
                }

                pub fn $all_mut(&mut self) -> &mut [$ty] {
                    let len = self.domain_len(Domain::$domain);
                    self.attributes.ensure($name, Domain::$domain, len)
                }

                #[must_use]
                pub fn $one(&self, index: usize) -> $ty {
                    self.$all()
                        .and_then(|values| values.get(index).copied())
                        .unwrap_or(<$ty as AttributeValue>::DEFAULT)
                }
            )*
        }
    };
}

well_known! {
    material_indices, material_indices_mut, material_index, MATERIAL_INDEX, Face, i32;
    sharp_faces, sharp_faces_mut, is_sharp_face, SHARP_FACE, Face, bool;
    sharp_edges, sharp_edges_mut, is_sharp_edge, SHARP_EDGE, Edge, bool;
    uv_seams, uv_seams_mut, is_uv_seam, UV_SEAM, Edge, bool;
    edge_creases, edge_creases_mut, edge_crease, CREASE_EDGE, Edge, f32;
    vertex_creases, vertex_creases_mut, vertex_crease, CREASE_VERT, Point, f32;
    edge_bevel_weights, edge_bevel_weights_mut, edge_bevel_weight, BEVEL_WEIGHT_EDGE, Edge, f32;
    vertex_bevel_weights, vertex_bevel_weights_mut, vertex_bevel_weight, BEVEL_WEIGHT_VERT, Point, f32;
    selected_points, selected_points_mut, is_point_selected, SELECT_VERT, Point, bool;
    selected_edges, selected_edges_mut, is_edge_selected, SELECT_EDGE, Edge, bool;
    selected_faces, selected_faces_mut, is_face_selected, SELECT_POLY, Face, bool;
    hidden_points, hidden_points_mut, is_point_hidden, HIDE_VERT, Point, bool;
    hidden_edges, hidden_edges_mut, is_edge_hidden, HIDE_EDGE, Edge, bool;
    hidden_faces, hidden_faces_mut, is_face_hidden, HIDE_POLY, Face, bool;
    custom_normals, custom_normals_mut, custom_normal, CUSTOM_NORMAL, Corner, Vec3;
    uv_selected_points, uv_selected_points_mut, is_uv_point_selected, UV_SELECT_VERT, Corner, bool;
    uv_selected_edges, uv_selected_edges_mut, is_uv_edge_selected, UV_SELECT_EDGE, Corner, bool;
    uv_selected_faces, uv_selected_faces_mut, is_uv_face_selected, UV_SELECT_FACE, Face, bool;
}

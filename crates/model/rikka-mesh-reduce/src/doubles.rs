use rikka_mesh_edit::{EdgeId, EditMesh, ElementKind, FaceId, IdMap, IdSet, SelectMode, VertId};
use rikka_spatial::PointTree;

use crate::common::{Counts, Removed, any_face_selected, selected_verts};
use crate::error::{Error, Result};
use crate::weld::WeldOptions;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RemoveDoublesOptions {
    pub threshold: f32,
    pub use_centroid: bool,
    pub use_unselected: bool,
    pub use_sharp_edge_from_normals: bool,
}

impl Default for RemoveDoublesOptions {
    fn default() -> RemoveDoublesOptions {
        RemoveDoublesOptions {
            threshold: 1.0e-4,
            use_centroid: true,
            use_unselected: false,
            use_sharp_edge_from_normals: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AutoMergeOptions {
    pub threshold: f32,
    pub use_centroid: bool,
}

impl Default for AutoMergeOptions {
    fn default() -> AutoMergeOptions {
        AutoMergeOptions {
            threshold: 0.001,
            use_centroid: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FindDoublesOptions {
    pub distance: f32,
    pub use_connected: bool,
}

pub fn find_doubles(
    mesh: &EditMesh,
    verts: &[VertId],
    keep: &IdSet<VertId>,
    options: &FindDoublesOptions,
) -> IdMap<VertId, VertId> {
    let verts: Vec<VertId> = verts
        .iter()
        .copied()
        .filter(|&vert| mesh.contains_vert(vert))
        .collect();
    let has_keep = verts.iter().any(|&vert| keep.contains(vert));
    let targets = match options.use_connected {
        true => connected_doubles(mesh, &verts, keep, has_keep, options.distance),
        false => {
            let tree = PointTree::from_indexed(
                verts
                    .iter()
                    .enumerate()
                    .map(|(index, &vert)| (index as u32, mesh.position(vert))),
            );
            tree.find_duplicate_clusters(options.distance, |index| {
                has_keep && keep.contains(verts[index as usize])
            })
            .into_targets()
        }
    };
    let mut map: IdMap<VertId, VertId> = IdMap::new();
    for (index, &target) in targets.iter().enumerate() {
        if target as usize != index {
            map.insert(verts[index], verts[target as usize]);
        }
    }
    map
}

fn connected_doubles(
    mesh: &EditMesh,
    verts: &[VertId],
    keep: &IdSet<VertId>,
    has_keep: bool,
    distance: f32,
) -> Vec<u32> {
    const UNSET: i64 = -1;
    let mut duplicates: Vec<i64> = verts
        .iter()
        .enumerate()
        .map(|(index, &vert)| match has_keep && keep.contains(vert) {
            true => index as i64,
            false => UNSET,
        })
        .collect();
    let mut lookup: IdMap<VertId, usize> = IdMap::new();
    for (index, &vert) in verts.iter().enumerate() {
        lookup.insert(vert, index);
    }
    let distance_squared = distance * distance;
    let mut stack: Vec<usize> = Vec::new();
    for index in 0..verts.len() {
        if duplicates[index] != UNSET && duplicates[index] != index as i64 {
            continue;
        }
        let origin = mesh.position(verts[index]);
        let mut check = Some(index);
        while let Some(current) = check {
            let vert = verts[current];
            let claim = |other: VertId, duplicates: &mut Vec<i64>, stack: &mut Vec<usize>| {
                if mesh.position(other).distance_squared(origin) < distance_squared
                    && let Some(other_index) = lookup.copied(other)
                    && duplicates[other_index] == UNSET
                {
                    duplicates[other_index] = index as i64;
                    stack.push(other_index);
                }
            };
            for edge in mesh.vert_edges(vert) {
                let Some(other) = mesh.edge_other_vert(edge, vert) else {
                    continue;
                };
                claim(other, &mut duplicates, &mut stack);
                for radial in mesh.edge_loops(edge) {
                    if mesh.loop_vert(radial) != vert {
                        continue;
                    }
                    if mesh.face_len(mesh.loop_face(radial)) <= 3 {
                        continue;
                    }
                    let end = mesh.loop_prev(radial);
                    let mut lp = mesh.loop_next(mesh.loop_next(radial));
                    loop {
                        claim(mesh.loop_vert(lp), &mut duplicates, &mut stack);
                        lp = mesh.loop_next(lp);
                        if lp == end {
                            break;
                        }
                    }
                }
            }
            check = stack.pop();
        }
    }
    duplicates
        .iter()
        .enumerate()
        .map(|(index, &target)| match target {
            UNSET => index as u32,
            target => target as u32,
        })
        .collect()
}

pub fn remove_doubles(mesh: &mut EditMesh, options: &RemoveDoublesOptions) -> Result<Removed> {
    if !(options.threshold >= 0.0 && options.threshold.is_finite()) {
        return Err(Error::OutOfRange("threshold"));
    }
    let counts = Counts::new(mesh);
    let verts = selected_verts(mesh);
    if verts.is_empty() {
        return Ok(counts.removed(mesh));
    }
    let mode = mesh.select_mode();
    let kind = if mode.contains(SelectMode::VERT) {
        ElementKind::Vert
    } else if mode.contains(SelectMode::EDGE) {
        ElementKind::Edge
    } else {
        ElementKind::Face
    };
    store_selection_as_tags(mesh, kind);
    if options.use_unselected {
        let moved: IdSet<VertId> = verts.iter().copied().collect();
        merge_into_unmoved(mesh, &moved, options.threshold, options.use_centroid);
    } else {
        let targets = find_doubles(
            mesh,
            &verts,
            &IdSet::new(),
            &FindDoublesOptions {
                distance: options.threshold,
                use_connected: false,
            },
        );
        mesh.weld_verts(
            &targets,
            &WeldOptions {
                use_centroid: options.use_centroid,
                average_vert_data: false,
            },
        );
    }
    restore_selection_from_tags(mesh, kind);
    mesh.select_mode_flush();
    if options.use_sharp_edge_from_normals {
        sharpen_from_custom_normals(mesh);
    }
    if counts.changed(mesh) {
        mesh.update_normals();
    }
    Ok(counts.removed(mesh))
}

pub fn auto_merge(
    mesh: &mut EditMesh,
    moved: &IdSet<VertId>,
    options: &AutoMergeOptions,
) -> Result<Removed> {
    if !(options.threshold >= 0.0 && options.threshold.is_finite()) {
        return Err(Error::OutOfRange("threshold"));
    }
    let counts = Counts::new(mesh);
    let had_face_selection = any_face_selected(mesh);
    merge_into_unmoved(mesh, moved, options.threshold, options.use_centroid);
    if had_face_selection && mesh.select_mode() == SelectMode::FACE {
        mesh.select_mode_flush_with(SelectMode::EDGE, false);
    }
    if counts.changed(mesh) {
        mesh.update_normals();
    }
    Ok(counts.removed(mesh))
}

fn merge_into_unmoved(
    mesh: &mut EditMesh,
    moved: &IdSet<VertId>,
    distance: f32,
    use_centroid: bool,
) {
    let verts: Vec<VertId> = mesh.verts().collect();
    let keep: IdSet<VertId> = verts
        .iter()
        .copied()
        .filter(|&vert| !moved.contains(vert) && !mesh.is_vert_hidden(vert))
        .collect();
    let targets = find_doubles(
        mesh,
        &verts,
        &keep,
        &FindDoublesOptions {
            distance,
            use_connected: false,
        },
    );
    mesh.weld_verts(
        &targets,
        &WeldOptions {
            use_centroid,
            average_vert_data: false,
        },
    );
}

fn store_selection_as_tags(mesh: &mut EditMesh, kind: ElementKind) {
    match kind {
        ElementKind::Vert => {
            let verts: Vec<VertId> = mesh.verts().collect();
            for vert in verts {
                if !mesh.is_vert_hidden(vert) {
                    let selected = mesh.is_vert_selected(vert);
                    mesh.set_vert_tag(vert, selected);
                }
            }
        }
        ElementKind::Edge => {
            let edges: Vec<EdgeId> = mesh.edges().collect();
            for edge in edges {
                if !mesh.is_edge_hidden(edge) {
                    let selected = mesh.is_edge_selected(edge);
                    mesh.set_edge_tag(edge, selected);
                }
            }
        }
        ElementKind::Face => {
            let faces: Vec<FaceId> = mesh.faces().collect();
            for face in faces {
                if !mesh.is_face_hidden(face) {
                    let selected = mesh.is_face_selected(face);
                    mesh.set_face_tag(face, selected);
                }
            }
        }
    }
}

fn restore_selection_from_tags(mesh: &mut EditMesh, kind: ElementKind) {
    match kind {
        ElementKind::Vert => {
            let verts: Vec<VertId> = mesh.verts().collect();
            for vert in verts {
                if !mesh.is_vert_hidden(vert) {
                    let tagged = mesh.is_vert_tagged(vert);
                    mesh.select_vert(vert, tagged);
                }
            }
        }
        ElementKind::Edge => {
            let edges: Vec<EdgeId> = mesh.edges().collect();
            for edge in edges {
                if !mesh.is_edge_hidden(edge) {
                    let tagged = mesh.is_edge_tagged(edge);
                    mesh.select_edge(edge, tagged);
                }
            }
        }
        ElementKind::Face => {
            let faces: Vec<FaceId> = mesh.faces().collect();
            for face in faces {
                if !mesh.is_face_hidden(face) {
                    let tagged = mesh.is_face_tagged(face);
                    mesh.select_face(face, tagged);
                }
            }
        }
    }
}

fn sharpen_from_custom_normals(mesh: &mut EditMesh) {
    let mut polymesh = mesh.to_polymesh();
    let Some(normals) = polymesh.custom_normals().map(<[_]>::to_vec) else {
        return;
    };
    polymesh.encode_custom_normals(&normals);
    let Some(sharp) = polymesh.sharp_edges().map(<[bool]>::to_vec) else {
        return;
    };
    let edges: Vec<EdgeId> = mesh.edges().collect();
    for (edge, is_sharp) in edges.into_iter().zip(sharp) {
        if is_sharp && !mesh.is_edge_sharp(edge) {
            mesh.set_edge_sharp(edge, true);
        }
    }
}

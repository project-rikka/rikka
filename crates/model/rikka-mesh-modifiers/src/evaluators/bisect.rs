use glam::Vec3;
use rikka_mesh_edit::{EdgeId, EditMesh, FaceId, LoopId, VertId};
use rikka_polymesh::Polymesh;

use super::editing::Editing;
use crate::note::SkipReason;

#[derive(Clone, Copy, Default)]
struct VertState {
    side: i16,
    skip: bool,
    distance: f32,
    center: bool,
    order: f32,
    loop_index: u32,
}

#[derive(Clone, Copy)]
struct Plane {
    normal: Vec3,
    offset: f32,
}

impl Plane {
    fn side(&self, point: Vec3) -> f32 {
        self.normal.dot(point) + self.offset
    }

    fn closest(&self, point: Vec3) -> Vec3 {
        point - self.normal * (self.side(point) / self.normal.length_squared())
    }
}

struct Bisector {
    states: Vec<VertState>,
    stacked: Vec<bool>,
}

impl Bisector {
    fn state(&mut self, vert: VertId) -> &mut VertState {
        if vert.index() >= self.states.len() {
            self.states.resize(vert.index() + 1, VertState::default());
        }
        &mut self.states[vert.index()]
    }

    fn get(&self, vert: VertId) -> VertState {
        self.states.get(vert.index()).copied().unwrap_or_default()
    }

    fn push_face(&mut self, face: FaceId, stack: &mut Vec<FaceId>) {
        if face.index() >= self.stacked.len() {
            self.stacked.resize(face.index() + 1, false);
        }
        if !self.stacked[face.index()] {
            self.stacked[face.index()] = true;
            stack.push(face);
        }
    }
}

pub(crate) fn bisect(
    mesh: &mut Polymesh,
    point: Vec3,
    normal: Vec3,
    distance: f32,
) -> Result<(), SkipReason> {
    if normal == Vec3::ZERO {
        return Ok(());
    }
    let plane = Plane {
        normal,
        offset: -normal.dot(point),
    };
    let mut editing = Editing::new(mesh)?;
    let edit = &mut editing.mesh;
    bisect_plane(edit, plane, distance).map_err(|_| SkipReason::InvalidMesh)?;
    let kept = Plane {
        normal,
        offset: plane.offset - distance,
    };
    let doomed: Vec<VertId> = edit
        .verts()
        .filter(|&vert| kept.side(edit.position(vert)) > 0.0)
        .collect();
    for vert in doomed {
        if edit.contains_vert(vert) {
            edit.kill_vert(vert).map_err(|_| SkipReason::InvalidMesh)?;
        }
    }
    *mesh = editing.finish();
    Ok(())
}

fn bisect_plane(mesh: &mut EditMesh, plane: Plane, epsilon: f32) -> rikka_mesh_edit::Result<()> {
    let edges: Vec<EdgeId> = mesh.edges().collect();
    let mut bisector = Bisector {
        states: vec![VertState::default(); mesh.vert_capacity()],
        stacked: vec![false; mesh.face_capacity()],
    };
    let mut moved: Vec<FaceId> = Vec::new();
    for vert in mesh.verts().collect::<Vec<VertId>>() {
        let position = mesh.position(vert);
        let side = plane.side(position);
        let state = bisector.state(vert);
        state.center = false;
        state.distance = side;
        state.side = if side <= -epsilon {
            -1
        } else if side >= epsilon {
            1
        } else {
            0
        };
        if state.side == 0 {
            let snapped = plane.closest(position);
            if snapped != position {
                mesh.set_position(vert, snapped);
                for face in mesh.vert_faces(vert).collect::<Vec<FaceId>>() {
                    if !moved.contains(&face) {
                        moved.push(face);
                    }
                }
            }
        }
    }
    for face in moved {
        mesh.update_face_normal(face);
    }
    let mut stack: Vec<FaceId> = Vec::new();
    for edge in edges {
        let [v1, v2] = mesh.edge_verts(edge);
        let (first, second) = (bisector.get(v1), bisector.get(v2));
        let sides = [first.side, second.side];
        if sides[0] != 0 && sides[1] != 0 && sides[0] != sides[1] {
            let factor = first.distance / (first.distance - second.distance);
            for face in mesh.edge_faces(edge).collect::<Vec<FaceId>>() {
                bisector.push_face(face, &mut stack);
            }
            let split = mesh.split_edge(edge, v1, factor)?;
            let state = bisector.state(split.vert);
            *state = VertState {
                center: true,
                ..VertState::default()
            };
        } else if sides[0] == 0 || sides[1] == 0 {
            for (slot, vert) in [v1, v2].into_iter().enumerate() {
                if sides[slot] == 0 && !bisector.get(vert).center {
                    bisector.state(vert).center = true;
                    for lp in mesh.vert_loops(vert).collect::<Vec<LoopId>>() {
                        bisector.push_face(mesh.loop_face(lp), &mut stack);
                    }
                }
            }
        }
    }
    while let Some(face) = stack.pop() {
        bisect_face(mesh, &mut bisector, face, plane)?;
    }
    Ok(())
}

fn bisect_face(
    mesh: &mut EditMesh,
    bisector: &mut Bisector,
    face: FaceId,
    plane: Plane,
) -> rikka_mesh_edit::Result<()> {
    let loops: Vec<LoopId> = mesh.face_loops(face).collect();
    let len = loops.len() as u32;
    let mut splits: Vec<VertId> = Vec::new();
    let mut sides_used = [false; 3];
    let mut center_edge = false;
    for &lp in &loops {
        let vert = mesh.loop_vert(lp);
        let previous = mesh.loop_vert(mesh.loop_prev(lp));
        let next = mesh.loop_vert(mesh.loop_next(lp));
        if bisector.get(vert).center {
            let skip = (bisector.get(previous).side ^ bisector.get(next).side) == 0;
            bisector.state(vert).skip = skip;
            splits.push(vert);
            if !center_edge && bisector.get(previous).center {
                center_edge = true;
            }
        }
        sides_used[(bisector.get(vert).side + 1) as usize] = true;
    }
    if splits.len() <= 1 || !(sides_used[0] && sides_used[2]) {
        return Ok(());
    }
    if splits.len() == 2 {
        let (Some(a), Some(b)) = (
            mesh.face_vert_share_loop(face, splits[0]),
            mesh.face_vert_share_loop(face, splits[1]),
        ) else {
            return Ok(());
        };
        if !mesh.is_loop_adjacent(a, b) {
            mesh.split_face(face, a, b, None, true)?;
        }
        return Ok(());
    }
    if center_edge {
        for (index, &lp) in loops.iter().enumerate() {
            bisector.state(mesh.loop_vert(lp)).loop_index = index as u32;
        }
        let mut first_outside = loops[0];
        while bisector.get(mesh.loop_vert(first_outside)).center {
            first_outside = mesh.loop_next(first_outside);
        }
        let mut lp = first_outside;
        loop {
            let vert = mesh.loop_vert(lp);
            let next_vert = mesh.loop_vert(mesh.loop_next(lp));
            if !bisector.get(vert).skip
                && bisector.get(vert).center
                && bisector.get(next_vert).center
            {
                let previous = mesh.loop_prev(lp);
                let mut after = mesh.loop_next(mesh.loop_next(lp));
                while bisector.get(mesh.loop_vert(after)).center {
                    after = mesh.loop_next(after);
                }
                let previous_side = bisector.get(mesh.loop_vert(previous)).side;
                let after_side = bisector.get(mesh.loop_vert(after)).side;
                if previous_side ^ after_side == 0 {
                    let mut walk = mesh.loop_next(previous);
                    while walk != after {
                        bisector.state(mesh.loop_vert(walk)).skip = true;
                        walk = mesh.loop_next(walk);
                    }
                }
                lp = mesh.loop_prev(after);
            }
            lp = mesh.loop_next(lp);
            if lp == first_outside {
                break;
            }
        }
    }
    let (sort_direction, length) = normalize_length(mesh.face_normal(face).cross(plane.normal));
    if length == 0.0 {
        return Ok(());
    }
    for &vert in &splits {
        let position = mesh.position(vert);
        bisector.state(vert).order = sort_direction.dot(position);
    }
    splits.sort_by(|a, b| {
        bisector
            .get(*a)
            .order
            .partial_cmp(&bisector.get(*b).order)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut faces = vec![face];
    let mut inside = false;
    for pair in splits.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        if center_edge {
            let delta = bisector
                .get(a)
                .loop_index
                .abs_diff(bisector.get(b).loop_index);
            if delta == 1 || delta == len - 1 {
                continue;
            }
        }
        if !bisector.get(a).skip {
            inside = !inside;
        }
        if !inside {
            continue;
        }
        let found = faces.iter().find_map(|&candidate| {
            Some((
                candidate,
                mesh.face_vert_share_loop(candidate, a)?,
                mesh.face_vert_share_loop(candidate, b)?,
            ))
        });
        if let Some((target, la, lb)) = found
            && !mesh.is_loop_adjacent(la, lb)
        {
            let split = mesh.split_face(target, la, lb, None, true)?;
            if split.face != target {
                faces.push(split.face);
            }
        }
    }
    Ok(())
}

fn normalize_length(vector: Vec3) -> (Vec3, f32) {
    let squared = vector.dot(vector);
    match squared > 1.0e-35 {
        true => {
            let length = squared.sqrt();
            (vector * (1.0 / length), length)
        }
        false => (Vec3::ZERO, 0.0),
    }
}

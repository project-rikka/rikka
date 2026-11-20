mod cache;
mod cage;
mod order;
mod refine;
mod resample;
mod stencil;

use std::sync::Arc;

use glam::{Vec2, Vec3};
use rikka_polymesh::{CUSTOM_NORMAL, Domain, Polymesh};

use self::cage::{Around, Cage, UvCage, uv_cage, vertex_cage};
use self::order::{Topology, Tracker};
use self::refine::{
    Extent, has_smooth_corners, limit_after_refine, limit_stencil, refine, refine_with,
    smooth_stencil,
};
use self::stencil::{Blendable, Stencil};
use super::Scope;
use crate::kinds::subdivision::{BoundarySmooth, Subdivision, SubdivisionType};
use crate::modifier::Purpose;
use crate::note::NoteKind;
use crate::origins;
use crate::parallel;

pub(crate) use self::cache::Cached;

const MAX_CORNERS: usize = 1 << 24;
const GRAIN: usize = 1 << 14;

struct Limit {
    steps: Vec<Stencil>,
    rows: Stencil,
}

impl Limit {
    fn new(cage: &Cage, budget: u32) -> Limit {
        let remaining = cage
            .edge_sharpness
            .iter()
            .chain(&cage.point_sharpness)
            .copied()
            .filter(|sharpness| sharpness.is_finite())
            .fold(0.0f32, f32::max);
        let around = Around::new(cage);
        let extra = match has_smooth_corners(cage, &around) {
            true => budget,
            false => (remaining.ceil() as u32).min(budget),
        };
        let points = cage.points;
        let mut steps = Vec::with_capacity(extra as usize);
        let mut refined: Option<(Cage, Around)> = None;
        for _ in 1..extra {
            let (parent, parent_around) = refined
                .as_ref()
                .map_or((cage, &around), |(cage, around)| (cage, around));
            let next = refine_with(parent, parent_around, false, Extent::Around(points));
            steps.push(next.smooth);
            let next_around = Around::new(&next.cage);
            refined = Some((next.cage, next_around));
        }
        let (last, last_around) = refined
            .as_ref()
            .map_or((cage, &around), |(cage, around)| (cage, around));
        let rows = match extra {
            0 => limit_stencil(last, last_around, points),
            _ => {
                steps.push(smooth_stencil(
                    last,
                    last_around,
                    false,
                    Extent::Around(points),
                ));
                limit_after_refine(last, last_around, points)
            }
        };
        Limit { steps, rows }
    }

    fn apply<T: Blendable>(&self, values: &[T]) -> Vec<T> {
        let mut refined: Option<Vec<T>> = None;
        for step in &self.steps {
            refined = Some(step.apply(refined.as_deref().unwrap_or(values)));
        }
        self.rows.apply(refined.as_deref().unwrap_or(values))
    }
}

pub(crate) struct Chain {
    levels: Vec<Stencil>,
    limit: Option<Limit>,
    order: Option<Vec<u32>>,
}

impl Chain {
    fn apply(&self, values: &[Vec3]) -> Vec<Vec3> {
        let mut current: Option<Vec<Vec3>> = None;
        for level in &self.levels {
            current = Some(level.apply(current.as_deref().unwrap_or(values)));
        }
        if let Some(limit) = &self.limit {
            current = Some(limit.apply(current.as_deref().unwrap_or(values)));
        }
        let current = current.unwrap_or_else(|| values.to_vec());
        match &self.order {
            Some(order) => {
                parallel::collect(order.len(), GRAIN, |index| current[order[index] as usize])
            }
            None => current,
        }
    }
}

struct UvLayer {
    name: String,
    cage: UvCage,
}

fn requested_levels(subdivision: &Subdivision, purpose: Purpose) -> u32 {
    match purpose {
        Purpose::Render => subdivision.render_levels,
        Purpose::Viewport | Purpose::EditMode | Purpose::EditCage => subdivision.levels,
    }
    .min(Subdivision::MAX_LEVELS) as u32
}

pub(crate) fn evaluate(
    subdivision: &Subdivision,
    mesh: &mut Polymesh,
    scope: &mut Scope<'_>,
    cache: Option<&mut Option<Arc<Cached>>>,
) {
    let requested = requested_levels(subdivision, scope.purpose);
    if requested == 0 || mesh.face_count() == 0 {
        return;
    }
    let mut levels = requested;
    while levels > 1
        && mesh
            .corner_count()
            .saturating_mul(4usize.saturating_pow(levels))
            > MAX_CORNERS
    {
        levels -= 1;
    }
    if levels < requested {
        scope.note(NoteKind::LevelsReduced {
            requested,
            evaluated: levels,
        });
    }
    let Some(slot) = cache else {
        subdivide(subdivision, mesh, levels);
        return;
    };
    if let Some(cached) = slot
        .as_deref()
        .filter(|cached| cached.matches(subdivision, levels, mesh))
    {
        cached.reuse(mesh);
        return;
    }
    let input = cache::snapshot(mesh);
    let chain = subdivide(subdivision, mesh, levels);
    *slot = Some(Arc::new(Cached::new(
        subdivision.clone(),
        levels,
        input,
        mesh,
        chain,
    )));
}

fn subdivide(subdivision: &Subdivision, mesh: &mut Polymesh, levels: u32) -> Chain {
    mesh.ensure_edges();
    let simple = subdivision.subdivision_type == SubdivisionType::Simple;
    let boundary_corners = subdivision.boundary_smooth == BoundarySmooth::PreserveCorners;
    let limit = !simple && subdivision.use_limit_surface;

    if !subdivision.use_custom_normals {
        mesh.attributes.remove(CUSTOM_NORMAL);
    }
    match subdivision.show_only_control_edges {
        true => origins::mark_control_edges(mesh, |_| true),
        false => origins::clear_control_edges(mesh),
    }
    let mut cage = vertex_cage(mesh, subdivision.use_creases, boundary_corners);
    let mut uv_layers: Vec<UvLayer> = mesh
        .uv_maps
        .names
        .iter()
        .filter_map(|name| {
            let uvs = mesh.uv_map(name)?;
            let mut layer = uv_cage(mesh, uvs, &cage, subdivision.uv_smooth, boundary_corners);
            layer.linear |= simple;
            Some(UvLayer {
                name: name.clone(),
                cage: layer,
            })
        })
        .collect();
    let vertex_creases = mesh.vertex_creases().is_some();
    let mut tracker = (levels >= 2).then(|| Tracker::new(mesh, levels));
    let mut stencils = Vec::with_capacity(levels as usize);

    for _ in 0..levels {
        let refined = refine(&cage, simple, Extent::Full);
        mesh.positions = refined.smooth.apply(&mesh.positions);
        for key in &mut mesh.shape_keys.keys {
            key.positions = refined.smooth.apply(&key.positions);
        }
        resample::interpolate(mesh, Domain::Point, &bilinear(&cage));
        origins::clear(mesh, Domain::Point, cage.points..mesh.point_count());
        if vertex_creases {
            mesh.vertex_creases_mut()[cage.points..].fill(0.0);
        }
        let corner_faces = cage.corner_faces();
        resample::split_edges(mesh, cage.edges.len() * 2 + cage.corners.len());
        resample::repeat_faces(mesh, &corner_faces);
        resample::interpolate(mesh, Domain::Corner, &corner_stencil(&cage, &corner_faces));
        if let Some(normals) = mesh
            .attributes
            .values_mut::<Vec3>(CUSTOM_NORMAL, Domain::Corner)
        {
            parallel::fill(normals, GRAIN, |_, normal| {
                *normal = normal.normalize_or_zero()
            });
        }
        for layer in &mut uv_layers {
            let refined_uv = refine(&layer.cage.cage, layer.cage.linear, Extent::Full);
            layer.cage.values = refined_uv.smooth.apply(&layer.cage.values);
            layer.cage.cage = refined_uv.cage;
        }
        if let Some(tracker) = tracker.as_mut() {
            tracker.refine(&Topology {
                points: cage.points,
                faces: &cage.faces,
                edges: &cage.edges,
            });
        }
        cage = refined.cage;
        stencils.push(refined.smooth);
        mesh.edges.clone_from(&cage.edges);
        mesh.face_offsets.clone_from(&cage.faces);
        mesh.corner_verts.clone_from(&cage.corners);
        mesh.corner_edges.clone_from(&cage.corner_edges);
    }
    let mut limited = None;
    if limit {
        let budget = u32::from(subdivision.quality).saturating_sub(levels);
        let positions = Limit::new(&cage, budget);
        mesh.positions = positions.apply(&mesh.positions);
        for key in &mut mesh.shape_keys.keys {
            key.positions = positions.apply(&key.positions);
        }
        for layer in &mut uv_layers {
            if !layer.cage.linear {
                layer.cage.values = Limit::new(&layer.cage.cage, budget).apply(&layer.cage.values);
            }
        }
        limited = Some(positions);
    }
    for layer in uv_layers {
        let corners = &layer.cage.cage.corners;
        let values = &layer.cage.values;
        if let Some(target) = mesh.uv_map_mut(&layer.name) {
            parallel::fill(target, GRAIN, |corner, uv: &mut Vec2| {
                *uv = values[corners[corner] as usize];
            });
        }
    }
    let order = tracker.as_ref().and_then(|tracker| tracker.reorder(mesh));
    debug_assert!(
        tracker.is_none() || order.is_some(),
        "subdivision result maps onto Blender's layout"
    );
    Chain {
        levels: stencils,
        limit: limited,
        order,
    }
}

pub(crate) fn limit_positions(mesh: &Polymesh, level: u32) -> Vec<Vec3> {
    if mesh.face_count() == 0 {
        return mesh.positions.clone();
    }
    let mut mesh = mesh.clone();
    mesh.ensure_edges();
    let cage = vertex_cage(&mesh, true, false);
    Limit::new(&cage, level).apply(&mesh.positions)
}

fn bilinear(cage: &Cage) -> Stencil {
    let points = cage.points;
    let edges = cage.edges.len();
    Stencil::build(points + edges + cage.face_count(), |row, out| {
        if row < points {
            out.push((row as u32, 1.0));
        } else if row < points + edges {
            let [a, b] = cage.edges[row - points];
            out.extend([(a, 0.5), (b, 0.5)]);
        } else {
            let range = cage.face(row - points - edges);
            let share = 1.0 / range.len() as f32;
            for corner in range {
                out.push((cage.corners[corner], share));
            }
        }
    })
}

fn corner_stencil(cage: &Cage, corner_faces: &[u32]) -> Stencil {
    Stencil::build(cage.corners.len() * 4, |row, out| {
        let corner = row / 4;
        let range = cage.face(corner_faces[corner] as usize);
        let size = range.len();
        let slot = corner - range.start;
        let child = match size {
            4 => (row % 4 + 4 - slot % 4) % 4,
            _ => row % 4,
        };
        let here = corner as u32;
        let next = (range.start + (slot + 1) % size) as u32;
        let previous = (range.start + (slot + size - 1) % size) as u32;
        match child {
            0 => out.push((here, 1.0)),
            1 => out.extend([(here, 0.5), (next, 0.5)]),
            2 => {
                let share = 1.0 / size as f32;
                out.extend((range.start..range.end).map(|corner| (corner as u32, share)));
            }
            _ => out.extend([(previous, 0.5), (here, 0.5)]),
        }
    })
}

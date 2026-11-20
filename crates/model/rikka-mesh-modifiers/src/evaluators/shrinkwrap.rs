use std::f32::consts::SQRT_2;

use glam::{Mat3, Vec2, Vec3};
use rikka_polymesh::Polymesh;
use rikka_spatial::{Candidate, Culling, Ray, closest_point_on_triangle};

use super::vertex_group::Group;
use super::{Scope, limit_positions, target};
use crate::kinds::shrinkwrap::{CullFace, Shrinkwrap, WrapMethod, WrapMode};
use crate::math::{interpolation_weights, manhattan_length, normalize, triangle_normal};
use crate::normals::{NormalsDomain, normals_domain, shading_point_normals};
use crate::note::SkipReason;
use crate::space::SpaceTransform;
use crate::target::Target;

const RAY_DISTANCE: f32 = f32::MAX / 2.0;
const NEWTON_ITERATIONS: usize = 20;

#[derive(Clone, Copy)]
struct Hit {
    triangle: u32,
    point: Vec3,
    normal: Vec3,
    distance: f32,
}

impl Candidate for Hit {
    fn distance(&self) -> f32 {
        self.distance
    }
}

struct Tree<'a> {
    target: &'a Target,
    corner_normals: bool,
}

impl<'a> Tree<'a> {
    fn new(target: &'a Target) -> Tree<'a> {
        Tree {
            target,
            corner_normals: normals_domain(target.mesh()) == NormalsDomain::Corner,
        }
    }

    fn has_faces(&self) -> bool {
        self.target.mesh().face_count() > 0
    }
}

pub(crate) fn evaluate(
    shrinkwrap: &Shrinkwrap,
    mesh: &mut Polymesh,
    scope: &Scope<'_>,
) -> Result<(), SkipReason> {
    let ctx = scope.ctx;
    let target_object = shrinkwrap.target.ok_or(SkipReason::MissingObject)?;
    let target = target(ctx, target_object)?;
    let target_matrix = ctx.matrix(target_object).ok_or(SkipReason::MissingObject)?;
    let auxiliary = match shrinkwrap
        .auxiliary_target
        .filter(|&object| Some(object) != ctx.object())
    {
        Some(object) => Some((
            super::target(ctx, object)?,
            ctx.matrix(object).ok_or(SkipReason::MissingObject)?,
        )),
        None => None,
    };
    let local = ctx.object_matrix();
    let transform = SpaceTransform::new(local, target_matrix);
    let group = Group::find(mesh, &shrinkwrap.vertex_group);
    let weights: Vec<f32> = (0..mesh.point_count())
        .map(|point| group.weight_or_zero(mesh, point, shrinkwrap.invert_vertex_group))
        .collect();
    let tree = Tree::new(&target);
    if target.mesh().point_count() == 0 {
        return Ok(());
    }
    match shrinkwrap.wrap_method {
        WrapMethod::NearestVertex => nearest_vertex(shrinkwrap, mesh, &tree, &transform, &weights),
        WrapMethod::NearestSurfacePoint | WrapMethod::TargetProject => {
            if tree.has_faces() {
                nearest_surface(shrinkwrap, mesh, &tree, &transform, &weights);
            }
        }
        WrapMethod::Project => {
            if tree.has_faces() {
                let auxiliary = auxiliary
                    .as_ref()
                    .filter(|(target, _)| target.mesh().face_count() > 0)
                    .map(|(target, matrix)| {
                        (Tree::new(target), SpaceTransform::new(local, *matrix))
                    });
                project(
                    shrinkwrap,
                    mesh,
                    &tree,
                    &transform,
                    auxiliary.as_ref(),
                    &weights,
                )?;
            }
        }
    }
    Ok(())
}

fn nearest_vertex(
    shrinkwrap: &Shrinkwrap,
    mesh: &mut Polymesh,
    tree: &Tree<'_>,
    transform: &SpaceTransform,
    weights: &[f32],
) {
    let points = tree.target.points();
    for (point, &weight) in weights.iter().enumerate() {
        if weight == 0.0 {
            continue;
        }
        let position = mesh.positions[point];
        let query = transform.apply(position);
        let Some(nearest) = points.nearest(query) else {
            continue;
        };
        let mut weight = weight;
        let distance_squared = nearest.position.distance_squared(query);
        if distance_squared > f32::EPSILON {
            let distance = distance_squared.sqrt();
            weight *= (distance - shrinkwrap.offset) / distance;
        }
        let found = transform.invert(nearest.position);
        mesh.positions[point] = position.lerp(found, weight);
    }
}

fn nearest_surface(
    shrinkwrap: &Shrinkwrap,
    mesh: &mut Polymesh,
    tree: &Tree<'_>,
    transform: &SpaceTransform,
    weights: &[f32],
) {
    for (point, &weight) in weights.iter().enumerate() {
        if weight == 0.0 {
            continue;
        }
        let position = mesh.positions[point];
        let query = transform.apply(position);
        let hit = match shrinkwrap.wrap_method {
            WrapMethod::TargetProject => {
                target_project(tree, query).or_else(|| nearest_point(tree, query))
            }
            _ => nearest_point(tree, query),
        };
        let Some(hit) = hit else {
            continue;
        };
        let snapped = snap(
            tree,
            None,
            shrinkwrap.wrap_mode,
            &hit,
            shrinkwrap.offset,
            query,
        );
        mesh.positions[point] = position.lerp(transform.invert(snapped), weight);
    }
}

fn nearest_point(tree: &Tree<'_>, query: Vec3) -> Option<Hit> {
    let target = tree.target;
    target
        .triangles()
        .bvh()
        .nearest_to_point(query, f32::INFINITY, |triangle, limit| {
            let corners = target.triangle_positions(triangle);
            let closest = closest_point_on_triangle(query, corners);
            let distance = closest.point.distance(query);
            (distance <= limit).then(|| Hit {
                triangle,
                point: closest.point,
                normal: triangle_normal(corners[0], corners[1], corners[2]),
                distance,
            })
        })
}

fn target_project(tree: &Tree<'_>, query: Vec3) -> Option<Hit> {
    let target = tree.target;
    let normals = target.point_normals();
    let boundary = target.boundary();
    target
        .triangles()
        .bvh()
        .nearest_to_point(query, f32::INFINITY, |triangle, limit| {
            let points = target.triangle_points(triangle);
            let corners = points.map(|point| target.mesh().positions[point as usize]);
            let closest = closest_point_on_triangle(query, corners).point;
            let distance_squared = query.distance_squared(closest);
            if distance_squared.sqrt() > limit {
                return None;
            }
            let corner_normals = points.map(|point| normals[point as usize]);
            let mut best: Option<Hit> = None;
            let mut consider = |point: Vec3, normal: Vec3| {
                let distance = point.distance(query);
                if distance <= limit && best.is_none_or(|best| distance < best.distance) {
                    best = Some(Hit {
                        triangle,
                        point,
                        normal: normalize(normal),
                        distance,
                    });
                }
            };
            match solve_triangle(corners, corner_normals, query, closest, distance_squared) {
                Some((point, normal)) => consider(point, normal),
                None => {
                    if boundary.exists() && boundary.triangle_has_boundary[triangle as usize] {
                        for edge in target.real_edges(triangle).into_iter().flatten() {
                            if boundary.edge_is_boundary[edge as usize] {
                                project_edge(target, edge, query, &mut consider);
                            }
                        }
                    }
                }
            }
            best
        })
}

fn solve_triangle(
    corners: [Vec3; 3],
    normals: [Vec3; 3],
    point: Vec3,
    hit: Vec3,
    distance_squared: f32,
) -> Option<(Vec3, Vec3)> {
    let distance = distance_squared.sqrt();
    let magnitude = distance
        + manhattan_length(corners[0])
        + manhattan_length(corners[1])
        + manhattan_length(corners[2]);
    let epsilon = magnitude * 1.0e-6;
    let weights = interpolation_weights(corners, hit);
    let normal = normals[0] * weights.x + normals[1] * weights.y + normals[2] * weights.z;
    let mut x = Vec3::new(
        weights.x,
        weights.y,
        match (point - hit).dot(normal) < 0.0 {
            true => -distance,
            false => distance,
        },
    );
    clamp_barycentric(&mut x);
    let n02 = normals[0] - normals[2];
    let n12 = normals[1] - normals[2];
    let c02 = corners[0] - corners[2];
    let c12 = corners[1] - corners[2];
    let evaluate = |x: Vec3| -> (Vec3, Vec3, Vec3) {
        let w = Vec3::new(x.x, x.y, 1.0 - x.x - x.y);
        let position = corners[0] * w.x + corners[1] * w.y + corners[2] * w.z;
        let normal = normals[0] * w.x + normals[1] * w.y + normals[2] * w.z;
        (position + normal * x.z - point, position, normal)
    };
    let jacobian = |x: Vec3| -> Mat3 {
        Mat3::from_cols(
            c02 + n02 * x.z,
            c12 + n12 * x.z,
            normals[2] + n02 * x.x + n12 * x.y,
        )
    };
    let epsilon = epsilon * epsilon;
    let (mut delta, mut position, mut normal) = evaluate(x);
    let mut delta_squared = delta.length_squared();
    let mut iteration = 0;
    while iteration == 0 || (iteration < NEWTON_ITERATIONS && delta_squared > epsilon) {
        let matrix = jacobian(x);
        if matrix.determinant() == 0.0 {
            return None;
        }
        let mut step = matrix.inverse() * delta;
        let mut next = x - step;
        if !correct_step(x, &mut step, &mut next) {
            return None;
        }
        let (mut next_delta, mut next_position, mut next_normal) = evaluate(next);
        let mut next_squared = next_delta.length_squared();
        while next_squared > delta_squared && next_squared > epsilon {
            let g0 = delta_squared.sqrt();
            let g1 = next_squared.sqrt();
            let g01 = -g0 / step.length();
            let det = 2.0 * (g1 - g0 - g01);
            let factor = match det == 0.0 {
                true => 0.1,
                false => -g01 / det,
            }
            .max(0.1);
            step *= factor;
            next = x - step;
            (next_delta, next_position, next_normal) = evaluate(next);
            next_squared = next_delta.length_squared();
        }
        x = next;
        delta = next_delta;
        position = next_position;
        normal = next_normal;
        delta_squared = next_squared;
        iteration += 1;
    }
    (delta_squared <= epsilon).then_some((position, normal))
}

fn clamp_barycentric(x: &mut Vec3) {
    x.x = x.x.max(0.0);
    x.y = x.y.max(0.0);
    if x.x + x.y > 1.0 {
        x.x /= x.x + x.y;
        x.y = 1.0 - x.x;
    }
}

fn correct_step(x: Vec3, step: &mut Vec3, next: &mut Vec3) -> bool {
    const EPSILON: f32 = 1.0e-5;
    const DIRECTION_EPSILON: f32 = 0.5;
    let mut fixed = false;
    let mut locked = false;
    let sum = x.x + x.y;
    let step_sum = -(step.x + step.y);
    if sum + step_sum > 1.0 {
        let gap = 1.0 - sum;
        if gap < EPSILON * SQRT_2 {
            let length = Vec2::new(step.x, step.y).length();
            if length > EPSILON && step_sum > length * DIRECTION_EPSILON * SQRT_2 {
                return false;
            }
            let shift = (sum + step_sum - 1.0) * 0.5;
            step.x += shift;
            step.y += shift;
            fixed = true;
            locked = true;
        } else {
            *step *= gap / step_sum;
            fixed = true;
        }
    }
    for axis in 0..2 {
        if step[axis] > x[axis] {
            if x[axis] < EPSILON {
                let length = Vec2::new(step.x, step.y).length();
                if length > EPSILON && (locked || step[axis] > length * DIRECTION_EPSILON) {
                    return false;
                }
                step[axis] = x[axis];
                fixed = true;
            } else {
                *step *= x[axis] / step[axis];
                fixed = true;
            }
        }
    }
    if fixed {
        *next = x - *step;
        clamp_barycentric(next);
    }
    true
}

fn project_edge(target: &Target, edge: u32, point: Vec3, consider: &mut impl FnMut(Vec3, Vec3)) {
    let mesh = target.mesh();
    let boundary = target.boundary();
    let [a, b] = mesh.edges[edge as usize];
    let (Some(first), Some(second)) = (
        boundary.vertex_id[a as usize],
        boundary.vertex_id[b as usize],
    ) else {
        return;
    };
    let (v0, v1) = (mesh.positions[a as usize], mesh.positions[b as usize]);
    let first = boundary.vertices[first as usize];
    let second = boundary.vertices[second as usize];
    let along = v1 - v0;
    let d0 = match first.direction.dot(along) < 0.0 {
        true => -first.normal_plane,
        false => first.normal_plane,
    };
    let d1 = match second.direction.dot(along) < 0.0 {
        true => -second.normal_plane,
        false => second.normal_plane,
    };
    let (d0v0, d0v1) = (d0.dot(v0), d0.dot(v1));
    let (d1v0, d1v1) = (d1.dot(v0), d1.dot(v1));
    let d0co = d0.dot(point);
    let a_term = d0v1 - d0v0 + d1v0 - d1v1;
    if a_term == 0.0 {
        return;
    }
    let b_term = 2.0 * d0v0 - d0v1 - d0co - d1v0 + d1.dot(point);
    let c_term = d0co - d0v0;
    let det = b_term * b_term - 4.0 * a_term * c_term;
    if det < 0.0 {
        return;
    }
    let epsilon = 1.0e-6;
    let root = det.sqrt();
    let normals = target.point_normals();
    let signs: &[f32] = match det > 0.0 {
        true => &[1.0, -1.0],
        false => &[-1.0],
    };
    for &sign in signs {
        let x = (-b_term + sign * root) / (2.0 * a_term);
        if (-epsilon..=1.0 + epsilon).contains(&x) {
            let x = x.clamp(0.0, 1.0);
            let position = v0.lerp(v1, x);
            let normal = normals[a as usize].lerp(normals[b as usize], x);
            consider(position, normal);
        }
    }
}

fn project(
    shrinkwrap: &Shrinkwrap,
    mesh: &mut Polymesh,
    tree: &Tree<'_>,
    transform: &SpaceTransform,
    auxiliary: Option<&(Tree<'_>, SpaceTransform)>,
    weights: &[f32],
) -> Result<(), SkipReason> {
    if !shrinkwrap.use_positive_direction && !shrinkwrap.use_negative_direction {
        return Ok(());
    }
    let over_normal = !shrinkwrap.use_project.contains(&true);
    let axis = match over_normal {
        true => Vec3::ZERO,
        false => {
            let axis = normalize(Vec3::new(
                f32::from(u8::from(shrinkwrap.use_project[0])),
                f32::from(u8::from(shrinkwrap.use_project[1])),
                f32::from(u8::from(shrinkwrap.use_project[2])),
            ));
            if axis.length_squared() < f32::EPSILON {
                return Err(SkipReason::NoDirection);
            }
            axis
        }
    };
    let origins: Vec<Vec3> = match (over_normal, shrinkwrap.subsurf_levels) {
        (true, levels) if levels > 0 => limit_positions(mesh, u32::from(levels)),
        _ => mesh.positions.clone(),
    };
    let normals = over_normal.then(|| shading_point_normals(mesh));
    let limit_squared = shrinkwrap.project_limit * shrinkwrap.project_limit;
    let cull = Cull {
        front: shrinkwrap.cull_face == CullFace::Front,
        back: shrinkwrap.cull_face == CullFace::Back,
    };
    let inverted_cull = match shrinkwrap.use_invert_cull && cull.any() {
        true => Cull {
            front: !cull.front,
            back: !cull.back,
        },
        false => cull,
    };
    for (point, &weight) in weights.iter().enumerate() {
        if weight == 0.0 {
            continue;
        }
        let position = mesh.positions[point];
        let (origin, direction) = match &normals {
            Some(normals) => (origins[point], normals[point]),
            None => (position, axis),
        };
        let mut best: Option<(Hit, bool)> = None;
        let mut distance = RAY_DISTANCE;
        let mut cast = |direction: Vec3, cull: Cull| {
            if let Some((aux_tree, aux_transform)) = auxiliary
                && let Some(hit) = project_normal(
                    Cull::NONE,
                    origin,
                    direction,
                    aux_transform,
                    aux_tree,
                    distance,
                )
            {
                distance = hit.distance;
                best = Some((hit, true));
            }
            if let Some(hit) = project_normal(cull, origin, direction, transform, tree, distance) {
                distance = hit.distance;
                best = Some((hit, false));
            }
        };
        if shrinkwrap.use_positive_direction {
            cast(direction, cull);
        }
        if shrinkwrap.use_negative_direction {
            cast(-direction, inverted_cull);
        }
        let Some((hit, is_auxiliary)) = best else {
            continue;
        };
        if limit_squared != 0.0 && hit.point.distance_squared(position) > limit_squared {
            continue;
        }
        let (snap_tree, snap_transform) = match (is_auxiliary, auxiliary) {
            (true, Some((aux_tree, aux_transform))) => (aux_tree, aux_transform),
            _ => (tree, transform),
        };
        let snapped = snap(
            snap_tree,
            Some(snap_transform),
            shrinkwrap.wrap_mode,
            &hit,
            shrinkwrap.offset,
            origin,
        );
        mesh.positions[point] = position.lerp(snapped, weight);
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct Cull {
    front: bool,
    back: bool,
}

impl Cull {
    const NONE: Cull = Cull {
        front: false,
        back: false,
    };

    fn any(self) -> bool {
        self.front || self.back
    }
}

fn project_normal(
    cull: Cull,
    origin: Vec3,
    direction: Vec3,
    transform: &SpaceTransform,
    tree: &Tree<'_>,
    max_distance: f32,
) -> Option<Hit> {
    let co = transform.apply(origin);
    let no = transform.apply_normal(direction);
    let ray = Ray::new(co, no)?;
    let target = tree.target;
    let found = target
        .triangles()
        .ray_cast(&ray, max_distance, Culling::None)?;
    if found.distance >= max_distance {
        return None;
    }
    let corners = target.triangle_positions(found.triangle);
    let normal = transform.invert_normal(triangle_normal(corners[0], corners[1], corners[2]));
    if cull.any() {
        let dot = direction.dot(normal);
        if (cull.front && dot <= 0.0) || (cull.back && dot >= 0.0) {
            return None;
        }
    }
    Some(Hit {
        triangle: found.triangle,
        point: transform.invert(ray.at(found.distance)),
        normal,
        distance: found.distance,
    })
}

fn snap(
    tree: &Tree<'_>,
    transform: Option<&SpaceTransform>,
    mode: WrapMode,
    hit: &Hit,
    goal: f32,
    point: Vec3,
) -> Vec3 {
    match mode {
        WrapMode::OnSurface => match goal != 0.0 {
            true => snap_with_side(point, hit.point, hit.normal, goal, 0.0, true),
            false => hit.point,
        },
        WrapMode::Inside => snap_with_side(point, hit.point, hit.normal, goal, -1.0, false),
        WrapMode::Outside => snap_with_side(point, hit.point, hit.normal, goal, 1.0, false),
        WrapMode::OutsideSurface => match goal != 0.0 {
            true => snap_with_side(point, hit.point, hit.normal, goal, 1.0, true),
            false => hit.point,
        },
        WrapMode::AboveSurface => match goal != 0.0 {
            true => hit.point + smooth_normal(tree, transform, hit) * goal,
            false => hit.point,
        },
    }
}

fn snap_with_side(
    point: Vec3,
    hit: Vec3,
    normal: Vec3,
    goal: f32,
    force_sign: f32,
    force_snap: bool,
) -> Vec3 {
    let delta = point - hit;
    let distance = delta.length();
    if distance < f32::EPSILON {
        return match force_snap || goal > 0.0 {
            true => hit + normal * (goal * force_sign),
            false => hit,
        };
    }
    let side = match delta.dot(normal) < 0.0 {
        true => -1.0,
        false => 1.0,
    };
    let force_sign = match force_sign == 0.0 {
        true => side,
        false => force_sign,
    };
    if force_snap || side * distance * force_sign < goal {
        let mut direction = delta * (side / distance);
        let threshold = (goal.abs() + manhattan_length(hit)) * 1.0e-4;
        if distance < threshold {
            direction = normal.lerp(direction, distance / threshold);
        }
        hit + direction * (goal * force_sign)
    } else {
        point
    }
}

fn smooth_normal(tree: &Tree<'_>, transform: Option<&SpaceTransform>, hit: &Hit) -> Vec3 {
    let target = tree.target;
    let mesh = target.mesh();
    let face = target.triangulation().faces[hit.triangle as usize] as usize;
    let smooth = mesh.sharp_faces().is_none() || mesh.is_sharp_face(face);
    if smooth {
        let corners = target.triangulation().triangles[hit.triangle as usize];
        let points = target.triangle_points(hit.triangle);
        let normals: [Vec3; 3] = match tree.corner_normals {
            true => corners.map(|corner| target.corner_normals()[corner as usize]),
            false => points.map(|point| target.point_normals()[point as usize]),
        };
        let local = transform.map_or(hit.point, |transform| transform.apply(hit.point));
        let weights = interpolation_weights(target.triangle_positions(hit.triangle), local);
        let normal = normals[0] * weights.x + normals[1] * weights.y + normals[2] * weights.z;
        match transform {
            Some(transform) => transform.invert_normal(normal),
            None => normalize(normal),
        }
    } else {
        let normal = target.face_normals()[face];
        match transform {
            Some(transform) => transform.invert_normal(normal),
            None => normal,
        }
    }
}

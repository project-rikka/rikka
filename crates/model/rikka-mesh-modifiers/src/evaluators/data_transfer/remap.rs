use glam::{Vec2, Vec3};
use rikka_polymesh::Polymesh;
use rikka_spatial::{PointTree, TriangleBvh, mean_value_weights};

use super::geometry::{
    Random, dominant_axis_matrix, face_center, fill_polygon_2d, line_point_factor, nearest_point,
    nearest_segment, nearest_triangle, polygon_area_2d, raycast_edges, raycast_triangles,
    signed_area_2d, slerp_safe,
};
use crate::kinds::data_transfer::{CornerMapping, EdgeMapping, FaceMapping, VertexMapping};
use crate::normals::shading_point_normals;
use crate::space::SpaceTransform;
use crate::target::Target;

const APPROXIMATE_RAYS: usize = 3;
const APPROXIMATE_FACTOR: f32 = 5.0;
const TRIANGLE_SAMPLES_MIN: i32 = 4;
const TRIANGLE_SAMPLES_MAX: i32 = 20;
const EDGE_RAYS_MIN: i32 = 5;
const EDGE_RAYS_MAX: i32 = 100;
const UNREACHED_FACTOR: f32 = 1.0e18;

pub(super) type Sources = Vec<(u32, f32)>;

pub(super) struct Query<'a> {
    pub(super) source: &'a Target,
    pub(super) transform: Option<SpaceTransform>,
    pub(super) max_distance: f32,
    pub(super) ray_radius: f32,
}

impl Query<'_> {
    fn point(&self, point: Vec3) -> Vec3 {
        self.transform
            .map_or(point, |transform| transform.apply(point))
    }

    fn normal(&self, normal: Vec3) -> Vec3 {
        self.transform
            .map_or(normal, |transform| transform.apply_normal(normal))
    }

    fn source_mesh(&self) -> &Polymesh {
        self.source.mesh()
    }

    fn face_of_triangle(&self, triangle: u32) -> usize {
        self.source.triangulation().faces[triangle as usize] as usize
    }

    fn approximate_cast<T>(&self, mut cast: impl FnMut(f32) -> Option<T>) -> Option<(T, f32)> {
        let attempts = match self.ray_radius > 0.0 {
            true => APPROXIMATE_RAYS,
            false => 1,
        };
        let mut weight = 1.0f32;
        for _ in 0..attempts {
            if let Some(hit) = cast(self.ray_radius / weight) {
                return Some((hit, weight));
            }
            weight /= APPROXIMATE_FACTOR;
        }
        None
    }
}

fn single(index: u32) -> Sources {
    vec![(index, 1.0)]
}

fn face_weights(mesh: &Polymesh, face: usize, point: Vec3, use_corners: bool) -> Sources {
    let range = mesh.face(face);
    let polygon: Vec<Vec3> = mesh.corner_verts[range.clone()]
        .iter()
        .map(|&vertex| mesh.positions[vertex as usize])
        .collect();
    let mut weights = vec![0.0f32; polygon.len()];
    mean_value_weights(&polygon, point, &mut weights);
    range
        .zip(weights)
        .map(|(corner, weight)| {
            let index = match use_corners {
                true => corner as u32,
                false => mesh.corner_verts[corner],
            };
            (index, weight)
        })
        .collect()
}

fn closest_of_face(mesh: &Polymesh, face: usize, point: Vec3, use_corners: bool) -> u32 {
    let mut best = f32::MAX;
    let mut index = 0;
    for corner in mesh.face(face) {
        let vertex = mesh.corner_verts[corner];
        let distance = point.distance_squared(mesh.positions[vertex as usize]);
        if distance < best {
            best = distance;
            index = match use_corners {
                true => corner as u32,
                false => vertex,
            };
        }
    }
    index
}

pub(super) fn vertices(
    mode: VertexMapping,
    query: &Query<'_>,
    destination: &Polymesh,
) -> Vec<Sources> {
    let count = destination.point_count();
    let source = query.source_mesh();
    let max = query.max_distance;
    match mode {
        VertexMapping::Topology => (0..count as u32).map(single).collect(),
        VertexMapping::Nearest => destination
            .positions
            .iter()
            .map(|&position| {
                nearest_point(query.source.points(), query.point(position), max)
                    .map_or_else(Vec::new, |hit| single(hit.primitive))
            })
            .collect(),
        VertexMapping::EdgeNearest | VertexMapping::EdgeInterpolatedNearest => destination
            .positions
            .iter()
            .map(|&position| {
                let point = query.point(position);
                let Some(hit) = nearest_segment(query.source.segments(), point, max) else {
                    return Vec::new();
                };
                let [a, b] = source.edges[hit.primitive as usize];
                let (first, second) = (source.positions[a as usize], source.positions[b as usize]);
                match mode {
                    VertexMapping::EdgeNearest => {
                        let index =
                            match point.distance_squared(first) > point.distance_squared(second) {
                                true => b,
                                false => a,
                            };
                        single(index)
                    }
                    _ => {
                        let weight = line_point_factor(point, second, first).clamp(0.0, 1.0);
                        vec![(a, weight), (b, 1.0 - weight)]
                    }
                }
            })
            .collect(),
        VertexMapping::FaceInterpolatedNormalProjected => {
            let normals = shading_point_normals(destination);
            destination
                .positions
                .iter()
                .zip(&normals)
                .map(|(&position, &normal)| {
                    let hit = raycast_triangles(
                        query.source.triangles(),
                        query.point(position),
                        query.normal(normal),
                        query.ray_radius,
                        max,
                    );
                    hit.map_or_else(Vec::new, |hit| {
                        face_weights(
                            source,
                            query.face_of_triangle(hit.primitive),
                            hit.point,
                            false,
                        )
                    })
                })
                .collect()
        }
        VertexMapping::FaceNearest | VertexMapping::FaceInterpolatedNearest => destination
            .positions
            .iter()
            .map(|&position| {
                let Some(hit) =
                    nearest_triangle(query.source.triangles(), query.point(position), max)
                else {
                    return Vec::new();
                };
                let face = query.face_of_triangle(hit.primitive);
                match mode {
                    VertexMapping::FaceNearest => {
                        single(closest_of_face(source, face, hit.point, false))
                    }
                    _ => face_weights(source, face, hit.point, false),
                }
            })
            .collect(),
    }
}

pub(super) fn edges(mode: EdgeMapping, query: &Query<'_>, destination: &Polymesh) -> Vec<Sources> {
    let count = destination.edge_count();
    let source = query.source_mesh();
    let max = query.max_distance;
    let midpoint = |edge: [u32; 2]| {
        destination.positions[edge[0] as usize].lerp(destination.positions[edge[1] as usize], 0.5)
    };
    match mode {
        EdgeMapping::Topology => (0..count as u32).map(single).collect(),
        EdgeMapping::VertexNearest => {
            let source_edges = source.point_edges();
            let mut nearest: Vec<Option<Option<(u32, f32)>>> =
                vec![None; destination.point_count()];
            destination
                .edges
                .iter()
                .map(|&edge| {
                    for vertex in [edge[0], edge[1]] {
                        let slot = &mut nearest[vertex as usize];
                        if slot.is_none() {
                            let point = query.point(destination.positions[vertex as usize]);
                            *slot = Some(
                                nearest_point(query.source.points(), point, max)
                                    .map(|hit| (hit.primitive, hit.distance)),
                            );
                        }
                    }
                    let mut best = f32::MAX;
                    let mut best_edge = None;
                    for (vertex, other) in [(edge[0], edge[1]), (edge[1], edge[0])] {
                        let Some(Some((source_vertex, first_distance))) = nearest[vertex as usize]
                        else {
                            continue;
                        };
                        for &candidate in source_edges.get(source_vertex as usize) {
                            let [a, b] = source.edges[candidate as usize];
                            let other_source = match a == source_vertex {
                                true => b,
                                false => a,
                            };
                            let total = first_distance
                                + source.positions[other_source as usize]
                                    .distance(destination.positions[other as usize]);
                            if total < best {
                                best = total;
                                best_edge = Some(candidate);
                            }
                        }
                    }
                    best_edge.map_or_else(Vec::new, single)
                })
                .collect()
        }
        EdgeMapping::Nearest => destination
            .edges
            .iter()
            .map(|&edge| {
                nearest_segment(query.source.segments(), query.point(midpoint(edge)), max)
                    .map_or_else(Vec::new, |hit| single(hit.primitive))
            })
            .collect(),
        EdgeMapping::FaceNearest => destination
            .edges
            .iter()
            .map(|&edge| {
                let point = query.point(midpoint(edge));
                let Some(hit) = nearest_triangle(query.source.triangles(), point, max) else {
                    return Vec::new();
                };
                let face = query.face_of_triangle(hit.primitive);
                let mut best = f32::MAX;
                let mut best_edge = None;
                for corner in source.face(face) {
                    let candidate = source.corner_edges[corner];
                    let [a, b] = source.edges[candidate as usize];
                    let center =
                        source.positions[a as usize].lerp(source.positions[b as usize], 0.5);
                    let distance = point.distance_squared(center);
                    if distance < best {
                        best = distance;
                        best_edge = Some(candidate);
                    }
                }
                best_edge.map_or_else(Vec::new, single)
            })
            .collect(),
        EdgeMapping::EdgeInterpolatedNormalProjected => {
            let normals = shading_point_normals(destination);
            let source_count = source.edge_count();
            let mut weights = vec![0.0f32; source_count];
            destination
                .edges
                .iter()
                .map(|&[a, b]| {
                    let p1 = query.point(destination.positions[a as usize]);
                    let p2 = query.point(destination.positions[b as usize]);
                    let n1 = query.normal(normals[a as usize]);
                    let n2 = query.normal(normals[b as usize]);
                    weights.fill(0.0);
                    let ratio = p1.distance(p2) / query.ray_radius + 0.5;
                    let grid = match ratio.is_finite() && ratio < i32::MAX as f32 {
                        true => ratio as i32,
                        false => i32::MIN,
                    }
                    .clamp(EDGE_RAYS_MIN, EDGE_RAYS_MAX);
                    let step = 1.0 / grid as f32;
                    let mut total = 0.0f32;
                    for ray in 0..grid {
                        let factor = step * ray as f32;
                        let origin = p1.lerp(p2, factor);
                        let normal = slerp_safe(n1, n2, factor);
                        if let Some((hit, weight)) = query.approximate_cast(|radius| {
                            raycast_edges(query.source, origin, normal, radius, max)
                        }) {
                            weights[hit.primitive as usize] += weight;
                            total += weight;
                        }
                    }
                    if total > grid as f32 / 2.0 {
                        weights
                            .iter()
                            .enumerate()
                            .filter(|&(_, &weight)| weight != 0.0)
                            .map(|(edge, &weight)| (edge as u32, weight / total))
                            .collect()
                    } else {
                        Vec::new()
                    }
                })
                .collect()
        }
    }
}

struct Island {
    points: Option<PointTree>,
    triangles: Option<(TriangleBvh, Vec<u32>)>,
}

#[derive(Clone, Copy)]
struct IslandResult {
    factor: f32,
    index: Option<u32>,
    point: Vec3,
}

impl IslandResult {
    const MISSED: IslandResult = IslandResult {
        factor: 0.0,
        index: None,
        point: Vec3::ZERO,
    };
}

pub(super) fn seam_islands(mesh: &Polymesh) -> Option<Vec<u32>> {
    if mesh.face_count() == 0 {
        return None;
    }
    let edge_faces = mesh.edge_faces();
    let mut group = vec![u32::MAX; mesh.face_count()];
    let mut count = 0u32;
    let mut stack = Vec::new();
    for start in 0..mesh.face_count() {
        if group[start] != u32::MAX {
            continue;
        }
        group[start] = count;
        stack.push(start);
        while let Some(face) = stack.pop() {
            for corner in mesh.face(face) {
                let edge = mesh.corner_edges[corner] as usize;
                if mesh.is_uv_seam(edge) {
                    continue;
                }
                for &other in edge_faces.get(edge) {
                    if group[other as usize] == u32::MAX {
                        group[other as usize] = count;
                        stack.push(other as usize);
                    }
                }
            }
        }
        count += 1;
    }
    Some(group)
}

pub(super) fn corners(
    mode: CornerMapping,
    query: &Query<'_>,
    destination: &Polymesh,
    use_islands: bool,
) -> Vec<Sources> {
    let count = destination.corner_count();
    if mode == CornerMapping::Topology {
        return (0..count as u32).map(single).collect();
    }
    let source = query.source_mesh();
    let max = query.max_distance;
    let from_vertex = matches!(
        mode,
        CornerMapping::NearestCornerNormal | CornerMapping::NearestFaceNormal
    );
    let face_normals_destination = destination.face_normals();
    let corner_normals_destination = destination.corner_normals();
    let face_normals_source = source.face_normals();
    let corner_normals_source = match mode {
        CornerMapping::NearestCornerNormal => source.corner_normals(),
        _ => Vec::new(),
    };
    let source_point_corners = source.point_corners();
    let source_point_faces = source.point_faces();
    let source_corner_faces = source.corner_faces();
    let source_centers: Vec<Vec3> = (0..source.face_count())
        .map(|face| face_center(&source.positions, source.face_verts(face)))
        .collect();

    let groups = use_islands.then(|| seam_islands(source)).flatten();
    let islands: Vec<Island> = match &groups {
        Some(groups) => {
            let island_count = groups.iter().copied().max().map_or(0, |last| last + 1);
            let mut faces_of: Vec<Vec<u32>> = vec![Vec::new(); island_count as usize];
            for (face, &group) in groups.iter().enumerate() {
                faces_of[group as usize].push(face as u32);
            }
            faces_of
                .into_iter()
                .map(|faces| island(query, &faces, from_vertex))
                .collect()
        }
        None => vec![Island {
            points: None,
            triangles: None,
        }],
    };
    let island_of_face = |face: usize| groups.as_ref().map(|groups| groups[face]);

    let mut map: Vec<Sources> = vec![Vec::new(); count];
    let mut results: Vec<Vec<IslandResult>> = vec![Vec::new(); islands.len()];
    for (face, &normal) in face_normals_destination.iter().enumerate() {
        let range = destination.face(face);
        let size = range.len();
        let face_normal = query.normal(normal);
        let mut center: Option<Vec3> = None;
        for (island_index, island) in islands.iter().enumerate() {
            let result = &mut results[island_index];
            result.clear();
            for corner in range.clone() {
                let vertex = destination.corner_verts[corner] as usize;
                let point = query.point(destination.positions[vertex]);
                let entry = if from_vertex {
                    let hit = match &island.points {
                        Some(tree) => nearest_point(tree, point, max),
                        None => nearest_point(query.source.points(), point, max),
                    };
                    match hit {
                        None => IslandResult::MISSED,
                        Some(hit) => {
                            let (normal, candidates, normals) = match mode {
                                CornerMapping::NearestCornerNormal => (
                                    query.normal(corner_normals_destination[corner]),
                                    source_point_corners.get(hit.primitive as usize),
                                    &corner_normals_source,
                                ),
                                _ => (
                                    face_normal,
                                    source_point_faces.get(hit.primitive as usize),
                                    &face_normals_source,
                                ),
                            };
                            let mut best_dot = -2.0f32;
                            let mut best_fallback = f32::MAX;
                            let mut best = None;
                            for &candidate in candidates {
                                let dot = normals[candidate as usize].dot(normal);
                                let source_face = match mode {
                                    CornerMapping::NearestCornerNormal => {
                                        source_corner_faces[candidate as usize] as usize
                                    }
                                    _ => candidate as usize,
                                };
                                if groups.is_some()
                                    && island_of_face(source_face) != Some(island_index as u32)
                                {
                                    continue;
                                }
                                if dot > best_dot - 1.0e-6 {
                                    let center_destination = *center.get_or_insert_with(|| {
                                        face_center(
                                            &destination.positions,
                                            destination.face_verts(face),
                                        )
                                    });
                                    let distance = center_destination
                                        .distance_squared(source_centers[source_face]);
                                    if dot > best_dot + 1.0e-6 || distance < best_fallback {
                                        best_dot = dot;
                                        best_fallback = distance;
                                        best = Some(candidate);
                                    }
                                }
                            }
                            let mut distance = hit.distance;
                            let index = match best {
                                None => {
                                    best_dot = -1.0;
                                    distance = f32::MAX;
                                    None
                                }
                                Some(candidate) => Some(match mode {
                                    CornerMapping::NearestCornerNormal => candidate,
                                    _ => source
                                        .face(candidate as usize)
                                        .find(|&corner| {
                                            source.corner_verts[corner] == hit.primitive
                                        })
                                        .map_or(candidate, |corner| corner as u32),
                                }),
                            };
                            let best_dot = (best_dot + 1.0) * 0.5;
                            IslandResult {
                                factor: match distance != 0.0 {
                                    true => best_dot / distance,
                                    false => UNREACHED_FACTOR,
                                },
                                index,
                                point: hit.point,
                            }
                        }
                    }
                } else if mode == CornerMapping::FaceInterpolatedNormalProjected {
                    let normal = query.normal(corner_normals_destination[corner]);
                    let cast = query.approximate_cast(|radius| match &island.triangles {
                        Some((tree, global)) => raycast_triangles(tree, point, normal, radius, max)
                            .map(|mut hit| {
                                hit.primitive = global[hit.primitive as usize];
                                hit
                            }),
                        None => {
                            raycast_triangles(query.source.triangles(), point, normal, radius, max)
                        }
                    });
                    match cast {
                        Some((hit, weight)) => IslandResult {
                            factor: match hit.distance != 0.0 {
                                true => 1.0 / hit.distance,
                                false => UNREACHED_FACTOR,
                            } * weight,
                            index: Some(query.face_of_triangle(hit.primitive) as u32),
                            point: hit.point,
                        },
                        None => match nearest_in(query, island, point, max) {
                            Some((triangle, hit_point, _)) => IslandResult {
                                factor: 0.0,
                                index: Some(query.face_of_triangle(triangle) as u32),
                                point: hit_point,
                            },
                            None => IslandResult::MISSED,
                        },
                    }
                } else {
                    match nearest_in(query, island, point, max) {
                        Some((triangle, hit_point, distance)) => IslandResult {
                            factor: match distance != 0.0 {
                                true => 1.0 / distance,
                                false => UNREACHED_FACTOR,
                            },
                            index: Some(query.face_of_triangle(triangle) as u32),
                            point: hit_point,
                        },
                        None => IslandResult::MISSED,
                    }
                };
                result.push(entry);
            }
        }
        let mut best_island = None;
        let mut best_factor = 0.0f32;
        for (island_index, result) in results.iter().enumerate() {
            let factor = result.iter().map(|entry| entry.factor).sum::<f32>() / size as f32;
            if factor > best_factor {
                best_factor = factor;
                best_island = Some(island_index);
            }
        }
        let Some(best_island) = best_island else {
            continue;
        };
        for (slot, corner) in range.enumerate() {
            let entry = results[best_island][slot];
            let Some(index) = entry.index else {
                continue;
            };
            map[corner] = match (from_vertex, mode) {
                (true, _) => single(index),
                (false, CornerMapping::FaceNearest) => {
                    single(closest_of_face(source, index as usize, entry.point, true))
                }
                (false, _) => face_weights(source, index as usize, entry.point, true),
            };
        }
    }
    map
}

fn island(query: &Query<'_>, faces: &[u32], from_vertex: bool) -> Island {
    let source = query.source_mesh();
    match from_vertex {
        true => {
            let mut active = vec![false; source.point_count()];
            for &face in faces {
                for &point in source.face_verts(face as usize) {
                    active[point as usize] = true;
                }
            }
            let tree = PointTree::from_indexed(
                active
                    .iter()
                    .enumerate()
                    .filter(|&(_, &used)| used)
                    .map(|(point, _)| (point as u32, source.positions[point])),
            );
            Island {
                points: Some(tree),
                triangles: None,
            }
        }
        false => {
            let mut in_island = vec![false; source.face_count()];
            for &face in faces {
                in_island[face as usize] = true;
            }
            let triangulation = query.source.triangulation();
            let global: Vec<u32> = (0..triangulation.triangles.len() as u32)
                .filter(|&triangle| in_island[triangulation.faces[triangle as usize] as usize])
                .collect();
            let indices = global
                .iter()
                .map(|&triangle| query.source.triangle_points(triangle))
                .collect();
            let tree = TriangleBvh::new(source.positions.clone(), indices)
                .expect("a validated mesh names only existing points");
            Island {
                points: None,
                triangles: Some((tree, global)),
            }
        }
    }
}

fn nearest_in(
    query: &Query<'_>,
    island: &Island,
    point: Vec3,
    max: f32,
) -> Option<(u32, Vec3, f32)> {
    match &island.triangles {
        Some((tree, global)) => nearest_triangle(tree, point, max)
            .map(|hit| (global[hit.primitive as usize], hit.point, hit.distance)),
        None => nearest_triangle(query.source.triangles(), point, max)
            .map(|hit| (hit.primitive, hit.point, hit.distance)),
    }
}

pub(super) fn faces(mode: FaceMapping, query: &Query<'_>, destination: &Polymesh) -> Vec<Sources> {
    let count = destination.face_count();
    let max = query.max_distance;
    let normals = destination.face_normals();
    let center = |face: usize| face_center(&destination.positions, destination.face_verts(face));
    match mode {
        FaceMapping::Topology => (0..count as u32).map(single).collect(),
        FaceMapping::Nearest => (0..count)
            .map(|face| {
                nearest_triangle(query.source.triangles(), query.point(center(face)), max)
                    .map_or_else(Vec::new, |hit| {
                        single(query.face_of_triangle(hit.primitive) as u32)
                    })
            })
            .collect(),
        FaceMapping::Normal => (0..count)
            .map(|face| {
                raycast_triangles(
                    query.source.triangles(),
                    query.point(center(face)),
                    query.normal(normals[face]),
                    query.ray_radius,
                    max,
                )
                .map_or_else(Vec::new, |hit| {
                    single(query.face_of_triangle(hit.primitive) as u32)
                })
            })
            .collect(),
        FaceMapping::FaceInterpolatedNormalProjected => {
            let source_faces = query.source_mesh().face_count();
            let mut random = Random::new(0);
            let mut weights = vec![0.0f32; source_faces];
            (0..count)
                .map(|face| {
                    weights.fill(0.0);
                    let normal = query.normal(normals[face]);
                    let to_plane = dominant_axis_matrix(normal);
                    let from_plane = to_plane.inverse();
                    let depth = (to_plane * query.point(center(face))).z;
                    let flat: Vec<Vec2> = destination
                        .face_verts(face)
                        .iter()
                        .map(|&point| {
                            (to_plane * query.point(destination.positions[point as usize]))
                                .truncate()
                        })
                        .collect();
                    let (low, high) = flat.iter().fold(
                        (Vec2::splat(f32::MAX), Vec2::splat(-f32::MAX)),
                        |(low, high), &point| (low.min(point), high.max(point)),
                    );
                    let size = high - low;
                    let side = match query.ray_radius != 0.0 {
                        true => (((size.x.max(size.y)) / query.ray_radius + 0.5) as i32)
                            .clamp(TRIANGLE_SAMPLES_MIN, TRIANGLE_SAMPLES_MAX),
                        false => TRIANGLE_SAMPLES_MIN,
                    };
                    let total_rays = side * side;
                    let area_inverse = 1.0 / polygon_area_2d(&flat).max(1.0e-9);
                    let mut done_area = 0.0f32;
                    let mut done_rays = 0i32;
                    let mut total = 0.0f32;
                    for triangle in fill_polygon_2d(&flat) {
                        let [a, b, c] = triangle.map(|index| flat[index as usize]);
                        done_area += signed_area_2d(a, b, c).abs();
                        let wanted = total_rays as f32 * done_area * area_inverse + 0.5;
                        let target = match wanted.is_finite() && wanted < i32::MAX as f32 {
                            true => wanted as i32,
                            false => i32::MIN,
                        };
                        let rays = target
                            .saturating_sub(done_rays)
                            .clamp(0, (total_rays - done_rays).max(0));
                        done_rays += rays;
                        for _ in 0..rays {
                            let sample = random.triangle_sample(a, b, c);
                            let origin = from_plane * sample.extend(depth);
                            if let Some((hit, weight)) = query.approximate_cast(|radius| {
                                raycast_triangles(
                                    query.source.triangles(),
                                    origin,
                                    normal,
                                    radius,
                                    max,
                                )
                            }) {
                                weights[query.face_of_triangle(hit.primitive)] += weight;
                                total += weight;
                            }
                        }
                    }
                    if total > 0.0 {
                        weights
                            .iter()
                            .enumerate()
                            .filter(|&(_, &weight)| weight != 0.0)
                            .map(|(face, &weight)| (face as u32, weight / total))
                            .collect()
                    } else {
                        Vec::new()
                    }
                })
                .collect()
        }
    }
}

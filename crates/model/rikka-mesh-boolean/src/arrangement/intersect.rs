use std::cmp::Ordering;
use std::num::NonZeroUsize;

use glam::Affine3A;
use rikka_spatial::{Aabb, Bvh};
use rustc_hash::FxHashMap;
use smallvec::SmallVec;

use crate::arrangement::kernel::{FaceKernel, Support, VertexRef};
use crate::arrangement::{Context, Triangle};
use crate::cdt::EdgeOrigin;
use crate::cdt::triangulation::{Triangulation, edge_key};
use crate::exact::{Point, VertexTable};
use crate::parallel::map_chunks;

const PAIRS_PER_TASK: usize = 512;

enum PointRef {
    Table(u32),
    New(Point),
}

enum Hit {
    None,
    Coplanar,
    Point(PointRef),
    Segment(PointRef, PointRef),
}

#[derive(Default)]
struct Constraints {
    points: Vec<u32>,
    segments: Vec<(u32, u32, u32)>,
}

enum Job {
    Single(u32),
    Cluster(Vec<u32>),
}

struct Piece {
    triangle: u32,
    verts: [VertexRef; 3],
    edges: [Option<u32>; 3],
    intersect: [bool; 3],
    reversed: bool,
}

struct JobOutput {
    created: Vec<Point>,
    pieces: Vec<Piece>,
}

pub(crate) fn intersect(context: &mut Context, triangles: &[Triangle]) -> Vec<Triangle> {
    let pairs = candidate_pairs(context, triangles);
    let threads = context.settings.threads;
    let hits: Vec<Hit> = {
        let context = &*context;
        map_chunks(pairs.len(), threads, PAIRS_PER_TASK, |range| {
            range
                .map(|index| {
                    let (a, b) = pairs[index];
                    intersect_pair(context, &triangles[a as usize], &triangles[b as usize])
                })
                .collect()
        })
    };
    let mut constraints: Vec<Constraints> = (0..triangles.len())
        .map(|_| Constraints::default())
        .collect();
    let mut parent: Vec<u32> = (0..triangles.len() as u32).collect();
    for (&(a, b), hit) in pairs.iter().zip(hits) {
        let mut register = |point: PointRef| match point {
            PointRef::Table(id) => id,
            PointRef::New(point) => context.table.find_or_insert(point),
        };
        match hit {
            Hit::None => {}
            Hit::Coplanar => union(&mut parent, a, b),
            Hit::Point(point) => {
                let id = register(point);
                constraints[a as usize].points.push(id);
                constraints[b as usize].points.push(id);
            }
            Hit::Segment(first, second) => {
                let (p, q) = (register(first), register(second));
                if p == q {
                    constraints[a as usize].points.push(p);
                    constraints[b as usize].points.push(p);
                } else {
                    constraints[a as usize].segments.push((p, q, b));
                    constraints[b as usize].segments.push((p, q, a));
                }
            }
        }
    }
    let mut clusters: FxHashMap<u32, Vec<u32>> = FxHashMap::default();
    for t in 0..triangles.len() as u32 {
        let root = find(&mut parent, t);
        clusters.entry(root).or_default().push(t);
    }
    let mut in_cluster = vec![false; triangles.len()];
    let mut jobs: Vec<(u32, Job)> = Vec::new();
    for members in clusters.into_values() {
        if members.len() > 1 {
            for &member in &members {
                in_cluster[member as usize] = true;
            }
            jobs.push((members[0], Job::Cluster(members)));
        }
    }
    for (t, list) in constraints.iter().enumerate() {
        if !in_cluster[t] && (!list.points.is_empty() || !list.segments.is_empty()) {
            jobs.push((t as u32, Job::Single(t as u32)));
        }
    }
    jobs.sort_by_key(|(first, _)| *first);
    let outputs: Vec<JobOutput> = {
        let table = &context.table;
        let constraints = &constraints;
        map_chunks(jobs.len(), threads, 1, |range| {
            range
                .map(|index| match &jobs[index].1 {
                    Job::Single(t) => run_job(table, triangles, constraints, &[*t]),
                    Job::Cluster(members) => run_job(table, triangles, constraints, members),
                })
                .collect()
        })
    };
    let mut replaced: Vec<Option<Vec<Triangle>>> = vec![None; triangles.len()];
    for ((_, job), output) in jobs.iter().zip(outputs) {
        match job {
            Job::Single(t) => replaced[*t as usize] = Some(Vec::new()),
            Job::Cluster(members) => {
                for &member in members {
                    replaced[member as usize] = Some(Vec::new());
                }
            }
        }
        let ids: Vec<u32> = output
            .created
            .into_iter()
            .map(|point| context.table.find_or_insert(point))
            .collect();
        for piece in output.pieces {
            let verts = piece.verts.map(|vertex| match vertex {
                VertexRef::Table(id) => id,
                VertexRef::New(index) => ids[index as usize],
            });
            let source = &triangles[piece.triangle as usize];
            let triangle = Triangle {
                verts,
                face: source.face,
                edges: piece.edges,
                intersect: piece.intersect,
                flipped: false,
            };
            let triangle = match piece.reversed {
                true => Triangle {
                    flipped: false,
                    ..triangle.flip()
                },
                false => triangle,
            };
            if let Some(list) = replaced[piece.triangle as usize].as_mut() {
                list.push(triangle);
            }
        }
    }
    let mut result = Vec::with_capacity(triangles.len());
    for (t, triangle) in triangles.iter().enumerate() {
        match replaced[t].take() {
            Some(list) => result.extend(list),
            None => result.push(*triangle),
        }
    }
    result
}

fn find(parent: &mut [u32], mut x: u32) -> u32 {
    while parent[x as usize] != x {
        parent[x as usize] = parent[parent[x as usize] as usize];
        x = parent[x as usize];
    }
    x
}

fn union(parent: &mut [u32], a: u32, b: u32) {
    let (ra, rb) = (find(parent, a), find(parent, b));
    if ra != rb {
        parent[ra.max(rb) as usize] = ra.min(rb);
    }
}

fn triangle_bounds(context: &Context, triangle: &Triangle) -> Aabb {
    Aabb::from_triangle(triangle.verts.map(|v| context.point(v).approx.as_vec3()))
}

fn candidate_pairs(context: &Context, triangles: &[Triangle]) -> Vec<(u32, u32)> {
    let threads = NonZeroUsize::new(context.settings.threads).unwrap_or(NonZeroUsize::MIN);
    let use_self = context.settings.use_self;
    let mut pairs: Vec<(u32, u32)> = Vec::new();
    let canonical = |a: u32, b: u32| (a.min(b), a.max(b));
    if context.shapes == 2 && !use_self {
        let mut groups: [Vec<u32>; 2] = [Vec::new(), Vec::new()];
        for (index, triangle) in triangles.iter().enumerate() {
            groups[context.shape(triangle).min(1) as usize].push(index as u32);
        }
        let bounds = |group: &[u32]| -> Vec<Aabb> {
            group
                .iter()
                .map(|&t| triangle_bounds(context, &triangles[t as usize]))
                .collect()
        };
        let first = Bvh::builder().threads(threads).build(&bounds(&groups[0]));
        let second = Bvh::builder().threads(threads).build(&bounds(&groups[1]));
        first.for_each_overlap(&second, &Affine3A::IDENTITY, |i, j| {
            pairs.push(canonical(groups[0][i as usize], groups[1][j as usize]));
        });
    } else {
        let bounds: Vec<Aabb> = triangles
            .iter()
            .map(|triangle| triangle_bounds(context, triangle))
            .collect();
        let tree = Bvh::builder().threads(threads).build(&bounds);
        tree.for_each_self_overlap(|i, j| {
            let (a, b) = (&triangles[i as usize], &triangles[j as usize]);
            let different_shape = context.shape(a) != context.shape(b);
            if different_shape || (use_self && a.face != b.face) {
                pairs.push(canonical(i, j));
            }
        });
    }
    pairs.sort_unstable();
    pairs.dedup();
    pairs
}

fn point_of<'a>(context: &'a Context, point: &'a PointRef) -> &'a Point {
    match point {
        PointRef::Table(id) => context.point(*id),
        PointRef::New(point) => point,
    }
}

fn section(
    context: &Context,
    verts: [u32; 3],
    signs: [i32; 3],
    plane: [&Point; 3],
) -> SmallVec<[PointRef; 2]> {
    let mut points: SmallVec<[PointRef; 2]> = SmallVec::new();
    for i in 0..3 {
        if signs[i] == 0 {
            points.push(PointRef::Table(verts[i]));
        }
    }
    for i in 0..3 {
        let j = (i + 1) % 3;
        if signs[i] * signs[j] < 0 {
            let point =
                context
                    .space()
                    .lpi(context.point(verts[i]), context.point(verts[j]), plane);
            points.push(PointRef::New(point));
        }
    }
    points
}

fn ordered(
    context: &Context,
    axis: usize,
    mut points: SmallVec<[PointRef; 2]>,
) -> (PointRef, PointRef) {
    let second = points.pop().expect("section has a point");
    let first = points.pop();
    match first {
        None => {
            let copy = match &second {
                PointRef::Table(id) => PointRef::Table(*id),
                PointRef::New(point) => PointRef::New(point.clone()),
            };
            (second, copy)
        }
        Some(first) => {
            let order = context.space().compare(
                axis,
                point_of(context, &first),
                point_of(context, &second),
            );
            match order == Ordering::Greater {
                true => (second, first),
                false => (first, second),
            }
        }
    }
}

fn intersect_pair(context: &Context, ta: &Triangle, tb: &Triangle) -> Hit {
    let space = context.space();
    let a = ta.verts.map(|v| context.point(v));
    let b = tb.verts.map(|v| context.point(v));
    let side =
        |s: [i32; 3]| (s[0] > 0 && s[1] > 0 && s[2] > 0) || (s[0] < 0 && s[1] < 0 && s[2] < 0);
    let sa = a.map(|p| space.orient3d(b[0], b[1], b[2], p));
    if side(sa) {
        return Hit::None;
    }
    let sb = b.map(|p| space.orient3d(a[0], a[1], a[2], p));
    if side(sb) {
        return Hit::None;
    }
    if sa == [0, 0, 0] {
        return Hit::Coplanar;
    }
    let axis = space.plane_line_axis(a, b);
    let (a_low, a_high) = ordered(context, axis, section(context, ta.verts, sa, b));
    let (b_low, b_high) = ordered(context, axis, section(context, tb.verts, sb, a));
    let compare = |x: &PointRef, y: &PointRef| {
        space.compare(axis, point_of(context, x), point_of(context, y))
    };
    let low = match compare(&a_low, &b_low) == Ordering::Less {
        true => b_low,
        false => a_low,
    };
    let high = match compare(&a_high, &b_high) == Ordering::Greater {
        true => b_high,
        false => a_high,
    };
    match compare(&low, &high) {
        Ordering::Greater => Hit::None,
        Ordering::Equal => Hit::Point(low),
        Ordering::Less => Hit::Segment(low, high),
    }
}

fn run_job(
    table: &VertexTable,
    triangles: &[Triangle],
    constraints: &[Constraints],
    members: &[u32],
) -> JobOutput {
    let plane = triangles[members[0] as usize].verts;
    let mut kernel = FaceKernel::new(table, triangles, plane);
    let mut local: FxHashMap<u32, u32> = FxHashMap::default();
    let mut add = |kernel: &mut FaceKernel, id: u32| -> u32 {
        *local
            .entry(id)
            .or_insert_with(|| kernel.add(VertexRef::Table(id)))
    };
    let reversed: Vec<bool> = members
        .iter()
        .map(|&m| members.len() > 1 && kernel.is_clockwise(triangles[m as usize].verts))
        .collect();
    let mut faces: Vec<[u32; 3]> = Vec::with_capacity(members.len());
    for (index, &member) in members.iter().enumerate() {
        let [v0, v1, v2] = triangles[member as usize].verts;
        let order = match reversed[index] {
            true => [v0, v2, v1],
            false => [v0, v1, v2],
        };
        faces.push(order.map(|v| add(&mut kernel, v)));
    }
    let mut segments: Vec<(u32, u32, u32)> = Vec::new();
    for &member in members {
        let list = &constraints[member as usize];
        for &point in &list.points {
            add(&mut kernel, point);
        }
        for &(p, q, other) in &list.segments {
            segments.push((add(&mut kernel, p), add(&mut kernel, q), other));
        }
    }
    let count = kernel.vertices.len() as u32;
    let mut triangulation = Triangulation::new(kernel);
    let order: Vec<u32> = (0..count).collect();
    triangulation.build(&order);
    for (index, face) in faces.iter().enumerate() {
        for position in 0..3 {
            let (a, b) = (face[position], face[(position + 1) % 3]);
            let table_ids = |v: u32| match triangulation.kernel.vertices[v as usize] {
                VertexRef::Table(id) => id,
                VertexRef::New(_) => unreachable!("face corners are table vertices"),
            };
            let support = Support::Edge(table_ids(a), table_ids(b));
            let origin = EdgeOrigin::FaceEdge {
                face: index as u32,
                position: position as u32,
            };
            triangulation.insert_segment(a, b, support, origin);
        }
    }
    for &(p, q, other) in &segments {
        triangulation.insert_segment(p, q, Support::Plane(other), EdgeOrigin::Edge(other));
    }
    let membership = triangulation.face_membership();
    let mut pieces = Vec::new();
    let edge_info = |a: u32, b: u32| -> (Option<u32>, bool) {
        let Some(info) = triangulation.constraints.get(&edge_key(a, b)) else {
            return (None, false);
        };
        let mut orig = None;
        let mut other = false;
        for origin in &info.origins {
            match origin.kind {
                EdgeOrigin::FaceEdge { face, position } => {
                    if orig.is_none() {
                        let member = members[face as usize];
                        let slot = match reversed[face as usize] {
                            true => [2, 1, 0][position as usize],
                            false => position as usize,
                        };
                        orig = triangles[member as usize].edges[slot];
                    }
                }
                EdgeOrigin::Edge(_) => other = true,
            }
        }
        (orig, orig.is_none() && other)
    };
    for tri in triangulation.real_triangles() {
        let Some(set) = membership.get(&tri) else {
            continue;
        };
        let v = triangulation.tris[tri as usize].v;
        let info: [(Option<u32>, bool); 3] =
            std::array::from_fn(|i| edge_info(v[i], v[(i + 1) % 3]));
        for &face in set {
            pieces.push(Piece {
                triangle: members[face as usize],
                verts: v.map(|x| triangulation.kernel.vertices[x as usize]),
                edges: info.map(|(orig, _)| orig),
                intersect: info.map(|(_, intersect)| intersect),
                reversed: reversed[face as usize],
            });
        }
    }
    JobOutput {
        created: std::mem::take(&mut triangulation.kernel.created),
        pieces,
    }
}

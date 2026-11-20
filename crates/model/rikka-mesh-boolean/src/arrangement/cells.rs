use std::cmp::Ordering;
use std::collections::BTreeMap;

use glam::DVec3;

use crate::arrangement::rational::{self, Closest, Rational};
use crate::arrangement::topology::{Edge, Topology, edge};
use crate::arrangement::{Context, Triangle, raycast};
use crate::operation::Operation;

const NONE: u32 = u32::MAX;
const EXTRA: u32 = u32::MAX - 1;
const COMPONENT_PAD: f64 = 20.0 * f32::EPSILON as f64;

pub(crate) struct Patch {
    pub(crate) tris: Vec<u32>,
    above: u32,
    below: u32,
    component: u32,
}

pub(crate) struct Patches {
    pub(crate) list: Vec<Patch>,
    tri_patch: Vec<u32>,
    pair_edges: BTreeMap<(u32, u32), Edge>,
}

#[derive(Default)]
struct Cell {
    patches: Vec<u32>,
    winding: Vec<i32>,
    merged_to: Option<u32>,
    assigned: bool,
    in_output: bool,
    zero_volume: bool,
}

impl Cell {
    fn add_patch(&mut self, patch: u32) {
        if let Err(position) = self.patches.binary_search(&patch) {
            self.patches.insert(position, patch);
        }
        self.zero_volume = false;
    }
}

struct Geometry<'c, 'a> {
    context: &'c mut Context<'a>,
    triangles: &'c [Triangle],
    topology: Topology,
    extra: [u32; 3],
}

impl Geometry<'_, '_> {
    fn verts(&self, tri: u32) -> [u32; 3] {
        match tri == EXTRA {
            true => self.extra,
            false => self.triangles[tri as usize].verts,
        }
    }

    fn flap(&self, tri: u32, key: Edge) -> (u32, bool) {
        let v = self.verts(tri);
        for i in 0..3 {
            let (a, b, c) = (v[i], v[(i + 1) % 3], v[(i + 2) % 3]);
            if a == key.0 && b == key.1 {
                return (c, false);
            }
            if a == key.1 && b == key.0 {
                return (c, true);
            }
        }
        (v[0], false)
    }

    fn class(&self, tri: u32, reference: u32, key: Edge) -> usize {
        let (flap_reference, reversed_reference) = self.flap(reference, key);
        let (flap, _) = self.flap(tri, key);
        let r = self.verts(reference).map(|v| self.context.point(v));
        let orientation = self
            .context
            .space()
            .orient3d(r[0], r[1], r[2], self.context.point(flap));
        match orientation.cmp(&0) {
            Ordering::Greater => match reversed_reference {
                true => 4,
                false => 3,
            },
            Ordering::Less => match reversed_reference {
                true => 3,
                false => 4,
            },
            Ordering::Equal => match flap == flap_reference {
                true => 1,
                false => 2,
            },
        }
    }

    fn sort_signed(&self, group: &mut [u32], key: Edge) {
        let mut signed: Vec<i64> = group
            .iter()
            .map(|&tri| {
                let (_, reversed) = self.flap(tri, key);
                match reversed {
                    true => -i64::from(tri),
                    false => i64::from(tri),
                }
            })
            .collect();
        signed.sort_unstable();
        for (slot, value) in group.iter_mut().zip(signed) {
            *slot = value.unsigned_abs() as u32;
        }
    }

    fn sort_around(&self, key: Edge, tris: &[u32], first: u32) -> Vec<u32> {
        if tris.is_empty() {
            return Vec::new();
        }
        let reference = tris[0];
        let mut groups: [Vec<u32>; 4] = [vec![reference], Vec::new(), Vec::new(), Vec::new()];
        for &tri in &tris[1..] {
            groups[self.class(tri, reference, key) - 1].push(tri);
        }
        for index in [0, 1] {
            if groups[index].len() > 1 {
                let mut group = std::mem::take(&mut groups[index]);
                self.sort_signed(&mut group, key);
                groups[index] = group;
            }
        }
        for index in [2, 3] {
            if groups[index].len() > 1 {
                groups[index] = self.sort_around(key, &groups[index], first);
            }
        }
        let order = match reference == first {
            true => [0, 3, 1, 2],
            false => [2, 0, 3, 1],
        };
        order
            .iter()
            .flat_map(|&g| groups[g].iter().copied())
            .collect()
    }
}

fn resolve(cells: &[Cell], mut cell: u32) -> u32 {
    while let Some(next) = cells[cell as usize].merged_to {
        cell = next;
    }
    cell
}

fn merge_cells(cells: &mut [Cell], patches: &mut Patches, to: u32, from: u32) {
    let to = resolve(cells, to);
    let from = resolve(cells, from);
    if to == from {
        return;
    }
    let moved = std::mem::take(&mut cells[from as usize].patches);
    for patch in moved {
        cells[to as usize].add_patch(patch);
        let record = &mut patches.list[patch as usize];
        if record.above == from {
            record.above = to;
        }
        if record.below == from {
            record.below = to;
        }
    }
    cells[from as usize].merged_to = Some(to);
}

fn same_verts(a: [u32; 3], b: [u32; 3]) -> bool {
    let mut x = a;
    let mut y = b;
    x.sort_unstable();
    y.sort_unstable();
    x == y
}

fn check_zero_volume(cells: &mut [Cell], cell: u32, patches: &Patches, triangles: &[Triangle]) {
    let record = &cells[cell as usize];
    if record.patches.len() != 2 {
        return;
    }
    let (p, q) = (
        &patches.list[record.patches[0] as usize],
        &patches.list[record.patches[1] as usize],
    );
    if p.tris.len() == 1
        && q.tris.len() == 1
        && same_verts(
            triangles[p.tris[0] as usize].verts,
            triangles[q.tris[0] as usize].verts,
        )
    {
        cells[cell as usize].zero_volume = true;
    }
}

pub(crate) fn find_patches(triangles: &[Triangle], topology: &Topology) -> Patches {
    let mut patches = Patches {
        list: Vec::new(),
        tri_patch: vec![NONE; triangles.len()],
        pair_edges: BTreeMap::new(),
    };
    let mut stack: Vec<u32> = Vec::new();
    for start in 0..triangles.len() as u32 {
        if patches.tri_patch[start as usize] != NONE {
            continue;
        }
        let current = patches.list.len() as u32;
        patches.list.push(Patch {
            tris: Vec::new(),
            above: NONE,
            below: NONE,
            component: NONE,
        });
        stack.push(start);
        while let Some(tri) = stack.pop() {
            if patches.tri_patch[tri as usize] != NONE {
                continue;
            }
            patches.tri_patch[tri as usize] = current;
            patches.list[current as usize].tris.push(tri);
            let verts = triangles[tri as usize].verts;
            for i in 0..3 {
                let key = edge(verts[i], verts[(i + 1) % 3]);
                match topology.other_if_manifold(key, tri) {
                    Some(other) => {
                        if patches.tri_patch[other as usize] == NONE {
                            stack.push(other);
                        }
                    }
                    None => {
                        for &other in topology.edge_tris(key) {
                            let other_patch = patches.tri_patch[other as usize];
                            if other == tri || other_patch == NONE || other_patch == current {
                                continue;
                            }
                            patches
                                .pair_edges
                                .entry((current, other_patch))
                                .or_insert(key);
                            patches
                                .pair_edges
                                .entry((other_patch, current))
                                .or_insert(key);
                        }
                    }
                }
            }
        }
    }
    patches
}

fn find_cells_from_edge(
    geometry: &Geometry,
    patches: &mut Patches,
    cells: &mut Vec<Cell>,
    key: Edge,
) {
    let tris = geometry.topology.edge_tris(key).to_vec();
    let sorted = geometry.sort_around(key, &tris, tris[0]);
    let count = sorted.len();
    for i in 0..count {
        let next = (i + 1) % count;
        let r = patches.tri_patch[sorted[i] as usize];
        let r_next = patches.tri_patch[sorted[next] as usize];
        let (_, r_flipped) = geometry.flap(sorted[i], key);
        let (_, next_flipped) = geometry.flap(sorted[next], key);
        let follow = match r_flipped {
            true => patches.list[r as usize].below,
            false => patches.list[r as usize].above,
        };
        let previous = match next_flipped {
            true => patches.list[r_next as usize].above,
            false => patches.list[r_next as usize].below,
        };
        let set_follow = |patches: &mut Patches, cell: u32| match r_flipped {
            true => patches.list[r as usize].below = cell,
            false => patches.list[r as usize].above = cell,
        };
        let set_previous = |patches: &mut Patches, cell: u32| match next_flipped {
            true => patches.list[r_next as usize].above = cell,
            false => patches.list[r_next as usize].below = cell,
        };
        match (follow == NONE, previous == NONE) {
            (true, true) => {
                let cell = cells.len() as u32;
                cells.push(Cell::default());
                set_follow(patches, cell);
                set_previous(patches, cell);
                cells[cell as usize].add_patch(r);
                cells[cell as usize].add_patch(r_next);
                check_zero_volume(cells, cell, patches, geometry.triangles);
            }
            (false, true) => {
                let cell = resolve(cells, follow);
                set_previous(patches, cell);
                cells[cell as usize].add_patch(r_next);
                check_zero_volume(cells, cell, patches, geometry.triangles);
            }
            (true, false) => {
                let cell = resolve(cells, previous);
                set_follow(patches, cell);
                cells[cell as usize].add_patch(r);
                check_zero_volume(cells, cell, patches, geometry.triangles);
            }
            (false, false) => {
                let (follow, previous) = (resolve(cells, follow), resolve(cells, previous));
                if follow != previous {
                    match cells[follow as usize].patches.len()
                        >= cells[previous as usize].patches.len()
                    {
                        true => merge_cells(cells, patches, follow, previous),
                        false => merge_cells(cells, patches, previous, follow),
                    }
                }
            }
        }
    }
}

fn find_cells(geometry: &Geometry, patches: &mut Patches) -> Vec<Cell> {
    let mut cells: Vec<Cell> = Vec::new();
    let mut edges: Vec<Edge> = patches
        .pair_edges
        .iter()
        .filter(|((p, q), _)| p < q)
        .map(|(_, &key)| key)
        .collect();
    edges.sort_unstable();
    edges.dedup();
    for key in edges {
        find_cells_from_edge(geometry, patches, &mut cells, key);
    }
    for patch in 0..patches.list.len() {
        if patches.list[patch].above == NONE {
            let cell = cells.len() as u32;
            cells.push(Cell::default());
            cells[cell as usize].add_patch(patch as u32);
            patches.list[patch].above = cell;
        }
        if patches.list[patch].below == NONE {
            let cell = cells.len() as u32;
            cells.push(Cell::default());
            cells[cell as usize].add_patch(patch as u32);
            patches.list[patch].below = cell;
        }
    }
    cells
}

fn patch_components(cells: &[Cell], patches: &mut Patches) -> Vec<Vec<u32>> {
    let mut components: Vec<Vec<u32>> = Vec::new();
    let mut processed = vec![false; cells.len()];
    for start in 0..patches.list.len() as u32 {
        if patches.list[start as usize].component != NONE {
            continue;
        }
        let component = components.len() as u32;
        let mut members = vec![start];
        patches.list[start as usize].component = component;
        let mut stack = vec![start];
        while let Some(patch) = stack.pop() {
            let (above, below) = (
                patches.list[patch as usize].above,
                patches.list[patch as usize].below,
            );
            for cell in [above, below] {
                let cell = resolve(cells, cell);
                if processed[cell as usize] {
                    continue;
                }
                processed[cell as usize] = true;
                for &neighbor in &cells[cell as usize].patches {
                    if patches.list[neighbor as usize].component == NONE {
                        patches.list[neighbor as usize].component = component;
                        stack.push(neighbor);
                        members.push(neighbor);
                    }
                }
            }
        }
        components.push(members);
    }
    components
}

fn find_cell_for_point_near_edge(
    geometry: &mut Geometry,
    patches: &Patches,
    cells: &[Cell],
    point: u32,
    key: Edge,
) -> u32 {
    geometry.extra = [key.0, key.1, point];
    let mut tris = geometry.topology.edge_tris(key).to_vec();
    tris.push(EXTRA);
    let sorted = geometry.sort_around(key, &tris, tris[0]);
    let position = sorted.iter().position(|&tri| tri == EXTRA).unwrap_or(0);
    let previous = sorted[(position + sorted.len() - 1) % sorted.len()];
    let patch = &patches.list[patches.tri_patch[previous as usize] as usize];
    let (_, flipped) = geometry.flap(previous, key);
    resolve(
        cells,
        match flipped {
            true => patch.below,
            false => patch.above,
        },
    )
}

fn find_ambient_cell(
    geometry: &mut Geometry,
    patches: &Patches,
    cells: &[Cell],
    component: Option<&[u32]>,
) -> Option<u32> {
    let tris: Vec<u32> = match component {
        Some(list) => list
            .iter()
            .flat_map(|&patch| patches.list[patch as usize].tris.iter().copied())
            .collect(),
        None => (0..geometry.triangles.len() as u32).collect(),
    };
    let first = *tris.first()?;
    let space = *geometry.context.space();
    let mut extreme = geometry.triangles[first as usize].verts[0];
    for &tri in &tris {
        for v in geometry.triangles[tri as usize].verts {
            if space.compare(
                0,
                geometry.context.point(v),
                geometry.context.point(extreme),
            ) == Ordering::Greater
            {
                extreme = v;
            }
        }
    }
    let allowed: Vec<bool> = match component {
        Some(_) => {
            let mut flags = vec![false; geometry.triangles.len()];
            for &tri in &tris {
                flags[tri as usize] = true;
            }
            flags
        }
        None => vec![true; geometry.triangles.len()],
    };
    let edges = geometry
        .topology
        .vert_edges(geometry.triangles, extreme, |tri| allowed[tri as usize]);
    let origin = space.coords(geometry.context.point(extreme));
    let mut best: Option<(Edge, crate::exact::int::Int, crate::exact::int::Int)> = None;
    for key in edges {
        let other = if key.0 == extreme { key.1 } else { key.0 };
        let target = space.coords(geometry.context.point(other));
        let dx = &(&target[0] * &origin[3]) - &(&origin[0] * &target[3]);
        let dy = &(&target[1] * &origin[3]) - &(&origin[1] * &target[3]);
        if dx.is_zero() {
            best = Some((key, dx, dy));
            break;
        }
        let abs = |value: crate::exact::int::Int| match value.is_negative() {
            true => -value,
            false => value,
        };
        let (dx, dy) = (abs(dx), abs(dy));
        let better = match &best {
            None => true,
            Some((_, best_dx, best_dy)) => (&dy * best_dx) > (best_dy * &dx),
        };
        if better {
            best = Some((key, dx, dy));
        }
    }
    let (key, _, _) = best?;
    let outside = space.offset_x(geometry.context.point(extreme), 1);
    let point = geometry.context.table.find_or_insert(outside);
    Some(find_cell_for_point_near_edge(
        geometry, patches, cells, point, key,
    ))
}

fn find_good_sorting_edge(
    geometry: &Geometry,
    patches: &Patches,
    test: u32,
    closest: u32,
    component: u32,
) -> Option<Edge> {
    let space = *geometry.context.space();
    let closest_vector = rational::vector(&space, geometry.context.point(closest));
    let test_vector = rational::vector(&space, geometry.context.point(test));
    let abscissa = rational::sub(&test_vector, &closest_vector);
    let axis = (0..3).find(|&k| !abscissa[k].is_zero())?;
    let next = (axis + 1) % 3;
    let mut ordinate = [Rational::zero(), Rational::zero(), Rational::zero()];
    ordinate[axis] = abscissa[next].clone();
    ordinate[next] = &Rational::zero() - &abscissa[axis];
    let normal = rational::cross(&abscissa, &ordinate);
    let length = rational::dot(&normal, &normal);
    let mut best: Option<(Edge, Rational)> = None;
    let in_component =
        |tri: u32| patches.list[patches.tri_patch[tri as usize] as usize].component == component;
    for key in geometry
        .topology
        .vert_edges(geometry.triangles, closest, in_component)
    {
        let other = if key.0 == closest { key.1 } else { key.0 };
        let vector = rational::sub(
            &rational::vector(&space, geometry.context.point(other)),
            &closest_vector,
        );
        let along = &rational::dot(&vector, &normal) / &length;
        let projected = rational::sub(&vector, &rational::scale(&normal, &along));
        let a = rational::dot(&projected, &abscissa);
        let o = rational::dot(&projected, &ordinate);
        if a.is_zero() {
            return Some(key);
        }
        let slope = (&o / &a).abs();
        if best.as_ref().is_none_or(|(_, value)| slope > *value) {
            best = Some((key, slope));
        }
    }
    best.map(|(key, _)| key)
}

fn find_containing_cell(
    geometry: &mut Geometry,
    patches: &Patches,
    cells: &[Cell],
    vertex: u32,
    tri: u32,
    closest: Closest,
) -> Option<u32> {
    let verts = geometry.triangles[tri as usize].verts;
    match closest {
        Closest::Interior => {
            let space = *geometry.context.space();
            let p = verts.map(|v| geometry.context.point(v));
            let orientation = space.orient3d(p[0], p[1], p[2], geometry.context.point(vertex));
            let patch = &patches.list[patches.tri_patch[tri as usize] as usize];
            Some(resolve(
                cells,
                match orientation > 0 {
                    true => patch.below,
                    false => patch.above,
                },
            ))
        }
        Closest::Edge(index) => {
            let key = edge(verts[index], verts[(index + 1) % 3]);
            Some(find_cell_for_point_near_edge(
                geometry, patches, cells, vertex, key,
            ))
        }
        Closest::Vertex(index) => {
            let mut corner = verts[index];
            if corner == vertex {
                corner = verts[(index + 1) % 3];
            }
            let component = patches.list[patches.tri_patch[tri as usize] as usize].component;
            let key = find_good_sorting_edge(geometry, patches, vertex, corner, component)?;
            Some(find_cell_for_point_near_edge(
                geometry, patches, cells, vertex, key,
            ))
        }
    }
}

struct Container {
    cell: u32,
    distance: Rational,
}

fn component_bounds(
    geometry: &Geometry,
    patches: &Patches,
    components: &[Vec<u32>],
) -> Vec<(DVec3, DVec3)> {
    let mut maximum = 0.0f64;
    let mut bounds: Vec<(DVec3, DVec3)> = components
        .iter()
        .map(|component| {
            let mut low = DVec3::splat(f64::INFINITY);
            let mut high = DVec3::splat(f64::NEG_INFINITY);
            for &patch in component {
                for &tri in &patches.list[patch as usize].tris {
                    for v in geometry.triangles[tri as usize].verts {
                        let point = geometry.context.point(v).approx;
                        low = low.min(point);
                        high = high.max(point);
                        maximum = maximum.max(point.abs().max_element());
                    }
                }
            }
            (low, high)
        })
        .collect();
    let pad = match maximum == 0.0 {
        true => COMPONENT_PAD,
        false => COMPONENT_PAD * maximum,
    };
    for (low, high) in &mut bounds {
        *low -= DVec3::splat(pad);
        *high += DVec3::splat(pad);
    }
    bounds
}

fn closest_float(p: DVec3, a: DVec3, b: DVec3, c: DVec3) -> f64 {
    let closest = rikka_spatial::closest_point_on_triangle(
        p.as_vec3(),
        [a.as_vec3(), b.as_vec3(), c.as_vec3()],
    );
    f64::from(closest.point.distance_squared(p.as_vec3()))
}

fn component_containers(
    geometry: &mut Geometry,
    patches: &Patches,
    cells: &[Cell],
    components: &[Vec<u32>],
    ambient: &[Option<u32>],
    bounds: &[(DVec3, DVec3)],
    component: usize,
) -> Vec<Container> {
    let test_tri = patches.list[components[component][0] as usize].tris[0];
    let test_vertex = geometry.triangles[test_tri as usize].verts[0];
    let test_float = geometry.context.point(test_vertex).approx;
    let space = *geometry.context.space();
    let test_exact = rational::vector(&space, geometry.context.point(test_vertex));
    let mut containers = Vec::new();
    for other in 0..components.len() {
        if other == component {
            continue;
        }
        let (low, high) = bounds[component];
        let (other_low, other_high) = bounds[other];
        if low.cmpgt(other_high).any() || other_low.cmpgt(high).any() {
            continue;
        }
        let mut nearest: Option<(u32, Rational, Closest, f64)> = None;
        for &patch in &components[other] {
            for &tri in &patches.list[patch as usize].tris {
                let verts = geometry.triangles[tri as usize].verts;
                let [a, b, c] = verts.map(|v| geometry.context.point(v).approx);
                let float_distance = closest_float(test_float, a, b, c);
                if let Some((_, _, _, best_float)) = &nearest
                    && float_distance - f64::from(f32::EPSILON) > *best_float
                {
                    continue;
                }
                let [ea, eb, ec] =
                    verts.map(|v| rational::vector(&space, geometry.context.point(v)));
                let (distance, closest) = rational::closest_on_triangle(&test_exact, &ea, &eb, &ec);
                if nearest
                    .as_ref()
                    .is_none_or(|(_, best, _, _)| distance < *best)
                {
                    nearest = Some((tri, distance, closest, float_distance));
                }
            }
        }
        let Some((tri, distance, closest, _)) = nearest else {
            continue;
        };
        let Some(cell) = find_containing_cell(geometry, patches, cells, test_vertex, tri, closest)
        else {
            continue;
        };
        if Some(cell) != ambient[other] {
            containers.push(Container { cell, distance });
        }
    }
    containers
}

fn finish_graph(geometry: &mut Geometry, patches: &mut Patches, cells: &mut [Cell]) {
    let components = patch_components(cells, patches);
    if components.len() <= 1 {
        return;
    }
    let ambient: Vec<Option<u32>> = components
        .iter()
        .map(|component| find_ambient_cell(geometry, patches, cells, Some(component)))
        .collect();
    let bounds = component_bounds(geometry, patches, &components);
    let containers: Vec<Vec<Container>> = (0..components.len())
        .map(|component| {
            component_containers(
                geometry,
                patches,
                cells,
                &components,
                &ambient,
                &bounds,
                component,
            )
        })
        .collect();
    let mut outer: Vec<usize> = Vec::new();
    for (component, list) in containers.iter().enumerate() {
        let Some(closest) = list.iter().min_by(|a, b| a.distance.cmp(&b.distance)) else {
            outer.push(component);
            continue;
        };
        if let Some(cell) = ambient[component] {
            merge_cells(cells, patches, closest.cell, cell);
        }
    }
    if let Some((&first, rest)) = outer.split_first()
        && let Some(target) = ambient[first]
    {
        for &component in rest {
            if let Some(cell) = ambient[component] {
                merge_cells(cells, patches, target, cell);
            }
        }
    }
}

fn graph_ok(cells: &[Cell], patches: &Patches) -> bool {
    cells
        .iter()
        .all(|cell| cell.merged_to.is_some() || !cell.patches.is_empty())
        && patches
            .list
            .iter()
            .all(|patch| patch.above != NONE && patch.below != NONE)
}

fn propagate(
    cells: &mut [Cell],
    patches: &Patches,
    triangles: &[Triangle],
    context: &Context,
    ambient: u32,
    operation: Operation,
) {
    let shapes = context.shapes;
    for cell in cells.iter_mut() {
        cell.winding = vec![0; shapes];
    }
    cells[ambient as usize].assigned = true;
    let mut queue = vec![ambient];
    let mut head = 0;
    while head < queue.len() {
        let cell = queue[head];
        head += 1;
        let list = cells[cell as usize].patches.clone();
        for patch in list {
            let record = &patches.list[patch as usize];
            let (above, below) = (resolve(cells, record.above), resolve(cells, record.below));
            let patch_above_cell = below == cell;
            let neighbor = match patch_above_cell {
                true => above,
                false => below,
            };
            if cells[neighbor as usize].assigned {
                continue;
            }
            let delta = match patch_above_cell {
                true => -1,
                false => 1,
            };
            let shape = context.shape(&triangles[record.tris[0] as usize]) as usize;
            let mut winding = cells[cell as usize].winding.clone();
            if shape < winding.len() {
                winding[shape] += delta;
            }
            let target = &mut cells[neighbor as usize];
            target.in_output = operation.keeps(&winding);
            target.winding = winding;
            target.assigned = true;
            queue.push(neighbor);
        }
    }
}

fn extract_zero_volume(
    cells: &[Cell],
    patches: &Patches,
    triangles: &[Triangle],
    output: &mut Vec<Triangle>,
) {
    let adjacent: Vec<bool> = patches
        .list
        .iter()
        .map(|patch| {
            cells[resolve(cells, patch.above) as usize].zero_volume
                || cells[resolve(cells, patch.below) as usize].zero_volume
        })
        .collect();
    let mut allocated = vec![false; patches.list.len()];
    for start in 0..patches.list.len() {
        if !adjacent[start] || allocated[start] {
            continue;
        }
        let mut stack = vec![start as u32];
        let mut flipped = vec![false];
        allocated[start] = true;
        let walk = |direction_above: bool,
                    stack: &mut Vec<u32>,
                    flipped: &mut Vec<bool>,
                    allocated: &mut Vec<bool>|
         -> u32 {
            let mut current = start as u32;
            let record = &patches.list[current as usize];
            let mut cell = resolve(
                cells,
                if direction_above {
                    record.above
                } else {
                    record.below
                },
            );
            while cells[cell as usize].zero_volume {
                let members = &cells[cell as usize].patches;
                let Some(&other) = members.iter().find(|&&p| p != current) else {
                    break;
                };
                if allocated[other as usize] {
                    break;
                }
                let other_record = &patches.list[other as usize];
                let flip = match direction_above {
                    true => resolve(cells, other_record.above) == cell,
                    false => resolve(cells, other_record.below) == cell,
                };
                flipped.push(flip);
                stack.push(other);
                allocated[other as usize] = true;
                current = other;
                let next = match (direction_above, flip) {
                    (true, true) | (false, false) => other_record.below,
                    (true, false) | (false, true) => other_record.above,
                };
                cell = resolve(cells, next);
            }
            cell
        };
        let above = walk(true, &mut stack, &mut flipped, &mut allocated);
        let below = walk(false, &mut stack, &mut flipped, &mut allocated);
        let (above_in, below_in) = (
            cells[above as usize].in_output,
            cells[below as usize].in_output,
        );
        if above_in == below_in {
            continue;
        }
        let need_flipped = above_in;
        match stack
            .iter()
            .zip(&flipped)
            .find(|&(_, &flip)| flip == need_flipped)
        {
            Some((&patch, _)) => {
                output.push(triangles[patches.list[patch as usize].tris[0] as usize])
            }
            None => output.push(triangles[patches.list[start].tris[0] as usize].flip()),
        }
    }
}

fn extract(cells: &[Cell], patches: &Patches, triangles: &[Triangle]) -> Vec<Triangle> {
    let mut output = Vec::with_capacity(triangles.len());
    let mut any_zero_volume = false;
    for (index, triangle) in triangles.iter().enumerate() {
        let patch = &patches.list[patches.tri_patch[index] as usize];
        let above = &cells[resolve(cells, patch.above) as usize];
        let below = &cells[resolve(cells, patch.below) as usize];
        let zero = above.zero_volume || below.zero_volume;
        any_zero_volume |= zero;
        if above.in_output != below.in_output && !zero {
            output.push(match above.in_output {
                true => triangle.flip(),
                false => *triangle,
            });
        }
    }
    if any_zero_volume {
        extract_zero_volume(cells, patches, triangles, &mut output);
    }
    output
}

pub(crate) fn classify(
    context: &mut Context,
    triangles: &[Triangle],
    operation: Operation,
) -> Vec<Triangle> {
    if triangles.is_empty() {
        return Vec::new();
    }
    let topology = Topology::new(triangles);
    if !topology.is_pwn(triangles) {
        let patches = match context.settings.hole_tolerant {
            true => None,
            false => Some(find_patches(triangles, &topology)),
        };
        return raycast::classify(context, triangles, operation, patches.as_ref());
    }
    let mut patches = find_patches(triangles, &topology);
    let mut geometry = Geometry {
        context,
        triangles,
        topology,
        extra: [0; 3],
    };
    let mut cells = find_cells(&geometry, &mut patches);
    finish_graph(&mut geometry, &mut patches, &mut cells);
    if !graph_ok(&cells, &patches) {
        return triangles.to_vec();
    }
    let Some(ambient) = find_ambient_cell(&mut geometry, &patches, &cells, None) else {
        return triangles.to_vec();
    };
    let ambient = resolve(&cells, ambient);
    propagate(
        &mut cells,
        &patches,
        triangles,
        geometry.context,
        ambient,
        operation,
    );
    extract(&cells, &patches, triangles)
}

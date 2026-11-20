use rustc_hash::FxHashMap;

use crate::domain::Domain;
use crate::polymesh::Polymesh;
use crate::topology::GroupedIndices;
use crate::validate::ordered;

const COLLAPSED: u32 = u32::MAX;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum MergeData {
    #[default]
    Mix,
    KeepTarget,
}

#[derive(Clone, Copy)]
struct WeldCorner {
    corner: u32,
    point: u32,
    edge: u32,
}

struct WeldFace {
    origin: u32,
    touched: bool,
    start: u32,
    len: u32,
}

struct WeldFaces {
    faces: Vec<WeldFace>,
    corners: Vec<WeldCorner>,
}

impl WeldFaces {
    fn corners(&self, face: &WeldFace) -> &[WeldCorner] {
        &self.corners[face.start as usize..(face.start + face.len) as usize]
    }

    fn push(&mut self, origin: u32, touched: bool, corners: impl IntoIterator<Item = WeldCorner>) {
        let start = self.corners.len() as u32;
        self.corners.extend(corners);
        self.faces.push(WeldFace {
            origin,
            touched,
            start,
            len: self.corners.len() as u32 - start,
        });
    }
}

impl Polymesh {
    pub fn merge_points(&mut self, targets: &[u32], data: MergeData) {
        let points = self.point_count();
        let roots = resolve(targets, points);
        let in_context: Vec<bool> = {
            let mut flags = vec![false; points];
            for (point, &root) in roots.iter().enumerate() {
                if root as usize != point {
                    flags[point] = true;
                    flags[root as usize] = true;
                }
            }
            flags
        };
        if !in_context.contains(&true) {
            return;
        }

        let mut point_map = vec![0u32; points];
        let mut survivors: Vec<u32> = Vec::new();
        for point in 0..points {
            if roots[point] as usize == point {
                point_map[point] = survivors.len() as u32;
                survivors.push(point as u32);
            }
        }
        for point in 0..points {
            point_map[point] = point_map[roots[point] as usize];
        }

        let (edge_dest, remaining_context_edges) = self.weld_edges(&roots, &in_context);
        let mut edge_map = vec![COLLAPSED; self.edge_count()];
        let mut edge_survivors: Vec<u32> = Vec::new();
        for edge in 0..self.edge_count() {
            if edge_dest[edge] == edge as u32 {
                edge_map[edge] = edge_survivors.len() as u32;
                edge_survivors.push(edge as u32);
            }
        }
        for edge in 0..self.edge_count() {
            if edge_dest[edge] != COLLAPSED {
                edge_map[edge] = edge_map[edge_dest[edge] as usize];
            }
        }

        let faces = self.weld_faces(&roots, &in_context, &edge_dest, remaining_context_edges);

        let point_groups = match data {
            MergeData::Mix => GroupedIndices::build(
                survivors.len(),
                (0..points as u32).map(|point| (point_map[point as usize], point)),
            ),
            MergeData::KeepTarget => GroupedIndices::build(
                survivors.len(),
                survivors
                    .iter()
                    .enumerate()
                    .map(|(index, &point)| (index as u32, point)),
            ),
        };
        let edge_groups = match data {
            MergeData::Mix => GroupedIndices::build(
                edge_survivors.len(),
                (0..self.edge_count() as u32)
                    .filter(|&edge| edge_dest[edge as usize] != COLLAPSED)
                    .map(|edge| (edge_map[edge as usize], edge)),
            ),
            MergeData::KeepTarget => GroupedIndices::build(
                edge_survivors.len(),
                edge_survivors
                    .iter()
                    .enumerate()
                    .map(|(index, &edge)| (index as u32, edge)),
            ),
        };

        let mut corner_groups: Vec<(u32, u32)> = Vec::with_capacity(faces.corners.len());
        let mut face_origins: Vec<u32> = Vec::with_capacity(faces.faces.len());
        let mut offsets = Vec::with_capacity(faces.faces.len() + 1);
        offsets.push(0u32);
        let mut corner_verts: Vec<u32> = Vec::with_capacity(faces.corners.len());
        let mut corner_edges: Vec<u32> = Vec::with_capacity(faces.corners.len());
        for face in &faces.faces {
            let original = self.face(face.origin as usize);
            for corner in faces.corners(face) {
                let new_corner = corner_verts.len() as u32;
                corner_verts.push(point_map[corner.point as usize]);
                corner_edges.push(edge_map[corner.edge as usize]);
                if !face.touched {
                    corner_groups.push((new_corner, corner.corner));
                    continue;
                }
                for source in original.clone() {
                    if roots[self.corner_verts[source] as usize] == corner.point
                        && (in_context[self.corner_verts[source] as usize]
                            || source as u32 == corner.corner)
                    {
                        corner_groups.push((new_corner, source as u32));
                    }
                }
            }
            offsets.push(corner_verts.len() as u32);
            face_origins.push(face.origin);
        }
        let corner_groups = GroupedIndices::build(corner_verts.len(), corner_groups.into_iter());

        let edges: Vec<[u32; 2]> = edge_survivors
            .iter()
            .map(|&edge| self.edges[edge as usize].map(|point| point_map[point as usize]))
            .collect();

        self.mix_groups(Domain::Point, &point_groups);
        self.mix_groups(Domain::Edge, &edge_groups);
        self.gather(Domain::Face, &face_origins);
        self.mix_groups(Domain::Corner, &corner_groups);
        self.edges = edges;
        self.face_offsets = offsets;
        self.corner_verts = corner_verts;
        self.corner_edges = corner_edges;
        self.ensure_edges();
    }

    fn mix_groups(&mut self, domain: Domain, groups: &GroupedIndices) {
        let single = groups.iter().all(|group| group.len() == 1);
        let firsts: Vec<u32> = groups.iter().map(|group| group[0]).collect();
        if single {
            self.gather(domain, &firsts);
            return;
        }
        let len = self.domain_len(domain);
        let mut sources: Vec<(u32, f32)> = Vec::new();
        for group in groups.iter() {
            sources.clear();
            let share = 1.0 / group.len() as f32;
            sources.extend(group.iter().map(|&element| (element, share)));
            self.push_interpolated(domain, &sources);
        }
        let mixed: Vec<u32> = (len as u32..(len + groups.len()) as u32).collect();
        self.gather(domain, &mixed);
    }

    fn weld_edges(&self, roots: &[u32], in_context: &[bool]) -> (Vec<u32>, usize) {
        let mut dest: Vec<u32> = (0..self.edge_count() as u32).collect();
        let mut first_of_pair: FxHashMap<[u32; 2], u32> = FxHashMap::default();
        let mut context_edges = 0usize;
        let mut doubles = 0usize;
        for (edge, pair) in self.edges.iter().enumerate() {
            let [a, b] = *pair;
            if !in_context[a as usize] && !in_context[b as usize] {
                continue;
            }
            let welded = [roots[a as usize], roots[b as usize]];
            if welded[0] == welded[1] {
                dest[edge] = COLLAPSED;
                continue;
            }
            context_edges += 1;
            match first_of_pair.get(&ordered(welded)) {
                Some(&first) => {
                    dest[edge] = first;
                    doubles += 1;
                }
                None => {
                    first_of_pair.insert(ordered(welded), edge as u32);
                }
            }
        }
        (dest, context_edges - doubles)
    }

    fn weld_faces(
        &self,
        roots: &[u32],
        in_context: &[bool],
        edge_dest: &[u32],
        remaining_context_edges: usize,
    ) -> WeldFaces {
        let mut kept = WeldFaces {
            faces: Vec::with_capacity(self.face_count()),
            corners: Vec::with_capacity(self.corner_count()),
        };
        let mut context_faces: Vec<usize> = Vec::new();
        let mut added = WeldFaces {
            faces: Vec::new(),
            corners: Vec::new(),
        };
        let weld_corner = |corner: usize| WeldCorner {
            corner: corner as u32,
            point: roots[self.corner_verts[corner] as usize],
            edge: match edge_dest[self.corner_edges[corner] as usize] {
                COLLAPSED => COLLAPSED,
                dest => dest,
            },
        };
        for face in 0..self.face_count() {
            let range = self.face(face);
            let touched = self.corner_verts[range.clone()]
                .iter()
                .any(|&point| in_context[point as usize]);
            if !touched {
                kept.push(face as u32, false, range.map(weld_corner));
                continue;
            }
            if remaining_context_edges == 0 {
                continue;
            }
            let remaining: Vec<WeldCorner> = range
                .map(weld_corner)
                .filter(|corner| corner.edge != COLLAPSED)
                .collect();
            if remaining.len() < 3 {
                continue;
            }
            let (main, pieces) = split_repeats(remaining, in_context);
            if let Some(main) = main {
                context_faces.push(kept.faces.len());
                kept.push(face as u32, true, main);
            }
            for corners in pieces {
                added.push(face as u32, true, corners);
            }
        }
        let mut seen: FxHashMap<Vec<u32>, ()> = FxHashMap::default();
        let mut doubled = vec![false; kept.faces.len()];
        let edge_set = |corners: &[WeldCorner]| {
            let mut edges: Vec<u32> = corners.iter().map(|corner| corner.edge).collect();
            edges.sort_unstable();
            edges
        };
        for &index in &context_faces {
            if seen
                .insert(edge_set(kept.corners(&kept.faces[index])), ())
                .is_some()
            {
                doubled[index] = true;
            }
        }
        if doubled.contains(&true) {
            let mut filtered = WeldFaces {
                faces: Vec::with_capacity(kept.faces.len()),
                corners: Vec::with_capacity(kept.corners.len()),
            };
            for (face, &doubled) in kept.faces.iter().zip(&doubled) {
                if !doubled {
                    filtered.push(
                        face.origin,
                        face.touched,
                        kept.corners(face).iter().copied(),
                    );
                }
            }
            kept = filtered;
        }
        for face in &added.faces {
            let corners = added.corners(face);
            if seen.insert(edge_set(corners), ()).is_none() {
                kept.push(face.origin, face.touched, corners.iter().copied());
            }
        }
        kept
    }
}

fn resolve(targets: &[u32], points: usize) -> Vec<u32> {
    let target = |point: usize| {
        targets
            .get(point)
            .copied()
            .filter(|&target| (target as usize) < points)
            .unwrap_or(point as u32)
    };
    (0..points)
        .map(|point| {
            let mut current = point;
            let mut steps = 0;
            while target(current) as usize != current && steps <= points {
                current = target(current) as usize;
                steps += 1;
            }
            current as u32
        })
        .collect()
}

fn split_repeats(
    corners: Vec<WeldCorner>,
    in_context: &[bool],
) -> (Option<Vec<WeldCorner>>, Vec<Vec<WeldCorner>>) {
    let mut pieces: Vec<Vec<WeldCorner>> = Vec::new();
    let mut current = corners;
    'scan: loop {
        let len = current.len();
        for first in 0..len {
            let point = current[first].point;
            if !in_context[point as usize] {
                continue;
            }
            let Some(offset) = current[first + 1..]
                .iter()
                .position(|corner| corner.point == point)
            else {
                continue;
            };
            let second = first + 1 + offset;
            let inner = second - first;
            let outer = len - inner;
            if inner == 2 && outer == 2 {
                return (None, pieces);
            }
            let piece: Vec<WeldCorner> = current[first..second].to_vec();
            let rest: Vec<WeldCorner> = current[..first]
                .iter()
                .chain(&current[second..])
                .copied()
                .collect();
            match (inner, outer) {
                (2, _) => current = rest,
                (_, 2) => current = piece,
                _ => {
                    let (piece_main, piece_extra) = split_repeats(piece, in_context);
                    pieces.extend(piece_main);
                    pieces.extend(piece_extra);
                    current = rest;
                }
            }
            continue 'scan;
        }
        return (Some(current), pieces);
    }
}

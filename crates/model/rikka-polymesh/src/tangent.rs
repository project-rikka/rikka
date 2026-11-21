use std::hash::BuildHasher;

use glam::{Vec2, Vec3, Vec4};
use rustc_hash::{FxBuildHasher, FxHashMap};

use crate::PARALLEL;
use crate::normals::acos_approx;

const UNSET: u32 = u32::MAX;
const WELD_GRAIN: usize = 1 << 14;
const WELD_SHARD_BITS: u32 = 6;

pub(crate) struct TangentFaces<'a> {
    pub(crate) offsets: &'a [u32],
    pub(crate) corners: &'a [u32],
    pub(crate) positions: &'a [Vec3],
    pub(crate) normals: &'a [Vec3],
    pub(crate) uvs: &'a [Vec2],
}

#[derive(PartialEq, Eq, Hash)]
struct WeldKey {
    position: [u32; 3],
    normal: [u32; 3],
    uv: [u32; 2],
}

#[derive(Clone)]
struct Triangle {
    neighbor: [u32; 3],
    group: [u32; 3],
    vertices: [u32; 3],
    tangent: Vec3,
    face: u32,
    space_offset: u32,
    face_vertex: [u8; 3],
    degenerate: bool,
    quad_one_degenerate: bool,
    group_with_any: bool,
    orient_preserving: bool,
}

struct Group {
    tangent: Vec3,
    representative: u32,
    orient_preserving: bool,
}

#[derive(Clone, Copy)]
struct Space {
    tangent: Vec3,
    counter: u32,
    orient_preserving: bool,
}

impl Space {
    fn accumulate(&mut self, group: &Group) {
        if self.counter == 0 {
            self.tangent = group.tangent;
        } else if self.tangent != group.tangent {
            self.tangent = normalize(self.tangent + group.tangent);
        }
        self.counter += 1;
        self.orient_preserving = group.orient_preserving;
    }
}

fn normalize(vector: Vec3) -> Vec3 {
    let length = vector.length();
    match length != 0.0 {
        true => vector * (1.0 / length),
        false => vector,
    }
}

fn project(normal: Vec3, vector: Vec3) -> Vec3 {
    normalize(vector - normal * normal.dot(vector))
}

fn not_zero(value: f32) -> bool {
    value.abs() > f32::MIN_POSITIVE
}

fn pack(face: u32, vertex: u8) -> u32 {
    (face << 2) | u32::from(vertex)
}

fn unpack(index: u32) -> (u32, u32) {
    (index >> 2, index & 3)
}

struct Mikk<'a> {
    input: &'a TangentFaces<'a>,
    triangles: Vec<Triangle>,
    groups: Vec<Group>,
    spaces: Vec<Space>,
    good: usize,
}

impl Mikk<'_> {
    fn corner(&self, packed: u32) -> usize {
        let (face, vertex) = unpack(packed);
        self.input.corners[self.input.offsets[face as usize] as usize + vertex as usize] as usize
    }

    fn position(&self, packed: u32) -> Vec3 {
        self.input.positions[self.corner(packed)]
    }

    fn normal(&self, packed: u32) -> Vec3 {
        self.input.normals[self.corner(packed)]
    }

    fn uv(&self, packed: u32) -> Vec2 {
        self.input.uvs[self.corner(packed)]
    }

    fn face_position(&self, face: u32, vertex: u32) -> Vec3 {
        self.position(pack(face, vertex as u8))
    }

    fn initial_triangles(&mut self) {
        let input = self.input;
        let faces = input.offsets.len().saturating_sub(1);
        let mut space_offset = 0u32;
        for face in 0..faces {
            let size = (input.offsets[face + 1] - input.offsets[face]) as usize;
            if size != 3 && size != 4 {
                continue;
            }
            let face = face as u32;
            let make = |face_vertex: [u8; 3]| Triangle {
                neighbor: [UNSET; 3],
                group: [UNSET; 3],
                vertices: face_vertex.map(|vertex| pack(face, vertex)),
                tangent: Vec3::ZERO,
                face,
                space_offset,
                face_vertex,
                degenerate: false,
                quad_one_degenerate: false,
                group_with_any: true,
                orient_preserving: false,
            };
            if size == 3 {
                self.triangles.push(make([0, 1, 2]));
            } else {
                let uv = |vertex: u8| self.uv(pack(face, vertex));
                let position = |vertex: u8| self.position(pack(face, vertex));
                let uv_02 = (uv(2) - uv(0)).length_squared();
                let uv_13 = (uv(3) - uv(1)).length_squared();
                let diagonal_02 = match uv_02 != uv_13 {
                    true => uv_02 < uv_13,
                    false => {
                        let distance_02 = (position(2) - position(0)).length_squared();
                        let distance_13 = (position(3) - position(1)).length_squared();
                        distance_13.partial_cmp(&distance_02) != Some(std::cmp::Ordering::Less)
                    }
                };
                match diagonal_02 {
                    true => {
                        self.triangles.push(make([0, 1, 2]));
                        self.triangles.push(make([0, 2, 3]));
                    }
                    false => {
                        self.triangles.push(make([0, 1, 3]));
                        self.triangles.push(make([1, 2, 3]));
                    }
                }
            }
            space_offset += size as u32;
        }
        self.spaces = vec![
            Space {
                tangent: Vec3::X,
                counter: 0,
                orient_preserving: false,
            };
            space_offset as usize
        ];
    }

    fn weld_key(&self, packed: u32) -> WeldKey {
        let bits = |vector: Vec3| (vector + Vec3::ZERO).to_array().map(f32::to_bits);
        let uv = self.uv(packed) + Vec2::ZERO;
        WeldKey {
            position: bits(self.position(packed)),
            normal: bits(self.normal(packed)),
            uv: uv.to_array().map(f32::to_bits),
        }
    }

    fn weld(&mut self) {
        let count = self.triangles.len() * 3;
        let this = &*self;
        let packed = |index: usize| this.triangles[index / 3].vertices[index % 3];
        let shard_of = PARALLEL.grain(WELD_GRAIN).collect(count, |index| {
            (FxBuildHasher.hash_one(this.weld_key(packed(index))) >> (64 - WELD_SHARD_BITS)) as u16
        });
        let shards = 1usize << WELD_SHARD_BITS;
        let mut offsets = vec![0u32; shards + 1];
        for &shard in &shard_of {
            offsets[shard as usize + 1] += 1;
        }
        for shard in 0..shards {
            offsets[shard + 1] += offsets[shard];
        }
        let mut cursor = offsets.clone();
        let mut members = vec![0u32; count];
        for (index, &shard) in shard_of.iter().enumerate() {
            let slot = &mut cursor[shard as usize];
            members[*slot as usize] = index as u32;
            *slot += 1;
        }
        let firsts = PARALLEL.grain(1).collect(shards, |shard| {
            let members = &members[offsets[shard] as usize..offsets[shard + 1] as usize];
            let mut seen: FxHashMap<WeldKey, u32> =
                FxHashMap::with_capacity_and_hasher(members.len(), Default::default());
            members
                .iter()
                .map(|&index| {
                    let vertex = packed(index as usize);
                    *seen.entry(this.weld_key(vertex)).or_insert(vertex)
                })
                .collect::<Vec<u32>>()
        });
        for (shard, firsts) in firsts.into_iter().enumerate() {
            let members = &members[offsets[shard] as usize..offsets[shard + 1] as usize];
            for (&index, first) in members.iter().zip(firsts) {
                let index = index as usize;
                self.triangles[index / 3].vertices[index % 3] = first;
            }
        }
    }

    fn degenerate_prologue(&mut self) {
        let total = self.triangles.len();
        let mut degenerate = 0;
        for triangle in 0..total {
            let [a, b, c] = self.triangles[triangle]
                .vertices
                .map(|packed| self.position(packed));
            if a == b || a == c || b == c {
                self.triangles[triangle].degenerate = true;
                degenerate += 1;
            }
        }
        self.good = total - degenerate;
        if degenerate == 0 {
            return;
        }
        for triangle in 0..total.saturating_sub(1) {
            let (first, second) = (&self.triangles[triangle], &self.triangles[triangle + 1]);
            if first.face == second.face && first.degenerate != second.degenerate {
                self.triangles[triangle].quad_one_degenerate = true;
                self.triangles[triangle + 1].quad_one_degenerate = true;
            }
        }
        let (mut good, bad): (Vec<Triangle>, Vec<Triangle>) = self
            .triangles
            .drain(..)
            .partition(|triangle| !triangle.degenerate);
        good.extend(bad);
        self.triangles = good;
    }

    fn texture_area(&self, triangle: usize) -> f32 {
        let [t1, t2, t3] = self.triangles[triangle]
            .vertices
            .map(|packed| self.uv(packed));
        let (t21, t31) = (t2 - t1, t3 - t1);
        (t21.x * t31.y - t21.y * t31.x).abs()
    }

    fn init_triangles(&mut self) {
        for triangle in 0..self.good {
            let vertices = self.triangles[triangle].vertices;
            let [v1, v2, v3] = vertices.map(|packed| self.position(packed));
            let [t1, t2, t3] = vertices.map(|packed| self.uv(packed));
            let t21x = t2.x - t1.x;
            let t31x = t3.x - t1.x;
            let t21y = t2.y - t1.y;
            let t31y = t3.y - t1.y;
            let d1 = v2 - v1;
            let d2 = v3 - v1;
            let signed_area = t21x * t31y - t21y * t31x;
            let os = d1 * t31y - d2 * t21y;
            let ot = d1 * -t31x + d2 * t21x;
            let entry = &mut self.triangles[triangle];
            entry.orient_preserving = signed_area > 0.0;
            if not_zero(signed_area) {
                let os_squared = os.length_squared();
                let ot_squared = ot.length_squared();
                let sign = if entry.orient_preserving { 1.0 } else { -1.0 };
                if not_zero(os_squared) {
                    entry.tangent = os * (sign / os_squared.sqrt());
                }
                if not_zero(os_squared) && not_zero(ot_squared) {
                    entry.group_with_any = false;
                }
            }
        }
        for triangle in 0..self.good.saturating_sub(1) {
            let (first, second) = (&self.triangles[triangle], &self.triangles[triangle + 1]);
            if first.face != second.face || first.degenerate || second.degenerate {
                continue;
            }
            if first.orient_preserving != second.orient_preserving {
                let choose_first = second.group_with_any
                    || self.texture_area(triangle) >= self.texture_area(triangle + 1);
                let (from, to) = match choose_first {
                    true => (triangle, triangle + 1),
                    false => (triangle + 1, triangle),
                };
                self.triangles[to].orient_preserving = self.triangles[from].orient_preserving;
            }
        }
    }

    fn build_neighbors(&mut self) {
        let good = self.good;
        let buckets = self
            .triangles
            .iter()
            .take(good)
            .flat_map(|triangle| triangle.vertices)
            .max()
            .map_or(0, |vertex| vertex as usize + 1);
        let mut offsets = vec![0u32; buckets + 1];
        for triangle in &self.triangles[..good] {
            for slot in 0..3 {
                let a = triangle.vertices[slot];
                let b = triangle.vertices[(slot + 1) % 3];
                offsets[a.max(b) as usize + 1] += 1;
            }
        }
        for bucket in 0..buckets {
            offsets[bucket + 1] += offsets[bucket];
        }
        let mut cursor = offsets.clone();
        let mut entries = vec![(0u32, 0u32, 0u32); offsets[buckets] as usize];
        for (triangle, entry) in self.triangles[..good].iter().enumerate() {
            for slot in 0..3u32 {
                let a = entry.vertices[slot as usize];
                let b = entry.vertices[((slot + 1) % 3) as usize];
                let position = &mut cursor[a.max(b) as usize];
                entries[*position as usize] = (a.min(b), triangle as u32, slot);
                *position += 1;
            }
        }
        let edge = |triangles: &[Triangle], (triangle, slot): (u32, u32)| {
            let vertices = triangles[triangle as usize].vertices;
            (vertices[slot as usize], vertices[((slot + 1) % 3) as usize])
        };
        for bucket in 0..buckets {
            let bucket = &mut entries[offsets[bucket] as usize..offsets[bucket + 1] as usize];
            bucket.sort_by_key(|&(low, _, _)| low);
            let mut start = 0;
            while start < bucket.len() {
                let low = bucket[start].0;
                let end = start
                    + bucket[start..]
                        .iter()
                        .position(|entry| entry.0 != low)
                        .unwrap_or(bucket.len() - start);
                let list = &bucket[start..end];
                for first in 0..list.len() {
                    let (_, triangle_a, slot_a) = list[first];
                    if self.triangles[triangle_a as usize].neighbor[slot_a as usize] != UNSET {
                        continue;
                    }
                    let (a0, a1) = edge(&self.triangles, (triangle_a, slot_a));
                    for &(_, triangle_b, slot_b) in &list[first + 1..] {
                        if self.triangles[triangle_b as usize].neighbor[slot_b as usize] != UNSET {
                            continue;
                        }
                        let (b1, b0) = edge(&self.triangles, (triangle_b, slot_b));
                        if a0 == b0 && a1 == b1 {
                            self.triangles[triangle_a as usize].neighbor[slot_a as usize] =
                                triangle_b;
                            self.triangles[triangle_b as usize].neighbor[slot_b as usize] =
                                triangle_a;
                            break;
                        }
                    }
                }
                start = end;
            }
        }
    }

    fn assign(&mut self, start: u32, group: u32, stack: &mut Vec<u32>) {
        stack.clear();
        stack.push(start);
        while let Some(triangle) = stack.pop() {
            if triangle == UNSET {
                continue;
            }
            let representative = self.groups[group as usize].representative;
            let orient = self.groups[group as usize].orient_preserving;
            let entry = &mut self.triangles[triangle as usize];
            let Some(slot) = entry
                .vertices
                .iter()
                .position(|&vertex| vertex == representative)
            else {
                continue;
            };
            if entry.group[slot] != UNSET {
                continue;
            }
            if entry.group_with_any && entry.group.iter().all(|&assigned| assigned == UNSET) {
                entry.orient_preserving = orient;
            }
            if entry.orient_preserving != orient {
                continue;
            }
            entry.group[slot] = group;
            let left = entry.neighbor[slot];
            let right = entry.neighbor[if slot > 0 { slot - 1 } else { 2 }];
            stack.push(right);
            stack.push(left);
        }
    }

    fn build_groups(&mut self) {
        let mut stack = Vec::new();
        for triangle in 0..self.good {
            for slot in 0..3 {
                let entry = &self.triangles[triangle];
                if entry.group_with_any || entry.group[slot] != UNSET {
                    continue;
                }
                let group = self.groups.len() as u32;
                self.groups.push(Group {
                    tangent: Vec3::ZERO,
                    representative: entry.vertices[slot],
                    orient_preserving: entry.orient_preserving,
                });
                let left = entry.neighbor[slot];
                let right = entry.neighbor[if slot > 0 { slot - 1 } else { 2 }];
                self.triangles[triangle].group[slot] = group;
                self.assign(left, group, &mut stack);
                self.assign(right, group, &mut stack);
            }
        }
    }

    fn generate_spaces(&mut self) {
        for triangle in 0..self.good {
            let entry = &self.triangles[triangle];
            if entry.group_with_any {
                continue;
            }
            let n = entry.vertices.map(|packed| self.normal(packed));
            let p = entry.vertices.map(|packed| self.position(packed));
            let cosines = [
                project(n[0], p[1] - p[0]).dot(project(n[0], p[2] - p[0])),
                project(n[1], p[2] - p[1]).dot(project(n[1], p[0] - p[1])),
                project(n[2], p[0] - p[2]).dot(project(n[2], p[1] - p[2])),
            ];
            for slot in 0..3 {
                let group = entry.group[slot];
                if group != UNSET {
                    let tangent = project(n[slot], entry.tangent)
                        * acos_approx(cosines[slot].clamp(-1.0, 1.0));
                    self.groups[group as usize].tangent += tangent;
                }
            }
        }
        for group in &mut self.groups {
            group.tangent = normalize(group.tangent);
        }
        for triangle in 0..self.good {
            let entry = &self.triangles[triangle];
            for slot in 0..3 {
                let group = entry.group[slot];
                if group == UNSET {
                    continue;
                }
                let index = entry.space_offset as usize + entry.face_vertex[slot] as usize;
                self.spaces[index].accumulate(&self.groups[group as usize]);
            }
        }
    }

    fn degenerate_epilogue(&mut self) {
        if self.good == self.triangles.len() {
            return;
        }
        let mut good_vertices: FxHashMap<u32, (u32, u32)> = FxHashMap::default();
        for (triangle, entry) in self.triangles[..self.good].iter().enumerate() {
            for (slot, &vertex) in entry.vertices.iter().enumerate() {
                good_vertices
                    .entry(vertex)
                    .or_insert((triangle as u32, slot as u32));
            }
        }
        for triangle in self.good..self.triangles.len() {
            if self.triangles[triangle].quad_one_degenerate {
                continue;
            }
            for slot in 0..3 {
                let Some(&(source, source_slot)) =
                    good_vertices.get(&self.triangles[triangle].vertices[slot])
                else {
                    continue;
                };
                let source = &self.triangles[source as usize];
                let from = source.space_offset as usize
                    + source.face_vertex[source_slot as usize] as usize;
                let target = &self.triangles[triangle];
                let to = target.space_offset as usize + target.face_vertex[slot] as usize;
                self.spaces[to] = self.spaces[from];
            }
        }
        for triangle in 0..self.good {
            let entry = &self.triangles[triangle];
            if !entry.quad_one_degenerate {
                continue;
            }
            let flags = entry
                .face_vertex
                .iter()
                .fold(0u32, |flags, &vertex| flags | (1 << vertex));
            let missing = if flags & 2 == 0 {
                1
            } else if flags & 4 == 0 {
                2
            } else if flags & 8 == 0 {
                3
            } else {
                0
            };
            let target = self.face_position(entry.face, missing);
            let offset = entry.space_offset as usize;
            if let Some(&vertex) = entry
                .face_vertex
                .iter()
                .find(|&&vertex| self.face_position(entry.face, u32::from(vertex)) == target)
            {
                self.spaces[offset + missing as usize] = self.spaces[offset + vertex as usize];
            }
        }
    }
}

pub(crate) fn tangents(input: &TangentFaces<'_>, corner_count: usize) -> Vec<Vec4> {
    let mut mikk = Mikk {
        input,
        triangles: Vec::new(),
        groups: Vec::new(),
        spaces: Vec::new(),
        good: 0,
    };
    mikk.initial_triangles();
    let mut tangents = vec![Vec4::new(1.0, 0.0, 0.0, 1.0); corner_count];
    if mikk.triangles.is_empty() {
        return tangents;
    }
    mikk.weld();
    mikk.degenerate_prologue();
    if mikk.good > 0 {
        mikk.init_triangles();
        mikk.build_neighbors();
        mikk.build_groups();
        mikk.generate_spaces();
        mikk.degenerate_epilogue();
    }
    let faces = input.offsets.len().saturating_sub(1);
    let mut index = 0;
    for face in 0..faces {
        let range = input.offsets[face] as usize..input.offsets[face + 1] as usize;
        if range.len() != 3 && range.len() != 4 {
            continue;
        }
        for corner in range {
            let space = mikk.spaces[index];
            index += 1;
            let sign = if space.orient_preserving { 1.0 } else { -1.0 };
            tangents[input.corners[corner] as usize] = space.tangent.extend(sign);
        }
    }
    tangents
}

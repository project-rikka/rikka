use std::sync::atomic::{AtomicU32, Ordering};

use crate::parallel;
use crate::polymesh::Polymesh;
use crate::validate::ordered;

const GRAIN: usize = 1 << 14;
const NONE: u32 = u32::MAX;
const GROUP: u32 = 1 << 31;

#[derive(Clone, Copy)]
struct Entry {
    high: u32,
    index: u32,
    corner: bool,
}

impl Polymesh {
    fn corner_key(&self, owners: &[u32], corner: usize) -> [u32; 2] {
        let next = self.next_corner(owners[corner] as usize, corner);
        ordered([self.corner_verts[corner], self.corner_verts[next]])
    }

    fn joined(&self, corner: usize, key: [u32; 2]) -> bool {
        self.edges
            .get(self.corner_edges[corner] as usize)
            .is_some_and(|edge| ordered(*edge) == key)
    }

    pub(crate) fn resolve_corner_edges(&mut self, first: usize) {
        let corners = self.corner_count();
        let owners = self.corner_faces();
        let mesh = &*self;
        let keys = parallel::collect(corners - first, GRAIN, |offset| {
            mesh.corner_key(&owners, first + offset)
        });
        let candidates: Vec<u32> = (first..corners)
            .filter(|&corner| !mesh.joined(corner, keys[corner - first]))
            .map(|corner| corner as u32)
            .collect();
        let buckets = keys
            .iter()
            .map(|key| key[0])
            .chain(self.edges.iter().map(|edge| ordered(*edge)[0]))
            .max()
            .map_or(0, |low| low as usize + 1);
        let mut offsets = vec![0u32; buckets + 1];
        for edge in &self.edges {
            offsets[ordered(*edge)[0] as usize + 1] += 1;
        }
        for &corner in &candidates {
            offsets[keys[corner as usize - first][0] as usize + 1] += 1;
        }
        for bucket in 0..buckets {
            offsets[bucket + 1] += offsets[bucket];
        }
        let mut cursor = offsets.clone();
        let mut entries = vec![
            Entry {
                high: 0,
                index: 0,
                corner: false
            };
            offsets[buckets] as usize
        ];
        for (index, edge) in self.edges.iter().enumerate() {
            let [low, high] = ordered(*edge);
            let slot = &mut cursor[low as usize];
            entries[*slot as usize] = Entry {
                high,
                index: index as u32,
                corner: false,
            };
            *slot += 1;
        }
        for (position, &corner) in candidates.iter().enumerate() {
            let [low, high] = keys[corner as usize - first];
            let slot = &mut cursor[low as usize];
            entries[*slot as usize] = Entry {
                high,
                index: position as u32,
                corner: true,
            };
            *slot += 1;
        }
        let resolved: Vec<AtomicU32> = (0..candidates.len())
            .map(|_| AtomicU32::new(NONE))
            .collect();
        parallel::collect(buckets, GRAIN, |bucket| {
            let bucket = &entries[offsets[bucket] as usize..offsets[bucket + 1] as usize];
            for entry in bucket.iter().filter(|entry| entry.corner) {
                let existing = bucket
                    .iter()
                    .filter(|other| !other.corner && other.high == entry.high)
                    .map(|other| other.index)
                    .min();
                let resolution = existing.unwrap_or_else(|| {
                    let leader = bucket
                        .iter()
                        .filter(|other| other.corner && other.high == entry.high)
                        .map(|other| candidates[other.index as usize])
                        .min()
                        .unwrap_or(candidates[entry.index as usize]);
                    leader | GROUP
                });
                resolved[entry.index as usize].store(resolution, Ordering::Relaxed);
            }
        });
        let mut assigned = vec![NONE; corners];
        for (position, &corner) in candidates.iter().enumerate() {
            let corner = corner as usize;
            let key = keys[corner - first];
            if self.joined(corner, key) {
                continue;
            }
            let resolution = resolved[position].load(Ordering::Relaxed);
            self.corner_edges[corner] = match resolution & GROUP {
                0 => resolution,
                _ => {
                    let leader = (resolution & !GROUP) as usize;
                    if assigned[leader] == NONE {
                        assigned[leader] = self.edges.len() as u32;
                        self.edges.push(key);
                    }
                    assigned[leader]
                }
            };
        }
    }
}

mod cluster;
mod envelope;
mod orient;
mod subdivide;

use glam::Vec3;

use crate::cloth_settings::Role;
use crate::figure::Figure;
use crate::geometry::{PointGrid, Shell, Union, area, is_proper};
use crate::proxy::{Proxy, UNMAPPED};
use cluster::Clusters;
use envelope::Envelope;

const TOPOLOGY: f32 = 1.75;

pub(crate) struct Sheet {
    members: Vec<u32>,
    seed_of: Vec<u32>,
    seeds: Vec<([u32; 3], f32)>,
    clusters: Clusters,
    start: f32,
    envelope: Option<Envelope>,
}

impl Sheet {
    pub(crate) fn new(figure: &Figure<'_>, members: &[u32], roles: &[Role]) -> Option<Sheet> {
        let same = figure.same_distance();
        let (seed_of, count) = weld(figure, members, same);
        let mut positions = vec![Vec3::ZERO; count];
        let mut counts = vec![0u32; count];
        let mut pinned = vec![false; count];
        for (member, seed) in members.iter().zip(&seed_of) {
            positions[*seed as usize] += figure.vertices[*member as usize].position;
            counts[*seed as usize] += 1;
            pinned[*seed as usize] |= roles[*member as usize] == Role::Pinned;
        }
        for (position, count) in positions.iter_mut().zip(&counts) {
            *position /= (*count).max(1) as f32;
        }
        let mut slot = vec![u32::MAX; figure.vertices.len()];
        for (member, seed) in members.iter().zip(&seed_of) {
            slot[*member as usize] = *seed;
        }
        let seeds: Vec<([u32; 3], f32)> = figure
            .triangles
            .iter()
            .map(|triangle| triangle.corners.map(|corner| slot[corner as usize]))
            .filter(|mapped| {
                !mapped.contains(&u32::MAX)
                    && is_proper(mapped.map(|seed| positions[seed as usize]))
            })
            .map(|mapped| (mapped, area(mapped.map(|seed| positions[seed as usize]))))
            .collect();
        if seeds.is_empty() {
            return None;
        }
        let triangles: Vec<[u32; 3]> = seeds.iter().map(|(corners, _)| *corners).collect();
        Some(Sheet {
            members: members.to_vec(),
            seed_of,
            clusters: Clusters::new(positions, pinned, &triangles),
            seeds,
            start: same * 2.0,
            envelope: None,
        })
    }

    pub(crate) fn reduce(&mut self, spacing: f32, body: &Shell) {
        self.clusters.reduce(self.start, spacing * TOPOLOGY);
        self.envelope = Some(Envelope::build(
            self.clusters.finish(),
            &self.seeds,
            spacing * TOPOLOGY,
            spacing,
            body,
        ));
    }

    pub(crate) fn count(&self) -> usize {
        self.envelope
            .as_ref()
            .map_or(0, |envelope| envelope.positions.len())
    }

    pub(crate) fn proxy(self, figure: &Figure<'_>, spreads: &[f32]) -> Option<Proxy> {
        let envelope = self.envelope?;
        if envelope.triangles.is_empty() {
            return None;
        }
        let count = envelope.positions.len();
        let mut proxy = Proxy {
            positions: envelope.positions,
            pinned: envelope.pinned,
            radius: vec![0.0; count],
            triangles: envelope.triangles,
            ..Proxy::default()
        };
        let grid = PointGrid::new(
            proxy
                .positions
                .iter()
                .enumerate()
                .map(|(index, position)| (index as u32, *position)),
            0.02,
        );
        proxy.vertex_map = self
            .members
            .iter()
            .zip(&self.seed_of)
            .map(|(member, seed)| {
                let target = match envelope.index[*seed as usize] {
                    UNMAPPED => {
                        proxy.unmapped.push(*member);
                        grid.nearest(figure.vertices[*member as usize].position, |index| {
                            proxy.positions[index as usize]
                        })
                        .unwrap_or(0)
                    }
                    target => target,
                };
                (*member, target)
            })
            .collect();
        proxy.unmapped.sort_unstable();
        proxy.gather(figure, spreads);
        proxy.blend(&envelope.blended);
        proxy.spread_mass();
        Some(proxy)
    }
}

fn weld(figure: &Figure<'_>, members: &[u32], radius: f32) -> (Vec<u32>, usize) {
    let grid = PointGrid::new(
        members
            .iter()
            .enumerate()
            .map(|(at, member)| (at as u32, figure.vertices[*member as usize].position)),
        radius * 2.0,
    );
    let mut union = Union::new(members.len());
    for (at, member) in members.iter().enumerate() {
        let position = figure.vertices[*member as usize].position;
        for other in grid.near(position, radius) {
            if other as usize > at
                && figure.vertices[members[other as usize] as usize]
                    .position
                    .distance(position)
                    <= radius
            {
                union.join(at as u32, other);
            }
        }
    }
    union.labels()
}

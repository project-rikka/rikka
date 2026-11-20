use std::ops::ControlFlow;

use glam::Vec3;

use super::{PointTree, UNSET, distance_squared};

const UNTOUCHED: i64 = -1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Duplicates {
    targets: Vec<u32>,
    merged: usize,
}

impl Duplicates {
    fn from_marks(marks: Vec<i64>) -> Self {
        let targets: Vec<u32> = marks
            .iter()
            .enumerate()
            .map(|(index, &mark)| match mark == UNTOUCHED {
                true => index as u32,
                false => mark as u32,
            })
            .collect();
        let merged = targets
            .iter()
            .enumerate()
            .filter(|&(index, &target)| target as usize != index)
            .count();
        Self { targets, merged }
    }

    pub fn targets(&self) -> &[u32] {
        &self.targets
    }

    pub fn target(&self, index: u32) -> u32 {
        self.targets.get(index as usize).copied().unwrap_or(index)
    }

    pub fn is_merged(&self, index: u32) -> bool {
        self.target(index) != index
    }

    pub fn merged_count(&self) -> usize {
        self.merged
    }

    pub fn into_targets(self) -> Vec<u32> {
        self.targets
    }
}

impl PointTree {
    pub fn find_duplicates(&self, distance: f32) -> Duplicates {
        let mut marks = vec![UNTOUCHED; self.index_bound];
        let mut node_of = vec![UNSET; self.index_bound];
        for (node_index, node) in self.nodes.iter().enumerate() {
            node_of[node.index as usize] = node_index as u32;
        }
        let distance_squared_limit = distance * distance;
        for index in 0..self.index_bound {
            let node_index = node_of[index];
            if node_index == UNSET {
                continue;
            }
            let mark = marks[index];
            if mark != UNTOUCHED && mark != index as i64 {
                continue;
            }
            let position = self.nodes[node_index as usize].position;
            let found = self.mark_within(
                position,
                index as u32,
                distance,
                distance_squared_limit,
                &mut marks,
            );
            if found {
                marks[index] = index as i64;
            }
        }
        Duplicates::from_marks(marks)
    }

    fn mark_within(
        &self,
        position: Vec3,
        search: u32,
        range: f32,
        range_squared: f32,
        marks: &mut [i64],
    ) -> bool {
        let mut found = false;
        let mut stack: Vec<u32> = Vec::with_capacity(64);
        if self.root != UNSET {
            stack.push(self.root);
        }
        while let Some(node_index) = stack.pop() {
            let node = &self.nodes[node_index as usize];
            let axis = node.axis as usize;
            if position[axis] + range <= node.position[axis] {
                if node.left != UNSET {
                    stack.push(node.left);
                }
            } else if position[axis] - range >= node.position[axis] {
                if node.right != UNSET {
                    stack.push(node.right);
                }
            } else {
                if node.index != search
                    && marks[node.index as usize] == UNTOUCHED
                    && distance_squared(node.position, position) <= range_squared
                {
                    marks[node.index as usize] = i64::from(search);
                    found = true;
                }
                if node.left != UNSET {
                    stack.push(node.left);
                }
                if node.right != UNSET {
                    stack.push(node.right);
                }
            }
        }
        found
    }

    pub fn find_duplicate_clusters(&self, distance: f32, keep: impl Fn(u32) -> bool) -> Duplicates {
        let mut marks = vec![UNTOUCHED; self.index_bound];
        let mut has_kept = false;
        for node in &self.nodes {
            if keep(node.index) {
                marks[node.index as usize] = i64::from(node.index);
                has_kept = true;
            }
        }
        if self.root == UNSET {
            return Duplicates::from_marks(marks);
        }
        if has_kept {
            self.merge_into_kept(distance, &mut marks);
        }
        let mut cluster: Vec<Member> = Vec::new();
        for node in &self.nodes {
            let index = node.index;
            if marks[index as usize] != UNTOUCHED {
                continue;
            }
            let _ = self.range_search(node.position, distance, |neighbor| {
                let neighbor_node = &self.nodes[neighbor as usize];
                if marks[neighbor_node.index as usize] == UNTOUCHED {
                    cluster.push(Member {
                        index: neighbor_node.index,
                        position: neighbor_node.position,
                    });
                }
                ControlFlow::Continue(())
            });
            if cluster.is_empty() {
                continue;
            }
            cluster.push(Member {
                index,
                position: node.position,
            });
            let target = cluster[cluster_target(&cluster)].index;
            for member in &cluster {
                marks[member.index as usize] = i64::from(target);
            }
            cluster.clear();
        }
        Duplicates::from_marks(marks)
    }

    fn merge_into_kept(&self, distance: f32, marks: &mut [i64]) {
        let mut best_distance = vec![0.0f32; self.index_bound];
        for node in &self.nodes {
            let index = node.index;
            if marks[index as usize] != i64::from(index) {
                continue;
            }
            let _ = self.range_search(node.position, distance, |neighbor| {
                let neighbor_node = &self.nodes[neighbor as usize];
                let neighbor_index = neighbor_node.index as usize;
                let neighbor_distance = distance_squared(neighbor_node.position, node.position);
                let target = marks[neighbor_index];
                if target == UNTOUCHED {
                    marks[neighbor_index] = i64::from(index);
                    best_distance[neighbor_index] = neighbor_distance;
                } else if target != neighbor_index as i64 {
                    let best = best_distance[neighbor_index];
                    if neighbor_distance < best
                        || (neighbor_distance == best && i64::from(index) < target)
                    {
                        best_distance[neighbor_index] = neighbor_distance;
                        marks[neighbor_index] = i64::from(index);
                    }
                }
                ControlFlow::Continue(())
            });
        }
    }
}

struct Member {
    index: u32,
    position: Vec3,
}

fn cluster_target(cluster: &[Member]) -> usize {
    if cluster.len() == 2 {
        return match cluster[0].index < cluster[1].index {
            true => 0,
            false => 1,
        };
    }
    let mut centroid = Vec3::ZERO;
    for member in cluster {
        centroid = Vec3::new(
            centroid.x + member.position.x,
            centroid.y + member.position.y,
            centroid.z + member.position.z,
        );
    }
    let count = cluster.len() as f32;
    let centroid = Vec3::new(centroid.x / count, centroid.y / count, centroid.z / count);
    let last = cluster.len() - 1;
    let mut best = last;
    let mut best_distance = distance_squared(centroid, cluster[best].position);
    for candidate in 0..last {
        let candidate_distance = distance_squared(centroid, cluster[candidate].position);
        if candidate_distance > best_distance {
            continue;
        }
        if candidate_distance == best_distance && cluster[candidate].index > cluster[best].index {
            continue;
        }
        best = candidate;
        best_distance = candidate_distance;
    }
    best
}

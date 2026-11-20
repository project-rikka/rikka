use std::collections::BTreeSet;

use rustc_hash::FxHashMap;

use crate::ids::{EdgeId, Element, VertId};
use crate::mesh::EditMesh;

type Reduction = fn(&[usize]) -> Vec<usize>;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Step {
    OverEdge,
    OverFace,
}

impl EditMesh {
    fn element_vert_list(&self, element: Element) -> Vec<VertId> {
        match element {
            Element::Face(face) => self.face_verts_vec(face),
            Element::Edge(edge) => self.edge_verts(edge).to_vec(),
            Element::Vert(vert) => vec![vert],
        }
    }

    fn element_edge_list(&self, element: Element) -> Vec<EdgeId> {
        match element {
            Element::Face(face) => self.face_edges_vec(face),
            Element::Edge(edge) => vec![edge],
            Element::Vert(vert) => self.vert_edges(vert).collect(),
        }
    }

    fn other_edges(&self, edge: EdgeId, step: Step) -> Vec<EdgeId> {
        match step {
            Step::OverFace => self
                .edge_loops(edge)
                .flat_map(|lp| {
                    [
                        self.loop_edge(self.loop_next(lp)),
                        self.loop_edge(self.loop_prev(lp)),
                    ]
                })
                .collect(),
            Step::OverEdge => {
                let wire = self.is_edge_wire(edge);
                self.edge_verts(edge)
                    .iter()
                    .flat_map(|&vert| self.vert_edges(vert).collect::<Vec<_>>())
                    .filter(|&other| other != edge && !wire)
                    .collect()
            }
        }
    }

    fn depth_measure(&self, target: Element, source: Element, step: Step) -> Option<Vec<usize>> {
        let mut current = self.element_edge_list(source);
        let mut visited: BTreeSet<EdgeId> = current.iter().copied().collect();
        let target_verts = self.element_vert_list(target);
        let mut remaining: BTreeSet<VertId> = target_verts.iter().copied().collect();
        let mut depths: FxHashMap<VertId, usize> = FxHashMap::default();
        let mut depth = 0;
        while !current.is_empty() && !remaining.is_empty() {
            let mut next = Vec::new();
            for &edge in &current {
                for vert in self.edge_verts(edge) {
                    if remaining.remove(&vert) {
                        depths.insert(vert, depth);
                    }
                }
                for other in self.other_edges(edge, step) {
                    if visited.insert(other) {
                        next.push(other);
                    }
                }
            }
            current = next;
            depth += 1;
        }
        match remaining.is_empty() {
            true => Some(target_verts.iter().map(|vert| depths[vert]).collect()),
            false => None,
        }
    }

    fn depth_search(
        &self,
        start: Element,
        depths: &[usize],
        step: Step,
        restrict: Option<&BTreeSet<Element>>,
    ) -> BTreeSet<Element> {
        let depth_max = depths.iter().copied().max().unwrap_or(0);
        let depth_min = depths.iter().copied().min().unwrap_or(0);
        let mut sorted = depths.to_vec();
        sorted.sort_unstable();
        let mut current = self.element_edge_list(start);
        let mut visited: BTreeSet<EdgeId> = current.iter().copied().collect();
        let mut vert_depths: FxHashMap<VertId, usize> = FxHashMap::default();
        let mut depth = 0;
        while !current.is_empty() && depth <= depth_max {
            let mut next = Vec::new();
            for &edge in &current {
                for vert in self.edge_verts(edge) {
                    vert_depths.entry(vert).or_insert(depth);
                }
                for other in self.other_edges(edge, step) {
                    if visited.insert(other) {
                        next.push(other);
                    }
                }
            }
            current = next;
            depth += 1;
        }
        let mut tests: BTreeSet<Element> = BTreeSet::new();
        for (&vert, &depth) in &vert_depths {
            if depth < depth_min {
                continue;
            }
            match start {
                Element::Face(_) => tests.extend(self.vert_faces(vert).map(Element::Face)),
                Element::Edge(_) => tests.extend(
                    self.vert_edges(vert)
                        .filter(|&edge| !self.is_edge_wire(edge))
                        .map(Element::Edge),
                ),
                Element::Vert(_) => {
                    tests.insert(Element::Vert(vert));
                }
            }
        }
        tests
            .into_iter()
            .filter(|&element| {
                let verts = self.element_vert_list(element);
                if verts.len() != depths.len()
                    || restrict.is_some_and(|restrict| !restrict.contains(&element))
                {
                    return false;
                }
                let found: Option<Vec<usize>> = verts
                    .iter()
                    .map(|vert| vert_depths.get(vert).copied())
                    .collect();
                found.is_some_and(|mut found| {
                    found.sort_unstable();
                    found == sorted
                })
            })
            .collect()
    }

    fn find_next(&self, target: Element, source: Element) -> Vec<Element> {
        let (Some(source_edge), Some(source_face)) = (
            self.depth_measure(target, source, Step::OverEdge),
            self.depth_measure(target, source, Step::OverFace),
        ) else {
            return Vec::new();
        };
        let candidates = self.depth_search(target, &source_edge, Step::OverEdge, None);
        let mut candidates =
            self.depth_search(target, &source_face, Step::OverFace, Some(&candidates));
        candidates.remove(&source);
        candidates.remove(&target);
        let mut best_difference = 0usize;
        let mut best: Vec<Element> = Vec::new();
        for &candidate in &candidates {
            let (Some(edge_depths), Some(face_depths)) = (
                self.depth_measure(target, candidate, Step::OverEdge),
                self.depth_measure(target, candidate, Step::OverFace),
            ) else {
                continue;
            };
            let difference: usize = (0..edge_depths.len())
                .map(|index| {
                    edge_depths[index].abs_diff(source_edge[index]).pow(2)
                        + face_depths[index].abs_diff(source_face[index]).pow(2)
                })
                .sum();
            if difference > best_difference {
                best_difference = difference;
                best = vec![candidate];
            } else if difference == best_difference {
                best.push(candidate);
            }
        }
        if best.len() > 1 {
            let tied = std::mem::take(&mut best);
            let mut accumulated_max: Option<usize> = None;
            for candidate in tied {
                let (Some(edge_depths), Some(face_depths)) = (
                    self.depth_measure(source, candidate, Step::OverEdge),
                    self.depth_measure(source, candidate, Step::OverFace),
                ) else {
                    continue;
                };
                let accumulated: usize =
                    edge_depths.iter().sum::<usize>() + face_depths.iter().sum::<usize>();
                if accumulated_max.is_none_or(|max| accumulated > max) {
                    accumulated_max = Some(accumulated);
                    best = vec![candidate];
                } else if Some(accumulated) == accumulated_max {
                    best.push(candidate);
                }
            }
        }
        best
    }

    fn element_signature(&self, element: Element) -> Vec<usize> {
        let mut signature: Vec<usize> = match element {
            Element::Face(face) => self
                .face_loops(face)
                .flat_map(|lp| {
                    self.edge_faces(self.loop_edge(lp))
                        .filter(|&other| other != face)
                        .map(|other| self.face_len(other))
                        .collect::<Vec<_>>()
                })
                .collect(),
            Element::Edge(edge) => self
                .edge_faces(edge)
                .map(|face| self.face_len(face))
                .collect(),
            Element::Vert(vert) => self
                .vert_faces(vert)
                .map(|face| self.face_len(face))
                .collect(),
        };
        signature.sort_unstable();
        signature
    }

    fn filter_by_signature(&self, reference: Element, candidates: Vec<Element>) -> Vec<Element> {
        let signature = self.element_signature(reference);
        let mut remaining: Vec<(Element, Vec<usize>)> = candidates
            .into_iter()
            .map(|candidate| (candidate, self.element_signature(candidate)))
            .collect();
        let reductions: [Reduction; 4] = [
            |values| values.to_vec(),
            |values| {
                let mut unique = values.to_vec();
                unique.dedup();
                unique
            },
            |values| {
                let mut unique = values.to_vec();
                unique.dedup();
                vec![unique.iter().sum()]
            },
            |values| vec![values.len()],
        ];
        for reduce in reductions {
            let expected = reduce(&signature);
            let matching: Vec<(Element, Vec<usize>)> = remaining
                .iter()
                .filter(|(_, candidate)| reduce(candidate) == expected)
                .cloned()
                .collect();
            match matching.len() {
                0 => {}
                1 => return vec![matching[0].0],
                _ => remaining = matching,
            }
        }
        Vec::new()
    }

    pub fn select_next_active(&mut self) -> bool {
        let count = self.history.len();
        if count < 2 {
            return false;
        }
        let target = self.history[count - 1];
        let source = self.history[count - 2];
        let mut next = self.find_next(target, source);
        if next.len() > 1 {
            next = self.filter_by_signature(target, next);
        }
        let [element] = next[..] else {
            return false;
        };
        if self.is_element_hidden(element) {
            return false;
        }
        self.select_element(element, true);
        self.history.retain(|&stored| stored != element);
        self.history.push(element);
        if let Element::Face(face) = element {
            self.active_face = Some(face.0);
        }
        self.select_mode_flush();
        true
    }
}

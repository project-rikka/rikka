use crate::domain::Domain;
use crate::polymesh::Polymesh;
use crate::topology::GroupedIndices;
use crate::values::{AttributeValue, Values};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Selection {
    All,
    Any,
}

impl Polymesh {
    #[must_use]
    pub fn adapt_domain(&self, values: &Values, from: Domain, to: Domain) -> Values {
        if from == to {
            return values.clone();
        }
        let (sources, selection) = self.domain_sources(from, to);
        match values {
            Values::Bool(list) => Values::Bool(
                sources
                    .iter()
                    .map(|group| match selection {
                        Selection::All => {
                            !group.is_empty() && group.iter().all(|&index| list[index as usize])
                        }
                        Selection::Any => group.iter().any(|&index| list[index as usize]),
                    })
                    .collect(),
            ),
            Values::Int(list) => Values::Int(average(list, &sources)),
            Values::Float(list) => Values::Float(average(list, &sources)),
            Values::Vec2(list) => Values::Vec2(average(list, &sources)),
            Values::Vec3(list) => Values::Vec3(average(list, &sources)),
            Values::Color(list) => Values::Color(average(list, &sources)),
        }
    }

    fn domain_sources(&self, from: Domain, to: Domain) -> (GroupedIndices, Selection) {
        let owners = || self.corner_faces();
        let face_members = |pick: &dyn Fn(usize) -> u32| {
            GroupedIndices::build(
                self.face_count(),
                (0..self.face_count()).flat_map(|face| {
                    self.face(face)
                        .map(move |corner| (face as u32, pick(corner)))
                        .collect::<Vec<_>>()
                }),
            )
        };
        match (from, to) {
            (Domain::Point, Domain::Corner) => (
                GroupedIndices::build(
                    self.corner_count(),
                    self.corner_verts
                        .iter()
                        .enumerate()
                        .map(|(corner, &point)| (corner as u32, point)),
                ),
                Selection::All,
            ),
            (Domain::Corner, Domain::Point) => (self.point_corners(), Selection::All),
            (Domain::Face, Domain::Corner) => {
                let owners = owners();
                (
                    GroupedIndices::build(
                        self.corner_count(),
                        owners
                            .iter()
                            .enumerate()
                            .map(|(corner, &face)| (corner as u32, face)),
                    ),
                    Selection::All,
                )
            }
            (Domain::Corner, Domain::Face) => {
                (face_members(&|corner| corner as u32), Selection::All)
            }
            (Domain::Point, Domain::Face) => (
                face_members(&|corner| self.corner_verts[corner]),
                Selection::All,
            ),
            (Domain::Face, Domain::Point) => (self.point_faces(), Selection::Any),
            (Domain::Point, Domain::Edge) => (
                GroupedIndices::build(
                    self.edge_count(),
                    self.edges
                        .iter()
                        .enumerate()
                        .flat_map(|(edge, pair)| [(edge as u32, pair[0]), (edge as u32, pair[1])]),
                ),
                Selection::All,
            ),
            (Domain::Edge, Domain::Point) => (self.point_edges(), Selection::Any),
            (Domain::Edge, Domain::Face) => (
                face_members(&|corner| self.corner_edges[corner]),
                Selection::All,
            ),
            (Domain::Face, Domain::Edge) => (self.edge_faces(), Selection::Any),
            (Domain::Edge, Domain::Corner) => {
                let owners = owners();
                (
                    GroupedIndices::build(
                        self.corner_count(),
                        (0..self.corner_count()).flat_map(|corner| {
                            let previous = self.previous_corner(owners[corner] as usize, corner);
                            [
                                (corner as u32, self.corner_edges[corner]),
                                (corner as u32, self.corner_edges[previous]),
                            ]
                        }),
                    ),
                    Selection::All,
                )
            }
            (Domain::Corner, Domain::Edge) => {
                let owners = owners();
                (
                    GroupedIndices::build(
                        self.edge_count(),
                        (0..self.corner_count()).flat_map(|corner| {
                            let next = self.next_corner(owners[corner] as usize, corner);
                            let edge = self.corner_edges[corner];
                            [(edge, corner as u32), (edge, next as u32)]
                        }),
                    ),
                    Selection::All,
                )
            }
            (Domain::Point, Domain::Point)
            | (Domain::Edge, Domain::Edge)
            | (Domain::Face, Domain::Face)
            | (Domain::Corner, Domain::Corner) => unreachable!("equal domains return early"),
        }
    }
}

fn average<T: AttributeValue>(list: &[T], sources: &GroupedIndices) -> Vec<T> {
    sources
        .iter()
        .map(|group| match group.len() {
            0 => T::DEFAULT,
            count => {
                let share = 1.0 / count as f32;
                T::mix(group.iter().map(|&index| (list[index as usize], share)))
            }
        })
        .collect()
}

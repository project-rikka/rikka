use std::num::NonZeroUsize;

use rikka_mesh_edit::{EdgeId, EditMesh, ElementKind, FaceId, IdMap, SelectAction, SelectMode};
use rikka_parallel::Threads;

use crate::arrangement::Mode;
use crate::boolean::DEFAULT_THRESHOLD;
use crate::edit_exact::{self, ExactEdit};
use crate::error::{Error, Result};
use crate::fast::intersect::{IntersectOptions, intersect};
use crate::fast::separate::separate_faces;
use crate::operation::{Operation, Solver};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum IntersectSource {
    SelfIntersect,
    #[default]
    SelectedUnselected,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum SeparateMode {
    All,
    #[default]
    Cut,
    Merge,
}

#[must_use]
#[derive(Clone, Copy, Debug)]
pub struct IntersectKnife {
    source: IntersectSource,
    separate: SeparateMode,
    threshold: f32,
    solver: Solver,
    threads: Option<NonZeroUsize>,
}

#[must_use]
#[derive(Clone, Copy, Debug)]
pub struct IntersectBoolean {
    operation: Operation,
    swap: bool,
    self_intersection: bool,
    threshold: f32,
    solver: Solver,
    threads: Option<NonZeroUsize>,
}

impl Default for IntersectKnife {
    fn default() -> IntersectKnife {
        IntersectKnife {
            source: IntersectSource::default(),
            separate: SeparateMode::default(),
            threshold: DEFAULT_THRESHOLD,
            solver: Solver::Exact,
            threads: None,
        }
    }
}

impl Default for IntersectBoolean {
    fn default() -> IntersectBoolean {
        IntersectBoolean {
            operation: Operation::Difference,
            swap: false,
            self_intersection: false,
            threshold: DEFAULT_THRESHOLD,
            solver: Solver::Exact,
            threads: None,
        }
    }
}

fn check_threshold(threshold: f32) -> Result<()> {
    match threshold >= 0.0 && threshold.is_finite() {
        true => Ok(()),
        false => Err(Error::InvalidThreshold(threshold)),
    }
}

fn sides(mesh: &EditMesh, side: impl Fn(&EditMesh, FaceId) -> i32) -> IdMap<FaceId, i32> {
    let mut map = IdMap::new();
    for face in mesh.faces() {
        map.insert(face, side(mesh, face));
    }
    map
}

fn pair_side(mesh: &EditMesh, face: FaceId, swap: bool) -> i32 {
    if mesh.is_face_hidden(face) {
        return -1;
    }
    i32::from(mesh.is_face_selected(face) != swap)
}

fn select_result(mesh: &mut EditMesh, changed: bool, tagged: &[EdgeId]) {
    if !changed {
        return;
    }
    mesh.select_all(SelectAction::Deselect);
    if mesh
        .select_mode()
        .intersects(SelectMode::VERT | SelectMode::EDGE)
    {
        for &edge in tagged {
            if mesh.contains_edge(edge) {
                mesh.select_edge(edge, true);
            }
        }
        mesh.select_mode_flush();
    }
}

fn has_selected_face(mesh: &EditMesh) -> bool {
    mesh.faces().any(|face| mesh.is_face_selected(face))
}

impl IntersectKnife {
    pub fn new() -> IntersectKnife {
        IntersectKnife::default()
    }

    pub fn source(mut self, source: IntersectSource) -> IntersectKnife {
        self.source = source;
        self
    }

    pub fn separate(mut self, separate: SeparateMode) -> IntersectKnife {
        self.separate = separate;
        self
    }

    pub fn threshold(mut self, threshold: f32) -> IntersectKnife {
        self.threshold = threshold;
        self
    }

    pub fn solver(mut self, solver: Solver) -> IntersectKnife {
        self.solver = solver;
        self
    }

    pub fn threads(mut self, threads: NonZeroUsize) -> IntersectKnife {
        self.threads = Some(threads);
        self
    }

    pub fn apply(&self, mesh: &mut EditMesh) -> Result<bool> {
        check_threshold(self.threshold)?;
        if !has_selected_face(mesh) {
            return Ok(false);
        }
        let use_self = self.source == IntersectSource::SelfIntersect;
        let separate_all =
            self.separate == SeparateMode::All || (self.separate == SeparateMode::Cut && use_self);
        let separate_cut = self.separate == SeparateMode::Cut && !use_self;
        let sides = match use_self {
            true => sides(mesh, |mesh, face| match mesh.is_face_selected(face) {
                true => 0,
                false => -1,
            }),
            false => sides(mesh, |mesh, face| pair_side(mesh, face, false)),
        };
        let (changed, tagged) = match self.solver {
            Solver::Exact => {
                let outcome = edit_exact::apply(
                    mesh,
                    &sides,
                    &ExactEdit {
                        mode: Mode::Knife,
                        shapes: if use_self { 1 } else { 2 },
                        use_self,
                        separate_all,
                        threads: self.threads.map_or_else(Threads::available, Threads::from),
                    },
                )?;
                (outcome.changed, outcome.tagged_edges)
            }
            Solver::Fast => {
                let mut sides = sides;
                let outcome = intersect(
                    mesh,
                    &mut sides,
                    &IntersectOptions {
                        use_self,
                        separate: separate_all,
                        dissolve: true,
                        island_connect: true,
                        partial_connect: true,
                        edge_tag: true,
                        boolean: None,
                        epsilon: self.threshold,
                    },
                )?;
                mesh.history_clear();
                (outcome.changed, outcome.tagged_edges)
            }
        };
        let tagged = match separate_cut {
            true => {
                mesh.clear_tags(ElementKind::Edge);
                for &edge in &tagged {
                    mesh.set_edge_tag(edge, true);
                }
                separate_faces(mesh, |mesh, face| mesh.is_face_selected(face))?;
                let separated: Vec<EdgeId> = mesh
                    .edges()
                    .filter(|&edge| mesh.is_edge_tagged(edge))
                    .collect();
                mesh.clear_tags(ElementKind::Edge);
                separated
            }
            false => tagged,
        };
        select_result(mesh, changed, &tagged);
        mesh.update_normals();
        Ok(changed)
    }
}

impl IntersectBoolean {
    pub fn new(operation: Operation) -> IntersectBoolean {
        IntersectBoolean {
            operation,
            ..IntersectBoolean::default()
        }
    }

    pub fn swap(mut self, swap: bool) -> IntersectBoolean {
        self.swap = swap;
        self
    }

    pub fn self_intersection(mut self, enabled: bool) -> IntersectBoolean {
        self.self_intersection = enabled;
        self
    }

    pub fn threshold(mut self, threshold: f32) -> IntersectBoolean {
        self.threshold = threshold;
        self
    }

    pub fn solver(mut self, solver: Solver) -> IntersectBoolean {
        self.solver = solver;
        self
    }

    pub fn threads(mut self, threads: NonZeroUsize) -> IntersectBoolean {
        self.threads = Some(threads);
        self
    }

    pub fn apply(&self, mesh: &mut EditMesh) -> Result<bool> {
        check_threshold(self.threshold)?;
        if !has_selected_face(mesh) {
            return Ok(false);
        }
        let swap = self.swap;
        let mut sides = sides(mesh, |mesh, face| pair_side(mesh, face, swap));
        let (changed, tagged) = match self.solver {
            Solver::Exact => {
                let outcome = edit_exact::apply(
                    mesh,
                    &sides,
                    &ExactEdit {
                        mode: Mode::Boolean(self.operation),
                        shapes: 2,
                        use_self: self.self_intersection,
                        separate_all: false,
                        threads: self.threads.map_or_else(Threads::available, Threads::from),
                    },
                )?;
                (outcome.changed, outcome.tagged_edges)
            }
            Solver::Fast => {
                let outcome = intersect(
                    mesh,
                    &mut sides,
                    &IntersectOptions {
                        use_self: false,
                        separate: false,
                        dissolve: true,
                        island_connect: true,
                        partial_connect: false,
                        edge_tag: true,
                        boolean: Some(self.operation),
                        epsilon: self.threshold,
                    },
                )?;
                mesh.history_clear();
                (outcome.changed, outcome.tagged_edges)
            }
        };
        select_result(mesh, changed, &tagged);
        mesh.update_normals();
        Ok(changed)
    }
}

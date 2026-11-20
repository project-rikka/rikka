use glam::Vec3;
use rikka_mesh_edit::{FaceId, IdMap, LoopId, NormalSpaces};
use rikka_polymesh::Domain;

use crate::math::normalize;
use crate::model::FaceKind;
use crate::options::FaceStrength;
use crate::state::Bevel;

const FACE_STRENGTH_LAYER: &str = "__mod_weightednormals_faceweight";
const FACE_STRENGTH_WEAK: i32 = -16384;
const FACE_STRENGTH_MEDIUM: i32 = 0;
const FACE_STRENGTH_STRONG: i32 = 16384;

impl Bevel<'_> {
    fn mark_sharp_boundary(&mut self) {
        let faces: Vec<FaceId> = self.mesh.faces().collect();
        for face in faces {
            if !self.mesh.is_face_smooth(face) || self.face_kind(face) != FaceKind::Recon {
                continue;
            }
            for lp in self.mesh.face_loops_vec(face) {
                let other = self.mesh.loop_radial_next(lp);
                if other == lp {
                    continue;
                }
                let kind = self.face_kind(self.mesh.loop_face(other));
                if kind == FaceKind::Edge || kind == FaceKind::Vert {
                    let edge = self.mesh.loop_edge(lp);
                    self.mesh.set_edge_sharp(edge, true);
                }
            }
        }
    }

    fn fan_kind(&self, lp: Option<LoopId>) -> FaceKind {
        lp.map_or(FaceKind::None, |lp| self.face_kind(self.mesh.loop_face(lp)))
    }

    fn step_cw(&self, lp: LoopId) -> Option<LoopId> {
        let mut edge = self.mesh.loop_edge(self.mesh.loop_prev(lp));
        self.mesh.vert_step_fan_loop(lp, &mut edge)
    }

    fn step_ccw(&self, lp: LoopId) -> Option<LoopId> {
        let mut edge = self.mesh.loop_edge(lp);
        self.mesh.vert_step_fan_loop(lp, &mut edge)
    }

    fn loop_face_normal(&self, lp: LoopId) -> Vec3 {
        self.face_normal(self.mesh.loop_face(lp))
    }

    fn hardened_normal(&self, lp: LoopId, kind: FaceKind) -> Option<Vec3> {
        let face_normal = self.loop_face_normal(lp);
        let lprev = self.step_cw(lp);
        let lnext = self.step_ccw(lp);
        let (prev_kind, next_kind) = (self.fan_kind(lprev), self.fan_kind(lnext));
        let long = |lp: LoopId| self.long_loops.contains(lp);
        match kind {
            FaceKind::Edge => {
                if prev_kind == FaceKind::Edge && long(lp) {
                    lprev.map(|other| normalize(face_normal + self.loop_face_normal(other)))
                } else if next_kind == FaceKind::Edge && lnext.is_some_and(long) {
                    lnext.map(|other| normalize(face_normal + self.loop_face_normal(other)))
                } else if prev_kind == FaceKind::Recon && long(lp) {
                    lprev.map(|other| self.loop_face_normal(other))
                } else if next_kind == FaceKind::Recon && long(self.mesh.loop_prev(lp)) {
                    lnext.map(|other| self.loop_face_normal(other))
                } else {
                    None
                }
            }
            FaceKind::Vert => {
                if prev_kind == FaceKind::Vert && next_kind == FaceKind::Vert {
                    Some(self.mesh.vert_normal(self.mesh.loop_vert(lp)))
                } else if prev_kind == FaceKind::Recon {
                    lprev.map(|other| self.loop_face_normal(other))
                } else if next_kind == FaceKind::Recon {
                    lnext.map(|other| self.loop_face_normal(other))
                } else {
                    let lprevprev = lprev.and_then(|other| self.step_cw(other));
                    let lnextnext = lnext.and_then(|other| self.step_ccw(other));
                    let prevprev_kind = self.fan_kind(lprevprev);
                    let nextnext_kind = self.fan_kind(lnextnext);
                    if prev_kind == FaceKind::Edge && prevprev_kind == FaceKind::Recon {
                        lprevprev.map(|other| self.loop_face_normal(other))
                    } else if prev_kind == FaceKind::Edge
                        && next_kind == FaceKind::Vert
                        && prevprev_kind == FaceKind::Edge
                    {
                        match (lprev, lprevprev) {
                            (Some(a), Some(b)) => Some(normalize(
                                self.loop_face_normal(a) + self.loop_face_normal(b),
                            )),
                            _ => None,
                        }
                    } else if next_kind == FaceKind::Edge
                        && prev_kind == FaceKind::Vert
                        && nextnext_kind == FaceKind::Edge
                    {
                        match (lnext, lnextnext) {
                            (Some(a), Some(b)) => Some(normalize(
                                self.loop_face_normal(a) + self.loop_face_normal(b),
                            )),
                            _ => None,
                        }
                    } else {
                        None
                    }
                }
            }
            _ => None,
        }
    }

    pub(crate) fn harden_normals(&mut self) {
        if self.params.offset == 0.0 || !self.params.harden_normals {
            return;
        }
        self.mesh.update_normals();
        let existing = self.mesh.custom_normals().map(<[Vec3]>::to_vec);
        if existing.is_none() {
            self.mark_sharp_boundary();
        }
        let mut targets: IdMap<LoopId, Vec3> = IdMap::new();
        for face in self.mesh.faces().collect::<Vec<_>>() {
            let kind = self.face_kind(face);
            if kind == FaceKind::Orig || kind == FaceKind::Recon {
                continue;
            }
            for lp in self.mesh.face_loops_vec(face) {
                if let Some(normal) = self.hardened_normal(lp, kind) {
                    targets.insert(lp, normal);
                }
            }
        }
        let spaces = NormalSpaces::new(self.mesh);
        let loops: Vec<LoopId> = self
            .mesh
            .faces()
            .flat_map(|face| self.mesh.face_loops(face))
            .collect();
        let encoded: Vec<[i16; 2]> = loops
            .iter()
            .map(|&lp| {
                let custom = targets
                    .copied(lp)
                    .or_else(|| existing.as_ref().map(|normals| normals[lp.index()]));
                match custom {
                    Some(custom) => spaces.space(lp).encode(custom),
                    None => [0, 0],
                }
            })
            .collect();
        let decoded = self.mesh.to_polymesh().decode_custom_normals(&encoded);
        let custom = self.mesh.custom_normals_mut();
        for (lp, normal) in loops.into_iter().zip(decoded) {
            custom[lp.index()] = normal;
        }
    }

    pub(crate) fn set_face_strength(&mut self) {
        let mode = self.params.face_strength;
        if mode == FaceStrength::None {
            return;
        }
        let faces: Vec<(FaceId, FaceKind)> = self
            .mesh
            .faces()
            .map(|face| (face, self.face_kind(face)))
            .collect();
        let values = self
            .mesh
            .ensure_attribute::<i32>(FACE_STRENGTH_LAYER, Domain::Face);
        for (face, kind) in faces {
            let strength = match kind {
                FaceKind::Vert if mode >= FaceStrength::New => FACE_STRENGTH_WEAK,
                FaceKind::Edge if mode >= FaceStrength::New => FACE_STRENGTH_MEDIUM,
                FaceKind::Recon if mode >= FaceStrength::Affected => FACE_STRENGTH_STRONG,
                FaceKind::Orig if mode == FaceStrength::All => FACE_STRENGTH_STRONG,
                _ => continue,
            };
            values[face.index()] = strength;
        }
    }
}

use std::sync::Arc;

use animate_cast::{FigureId, Shot, frame_of};
use animate_fluid::{Emitter, EmitterId, EmitterState};
use animate_look::Look;
use animate_motion::CameraMotion;
use animate_wind::{Wind, WindLevels, WindMotion};
use rikka_model::MaterialId;
use rikka_model::mmd::FRAMES_PER_SECOND;
use rikka_physics_tailor::Quality;
use rustc_hash::FxHashMap;

use crate::cloth::FigureCloth;
use crate::fluid::Liquids;
use crate::pending::Pending;
use crate::pour::CastMember;
use crate::unkeyed::Drafts;
use crate::wind::Blowing;

#[derive(Clone, Debug)]
pub(super) struct Scene {
    pub(super) members: Vec<CastMember>,
    pub(super) camera: Option<Arc<CameraMotion>>,
    pub(super) follows_camera: bool,
    pub(super) free: Shot,
    pub(super) physics: bool,
    pub(super) restarts: u64,
    pub(super) quality: Quality,
    pub(super) look: Arc<Look>,
    pub(super) highlight: Option<(FigureId, Arc<[MaterialId]>)>,
    pub(super) liquids: Arc<Liquids>,
    pub(super) pouring: Option<EmitterId>,
    pub(super) wind: Arc<WindMotion>,
    pub(super) blowing: bool,
    pub(super) drafts: Drafts,
    pub(super) pending: Option<Pending>,
    pub(super) focus: Option<FigureId>,
    pub(super) figure_cloth: FxHashMap<FigureId, FigureCloth>,
    pub(super) paint_figure: Option<FigureId>,
    pub(super) liquid_paused: bool,
    pub(super) recomputes: u64,
}

impl Scene {
    pub(super) fn new(physics: bool, quality: Quality) -> Self {
        Self {
            members: Vec::new(),
            camera: None,
            follows_camera: false,
            free: Shot::default(),
            physics,
            restarts: 0,
            quality,
            look: Arc::default(),
            highlight: None,
            liquids: Arc::default(),
            pouring: None,
            wind: Arc::default(),
            blowing: false,
            drafts: Drafts::default(),
            pending: None,
            focus: None,
            figure_cloth: FxHashMap::default(),
            paint_figure: None,
            liquid_paused: false,
            recomputes: 0,
        }
    }

    pub(super) fn shot(&self, time: f32, running: bool) -> Shot {
        self.camera
            .as_ref()
            .filter(|_| self.follows_camera && running)
            .and_then(|camera| camera.pose_at(time))
            .map_or(self.free, Shot::from)
    }

    pub(super) fn wind_levels_at(&self, time: f32) -> WindLevels {
        let mut levels = self.wind.levels_at(time * FRAMES_PER_SECOND);
        if let Some(held) = self.drafts.wind.at(Blowing, frame_of(time)) {
            held.apply(|setting, value| levels.set(setting, value));
        }
        levels
    }

    pub(super) fn wind_at(&self, time: f32) -> Wind {
        self.wind_levels_at(time).wind()
    }

    pub(super) fn emitter_state(&self, emitter: &Emitter, time: f32) -> EmitterState {
        let mut state = emitter.sample(time * FRAMES_PER_SECOND);
        if let Some(held) = self.drafts.pour.at(emitter.id, frame_of(time)) {
            held.apply(|setting, value| state.set(setting, value));
        }
        state
    }

    pub(super) fn has_enabled_cloth(&self, figure: FigureId) -> bool {
        self.figure_cloth
            .get(&figure)
            .and_then(|figure_cloth| figure_cloth.analysis.as_ref())
            .is_some_and(|analysis| analysis.pieces().iter().any(|piece| piece.enabled))
    }
}

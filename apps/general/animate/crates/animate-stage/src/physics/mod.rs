mod cloth_assets;
mod cloth_error;
mod stillness;

use std::sync::Arc;
use std::time::Instant;

use animate_cast::FigureId;
use rikka_gpu::Compute;
use rikka_model::NodeId;
use rikka_model::glam::Vec3;
use rikka_model::mmd::FRAMES_PER_SECOND;
use rikka_physics::{ClothBuilder, LiveCloth, LiveInputs, Pace, Step};
use rikka_physics_tailor::{Budget, ClothAsset, Quality, Wind};
use rikka_renderer_3d::{Drape, Frame};

use super::actor::Actor;
use super::holding::{self, ClothSnapshot, FigureParticles, Performer};
pub(crate) use cloth_assets::Wanted;
use cloth_assets::{ClothAssets, Fit};
pub(crate) use cloth_error::ClothError;
use stillness::Stillness;

const GRAVITY: f32 = 7.84;
const FLOOR: f32 = 0.0;
const LEAP: f32 = 1.0;
const LEAD_IN: f32 = 2.0;
const LEAD_STEP: f32 = 1.0 / FRAMES_PER_SECOND;

#[derive(Clone, Debug)]
pub(crate) struct ClothScene {
    pub cloth: ClothBuilder,
    pub assets: Vec<(FigureId, Arc<ClothAsset>)>,
}

impl ClothScene {
    pub(crate) fn cloth_asset(&self, figure: FigureId) -> Option<&Arc<ClothAsset>> {
        self.assets
            .iter()
            .find(|(candidate, _)| *candidate == figure)
            .map(|(_, cloth_asset)| cloth_asset)
    }

    pub(crate) fn same_as(&self, other: &ClothScene) -> bool {
        self.cloth == other.cloth
            && self.assets.len() == other.assets.len()
            && self
                .assets
                .iter()
                .zip(&other.assets)
                .all(|((one, cloth_asset), (two, other))| {
                    one == two && Arc::ptr_eq(cloth_asset, other)
                })
    }
}

#[derive(Debug, Default)]
pub(super) struct StepOutput {
    pub(super) error: Option<ClothError>,
    pub(super) cloth_changed: bool,
}

pub(super) struct Simulation {
    enabled: bool,
    stale: bool,
    quality: Quality,
    live: Option<LiveCloth>,
    broken: bool,
    assets: ClothAssets,
    stillness: Stillness,
    followed: Option<f32>,
    fabric_step: Option<u64>,
}

impl Simulation {
    pub(super) fn new(enabled: bool) -> Self {
        Self {
            enabled,
            stale: true,
            quality: Quality::High,
            live: None,
            broken: false,
            assets: ClothAssets::default(),
            stillness: Stillness::default(),
            followed: None,
            fabric_step: None,
        }
    }

    pub(super) fn replicate(&self) -> Self {
        Self {
            quality: self.quality,
            assets: self.assets.replicate(),
            ..Self::new(self.enabled)
        }
    }

    pub(super) fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub(super) fn is_clothed(&self) -> bool {
        self.enabled && self.live.is_some() && self.assets.all().next().is_some()
    }

    pub(super) fn set_mode(&mut self, enabled: bool) -> bool {
        if self.enabled == enabled {
            return false;
        }
        self.enabled = enabled;
        self.stale = true;
        self.drop_live_cloth();
        true
    }

    pub(super) fn set_quality(&mut self, quality: Quality) -> bool {
        if self.quality == quality {
            return false;
        }
        self.quality = quality;
        self.drop_live_cloth();
        true
    }

    pub(super) fn budget(&self) -> Budget {
        Budget::from(self.quality)
    }

    fn cloth(&self) -> ClothBuilder {
        ClothBuilder::default()
            .gravity(Vec3::new(0.0, -GRAVITY, 0.0))
            .floor(FLOOR)
            .quality(self.quality)
    }

    fn drop_live_cloth(&mut self) {
        self.live = None;
        self.broken = false;
        self.followed = None;
        self.stillness.wake();
    }

    pub(super) fn recast(&mut self) {
        self.stale = true;
    }

    pub(super) fn restart(&mut self) {
        if let Some(live) = &mut self.live {
            live.restart();
        }
        self.broken = false;
        self.followed = None;
        self.stillness.wake();
    }

    pub(super) fn wake(&mut self) {
        self.stillness.wake();
    }

    pub(super) fn is_restless(&self) -> bool {
        self.enabled
            && (self.assets.is_busy()
                || self.live.as_ref().is_some_and(|live| !live.is_empty())
                    && !self.stillness.is_settled())
    }

    pub(super) fn want(&mut self, wanted: Wanted<'_>) -> Result<(), ClothError> {
        if !self.enabled {
            return Ok(());
        }
        let budget = self.budget();
        self.assets.want(wanted, budget)
    }

    pub(super) fn wait_for_cloth_assets(&mut self) {
        self.assets.wait();
    }

    pub(super) fn has_cloth_asset(&self, figure: FigureId) -> bool {
        self.assets.holds(figure)
    }

    pub(super) fn forget(&mut self, figure: FigureId) {
        self.assets.forget(figure);
        if let Some(live) = &mut self.live
            && live.remove_figure(figure.slot()).is_err()
        {
            self.drop_live_cloth();
        }
    }

    pub(super) fn cloth_scene(&self) -> Option<ClothScene> {
        if !self.enabled {
            return None;
        }
        let mut assets: Vec<(FigureId, Arc<ClothAsset>)> = self
            .assets
            .all()
            .map(|(figure, asset)| (figure, Arc::clone(&asset.cloth_asset)))
            .collect();
        assets.sort_by_key(|(figure, _)| *figure);
        Some(ClothScene {
            cloth: self.cloth(),
            assets,
        })
    }

    pub(super) fn lead_in(&self, time: f32) -> Vec<f32> {
        let continues = self
            .followed
            .is_some_and(|followed| (0.0..=LEAP).contains(&(time - followed)));
        let has_assets = self.enabled && self.assets.all().next().is_some();
        if continues || !has_assets {
            return vec![time];
        }
        let start = (time - LEAD_IN).max(0.0);
        let steps = ((time - start) / LEAD_STEP).ceil() as usize;
        (0..steps)
            .map(|step| start + step as f32 * LEAD_STEP)
            .chain([time])
            .collect()
    }

    pub(super) fn step(
        &mut self,
        actors: &mut [Actor],
        compute: rikka_renderer::Result<Compute>,
        input: StepInput,
    ) -> StepOutput {
        let StepInput {
            time,
            running,
            now,
            stepping,
            wind,
        } = input;
        let mut output = StepOutput::default();
        if std::mem::take(&mut self.stale) {
            for actor in actors.iter_mut() {
                actor.rested = false;
            }
            self.stillness.wake();
        }
        for actor in actors.iter_mut() {
            if actor.animate(time) {
                actor.instance.refresh();
            }
        }
        let idle = self.stillness.elapsed(now);
        if !self.enabled {
            return output;
        }
        for (figure, fitted) in self.assets.collect() {
            output.cloth_changed = true;
            self.broken = false;
            self.fabric_step = None;
            self.stillness.wake();
            let applied = match fitted {
                Ok(fit) => self.apply_fit(figure, fit),
                Err(error) => Err(error),
            };
            if let Err(error) = applied {
                output.error = Some(error);
            }
        }
        if !stepping {
            return output;
        }
        if let Err(error) = self.build_live_cloth(compute) {
            output.error = Some(error);
            self.broken = true;
            return output;
        }
        let grabs: Vec<_> = holding::active_grabs(time, &self.performers(actors))
            .into_iter()
            .map(|active_grabs| (actors[active_grabs.owner].id.slot(), active_grabs.grab))
            .collect();
        let Some(live) = self.live.as_mut().filter(|live| !live.is_empty()) else {
            return output;
        };
        let pace = match running {
            true => Pace::Playing,
            false => Pace::Stopped { idle },
        };
        let inputs = LiveInputs::new()
            .wind(wind)
            .figures(
                actors
                    .iter()
                    .map(|actor| (actor.id.slot(), &actor.instance)),
            )
            .grabs(grabs);
        match live.follow(time, pace, &inputs) {
            Ok(Step::Advance(_)) if !running => self.stillness.watch(live.motion(), idle),
            Ok(_) => self.stillness.wake(),
            Err(failure) => {
                output.error = Some(failure.into());
                self.live = None;
                self.broken = true;
            }
        }
        self.followed = Some(time);
        output
    }

    pub(super) fn performers<'a>(&'a self, actors: &'a [Actor]) -> Vec<Performer<'a>> {
        actors
            .iter()
            .map(|actor| Performer {
                key: actor.id.number(),
                name: &actor.name,
                instance: &actor.instance,
                pinches: actor.pinches,
                holds: actor.holds(),
                cloth_asset: self
                    .assets
                    .current(actor.id)
                    .filter(|_| self.enabled)
                    .map(|asset| &asset.cloth_asset),
            })
            .collect()
    }

    pub(super) fn cloth_snapshot(
        &mut self,
        actors: &[Actor],
    ) -> Result<Option<ClothSnapshot>, ClothError> {
        let Some(live) = self.live.as_ref().filter(|_| self.enabled) else {
            return Ok(None);
        };
        let steps = live.steps();
        if self.fabric_step == Some(steps) {
            return Ok(None);
        }
        self.fabric_step = Some(steps);
        let actors_with_assets: Vec<_> = actors
            .iter()
            .filter_map(|actor| Some((actor, self.assets.current(actor.id)?)))
            .collect();
        let slots: Vec<usize> = actors_with_assets
            .iter()
            .map(|(actor, _)| actor.id.slot())
            .collect();
        let read = live.positions(&slots)?;
        let figures = actors_with_assets
            .into_iter()
            .zip(read)
            .filter_map(|((actor, asset), positions)| {
                Some(FigureParticles {
                    id: actor.id,
                    name: actor.name.clone(),
                    cloth_asset: Arc::clone(&asset.cloth_asset),
                    positions: positions?,
                })
            })
            .collect();
        Ok(Some(ClothSnapshot { figures }))
    }

    fn apply_fit(&mut self, figure: FigureId, fit: Fit) -> Result<(), ClothError> {
        let (Some(live), Some(asset)) = (&mut self.live, self.assets.current(figure)) else {
            return Ok(());
        };
        let slot = figure.slot();
        match fit {
            _ if asset.cloth_asset.pieces.is_empty() => live.remove_figure(slot),
            Fit::Cut => live.add_figure(slot, Arc::clone(&asset.cloth_asset)),
            Fit::Restyled(styles) if live.has_figure(slot) => styles
                .into_iter()
                .try_for_each(|(piece, style)| live.set_style(slot, piece, style)),
            Fit::Restyled(_) => live.add_figure(slot, Arc::clone(&asset.cloth_asset)),
        }?;
        Ok(())
    }

    fn build_live_cloth(
        &mut self,
        compute: rikka_renderer::Result<Compute>,
    ) -> Result<(), ClothError> {
        if self.live.is_some() || self.broken || self.assets.all().next().is_none() {
            return Ok(());
        }
        let compute = compute.map_err(ClothError::Compute)?;
        let mut live = LiveCloth::new(&compute, self.cloth())?;
        for (figure, asset) in self.assets.all() {
            live.add_figure(figure.slot(), Arc::clone(&asset.cloth_asset))?;
        }
        self.live = Some(live);
        self.followed = None;
        Ok(())
    }

    pub(super) fn drapes(&self, frame: &mut Frame, figure: FigureId) -> Vec<(NodeId, Drape)> {
        match &self.live {
            Some(live) if self.enabled => live.drapes(frame, figure.slot()),
            _ => Vec::new(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct StepInput {
    pub(super) time: f32,
    pub(super) running: bool,
    pub(super) now: Instant,
    pub(super) stepping: bool,
    pub(super) wind: Wind,
}

impl std::fmt::Debug for Simulation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Simulation")
            .field("enabled", &self.enabled)
            .field("quality", &self.quality)
            .field("live", &self.live)
            .field("assets", &self.assets)
            .finish_non_exhaustive()
    }
}

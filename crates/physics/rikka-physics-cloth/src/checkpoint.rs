use std::sync::Arc;

use glam::Mat4;
use rikka_gpu::{Batch, Download};
use rikka_physics_gpu::words;
use rikka_physics_tailor::{ClothAsset, Style};

use crate::cloth::Cloth;
use crate::error::{Error, Result};
use crate::figure::{Figure, FigureId, Motion};
use crate::grab::GrabState;
use crate::inputs::Posture;
use crate::readback::StepStats;
use crate::recorded::Recorded;
use crate::step::Rest;

pub struct Checkpoint {
    steps: u64,
    rest: Rest,
    measured: Vec<StepStats>,
    figures: Vec<Captured>,
}

struct Captured {
    id: FigureId,
    asset: Arc<ClothAsset>,
    styles: Vec<Style>,
    state: Vec<u8>,
    contacts: Vec<u8>,
    shown: Vec<Mat4>,
    motion: Motion,
    posture: Option<Posture>,
    grabbing: GrabState,
}

impl Checkpoint {
    #[must_use]
    pub fn steps(&self) -> u64 {
        self.steps
    }

    #[must_use]
    pub fn bytes(&self) -> usize {
        self.figures
            .iter()
            .map(|captured| captured.state.len() + captured.contacts.len())
            .sum()
    }
}

impl std::fmt::Debug for Checkpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Checkpoint")
            .field("steps", &self.steps)
            .field("figures", &self.figures.len())
            .finish_non_exhaustive()
    }
}

#[must_use = "checkpoint is complete only after its batch is submitted and it is finished"]
pub struct PendingCheckpoint<'a> {
    cloth: &'a mut Cloth,
    downloads: Vec<(Download, Download)>,
}

impl PendingCheckpoint<'_> {
    pub fn finish(self) -> Result<Checkpoint> {
        let PendingCheckpoint { cloth, downloads } = self;
        let measured = cloth.stats_readback.history(cloth.steps)?;
        let mut figures = Vec::with_capacity(cloth.figures.len());
        for ((&id, figure), (download, contacts)) in cloth.figures.iter().zip(downloads) {
            figures.push(Captured {
                id,
                asset: Arc::clone(&figure.asset),
                styles: figure.styles.clone(),
                state: words(download, figure.state.size(), Error::ShortReadback)?,
                contacts: words(contacts, figure.contacts.size(), Error::ShortReadback)?,
                shown: figure.shown.clone(),
                motion: figure.motion.clone(),
                posture: figure.posture.clone(),
                grabbing: figure.grabbing.clone(),
            });
        }
        Ok(Checkpoint {
            steps: cloth.steps,
            rest: cloth.rest,
            measured,
            figures,
        })
    }
}

impl std::fmt::Debug for PendingCheckpoint<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PendingCheckpoint")
            .field("steps", &self.cloth.steps)
            .field("figures", &self.downloads.len())
            .finish()
    }
}

impl Cloth {
    pub fn checkpoint(&mut self) -> Result<Checkpoint> {
        let mut batch = self.compute.batch();
        let pending_checkpoint = self.checkpoint_into(&mut batch);
        batch.submit()?;
        pending_checkpoint.finish()
    }

    pub fn checkpoint_into(&mut self, batch: &mut Batch) -> PendingCheckpoint<'_> {
        let downloads = self
            .figures
            .values()
            .map(|figure| {
                (
                    batch.download_buffer(&figure.state),
                    batch.download_buffer(&figure.contacts),
                )
            })
            .collect();
        PendingCheckpoint {
            cloth: self,
            downloads,
        }
    }

    pub fn restore(&mut self, checkpoint: &Checkpoint) -> Result<()> {
        let mut batch = self.compute.batch();
        let recorded = self.restore_into(&mut batch, checkpoint)?;
        let ready = batch.submit()?;
        self.commit(recorded, &ready);
        Ok(())
    }

    pub fn restore_into(&mut self, batch: &mut Batch, checkpoint: &Checkpoint) -> Result<Recorded> {
        let matches = checkpoint.figures.len() == self.figures.len()
            && checkpoint
                .figures
                .iter()
                .zip(&self.figures)
                .all(|(captured, (&id, figure))| {
                    captured.id == id && Arc::ptr_eq(&captured.asset, &figure.asset)
                });
        if !matches {
            return Err(Error::ForeignCheckpoint);
        }
        self.stats_readback.resume(&checkpoint.measured)?;
        let compute = self.compute.clone();
        let mut shown = Vec::new();
        for captured in &checkpoint.figures {
            let figure = self.figure_mut(captured.id)?;
            if figure.styles != captured.styles {
                figure.styles.clone_from(&captured.styles);
                figure.restyle(&compute)?;
            }
            batch.write(&figure.state, 0, &captured.state);
            batch.write(&figure.contacts, 0, &captured.contacts);
            figure.motion = captured.motion.clone();
            figure.posture.clone_from(&captured.posture);
            figure.grabbing.clone_from(&captured.grabbing);
            figure.shown.clone_from(&captured.shown);
            if figure.posture.is_some() && !figure.motion.fresh && !figure.shown.is_empty() {
                batch.write(
                    &figure.frame,
                    figure.frame_layout.palettes,
                    bytemuck::cast_slice(&figure.shown),
                );
                shown.push(captured.id);
            }
        }
        let skinned: Vec<&Figure> = shown.iter().map(|id| &self.figures[id]).collect();
        self.skin(batch, &skinned, 1);
        self.present(batch, &shown, 0, true)?;
        self.steps = checkpoint.steps;
        self.rest = checkpoint.rest;
        Ok(Recorded { shown })
    }
}

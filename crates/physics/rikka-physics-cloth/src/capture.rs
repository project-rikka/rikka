use std::sync::Arc;

use glam::Mat4;
use rikka_gpu::{Batch, ComputeBuffer, Ticket};
use rikka_model_core::Model;

use crate::cloth::Cloth;
use crate::drape::{Outputs, handedness};
use crate::error::{Error, Result};
use crate::figure::{Figure, FigureId, VEC4};
use crate::layout::NEAREST_WORDS;
use crate::recorded::Recorded;

const WORD: u64 = 4;

#[must_use = "capture is complete only after it is finished"]
pub struct PendingCapture {
    capture: Capture,
    ready: Ticket,
}

impl PendingCapture {
    pub fn finish(self) -> Result<Capture> {
        self.ready.wait()?;
        Ok(self.capture)
    }

    #[must_use]
    pub fn is_ready(&self) -> bool {
        self.ready.is_ready()
    }
}

impl std::fmt::Debug for PendingCapture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PendingCapture")
            .field("capture", &self.capture)
            .finish_non_exhaustive()
    }
}

#[derive(Clone)]
pub struct Capture {
    figures: Vec<Captured>,
}

#[derive(Clone)]
struct Captured {
    id: FigureId,
    count: u32,
    buffer: ComputeBuffer,
    palettes: Vec<Mat4>,
    model: Arc<Model>,
}

impl Capture {
    #[must_use]
    pub fn bytes(&self) -> usize {
        self.figures
            .iter()
            .map(|captured| {
                captured.buffer.size() as usize + size_of_val(captured.palettes.as_slice())
            })
            .sum()
    }
}

impl std::fmt::Debug for Capture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Capture")
            .field("figures", &self.figures.len())
            .field("bytes", &self.bytes())
            .finish()
    }
}

impl Cloth {
    pub fn capture(&self) -> Result<PendingCapture> {
        let mut batch = self.compute.batch();
        let capture = self.capture_into(&mut batch)?;
        let ready = batch.submit()?;
        Ok(PendingCapture { capture, ready })
    }

    pub fn capture_into(&self, batch: &mut Batch) -> Result<Capture> {
        let mut figures = Vec::new();
        for (&id, figure) in &self.figures {
            let Some(posture) = &figure.posture else {
                continue;
            };
            if figure.shown.is_empty() {
                continue;
            }
            let count = u64::from(figure.count());
            let positions = count * VEC4;
            let buffer = figure.captures.lock().acquire(&self.compute)?;
            batch.copy_buffer(&figure.state, 0, &buffer, 0, positions);
            batch.copy_buffer(
                &figure.scratch,
                figure.scratch_layout.nearest,
                &buffer,
                positions,
                count * NEAREST_WORDS * WORD,
            );
            figures.push(Captured {
                id,
                count: figure.count(),
                buffer,
                palettes: figure.shown.clone(),
                model: Arc::clone(&posture.model),
            });
        }
        Ok(Capture { figures })
    }

    pub fn show(&mut self, capture: &Capture) -> Result<()> {
        let mut batch = self.compute.batch();
        let recorded = self.show_into(&mut batch, capture)?;
        let ready = batch.submit()?;
        self.commit(recorded, &ready);
        Ok(())
    }

    pub fn show_into(&mut self, batch: &mut Batch, capture: &Capture) -> Result<Recorded> {
        for captured in &capture.figures {
            let figure = self.figure(captured.id)?;
            if figure.count() != captured.count
                || figure.topology.slots.len() != captured.palettes.len()
            {
                return Err(Error::ForeignCheckpoint);
            }
        }
        let mut shown = Vec::new();
        for captured in &capture.figures {
            let figure = self.figure_mut(captured.id)?;
            if figure.outputs.is_none() {
                figure.outputs = Some(Outputs::new(
                    &captured.model,
                    &figure.topology.nodes,
                    &figure.bound,
                )?);
                let hands = handedness(&captured.model, &figure.bound);
                if !hands.is_empty() {
                    batch.write(&figure.hands, 0, bytemuck::cast_slice(&hands));
                }
            }
            let count = u64::from(captured.count);
            let positions = count * VEC4;
            batch.copy_buffer(&captured.buffer, 0, &figure.state, 0, positions);
            batch.copy_buffer(
                &captured.buffer,
                positions,
                &figure.scratch,
                figure.scratch_layout.nearest,
                count * NEAREST_WORDS * WORD,
            );
            batch.write(
                &figure.frame,
                figure.frame_layout.palettes,
                bytemuck::cast_slice(&captured.palettes),
            );
            figure.shown.clone_from(&captured.palettes);
            shown.push(captured.id);
        }
        let skinned: Vec<&Figure> = shown.iter().map(|id| &self.figures[id]).collect();
        self.skin(batch, &skinned, 1);
        self.present(batch, &shown, 0, true)?;
        Ok(Recorded { shown })
    }
}

use rikka_sync::Mutex;

use rikka_gpu::Compute;
use rikka_gpu::{Batch, ComputeBuffer, Download};
use rikka_physics_gpu::{Pass, words};

use crate::cloth::Cloth;
use crate::error::{Error, Result};
use crate::figure::{Figure, FigureId};

const LANES: u64 = 256;
const TAPS_PER_SHEET: u64 = 64;
const WORD: u64 = 4;
const SHEET_BYTES: u64 = TAPS_PER_SHEET * LANES * WORD;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Traced {
    pub round: u32,
    pub epoch: Option<u32>,
    pub pass: &'static str,
    pub figure: Option<FigureId>,
    pub buffer: &'static str,
    pub hash: u64,
}

#[derive(Clone, Copy)]
struct Label {
    round: u32,
    epoch: Option<u32>,
    pass: &'static str,
    figure: Option<FigureId>,
    buffer: &'static str,
}

#[derive(Default)]
struct Taps {
    sheets: Vec<ComputeBuffer>,
    labels: Vec<Label>,
    round: u32,
    failed: Option<Error>,
}

#[derive(Default)]
pub(crate) struct Tracer {
    taps: Mutex<Taps>,
}

pub(crate) struct Pending {
    labels: Vec<Label>,
    sheets: Vec<Download>,
    failed: Option<Error>,
}

impl Tracer {
    fn taps(&self) -> rikka_sync::MutexGuard<'_, Taps> {
        self.taps.lock()
    }

    pub(crate) fn begin_round(&self) {
        self.taps().round += 1;
    }

    pub(crate) fn finish(&self, batch: &mut Batch) -> Pending {
        let taps = std::mem::take(&mut *self.taps());
        let used = taps.labels.len() as u64;
        let sheets = taps
            .sheets
            .iter()
            .enumerate()
            .map(|(index, sheet)| {
                let taps = (used - index as u64 * TAPS_PER_SHEET).min(TAPS_PER_SHEET);
                batch.download_range(sheet, 0, taps * LANES * WORD)
            })
            .collect();
        Pending {
            labels: taps.labels,
            sheets,
            failed: taps.failed,
        }
    }

    fn tap(
        &self,
        compute: &Compute,
        kernel: &rikka_physics_gpu::NamedKernel,
        batch: &mut Batch,
        label: Label,
        buffer: &ComputeBuffer,
    ) {
        let mut taps = self.taps();
        let index = taps.labels.len() as u64;
        if index.is_multiple_of(TAPS_PER_SHEET) {
            match compute.buffer(SHEET_BYTES) {
                Ok(sheet) => taps.sheets.push(sheet),
                Err(error) => {
                    taps.failed.get_or_insert(error.into());
                    return;
                }
            }
        }
        let round = taps.round;
        taps.labels.push(Label { round, ..label });
        let sheet = &taps.sheets[(index / TAPS_PER_SHEET) as usize];
        Pass::direct(kernel, LANES as u32, LANES as u32)
            .label("trace")
            .read(buffer)
            .write(sheet)
            .uint((buffer.size() / WORD) as u32)
            .uint((index % TAPS_PER_SHEET) as u32)
            .run(batch);
    }
}

impl Pending {
    fn resolve(self) -> Result<Vec<Traced>> {
        if let Some(error) = self.failed {
            return Err(error);
        }
        let mut hashes: Vec<u32> = Vec::with_capacity(self.labels.len() * LANES as usize);
        for (index, sheet) in self.sheets.into_iter().enumerate() {
            let taps =
                (self.labels.len() as u64 - index as u64 * TAPS_PER_SHEET).min(TAPS_PER_SHEET);
            hashes.extend(words::<u32, Error>(
                sheet,
                taps * LANES * WORD,
                Error::ShortReadback,
            )?);
        }
        Ok(self
            .labels
            .iter()
            .zip(hashes.chunks(LANES as usize))
            .map(|(label, lanes)| Traced {
                round: label.round,
                epoch: label.epoch,
                pass: label.pass,
                figure: label.figure,
                buffer: label.buffer,
                hash: lanes.iter().fold(0xcbf2_9ce4_8422_2325_u64, |hash, &word| {
                    (hash ^ u64::from(word)).wrapping_mul(0x0000_0100_0000_01b3)
                }),
            })
            .collect())
    }
}

impl Cloth {
    pub fn set_tracing(&mut self, tracing: bool) {
        self.tracer = tracing.then(Tracer::default);
        self.traced = None;
    }

    pub fn trace(&mut self) -> Result<Vec<Traced>> {
        match self.traced.take() {
            Some(pending) => pending.resolve(),
            None => Ok(Vec::new()),
        }
    }

    pub(crate) fn tap(
        &self,
        batch: &mut Batch,
        pass: &'static str,
        epoch: Option<u32>,
        figures: &[&Figure],
    ) {
        let Some(tracer) = &self.tracer else {
            return;
        };
        for figure in figures {
            for (buffer, traced) in [
                ("state", &figure.state),
                ("contacts", &figure.contacts),
                ("scratch", &figure.scratch),
                ("posed", &figure.posed),
                ("normals", &figure.normals),
            ] {
                let label = Label {
                    round: 0,
                    epoch,
                    pass,
                    figure: Some(figure.id),
                    buffer,
                };
                tracer.tap(&self.compute, &self.kernels.hash, batch, label, traced);
            }
        }
    }

    pub(crate) fn tap_grid(&self, batch: &mut Batch, epoch: u32) {
        let Some(tracer) = &self.tracer else {
            return;
        };
        let label = Label {
            round: 0,
            epoch: Some(epoch),
            pass: "obstacles",
            figure: None,
            buffer: "cells",
        };
        tracer.tap(
            &self.compute,
            &self.kernels.hash,
            batch,
            label,
            &self.grid.cells,
        );
    }
}

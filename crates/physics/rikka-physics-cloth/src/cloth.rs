use std::collections::BTreeMap;
use std::sync::Arc;

use rikka_gpu::Compute;
use rikka_gpu::{ComputeBuffer, Span, Timing};
use rikka_model_core::NodeId;
use rikka_physics_gpu::Scan;
use rikka_physics_tailor::{ClothAsset, Style};

use crate::builder::{ClothBuilder, Settings};
use crate::drape::Drape;
use crate::error::{Error, Result};
use crate::figure::{Figure, FigureId};
use crate::gpu::{Kernels, Solvers};
use crate::grid::Grid;
use crate::layout::validate;
use crate::readback::StatsReadback;
use crate::step::Rest;
use crate::trace::{Pending, Tracer};

pub struct Cloth {
    pub(crate) compute: Compute,
    pub(crate) kernels: Kernels,
    pub(crate) solvers: Solvers,
    pub(crate) settings: Settings,
    pub(crate) figures: BTreeMap<FigureId, Figure>,
    pub(crate) next: u32,
    pub(crate) scan: Scan,
    pub(crate) grid: Grid,
    pub(crate) jobs: ComputeBuffer,
    pub(crate) tables: ComputeBuffer,
    pub(crate) stats: ComputeBuffer,
    pub(crate) steps: u64,
    pub(crate) rest: Rest,
    pub(crate) substeps: u32,
    pub(crate) stats_readback: StatsReadback,
    pub(crate) profiling: bool,
    pub(crate) timing: Option<Timing>,
    pub(crate) tracer: Option<Tracer>,
    pub(crate) traced: Option<Pending>,
}

impl Cloth {
    pub fn builder() -> ClothBuilder {
        ClothBuilder::default()
    }

    pub(crate) fn new(compute: &Compute, settings: Settings) -> Result<Cloth> {
        Ok(Cloth {
            kernels: Kernels::new(compute)?,
            solvers: Solvers::new(compute)?,
            scan: Scan::new(compute)?,
            grid: Grid::new(compute)?,
            jobs: compute.buffer(4096)?,
            tables: compute.buffer(4096)?,
            stats: compute.buffer(256)?,
            compute: compute.clone(),
            settings,
            figures: BTreeMap::new(),
            next: 0,
            steps: 0,
            rest: Rest::default(),
            substeps: 0,
            stats_readback: StatsReadback::default(),
            profiling: false,
            timing: None,
            tracer: None,
            traced: None,
        })
    }

    pub fn add_figure(&mut self, asset: Arc<ClothAsset>) -> Result<FigureId> {
        let styles: Vec<Style> = asset
            .pieces
            .iter()
            .map(|piece| piece.style.clone())
            .collect();
        self.add_styled_figure(asset, styles)
    }

    pub fn add_styled_figure(
        &mut self,
        asset: Arc<ClothAsset>,
        styles: impl IntoIterator<Item = Style>,
    ) -> Result<FigureId> {
        let styles: Vec<Style> = styles.into_iter().collect();
        validate::asset(&asset)?;
        if styles.len() != asset.pieces.len() {
            return Err(Error::StyleCount {
                pieces: asset.pieces.len(),
                styles: styles.len(),
            });
        }
        for style in &styles {
            validate::style(style)?;
        }
        let id = FigureId(self.next);
        let figure = Figure::new(&self.compute, id, asset, styles, self.boundaries())?;
        self.next += 1;
        self.figures.insert(id, figure);
        Ok(id)
    }

    pub fn remove_figure(&mut self, figure: FigureId) -> Result<()> {
        self.figures
            .remove(&figure)
            .map(drop)
            .ok_or(Error::UnknownFigure(figure))
    }

    pub fn set_style(&mut self, figure: FigureId, piece: usize, style: Style) -> Result<()> {
        validate::style(&style)?;
        let compute = self.compute.clone();
        let state = self.figure_mut(figure)?;
        let Some(slot) = state.styles.get_mut(piece) else {
            return Err(Error::UnknownPiece { figure, piece });
        };
        *slot = style;
        state.restyle(&compute)
    }

    pub fn style(&self, figure: FigureId, piece: usize) -> Result<&Style> {
        self.figure(figure)?
            .styles
            .get(piece)
            .ok_or(Error::UnknownPiece { figure, piece })
    }

    pub fn figures(&self) -> impl Iterator<Item = FigureId> + '_ {
        self.figures.keys().copied()
    }

    pub fn drapes(&self, figure: FigureId) -> Result<Vec<Drape>> {
        Ok(self
            .figure(figure)?
            .outputs
            .as_ref()
            .map(crate::drape::Outputs::drapes)
            .unwrap_or_default())
    }

    pub fn recorded_drapes(
        &self,
        figure: FigureId,
    ) -> Result<impl Iterator<Item = (NodeId, &ComputeBuffer)>> {
        Ok(self
            .figure(figure)?
            .outputs
            .iter()
            .flat_map(crate::drape::Outputs::recorded))
    }

    pub fn reset(&mut self) -> Result<()> {
        self.stats_readback.resume(&[])?;
        self.steps = 0;
        self.rest = Rest::default();
        for figure in self.figures.values_mut() {
            figure.motion.fresh = true;
            figure.grabbing.release();
        }
        Ok(())
    }

    #[must_use]
    pub fn steps(&self) -> u64 {
        self.steps
    }

    #[must_use]
    pub fn last_substeps(&self) -> u32 {
        self.substeps
    }

    #[must_use]
    pub fn rate(&self) -> f32 {
        self.settings.rate
    }

    pub fn set_profiling(&mut self, profiling: bool) {
        self.profiling = profiling;
        if !profiling {
            self.timing = None;
        }
    }

    pub fn profile(&mut self) -> Result<Vec<Span>> {
        match self.timing.take() {
            Some(timing) => Ok(timing.wait()?),
            None => Ok(Vec::new()),
        }
    }

    pub(crate) fn boundaries(&self) -> u32 {
        self.settings.effort.epochs + 1
    }

    pub(crate) fn figure(&self, figure: FigureId) -> Result<&Figure> {
        self.figures
            .get(&figure)
            .ok_or(Error::UnknownFigure(figure))
    }

    pub(crate) fn figure_mut(&mut self, figure: FigureId) -> Result<&mut Figure> {
        self.figures
            .get_mut(&figure)
            .ok_or(Error::UnknownFigure(figure))
    }
}

impl std::fmt::Debug for Cloth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Cloth")
            .field("figures", &self.figures.len())
            .field("steps", &self.steps)
            .finish_non_exhaustive()
    }
}

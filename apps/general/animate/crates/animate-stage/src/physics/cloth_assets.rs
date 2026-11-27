use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};

use animate_cast::FigureId;
use rikka_model::Model;
use rikka_model::humanoid::Anatomy;
use rikka_physics_tailor::{Budget, ClothAnalysis, ClothAsset, ClothSettings, Style};
use rustc_hash::FxHashMap;

use super::ClothError;

const CLOTH_ASSET_THREAD: &str = "rikka-animate-tailor";

type Restyles = Vec<(usize, Style)>;

#[derive(Clone, Debug)]
pub(crate) struct FigureAsset {
    pub cloth_asset: Arc<ClothAsset>,
    cloth_settings: Arc<ClothSettings>,
    revision: u64,
    budget: Budget,
}

impl FigureAsset {
    fn fits(&self, revision: u64, budget: Budget) -> bool {
        self.revision == revision && self.budget == budget
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum Fit {
    Cut,
    Restyled(Restyles),
}

pub(super) struct FitRequest {
    figure: FigureId,
    revision: u64,
    budget: Budget,
    model: Arc<Model>,
    anatomy: Arc<Anatomy>,
    cloth_settings: Arc<ClothSettings>,
    analysis: Option<Arc<ClothAnalysis>>,
    current: Option<FigureAsset>,
}

struct FitResult {
    figure: FigureId,
    revision: u64,
    budget: Budget,
    outcome: Result<(FigureAsset, Fit), ClothError>,
}

struct Worker {
    requests: Sender<FitRequest>,
    fitted: Receiver<FitResult>,
}

impl Worker {
    fn spawn() -> Result<Self, ClothError> {
        let (requests, request_receiver) = mpsc::channel();
        let (handed, fitted) = mpsc::channel();
        std::thread::Builder::new()
            .name(CLOTH_ASSET_THREAD.to_owned())
            .spawn(move || run_worker(&request_receiver, &handed))
            .map_err(ClothError::Worker)?;
        Ok(Self { requests, fitted })
    }
}

#[derive(Default)]
pub(super) struct ClothAssets {
    current: FxHashMap<FigureId, FigureAsset>,
    requested: FxHashMap<FigureId, (u64, Budget)>,
    refused: FxHashMap<FigureId, (u64, Budget)>,
    ready: Vec<(FigureId, Result<Fit, ClothError>)>,
    worker: Option<Worker>,
}

pub(crate) struct Wanted<'a> {
    pub(crate) figure: FigureId,
    pub(crate) model: &'a Arc<Model>,
    pub(crate) anatomy: &'a Arc<Anatomy>,
    pub(crate) cloth_settings: &'a Arc<ClothSettings>,
    pub(crate) analysis: Option<&'a Arc<ClothAnalysis>>,
    pub(crate) revision: u64,
}

impl ClothAssets {
    pub(super) fn want(&mut self, wanted: Wanted<'_>, budget: Budget) -> Result<(), ClothError> {
        let Wanted {
            figure,
            model,
            anatomy,
            cloth_settings,
            analysis,
            revision,
        } = wanted;
        if let Some((requested, requested_budget)) = self.requested.get(&figure) {
            if *requested == revision && *requested_budget == budget {
                return Ok(());
            }
        } else if self
            .current
            .get(&figure)
            .is_some_and(|current| current.fits(revision, budget))
            || self.refused.get(&figure) == Some(&(revision, budget))
        {
            return Ok(());
        }
        let request = FitRequest {
            figure,
            revision,
            budget,
            model: Arc::clone(model),
            anatomy: Arc::clone(anatomy),
            cloth_settings: Arc::clone(cloth_settings),
            analysis: analysis.cloned(),
            current: self.current.get(&figure).cloned(),
        };
        self.worker()?
            .requests
            .send(request)
            .map_err(|_| ClothError::WorkerStopped)?;
        self.requested.insert(figure, (revision, budget));
        Ok(())
    }

    fn worker(&mut self) -> Result<&Worker, ClothError> {
        match &mut self.worker {
            Some(worker) => Ok(worker),
            slot @ None => Ok(slot.insert(Worker::spawn()?)),
        }
    }

    pub(super) fn collect(&mut self) -> Vec<(FigureId, Result<Fit, ClothError>)> {
        let mut loaded = std::mem::take(&mut self.ready);
        while let Some(worker) = &self.worker {
            match worker.fitted.try_recv() {
                Ok(fitted) => self.take(fitted, &mut loaded),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => self.stop(&mut loaded),
            }
        }
        loaded
    }

    pub(super) fn wait(&mut self) {
        let mut ready = std::mem::take(&mut self.ready);
        while !self.requested.is_empty()
            && let Some(worker) = &self.worker
        {
            match worker.fitted.recv() {
                Ok(fitted) => self.take(fitted, &mut ready),
                Err(_) => self.stop(&mut ready),
            }
        }
        self.ready = ready;
    }

    fn stop(&mut self, loaded: &mut Vec<(FigureId, Result<Fit, ClothError>)>) {
        for figure in self.requested.keys() {
            loaded.push((*figure, Err(ClothError::WorkerStopped)));
        }
        self.requested.clear();
        self.worker = None;
    }

    fn take(&mut self, fitted: FitResult, loaded: &mut Vec<(FigureId, Result<Fit, ClothError>)>) {
        if self.requested.get(&fitted.figure) != Some(&(fitted.revision, fitted.budget)) {
            return;
        }
        self.requested.remove(&fitted.figure);
        match fitted.outcome {
            Ok((current, fit)) => {
                self.refused.remove(&fitted.figure);
                self.current.insert(fitted.figure, current);
                loaded.push((fitted.figure, Ok(fit)));
            }
            Err(error) => {
                self.refused
                    .insert(fitted.figure, (fitted.revision, fitted.budget));
                loaded.push((fitted.figure, Err(error)));
            }
        }
    }

    pub(super) fn forget(&mut self, figure: FigureId) {
        self.current.remove(&figure);
        self.requested.remove(&figure);
        self.refused.remove(&figure);
        self.ready.retain(|(ready, _)| *ready != figure);
    }

    pub(super) fn holds(&self, figure: FigureId) -> bool {
        self.current.contains_key(&figure) || self.requested.contains_key(&figure)
    }

    pub(super) fn is_busy(&self) -> bool {
        !self.requested.is_empty()
    }

    pub(super) fn current(&self, figure: FigureId) -> Option<&FigureAsset> {
        self.current.get(&figure)
    }

    pub(super) fn all(&self) -> impl Iterator<Item = (FigureId, &FigureAsset)> {
        self.current
            .iter()
            .filter(|(_, current)| !current.cloth_asset.pieces.is_empty())
            .map(|(figure, current)| (*figure, current))
    }

    pub(super) fn replicate(&self) -> Self {
        Self {
            current: self.current.clone(),
            requested: FxHashMap::default(),
            refused: self.refused.clone(),
            ready: Vec::new(),
            worker: None,
        }
    }
}

impl std::fmt::Debug for ClothAssets {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClothAssets")
            .field("current", &self.current.len())
            .field("requested", &self.requested.len())
            .finish_non_exhaustive()
    }
}

fn run_worker(requests: &Receiver<FitRequest>, fitted: &Sender<FitResult>) {
    while let Ok(first) = requests.recv() {
        let mut latest = vec![first];
        while let Ok(more) = requests.try_recv() {
            latest.retain(|queued| queued.figure != more.figure);
            latest.push(more);
        }
        for request in latest {
            let handed = FitResult {
                figure: request.figure,
                revision: request.revision,
                budget: request.budget,
                outcome: fit(&request),
            };
            if fitted.send(handed).is_err() {
                return;
            }
        }
    }
}

pub(super) fn fit(request: &FitRequest) -> Result<(FigureAsset, Fit), ClothError> {
    let restyled = request
        .current
        .as_ref()
        .filter(|current| {
            current.budget == request.budget
                && request
                    .cloth_settings
                    .same_topology(&current.cloth_settings)
        })
        .and_then(|current| restyle(request, &current.cloth_asset));
    let (cloth_asset, fit) = match restyled {
        Some((cloth_asset, changed)) => (cloth_asset, Fit::Restyled(changed)),
        None => {
            let cloth_asset = match &request.analysis {
                Some(analysis) => ClothAsset::from_analysis(
                    &request.model,
                    &request.anatomy,
                    analysis,
                    request.budget,
                )?,
                None => ClothAsset::build(
                    &request.model,
                    &request.anatomy,
                    &request.cloth_settings,
                    request.budget,
                )?,
            };
            (Arc::new(cloth_asset), Fit::Cut)
        }
    };
    Ok((
        FigureAsset {
            cloth_asset,
            cloth_settings: Arc::clone(&request.cloth_settings),
            revision: request.revision,
            budget: request.budget,
        },
        fit,
    ))
}

fn restyle(request: &FitRequest, cloth_asset: &ClothAsset) -> Option<(Arc<ClothAsset>, Restyles)> {
    let analysis = match &request.analysis {
        Some(analysis) => Arc::clone(analysis),
        None => Arc::new(
            ClothAnalysis::new(&request.model, &request.anatomy, &request.cloth_settings).ok()?,
        ),
    };
    let mut restyled = cloth_asset.clone();
    let mut changed = Vec::new();
    for (index, piece) in restyled.pieces.iter_mut().enumerate() {
        let analyzed = analysis
            .pieces()
            .iter()
            .find(|analyzed| analyzed.key == piece.key)?;
        if analyzed.style.thickness != piece.style.thickness {
            return None;
        }
        if analyzed.style != piece.style {
            piece.style = analyzed.style.clone();
            changed.push((index, analyzed.style.clone()));
        }
    }
    Some((Arc::new(restyled), changed))
}

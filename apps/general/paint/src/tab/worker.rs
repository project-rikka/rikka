use std::cell::Cell;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Weak};
use std::time::Duration;

use paint_document::{
    Blank, Change, Document, Edit, Error, Figure, FigureIds, Layer, LayerId, PixelChange, Region,
    Saved, Saving, Thumbnail, Tile,
};
use rikka_image_edit::{Histogram, Imaging, Picture, Recipe};
use rikka_types::Color;

use super::detached::Detached;
use super::facts::DocumentInfo;
use super::history_view::HistoryView;

const TILE: u32 = 2048;
const THUMBNAIL: u32 = 96;
const GPU_WAIT_TIMEOUT: Duration = Duration::from_secs(10);
const REST: Duration = Duration::from_secs(3);
const THREAD_NAME: &str = "rikka-paint-document";

#[derive(Debug)]
pub(crate) enum Report {
    Opened {
        facts: DocumentInfo,
        layers: Vec<Layer>,
        figure_ids: FigureIds,
    },
    Failed(Error),
    Changed {
        facts: DocumentInfo,
        layers: Vec<Layer>,
        canvas: bool,
        active_layer: Option<LayerId>,
    },
    Drawn(u64, Vec<Tile>),
    Undrawn(u64, Error),
    Unchanged(u64),
    Refused(Error),
    Thumbnails(Vec<(LayerId, Thumbnail)>),
    ThumbnailRefresh(Vec<(LayerId, Thumbnail)>),
    Chronicled(HistoryView),
    Selected {
        generation: u64,
        region: Region,
    },
    Previewed {
        preview: u64,
        tiles: Vec<Tile>,
    },
    Unpreviewed {
        preview: u64,
        error: Error,
    },
    Measured {
        request_id: u64,
        histogram: Histogram,
    },
    Saved {
        path: PathBuf,
        saved: Saved,
        facts: DocumentInfo,
    },
    Unsaved(Error),
    Sampled {
        request_id: u64,
        color: Color,
    },
    OrderDone {
        generation: u64,
        refusal: Option<String>,
        notes: Vec<String>,
    },
}

pub(crate) struct Refusal(pub(crate) String);

impl From<Error> for Refusal {
    fn from(error: Error) -> Refusal {
        Refusal(error.to_string())
    }
}

type Composer = Box<dyn FnOnce(&mut Document) -> Result<Vec<String>, Refusal> + Send>;

pub(crate) struct Composition {
    name: String,
    compose: Composer,
}

impl Composition {
    pub(crate) fn new(
        name: String,
        compose: impl FnOnce(&mut Document) -> Result<Vec<String>, Refusal> + Send + 'static,
    ) -> Composition {
        Composition {
            name,
            compose: Box::new(compose),
        }
    }
}

pub(crate) struct WorkerView<'a> {
    pub(crate) document: &'a mut Document,
    pub(crate) before: Option<&'a Picture>,
}

pub(crate) struct Reading(Box<dyn FnOnce(WorkerView<'_>) + Send>);

impl Reading {
    pub(crate) fn new(reading: impl FnOnce(WorkerView<'_>) + Send + 'static) -> Reading {
        Reading(Box::new(reading))
    }
}

#[derive(Debug)]
struct Previewing {
    preview: u64,
    layer: LayerId,
    content: PreviewContent,
    scale: f32,
}

#[derive(Debug)]
enum PreviewContent {
    Developed(Recipe),
    Figures(Vec<Figure>),
}

#[derive(Debug)]
enum Order {
    Edit {
        generation: u64,
        edit: Edit,
    },
    Preview(Previewing),
    EndPreview,
    Histogram {
        request_id: u64,
        layer: LayerId,
        within: Option<Region>,
    },
    Save {
        path: PathBuf,
        saving: Saving,
    },
    Sample {
        request_id: u64,
        at: (u32, u32),
        size: u32,
        layer: Option<LayerId>,
    },
    Errand {
        generation: u64,
        edit: Edit,
    },
    Read(Reading),
    Compose {
        generation: u64,
        composition: Composition,
    },
    Redraw,
}

impl std::fmt::Debug for Composition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Composition")
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

impl std::fmt::Debug for Reading {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Reading")
    }
}

#[derive(Debug)]
pub(crate) enum Source {
    File(PathBuf),
    Blank(Blank),
}

impl Source {
    fn document(&self, imaging: &Imaging) -> paint_document::Result<Document> {
        match self {
            Source::File(path) => Document::open(path, imaging),
            Source::Blank(blank) => Document::new_blank(blank, imaging),
        }
    }
}

#[derive(Debug)]
pub(crate) struct Worker {
    orders: mpsc::Sender<Order>,
    level: Arc<AtomicU32>,
    _alive: Arc<()>,
    _thread: Detached,
}

#[derive(Clone, Debug)]
pub(crate) struct Detail {
    orders: mpsc::Sender<Order>,
    level: Arc<AtomicU32>,
}

impl Detail {
    pub(crate) fn want(&self, level: u32) {
        if self.level.swap(level, Ordering::Relaxed) != level {
            let _ = self.orders.send(Order::Redraw);
        }
    }
}

const UNASKED: u32 = u32::MAX;

struct Drawing {
    level: Arc<AtomicU32>,
    last: Cell<Option<(u64, u32)>>,
}

impl Drawing {
    fn draw(&self, document: &mut Document, generation: u64, report: &impl Fn(Report)) {
        let level = self.level.load(Ordering::Relaxed);
        self.last.set(Some((generation, level)));
        if level == UNASKED {
            return;
        }
        match document.tiles(TILE, level) {
            Ok(tiles) => report(Report::Drawn(generation, tiles)),
            Err(error) => report(Report::Undrawn(generation, error)),
        }
    }

    fn follow(&self, document: &mut Document, report: &impl Fn(Report)) {
        let Some((generation, drawn)) = self.last.get() else {
            return;
        };
        if drawn != self.level.load(Ordering::Relaxed) {
            self.draw(document, generation, report);
        }
    }
}

impl Worker {
    pub(crate) fn open(
        source: Source,
        report: impl Fn(Report) + Send + 'static,
    ) -> std::io::Result<Worker> {
        let (orders, inbox) = mpsc::channel();
        let alive = Arc::new(());
        let watched = Arc::downgrade(&alive);
        let level = Arc::new(AtomicU32::new(UNASKED));
        let drawing = Drawing {
            level: Arc::clone(&level),
            last: Cell::new(None),
        };
        let thread = Detached::spawn(THREAD_NAME, move || {
            work(&source, &inbox, &watched, &drawing, &report);
        })?;
        Ok(Worker {
            orders,
            level,
            _alive: alive,
            _thread: thread,
        })
    }

    pub(crate) fn detail(&self) -> Detail {
        Detail {
            orders: self.orders.clone(),
            level: Arc::clone(&self.level),
        }
    }

    pub(crate) fn edit(&self, generation: u64, edit: Edit) {
        self.send(Order::Edit { generation, edit });
    }

    pub(crate) fn preview(&self, preview: u64, layer: LayerId, recipe: Recipe, scale: f32) {
        self.send(Order::Preview(Previewing {
            preview,
            layer,
            content: PreviewContent::Developed(recipe),
            scale,
        }));
    }

    pub(crate) fn preview_figures(
        &self,
        preview: u64,
        layer: LayerId,
        figures: Vec<Figure>,
        scale: f32,
    ) {
        self.send(Order::Preview(Previewing {
            preview,
            layer,
            content: PreviewContent::Figures(figures),
            scale,
        }));
    }

    pub(crate) fn end_preview(&self) {
        self.send(Order::EndPreview);
    }

    pub(crate) fn measure(&self, request_id: u64, layer: LayerId, within: Option<Region>) {
        self.send(Order::Histogram {
            request_id,
            layer,
            within,
        });
    }

    pub(crate) fn save(&self, path: PathBuf, saving: Saving) {
        self.send(Order::Save { path, saving });
    }

    pub(crate) fn sample(
        &self,
        request_id: u64,
        at: (u32, u32),
        size: u32,
        layer: Option<LayerId>,
    ) {
        self.send(Order::Sample {
            request_id,
            at,
            size,
            layer,
        });
    }

    pub(crate) fn errand(&self, generation: u64, edit: Edit) {
        self.send(Order::Errand { generation, edit });
    }

    pub(crate) fn read(&self, reading: Reading) {
        self.send(Order::Read(reading));
    }

    pub(crate) fn compose(&self, generation: u64, composition: Composition) {
        self.send(Order::Compose {
            generation,
            composition,
        });
    }

    fn send(&self, order: Order) {
        let _ = self.orders.send(order);
    }
}

fn work(
    source: &Source,
    inbox: &Receiver<Order>,
    alive: &Weak<()>,
    drawing: &Drawing,
    report: &impl Fn(Report),
) {
    let Some(compute) = rikka_picture::SharedGpu::compute(GPU_WAIT_TIMEOUT) else {
        return report(Report::Failed(Error::NoGpu));
    };
    let imaging = Imaging::shared(&compute);
    serve(source, &imaging, inbox, alive, drawing, report);
    if let Err(error) = imaging.compute().wait_idle() {
        tracing::warn!(%error, "closed document's GPU work did not finish");
    }
}

fn next_order(inbox: &Receiver<Order>, imaging: &Imaging) -> Option<Order> {
    match inbox.recv_timeout(REST) {
        Ok(order) => Some(order),
        Err(mpsc::RecvTimeoutError::Timeout) => {
            imaging.compute().trim();
            inbox.recv().ok()
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => None,
    }
}

fn serve(
    source: &Source,
    imaging: &Imaging,
    inbox: &Receiver<Order>,
    alive: &Weak<()>,
    drawing: &Drawing,
    report: &impl Fn(Report),
) {
    let is_closed = || alive.strong_count() == 0;
    let mut document = match source.document(imaging) {
        Ok(document) => document,
        Err(error) => return report(Report::Failed(error)),
    };
    if is_closed() {
        return;
    }
    report(Report::Opened {
        facts: DocumentInfo::from_document(&document),
        layers: document.layers(),
        figure_ids: document.figure_ids(),
    });
    report(Report::Chronicled(HistoryView::from_document(&document)));
    drawing.draw(&mut document, 0, report);
    if is_closed() {
        return;
    }
    report(Report::Thumbnails(document.thumbnails(THUMBNAIL)));
    let mut before = None;
    while let Some(first) = next_order(inbox, imaging) {
        let mut batch = Batch::default();
        let mut preview = None;
        let mut redraw = false;
        for order in std::iter::once(first).chain(inbox.try_iter()) {
            match order {
                Order::Edit { generation, edit } => {
                    preview = None;
                    batch.apply(&mut document, generation, &edit);
                }
                Order::Preview(previewing) => preview = Some(previewing),
                Order::EndPreview => {
                    preview = None;
                    document.end_preview();
                }
                Order::Histogram {
                    request_id,
                    layer,
                    within,
                } => {
                    batch.settle(&mut document, drawing, report);
                    match document.histogram(layer, within.as_ref()) {
                        Ok(histogram) => report(Report::Measured {
                            request_id,
                            histogram,
                        }),
                        Err(error) => report(Report::Refused(error)),
                    }
                }
                Order::Save { path, saving } => {
                    batch.settle(&mut document, drawing, report);
                    save(&mut document, path, &saving, report);
                }
                Order::Sample {
                    request_id,
                    at: (x, y),
                    size,
                    layer,
                } => {
                    batch.settle(&mut document, drawing, report);
                    match document.sample(x, y, size, layer) {
                        Ok(color) => report(Report::Sampled { request_id, color }),
                        Err(error) => report(Report::Refused(error)),
                    }
                }
                Order::Errand { generation, edit } => {
                    preview = None;
                    batch.settle(&mut document, drawing, report);
                    let before_composite =
                        looks(&edit).then(|| document.composite().ok()).flatten();
                    batch.apply(&mut document, generation, &edit);
                    let refusal = batch.refusal.take();
                    if refusal.is_none()
                        && before_composite.is_some()
                        && batch.change.pixels != PixelChange::Unchanged
                    {
                        before = before_composite;
                    }
                    batch.settle(&mut document, drawing, report);
                    report(Report::OrderDone {
                        generation,
                        refusal: refusal.map(|refusal| refusal.to_string()),
                        notes: Vec::new(),
                    });
                }
                Order::Compose {
                    generation,
                    composition,
                } => {
                    preview = None;
                    batch.settle(&mut document, drawing, report);
                    let composite = document.composite().ok();
                    let Composition { name, compose } = composition;
                    let (refusal, notes) = match document.compose(&name, compose) {
                        Ok((notes, change)) => {
                            if change.pixels != PixelChange::Unchanged {
                                before = composite;
                            }
                            batch.landed(generation, change);
                            (None, notes)
                        }
                        Err(Refusal(reason)) => {
                            batch.generation = Some(generation);
                            (Some(reason), Vec::new())
                        }
                    };
                    batch.settle(&mut document, drawing, report);
                    report(Report::OrderDone {
                        generation,
                        refusal,
                        notes,
                    });
                }
                Order::Redraw => redraw = true,
                Order::Read(reading) => {
                    batch.settle(&mut document, drawing, report);
                    (reading.0)(WorkerView {
                        document: &mut document,
                        before: before.as_ref(),
                    });
                }
            }
            if is_closed() {
                return;
            }
        }
        batch.settle(&mut document, drawing, report);
        if redraw {
            drawing.follow(&mut document, report);
        }
        if let Some(previewing) = preview {
            show_preview(&mut document, previewing, report);
        }
        if is_closed() {
            return;
        }
    }
}

#[derive(Default)]
enum ThumbnailRefresh {
    #[default]
    Nothing,
    Layers(Vec<LayerId>),
    Everything,
}

impl ThumbnailRefresh {
    fn note(&mut self, edit: &Edit) {
        let layer = match edit {
            Edit::Show { .. }
            | Edit::RenameLayer { .. }
            | Edit::SetOpacity { .. }
            | Edit::SetBlend { .. }
            | Edit::SetProperties { .. } => return,
            Edit::Batch { edits, .. } => {
                edits.iter().for_each(|edit| self.note(edit));
                return;
            }
            Edit::Develop { layer, .. }
            | Edit::Fill { layer, .. }
            | Edit::Erase { layer, .. }
            | Edit::Bucket { layer, .. }
            | Edit::Figures { layer, .. } => *layer,
            _ => {
                *self = ThumbnailRefresh::Everything;
                return;
            }
        };
        match self {
            ThumbnailRefresh::Nothing => *self = ThumbnailRefresh::Layers(vec![layer]),
            ThumbnailRefresh::Layers(layers) if !layers.contains(&layer) => layers.push(layer),
            ThumbnailRefresh::Layers(_) | ThumbnailRefresh::Everything => {}
        }
    }

    fn report(self, document: &mut Document, report: &impl Fn(Report)) {
        match self {
            ThumbnailRefresh::Nothing => {}
            ThumbnailRefresh::Layers(layers) => report(Report::ThumbnailRefresh(
                layers
                    .into_iter()
                    .filter_map(|layer| Some((layer, document.thumbnail(layer, THUMBNAIL)?)))
                    .collect(),
            )),
            ThumbnailRefresh::Everything => {
                report(Report::Thumbnails(document.thumbnails(THUMBNAIL)))
            }
        }
    }
}

#[derive(Default)]
struct Batch {
    generation: Option<u64>,
    change: Change,
    retouched: ThumbnailRefresh,
    refusal: Option<Error>,
}

impl Batch {
    fn apply(&mut self, document: &mut Document, generation: u64, edit: &Edit) {
        self.generation = Some(generation);
        match document.apply(edit) {
            Ok(made) => {
                if made.pixels != PixelChange::Unchanged {
                    self.retouched.note(edit);
                }
                self.change = self.change.and(made);
            }
            Err(error) => self.refusal = Some(error),
        }
    }

    fn landed(&mut self, generation: u64, change: Change) {
        self.generation = Some(generation);
        if change.pixels != PixelChange::Unchanged {
            self.retouched = ThumbnailRefresh::Everything;
        }
        self.change = self.change.and(change);
    }

    fn settle(&mut self, document: &mut Document, drawing: &Drawing, report: &impl Fn(Report)) {
        let Some(generation) = self.generation else {
            return;
        };
        let Batch {
            change,
            retouched,
            refusal,
            ..
        } = std::mem::take(self);
        if change.pixels != PixelChange::Unchanged || change.layers {
            report(Report::Changed {
                facts: DocumentInfo::from_document(document),
                layers: document.layers(),
                canvas: change.pixels == PixelChange::Canvas,
                active_layer: change.layer,
            });
        }
        match change.pixels {
            PixelChange::Unchanged => report(Report::Unchanged(generation)),
            PixelChange::Content | PixelChange::Canvas => {
                drawing.draw(document, generation, report)
            }
        }
        if change.selection {
            report(Report::Selected {
                generation,
                region: document.selection().clone(),
            });
        }
        if change.history {
            report(Report::Chronicled(HistoryView::from_document(document)));
        }
        if let Some(error) = refusal {
            report(Report::Refused(error));
        }
        match change.pixels {
            PixelChange::Canvas => ThumbnailRefresh::Everything.report(document, report),
            PixelChange::Content | PixelChange::Unchanged => retouched.report(document, report),
        }
    }
}

fn looks(edit: &Edit) -> bool {
    match edit {
        Edit::Select { .. } => false,
        Edit::Batch { edits, .. } => edits.iter().any(looks),
        _ => true,
    }
}

fn show_preview(document: &mut Document, previewing: Previewing, report: &impl Fn(Report)) {
    let Previewing {
        preview,
        layer,
        content,
        scale,
    } = previewing;
    let tiles = match content {
        PreviewContent::Developed(recipe) => document.preview(layer, &recipe, scale),
        PreviewContent::Figures(figures) => document.preview_figures(layer, &figures, scale),
    };
    match tiles {
        Ok(tiles) => report(Report::Previewed { preview, tiles }),
        Err(error) => report(Report::Unpreviewed { preview, error }),
    }
}

fn save(document: &mut Document, path: PathBuf, saving: &Saving, report: &impl Fn(Report)) {
    match document.save(&path, saving) {
        Ok(saved) => {
            report(Report::Saved {
                path,
                saved,
                facts: DocumentInfo::from_document(document),
            });
            report(Report::Chronicled(HistoryView::from_document(document)));
        }
        Err(error) => report(Report::Unsaved(error)),
    }
}

mod arranging;
mod detached;
mod drawing;
mod facts;
mod figuring;
mod history_view;
mod session;
mod typing;
mod worker;

use rikka_sync::Mutex;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use paint_document::{
    Blank, Combine, Edit, FigureIds, Layer, LayerId, LayerKind, Region, Reshape, Saved, Saving,
    SelectionAction, Shape,
};
use rikka_app::prelude::*;
use rikka_image_edit::Histogram;
use rikka_ui::ImageSource;

use crate::camera::Camera;
use crate::colors::Slot;
use crate::figures::{FigureSelection, Restyle};
use crate::ink::Ink;
use crate::paint::Paint;
use crate::picture::{Picture, SwapProgress};
use crate::pointer::Pointer;
use crate::selection::Selection;
use crate::session::{Context, Session};
use crate::thumbnail::source_of;
use crate::typing::Typing;
use crate::wording;

use arranging::Arranging;
pub(crate) use arranging::Layering;
pub(crate) use detached::drain;
pub(crate) use facts::DocumentInfo;
use figuring::LiftedFigures;
pub(crate) use history_view::HistoryView;
pub(crate) use worker::{Composition, Detail, Reading, Refusal, Report, WorkerView};
use worker::{Source, Worker};

static NEXT_TAB: AtomicU64 = AtomicU64::new(0);

type Mailbox = Arc<Mutex<Sender<Paint>>>;

#[derive(Clone, Debug)]
pub(crate) enum Phase {
    Opening,
    Open(DocumentInfo),
    Failed(Label),
}

#[derive(Debug)]
pub(crate) enum TabEvent {
    Nothing,
    Opened(&'static str),
    Saved {
        path: PathBuf,
        mime_type: &'static str,
    },
    Copied {
        path: PathBuf,
    },
    Unsaved,
    Sampled {
        color: Color,
        slot: Slot,
    },
    OrderDone {
        generation: u64,
        refusal: Option<String>,
        notes: Vec<String>,
    },
}

#[derive(Debug, Default)]
struct Previews {
    request_id: u64,
    answered: u64,
    floor: u64,
    scale: f32,
    retire: Option<u64>,
}

#[derive(Debug, Default)]
struct Measured {
    request_id: u64,
    histogram: Option<Histogram>,
}

#[derive(Debug)]
struct Sampled {
    request_id: u64,
    slot: Slot,
}

impl Default for Sampled {
    fn default() -> Sampled {
        Sampled {
            request_id: 0,
            slot: Slot::Primary,
        }
    }
}

pub(crate) struct Tab {
    id: u64,
    path: Option<PathBuf>,
    untitled: String,
    worker: Option<Worker>,
    mailbox: Mailbox,
    phase: Phase,
    layers: Rc<Vec<Layer>>,
    closed: HashSet<LayerId>,
    thumbnails: HashMap<LayerId, ImageSource>,
    active_layer: Option<LayerId>,
    failure: Option<Label>,
    ordered: u64,
    drawn: Option<u64>,
    reshaped: Option<Size>,
    reshaping: Option<u64>,
    selected: u64,
    history_view: HistoryView,
    session: Option<Session>,
    previews: Previews,
    measured: Measured,
    saving: Option<PathBuf>,
    camera: Camera,
    picture: Picture,
    pointer: Pointer,
    selection: Selection,
    ink: Ink,
    sampled: Sampled,
    typing: Typing,
    arranging: Arranging,
    figure_ids: Option<FigureIds>,
    picked: FigureSelection,
    lift: Option<LiftedFigures>,
    figure_menu: Option<Point>,
    restyling: Option<Restyle>,
}

impl Tab {
    pub(crate) fn open(path: PathBuf, sender: Sender<Paint>) -> Tab {
        Tab::start(
            Source::File(path.clone()),
            Some(path),
            String::new(),
            sender,
        )
    }

    pub(crate) fn create(blank: Blank, untitled: String, sender: Sender<Paint>) -> Tab {
        Tab::start(Source::Blank(blank), None, untitled, sender)
    }

    fn start(
        source: Source,
        path: Option<PathBuf>,
        untitled: String,
        sender: Sender<Paint>,
    ) -> Tab {
        let id = NEXT_TAB.fetch_add(1, Ordering::Relaxed);
        let mailbox: Mailbox = Arc::new(Mutex::new(sender));
        let posted = Arc::clone(&mailbox);
        let worker = Worker::open(source, move |report| {
            let sender = posted.lock().clone();
            let _ = sender.send(move |paint: &mut Paint| paint.report(id, report));
        });
        let (worker, phase) = match worker {
            Ok(worker) => (Some(worker), Phase::Opening),
            Err(error) => (
                None,
                Phase::Failed(tr!("paint.trouble.unstarted", reason = &error.to_string())),
            ),
        };
        let picture = Picture::default();
        if let Some(worker) = &worker {
            picture.drawn_by(worker.detail());
        }
        Tab {
            id,
            path,
            untitled,
            worker,
            mailbox,
            phase,
            layers: Rc::default(),
            closed: HashSet::new(),
            thumbnails: HashMap::new(),
            active_layer: None,
            failure: None,
            ordered: 0,
            drawn: None,
            reshaped: None,
            reshaping: None,
            selected: 0,
            history_view: HistoryView::default(),
            session: None,
            previews: Previews::default(),
            measured: Measured::default(),
            saving: None,
            camera: Camera::default(),
            picture,
            pointer: Pointer::default(),
            selection: Selection::default(),
            ink: Ink::default(),
            sampled: Sampled::default(),
            typing: Typing::default(),
            arranging: Arranging::default(),
            figure_ids: None,
            picked: FigureSelection::default(),
            lift: None,
            figure_menu: None,
            restyling: None,
        }
    }

    pub(crate) fn rewire(&self, sender: Sender<Paint>) {
        *self.mailbox.lock() = sender;
    }

    pub(crate) fn id(&self) -> u64 {
        self.id
    }

    pub(crate) fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub(crate) fn name(&self) -> String {
        self.path
            .as_deref()
            .map_or_else(|| self.untitled.clone(), wording::file_name)
    }

    pub(crate) fn title(&self) -> String {
        match self.is_modified() {
            true => format!("• {}", self.name()),
            false => self.name(),
        }
    }

    pub(crate) fn phase(&self) -> &Phase {
        &self.phase
    }

    pub(crate) fn facts(&self) -> Option<&DocumentInfo> {
        match &self.phase {
            Phase::Open(facts) => Some(facts),
            Phase::Opening | Phase::Failed(_) => None,
        }
    }

    pub(crate) fn layers(&self) -> &[Layer] {
        &self.layers
    }

    pub(crate) fn shared_layers(&self) -> Rc<Vec<Layer>> {
        Rc::clone(&self.layers)
    }

    pub(crate) fn is_open_group(&self, layer: LayerId) -> bool {
        !self.closed.contains(&layer)
    }

    pub(crate) fn thumbnail(&self, layer: LayerId) -> Option<&ImageSource> {
        self.thumbnails.get(&layer)
    }

    pub(crate) fn active_layer(&self) -> Option<&Layer> {
        Layer::find(&self.layers, self.active_layer?)
    }

    pub(crate) fn error_message(&self) -> Option<&Label> {
        self.failure.as_ref()
    }

    pub(crate) fn history_view(&self) -> &HistoryView {
        &self.history_view
    }

    pub(crate) fn is_modified(&self) -> bool {
        self.history_view.is_modified()
    }

    pub(crate) fn session(&self) -> Option<&Session> {
        self.session.as_ref()
    }

    pub(crate) fn is_saving(&self) -> bool {
        self.saving.is_some()
    }

    pub(crate) fn camera(&self) -> &Camera {
        &self.camera
    }

    pub(crate) fn picture(&self) -> &Picture {
        &self.picture
    }

    pub(crate) fn pointer(&self) -> &Pointer {
        &self.pointer
    }

    pub(crate) fn selection(&self) -> &Selection {
        &self.selection
    }

    pub(crate) fn ink(&self) -> &Ink {
        &self.ink
    }

    pub(crate) fn canvas_area(&self) -> Option<paint_document::Area> {
        self.facts().map(DocumentInfo::canvas)
    }

    pub(crate) fn is_loaded(&self) -> bool {
        self.facts().is_some() && self.picture.is_shown()
    }

    pub(crate) fn edits(&self) -> bool {
        self.worker.is_some() && self.is_loaded() && self.session.is_none()
    }

    pub(crate) fn takes_selection(&self) -> bool {
        self.edits() && self.reshaping.is_none()
    }

    pub(crate) fn has_history(&self) -> bool {
        self.history_view.can_undo() || self.drawn != Some(self.ordered)
    }

    pub(crate) fn context(&self) -> Option<Context<'_>> {
        Some(Context {
            facts: self.facts()?,
            layer: self.active_layer(),
            selection: self.selection.region(),
            histogram: self.measured.histogram.as_ref(),
        })
    }

    pub(crate) fn develop_refusal(&self) -> Option<Label> {
        self.context().map_or_else(
            || Some(Label::default()),
            |context| context.develop_refusal(),
        )
    }

    pub(crate) fn activate(&mut self, layer: LayerId) {
        if self.active_layer != Some(layer) {
            self.unpick();
        }
        self.active_layer = Some(layer);
    }

    pub(crate) fn toggle_group(&mut self, layer: LayerId) {
        if !self.closed.remove(&layer) {
            self.closed.insert(layer);
        }
    }

    pub(crate) fn show_layer(&mut self, layer: LayerId, visible: bool) {
        if self.edits()
            && set_visible(
                Rc::make_mut(&mut self.layers).as_mut_slice(),
                layer,
                visible,
            )
        {
            self.order(Edit::Show { layer, visible });
        }
    }

    pub(crate) fn select_all(&mut self) {
        if let Some(canvas) = self.canvas_area() {
            self.reselect(Region::rectangle(canvas), SelectionAction::All);
        }
    }

    pub(crate) fn deselect(&mut self) {
        self.reselect(Region::default(), SelectionAction::Deselect);
    }

    pub(crate) fn invert_selection(&mut self) {
        if let Some(canvas) = self.canvas_area() {
            let inverted = self.selection.region().inverted(canvas);
            self.reselect(inverted, SelectionAction::Invert);
        }
    }

    fn reselect(&mut self, region: Region, by: SelectionAction) {
        if !self.takes_selection() {
            return;
        }
        self.selection.set(region.clone());
        self.order_choice(Edit::Select { region, by });
    }

    pub(crate) fn select_shape(&mut self, shape: Shape, combine: Combine) {
        if self.takes_selection() {
            self.order_choice(Edit::SelectShape { shape, combine });
        }
    }

    pub(crate) fn move_selection(&mut self, (dx, dy): (i32, i32)) {
        if !self.takes_selection() || self.selection.is_empty() {
            return;
        }
        let (x, y) = self.selection.offset();
        self.selection.displace((x + dx, y + dy));
        self.order_choice(Edit::MoveSelection { dx, dy });
    }

    pub(crate) fn selects_layer(&self) -> bool {
        self.takes_selection()
            && self
                .active_layer()
                .is_some_and(|layer| layer.kind == LayerKind::Pixel)
    }

    pub(crate) fn select_layer_pixels(&mut self) {
        if let Some(layer) = self.active_layer
            && self.selects_layer()
        {
            self.order_choice(Edit::SelectLayer {
                layer,
                combine: Combine::Replace,
            });
        }
    }

    fn order_choice(&mut self, edit: Edit) {
        self.order(edit);
        self.selected = self.ordered;
    }

    pub(crate) fn crop_to_selection(&mut self) {
        let region = self.selection.region();
        if !region.is_empty() && self.takes_selection() {
            self.order_reshaping(Edit::Crop(Region::clone(&region)));
        }
    }

    pub(crate) fn reshape(&mut self, reshape: Reshape) {
        if self.edits() {
            self.order_reshaping(Edit::Reshape(reshape));
        }
    }

    pub(crate) fn erase_selection(&mut self) {
        if let Some(layer) = self.active_layer
            && self.edits()
            && self.develop_refusal().is_none()
        {
            self.order(Edit::Erase {
                layer,
                background: Color::WHITE,
            });
        }
    }

    pub(crate) fn travel(&mut self, edit: Edit) {
        if self.edits() {
            self.order(edit);
        }
    }

    pub(crate) fn is_reshaping(&self) -> bool {
        self.reshaping.is_some()
    }

    pub(crate) fn errand(&mut self, edit: Edit) -> Option<u64> {
        let worker = self.worker.as_ref()?;
        self.ordered += 1;
        let reshapes = reshapes(&edit);
        worker.errand(self.ordered, edit);
        if reshapes {
            self.reshaping = Some(self.ordered);
        }
        Some(self.ordered)
    }

    pub(crate) fn compose(&mut self, composition: Composition, reshapes: bool) -> Option<u64> {
        let worker = self.worker.as_ref()?;
        self.ordered += 1;
        worker.compose(self.ordered, composition);
        if reshapes {
            self.reshaping = Some(self.ordered);
        }
        Some(self.ordered)
    }

    pub(crate) fn read(&self, reading: impl FnOnce(WorkerView<'_>) + Send + 'static) -> bool {
        match &self.worker {
            Some(worker) => {
                worker.read(Reading::new(reading));
                true
            }
            None => false,
        }
    }

    fn order_reshaping(&mut self, edit: Edit) {
        self.order(edit);
        self.reshaping = Some(self.ordered);
    }

    fn order(&mut self, edit: Edit) {
        if let Some(worker) = &self.worker {
            self.ordered += 1;
            worker.edit(self.ordered, edit);
        }
    }

    pub(crate) fn save_to(&mut self, path: PathBuf, saving: Saving) {
        if let Some(worker) = &self.worker
            && self.is_loaded()
        {
            worker.save(path.clone(), saving);
            self.saving = Some(path);
        }
    }

    pub(crate) fn report(&mut self, report: Report) -> TabEvent {
        match report {
            Report::Opened {
                facts,
                layers,
                figure_ids,
            } => {
                self.figure_ids = Some(figure_ids);
                let mime_type = facts.format.mime_type();
                self.camera
                    .show(Size::new(facts.width as f32, facts.height as f32));
                self.phase = Phase::Open(facts);
                self.active_layer = layers.first().map(|layer| layer.id);
                self.layers = Rc::new(layers);
                return TabEvent::Opened(mime_type);
            }
            Report::Changed {
                facts,
                layers,
                canvas,
                active_layer,
            } => {
                if canvas {
                    self.reshaped = Some(Size::new(facts.width as f32, facts.height as f32));
                }
                self.phase = Phase::Open(facts);
                self.regroup(&layers);
                if let Some(active_layer) =
                    active_layer.filter(|id| Layer::find(&layers, *id).is_some())
                {
                    self.active_layer = Some(active_layer);
                }
                if self
                    .active_layer
                    .is_none_or(|id| Layer::find(&layers, id).is_none())
                {
                    self.active_layer = layers.first().map(|layer| layer.id);
                }
                self.layers = Rc::new(layers);
                self.keep_picks();
            }
            Report::Failed(error) => {
                self.phase = Phase::Failed(wording::error_message(&error));
            }
            Report::Drawn(generation, tiles) => {
                self.picture.show(generation, tiles);
                self.failure = None;
            }
            Report::Undrawn(generation, error) => {
                if self.normalize_extent() {
                    self.picture.clear();
                }
                self.picture.unchanged(generation);
                self.failure = Some(wording::error_message(&error));
            }
            Report::Unchanged(generation) => self.picture.unchanged(generation),
            Report::Refused(error) => self.failure = Some(wording::error_message(&error)),
            Report::Thumbnails(thumbnails) => {
                self.thumbnails = thumbnails
                    .into_iter()
                    .map(|(layer, thumbnail)| (layer, source_of(thumbnail)))
                    .collect();
            }
            Report::ThumbnailRefresh(thumbnails) => self.thumbnails.extend(
                thumbnails
                    .into_iter()
                    .map(|(layer, thumbnail)| (layer, source_of(thumbnail))),
            ),
            Report::Chronicled(history_view) => self.history_view = history_view,
            Report::Selected { generation, region } => {
                if generation >= self.selected {
                    self.selection.adopt(region);
                }
            }
            Report::Previewed { preview, tiles } => self.previewed(preview, Ok(tiles)),
            Report::Unpreviewed { preview, error } => self.previewed(preview, Err(error)),
            Report::Measured {
                request_id,
                histogram,
            } => {
                if request_id == self.measured.request_id {
                    self.measured.histogram = Some(histogram);
                }
            }
            Report::Sampled { request_id, color } => {
                if request_id == self.sampled.request_id {
                    return TabEvent::Sampled {
                        color,
                        slot: self.sampled.slot,
                    };
                }
            }
            Report::Saved { path, saved, facts } => {
                self.saving = None;
                self.failure = None;
                return match saved {
                    Saved::Document => {
                        let mime_type = facts.format.mime_type();
                        self.path = Some(path.clone());
                        self.phase = Phase::Open(facts);
                        TabEvent::Saved { path, mime_type }
                    }
                    Saved::Copy => TabEvent::Copied { path },
                };
            }
            Report::Unsaved(error) => {
                self.saving = None;
                self.failure = Some(wording::error_message(&error));
                return TabEvent::Unsaved;
            }
            Report::OrderDone {
                generation,
                refusal,
                notes,
            } => {
                return TabEvent::OrderDone {
                    generation,
                    refusal,
                    notes,
                };
            }
        }
        TabEvent::Nothing
    }

    pub(crate) fn advance_picture(&mut self) -> SwapProgress {
        let (id, mailbox) = (self.id, Arc::clone(&self.mailbox));
        let resume = move || {
            let sender = mailbox.lock().clone();
            let resumed = sender.send(move |paint: &mut Paint| paint.resume_picture_swap(id));
            if resumed.is_err() {
                tracing::debug!(tab = id, "window closed before picture swap");
            }
        };
        let progress = self
            .picture
            .advance_swap(self.camera.visible_area(), resume);
        if let Some(generation) = progress.landed {
            self.normalize_extent();
            self.reach(generation);
        }
        progress
    }

    fn normalize_extent(&mut self) -> bool {
        let reshaped = self.reshaped.take();
        if let Some(extent) = reshaped {
            self.camera.show(extent);
        }
        reshaped.is_some()
    }

    fn reach(&mut self, generation: u64) {
        self.drawn = Some(generation);
        self.ink.settled(generation);
        self.typing.settled(generation);
        self.picked.settled(generation);
        if self
            .reshaping
            .is_some_and(|reshaping| reshaping <= generation)
        {
            self.reshaping = None;
        }
        if self
            .previews
            .retire
            .is_some_and(|retire| retire <= generation)
        {
            self.previews.retire = None;
            self.picture.unpreview();
        }
    }
}

impl std::fmt::Debug for Tab {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Tab")
            .field("id", &self.id)
            .field("path", &self.path)
            .field("phase", &self.phase)
            .field("ordered", &self.ordered)
            .field("drawn", &self.drawn)
            .field("chronicle", &self.history_view)
            .field("session", &self.session)
            .field("previews", &self.previews)
            .field("saving", &self.saving)
            .field("trouble", &self.failure)
            .finish_non_exhaustive()
    }
}

fn reshapes(edit: &Edit) -> bool {
    match edit {
        Edit::Crop(_) | Edit::Reshape(_) => true,
        Edit::Batch { edits, .. } => edits.iter().any(reshapes),
        _ => false,
    }
}

fn set_visible(layers: &mut [Layer], id: LayerId, visible: bool) -> bool {
    layers.iter_mut().any(|layer| match layer.id == id {
        true => {
            let changed = layer.visible != visible;
            layer.visible = visible;
            changed
        }
        false => set_visible(&mut layer.children, id, visible),
    })
}

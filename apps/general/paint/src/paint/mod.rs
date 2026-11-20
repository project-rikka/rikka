mod closing;
mod creating;
mod developing;
mod drawing;
mod figuring;
mod layering;
mod saving;
mod serving;
mod typing;

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

use paint_document::{Edit, FileFormat, LayerId, Reshape, readable_mime_types};
use rikka_app::prelude::*;
use rikka_assist::ToolRegistration;
use rikka_dialog::{Chooser, ChooserResponse, Filter};
use rikka_effects::Effect;
use rikka_ui::DragOffer;
use rikka_xdg::RecentFile;

use crate::camera::Camera;
use crate::canvas::Grip;
use crate::colors::Colors;
use crate::new_image::NewImage;
use crate::options::Options;
use crate::panel::Panel;
use crate::session::{Dialog, EffectDialog, Kind, LastEffect, SaveChoices, SaveOptions};
use crate::tab::{Report, Tab, TabEvent};
use crate::tool::Tool;
use crate::{recent, wording};

pub(crate) use closing::UnsavedPrompt;

const PICTURE_SWAP_DEADLINE: &str = "paint.picture.swap_deadline";

#[derive(State)]
pub(crate) struct Paint {
    #[untracked]
    sender: Sender<Paint>,
    tabs: Vec<Tab>,
    active: usize,
    tool: Tool,
    grip: Grip,
    recent: Vec<RecentFile>,
    folder: Option<PathBuf>,
    notice: Option<Label>,
    panel: Panel,
    savings: HashMap<FileFormat, SaveChoices>,
    unsaved_prompt: Option<UnsavedPrompt>,
    colors: Colors,
    options: Options,
    sampled: Option<Color>,
    last_effect: Option<LastEffect>,
    #[untracked]
    seat: Option<ToolRegistration>,
    #[untracked]
    waiting: Vec<serving::Waiting>,
    creating: Option<NewImage>,
}

impl Paint {
    pub(crate) fn new(sender: Sender<Paint>, paths: Vec<PathBuf>) -> Paint {
        let mut paint = Paint::empty(sender);
        paint.open_paths(paths);
        paint
    }

    pub(crate) fn holding(sender: Sender<Paint>, tab: Tab) -> Paint {
        let mut paint = Paint::empty(sender);
        paint.land(0, tab);
        paint
    }

    fn empty(sender: Sender<Paint>) -> Paint {
        Paint {
            sender,
            tabs: Vec::new(),
            active: 0,
            tool: Tool::default(),
            grip: Grip::default(),
            recent: Vec::new(),
            folder: None,
            notice: None,
            panel: Panel::default(),
            savings: HashMap::new(),
            unsaved_prompt: None,
            colors: Colors::default(),
            options: Options::default(),
            sampled: None,
            last_effect: None,
            seat: None,
            waiting: Vec::new(),
            creating: None,
        }
    }

    pub(crate) fn start(&mut self) {
        spawn_blocking(recent::load, |paint: &mut Paint, files| {
            paint.remember(files)
        });
        self.offer();
    }

    pub(crate) fn tabs(&self) -> &[Tab] {
        &self.tabs
    }

    pub(crate) fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    pub(crate) fn active_index(&self) -> usize {
        self.active
    }

    pub(crate) fn active_tab(&self) -> Option<&Tab> {
        self.tabs.get(self.active)
    }

    fn active_tab_mut(&mut self) -> Option<&mut Tab> {
        self.tabs.get_mut(self.active)
    }

    fn tab_mut(&mut self, id: u64) -> Option<&mut Tab> {
        self.tabs.iter_mut().find(|tab| tab.id() == id)
    }

    pub(crate) fn tool(&self) -> Tool {
        self.tool
    }

    pub(crate) fn grip(&self) -> &Grip {
        &self.grip
    }

    pub(crate) fn recent(&self) -> &[RecentFile] {
        &self.recent
    }

    pub(crate) fn notice(&self) -> Option<&Label> {
        self.notice.as_ref()
    }

    pub(crate) fn dismiss_notice(&mut self) {
        self.notice = None;
    }

    pub(crate) fn panel(&self) -> Panel {
        self.panel
    }

    pub(crate) fn show_panel(&mut self, panel: Panel) {
        self.panel = panel;
    }

    pub(crate) fn shows_picture(&self) -> bool {
        self.active_tab().is_some_and(Tab::is_loaded)
    }

    pub(crate) fn can_zoom(&self, inward: bool) -> bool {
        self.active_tab()
            .filter(|tab| tab.is_loaded())
            .is_some_and(|tab| tab.camera().can_zoom(inward))
    }

    pub(crate) fn edits(&self) -> bool {
        self.unsaved_prompt.is_none() && self.active_tab().is_some_and(Tab::edits)
    }

    pub(crate) fn takes_selection(&self) -> bool {
        self.unsaved_prompt.is_none() && self.active_tab().is_some_and(Tab::takes_selection)
    }

    pub(crate) fn has_selection(&self) -> bool {
        self.takes_selection()
            && self
                .active_tab()
                .is_some_and(|tab| !tab.selection().is_empty())
    }

    pub(crate) fn selects_layer(&self) -> bool {
        self.unsaved_prompt.is_none() && self.active_tab().is_some_and(Tab::selects_layer)
    }

    pub(crate) fn can_undo(&self) -> bool {
        self.edits() && self.active_tab().is_some_and(Tab::has_history)
    }

    pub(crate) fn can_redo(&self) -> bool {
        self.edits()
            && self
                .active_tab()
                .is_some_and(|tab| tab.history_view().can_redo())
    }

    pub(crate) fn can_erase(&self) -> bool {
        self.has_selection()
            && self
                .active_tab()
                .is_some_and(|tab| tab.develop_refusal().is_none())
    }

    pub(crate) fn in_session(&self) -> bool {
        self.unsaved_prompt.is_none()
            && self.active_tab().is_some_and(|tab| tab.session().is_some())
    }

    pub(crate) fn open_paths(&mut self, paths: Vec<PathBuf>) {
        for path in paths {
            match self
                .tabs
                .iter()
                .position(|tab| tab.path() == Some(path.as_path()))
            {
                Some(index) => self.active = index,
                None => {
                    self.tabs.push(Tab::open(path, self.sender.clone()));
                    self.active = self.tabs.len() - 1;
                }
            }
        }
        self.notice = None;
        self.retitle();
    }

    pub(crate) fn show_open_dialog(&mut self) {
        let pictures = readable_mime_types().into_iter().fold(
            Filter::named(tr!("paint.open.pictures").to_string()),
            Filter::mime_type,
        );
        let chooser = Chooser::open(tr!("paint.open.title").to_string())
            .parent(window_identifier())
            .accept_label(tr!("paint.open.accept").to_string())
            .multiple(true)
            .filter(pictures);
        let chooser = match &self.folder {
            Some(folder) => chooser.current_folder(folder),
            None => chooser,
        };
        spawn_blocking(
            move || chooser.run(),
            |paint: &mut Paint, active_layer: rikka_dialog::Result<ChooserResponse>| {
                match active_layer {
                    Ok(ChooserResponse::Files(paths)) => {
                        paint.folder = paths
                            .first()
                            .and_then(|path| path.parent())
                            .map(PathBuf::from);
                        paint.open_paths(paths);
                    }
                    Ok(ChooserResponse::Cancelled) => skip_view(),
                    Err(error) => {
                        paint.notice = Some(tr!("paint.open.unasked", reason = &error.to_string()));
                    }
                }
            },
        );
    }

    pub(crate) fn dropped(offer: DragOffer) {
        spawn_blocking(
            move || {
                offer
                    .uris()
                    .iter()
                    .filter_map(|uri| rikka_open::uri_to_path(uri))
                    .collect::<Vec<PathBuf>>()
            },
            |paint: &mut Paint, paths| paint.open_paths(paths),
        );
    }

    pub(crate) fn report(&mut self, tab: u64, report: Report) {
        let Some(tab_ref) = self.tab_mut(tab) else {
            skip_view();
            return;
        };
        let path = tab_ref.path().map(PathBuf::from);
        let document = tab_ref.name();
        let event = tab_ref.report(report);
        self.advance_picture_swap(tab);
        self.handle_tab_event(tab, &event);
        match event {
            TabEvent::Nothing | TabEvent::OrderDone { .. } => {}
            TabEvent::Opened(mime_type) => {
                if let Some(opened_path) = path {
                    self.note_recent(opened_path, mime_type);
                }
            }
            TabEvent::Saved { path, mime_type } => {
                self.note_recent(path, mime_type);
                self.resolve_unsaved_prompt(tab, true);
            }
            TabEvent::Copied { path } => {
                self.notice = Some(tr!(
                    "paint.save.copied",
                    name = &wording::file_name(&path),
                    document = &document
                ));
                self.resolve_unsaved_prompt(tab, false);
            }
            TabEvent::Unsaved => self.resolve_unsaved_prompt(tab, false),
            TabEvent::Sampled { color, slot } => self.sampled(color, slot),
        }
    }

    pub(crate) fn resume_picture_swap(&mut self, tab: u64) {
        if !self.advance_picture_swap(tab) {
            skip_view();
        }
    }

    fn advance_picture_swap(&mut self, tab: u64) -> bool {
        let Some(tab_ref) = self.tab_mut(tab) else {
            return false;
        };
        let progress = tab_ref.advance_picture();
        if let Some(deadline) = progress.deadline {
            after_keyed(
                (PICTURE_SWAP_DEADLINE, tab),
                deadline.saturating_duration_since(Instant::now()),
                move |paint: &mut Paint| paint.resume_picture_swap(tab),
            );
        }
        progress.landed.is_some()
    }

    fn note_recent(&mut self, path: PathBuf, mime_type: &'static str) {
        spawn_blocking(
            move || recent::note(&path, mime_type),
            |paint: &mut Paint, files| paint.remember(files),
        );
    }

    fn remember(&mut self, files: Vec<RecentFile>) {
        if self.folder.is_none() {
            self.folder = recent::folder_of(&files);
        }
        self.recent = files;
    }

    pub(crate) fn select(&mut self, index: usize) {
        if index < self.tabs.len() {
            self.active = index;
        }
    }

    pub(crate) fn step_tab(&mut self, forward: bool) {
        let count = self.tabs.len();
        if count < 2 {
            return;
        }
        self.active = match forward {
            true => (self.active + 1) % count,
            false => (self.active + count - 1) % count,
        };
    }

    pub(crate) fn lift(&mut self, index: usize) -> Option<Tab> {
        if index >= self.tabs.len() {
            return None;
        }
        let tab = self.tabs.remove(index);
        if index < self.active || self.active >= self.tabs.len() {
            self.active = self.active.saturating_sub(1);
        }
        self.retitle();
        Some(tab)
    }

    pub(crate) fn land(&mut self, at: usize, tab: Tab) {
        tab.rewire(self.sender.clone());
        let id = tab.id();
        let at = at.min(self.tabs.len());
        self.tabs.insert(at, tab);
        self.active = at;
        self.retitle();
        self.advance_picture_swap(id);
    }

    pub(crate) fn settled(&mut self) {
        if self.tabs.is_empty() {
            request_window(WindowRequest::Close);
        }
    }

    pub(crate) fn pick(&mut self, tool: Tool) {
        if tool != Tool::Text {
            self.commit_text();
        }
        if !tool.picks() {
            self.unpick();
        }
        self.tool = tool;
    }

    pub(crate) fn zoom(&mut self, inward: bool) {
        self.aim(|camera| camera.step(inward, None));
    }

    pub(crate) fn fit(&mut self) {
        self.aim(Camera::fit);
    }

    pub(crate) fn actual_size(&mut self) {
        self.zoom_to(1.0);
    }

    pub(crate) fn zoom_to(&mut self, scale: f32) {
        self.aim(|camera| camera.zoom_to(scale, None));
    }

    fn aim(&mut self, change: impl FnOnce(&Camera)) {
        if let Some(tab) = self.active_tab() {
            change(tab.camera());
        }
        self.rescaled();
    }

    pub(crate) fn rescaled(&mut self) {
        if !self.active_tab_mut().is_some_and(Tab::rescaled) {
            skip_view();
        }
    }

    pub(crate) fn select_all(&mut self) {
        self.mark(Tab::select_all);
    }

    pub(crate) fn deselect(&mut self) {
        self.mark(Tab::deselect);
    }

    pub(crate) fn invert_selection(&mut self) {
        self.mark(Tab::invert_selection);
    }

    fn mark(&mut self, change: impl FnOnce(&mut Tab)) {
        if self.unsaved_prompt.is_none()
            && let Some(tab) = self.active_tab_mut()
        {
            change(tab);
        }
        skip_view();
    }

    pub(crate) fn crop_to_selection(&mut self) {
        self.edit(Tab::crop_to_selection);
    }

    pub(crate) fn erase_selection(&mut self) {
        self.edit(Tab::erase_selection);
    }

    pub(crate) fn reshape(&mut self, reshape: Reshape) {
        self.edit(|tab| tab.reshape(reshape));
    }

    pub(crate) fn undo(&mut self) {
        self.edit(|tab| tab.travel(Edit::Undo));
    }

    pub(crate) fn redo(&mut self) {
        self.edit(|tab| tab.travel(Edit::Redo));
    }

    pub(crate) fn revisit(&mut self, index: usize) {
        self.edit(|tab| tab.travel(Edit::Revisit(index)));
    }

    fn edit(&mut self, change: impl FnOnce(&mut Tab)) {
        if self.unsaved_prompt.is_none()
            && let Some(tab) = self.active_tab_mut()
        {
            change(tab);
        }
    }

    pub(crate) fn begin(&mut self, kind: Kind) {
        self.edit(|tab| tab.begin(kind));
    }

    pub(crate) fn tune_effect(&mut self, effect: Effect) {
        let unchanged = self
            .active_tab()
            .and_then(Tab::session)
            .and_then(|session| session.dialog_as::<EffectDialog>())
            .is_some_and(|dialog| *dialog.effect() == effect);
        match unchanged {
            true => skip_view(),
            false => self.tune(|dialog: &mut EffectDialog| dialog.set(effect)),
        }
    }

    pub(crate) fn tune<D: Dialog>(&mut self, change: impl FnOnce(&mut D)) {
        if !self.active_tab_mut().is_some_and(|tab| tab.tune(change)) {
            skip_view();
        }
    }

    pub(crate) fn preview_session(&mut self, previewing: bool) {
        self.edit(|tab| tab.preview_session(previewing));
    }

    pub(crate) fn reset_session(&mut self) {
        self.edit(Tab::reset_session);
    }

    pub(crate) fn cancel_session(&mut self) {
        self.edit(Tab::cancel_session);
    }

    pub(crate) fn apply_session(&mut self) {
        let active_session = self.active_tab().and_then(Tab::session);
        let remembered = active_session
            .and_then(|session| session.dialog_as::<SaveOptions>())
            .map(SaveOptions::remembered);
        let last_effect = self.active_tab().and_then(|tab| {
            let context = tab.context()?;
            tab.session()?.last_effect(&context)
        });
        self.edit(Tab::apply_session);
        if let Some((format, choices)) = remembered {
            self.savings.insert(format, choices);
        }
        let applied = self.active_tab().is_some_and(|tab| tab.session().is_none());
        if applied && last_effect.is_some() {
            self.last_effect = last_effect;
        }
    }

    pub(crate) fn activate_layer(&mut self, layer: LayerId) {
        self.edit(|tab| tab.activate(layer));
    }

    pub(crate) fn show_layer(&mut self, layer: LayerId, visible: bool) {
        self.edit(|tab| tab.show_layer(layer, visible));
    }

    pub(crate) fn toggle_group(&mut self, layer: LayerId) {
        if let Some(tab) = self.active_tab_mut() {
            tab.toggle_group(layer);
        }
    }

    pub(crate) fn hold_space(&mut self, pressed: bool) {
        self.grip.hold_space(pressed);
        skip_view();
    }
}

impl std::fmt::Debug for Paint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Paint")
            .field("tabs", &self.tabs)
            .field("active", &self.active)
            .field("tool", &self.tool)
            .field("asking", &self.unsaved_prompt)
            .finish_non_exhaustive()
    }
}

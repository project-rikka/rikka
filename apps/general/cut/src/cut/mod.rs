mod animating;
mod animation_presets;
mod attributing;
mod clipboard;
mod drawing;
mod editing;
mod effecting;
mod exporting;
mod files;
mod importing;
mod keyframing;
mod marking;
mod playing;
mod popup;
mod previewing;
mod reformatting;
mod relinking;
mod session;
mod sheet;
mod styling;
mod timeline_display;
mod transitioning;
mod trimming;
mod tweening;
mod viewing;
mod writing;

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;

use cut_project::{ClipId, Document, MediaId, Project};
use rikka_app::prelude::*;
use rikka_figure::ShapeKind;

use cut_export::Job;
use rikka_ui::ComputedImage;

use crate::gallery::TransitionPreviews;
use crate::live::LivePreview;
use crate::media_previews::{Display, MediaPreviews};
use crate::playback::Playback;
use crate::preferences::Preferences;
use crate::quality::Quality;
use crate::timeline::Scale;
use crate::tool::Tool;
use crate::transport::Transport;
use crate::trimmer::Trimmer;

pub(crate) use animation_presets::{PresetSlot, PresetStyle};
pub(crate) use clipboard::Clipboard;
use files::Linked;
pub(crate) use files::UnsavedPrompt;
pub(crate) use keyframing::{Keyframing, Plot};
pub(crate) use marking::Marking;
pub(crate) use popup::Popup;
use session::Session;
use sheet::Sheet;
pub(crate) use styling::Adjustable;
pub(crate) use transitioning::{Side, TransitionPreview};
pub(crate) use trimming::EditDirection;
pub(crate) use tweening::Endpoint;
use viewing::Dollying;
pub(crate) use writing::Draft;

const PREVIEW_SHARE: f32 = 0.58;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Pointing {
    pub(crate) at: Point,
    pub(crate) frame: i64,
    pub(crate) layer: usize,
}

#[derive(State)]
pub(crate) struct Cut {
    document: Document,
    #[state]
    session: Session,
    sheet: Option<Sheet>,
    offline: Arc<BTreeSet<MediaId>>,
    snapping: bool,
    volume: f32,
    clipboard: Option<Clipboard>,
    notice: Option<Label>,
    unsaved_prompt: Option<UnsavedPrompt>,
    folder: Option<PathBuf>,
    importing: usize,
    tool: Tool,
    range_shape: ShapeKind,
    quality: Quality,
    side: Side,
    job: Option<Job>,
    export_preview: ComputedImage,
    display: Display,
    trimmer: Trimmer,
    #[untracked]
    linked: Option<Linked>,
    #[untracked]
    asking_relink: bool,
    #[untracked]
    offline_watch: Option<rikka_xdg::Watch>,
    #[untracked]
    export_sheets: u64,
    #[untracked]
    preferences: Preferences,
    #[untracked]
    media_previews: MediaPreviews,
    #[untracked]
    sender: Sender<Cut>,
    #[untracked]
    transition_previews: TransitionPreviews,
    #[untracked]
    live: LivePreview,
    #[untracked]
    arriving: Vec<PathBuf>,
    #[untracked]
    playback: Playback,
    #[untracked]
    transport: Transport,
    #[untracked]
    scale: Scale,
    #[untracked]
    ratio: Ratio,
    #[untracked]
    origin: i64,
    #[untracked]
    ticking: bool,
    #[untracked]
    dollying: Option<Dollying>,
}

impl Cut {
    pub(crate) fn new(paths: Vec<PathBuf>, sender: Sender<Cut>) -> Cut {
        let preferences = Preferences::open();
        Cut {
            document: Document::new(),
            session: Session::default(),
            sheet: None,
            offline: Arc::default(),
            snapping: true,
            volume: 1.0,
            clipboard: None,
            notice: None,
            unsaved_prompt: None,
            folder: None,
            importing: 0,
            tool: Tool::Select,
            range_shape: ShapeKind::Rectangle,
            quality: Quality::default(),
            side: Side::default(),
            job: None,
            export_preview: ComputedImage::default(),
            display: preferences.display(),
            trimmer: Trimmer::Select,
            linked: None,
            asking_relink: false,
            offline_watch: None,
            export_sheets: 0,
            preferences,
            media_previews: MediaPreviews::default(),
            sender,
            transition_previews: TransitionPreviews::default(),
            live: LivePreview::default(),
            arriving: paths,
            playback: Playback::default(),
            transport: Transport::default(),
            scale: Scale::default(),
            ratio: Ratio::new(PREVIEW_SHARE),
            origin: 0,
            ticking: false,
            dollying: None,
        }
    }

    pub(crate) fn start(&mut self) {
        let arriving = std::mem::take(&mut self.arriving);
        self.arrive(arriving, None);
    }

    pub(crate) fn document(&self) -> &Document {
        &self.document
    }

    pub(crate) fn project(&self) -> &Arc<Project> {
        self.document.project()
    }

    pub(crate) fn selected(&self) -> &Arc<BTreeSet<ClipId>> {
        &self.session.selected
    }

    pub(crate) fn offline(&self) -> &Arc<BTreeSet<MediaId>> {
        &self.offline
    }

    pub(crate) fn transition_previews(&self) -> &TransitionPreviews {
        &self.transition_previews
    }

    pub(crate) fn snapping(&self) -> bool {
        self.snapping
    }

    pub(crate) fn volume(&self) -> f32 {
        self.volume
    }

    pub(crate) fn pointing(&self) -> Option<&Pointing> {
        self.session.pointing.as_ref()
    }

    pub(crate) fn transport(&self) -> &Transport {
        &self.transport
    }

    pub(crate) fn scale(&self) -> &Scale {
        &self.scale
    }

    pub(crate) fn playback(&self) -> &Playback {
        &self.playback
    }

    pub(crate) fn ratio(&self) -> &Ratio {
        &self.ratio
    }

    pub(crate) fn quality(&self) -> Quality {
        self.quality
    }

    pub(crate) fn set_quality(&mut self, quality: Quality) {
        self.quality = quality;
    }

    pub(crate) fn notice(&self) -> Option<&Label> {
        self.notice.as_ref()
    }

    pub(crate) fn is_busy(&self) -> bool {
        self.importing > 0
    }

    pub(crate) fn dismiss_notice(&mut self) {
        self.notice = None;
    }

    pub(crate) fn toggle_snapping(&mut self) {
        self.snapping = !self.snapping;
    }

    pub(crate) fn set_volume(&mut self, volume: f32) {
        self.volume = volume.clamp(0.0, 1.0);
        self.playback.set_volume(self.volume);
    }
}

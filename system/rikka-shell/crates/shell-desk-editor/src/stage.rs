use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use desk_layout::{Area, Content, Kind, Layout, Size as Step, Widget, WidgetId};
use rikka_session::wallpaper::{self, Preview, Thumbnail, WallpaperEntry};
use rikka_types::{Rect, Size};
use rikka_ui::{
    ImageSource, State, TextDocument, after, after_keyed, skip_view, spawn_blocking, stream,
    stream_keyed,
};
use shell_desk::widgets::Live;
use shell_kit::taskbar::Shape;
use shell_kit::toplevels::{self, Toplevels};
use shell_kit::{DesktopFrost, Placement, appearance};
use shell_wallpaper::{self as library, Media};

use crate::editing::Editing;
use crate::editor_canvas::{Thumb, frame};
use crate::session::Session;
use crate::shared::Shared;
use crate::wallpaper_loader::{Pictures, WallpaperLoader};

const ARRIVING: &str = "desk.editor.wallpaper";

const LINGERING: &str = "desk.editor.linger";

#[derive(State)]
pub(crate) struct Editor {
    shared: Shared,
    place: Placement,
    #[state]
    live: Live,
    #[state]
    behind: Toplevels,
    session: Session,
    wallpaper_loader: WallpaperLoader,
    wallpapers: Option<Vec<WallpaperEntry>>,
    picking: Option<String>,
    selected: Option<WidgetId>,
    query: TextDocument,
    search: String,
    clearing: bool,
    last_wallpaper: Option<(Option<u64>, bool)>,
    arrived: bool,
}

impl Editor {
    #[must_use]
    pub(crate) fn new(shared: Shared, place: Placement) -> Editor {
        let showing = shared.desk_scene.board.showing();
        let wallpaper_loader = WallpaperLoader::new(library::cover_of(&place), showing);
        Editor {
            live: Live::new(shared.desk_scene.clone()).everything(),
            behind: Toplevels::default(),
            selected: shared.focused(),
            shared,
            place,
            session: Session::default(),
            wallpaper_loader,
            wallpapers: None,
            picking: None,
            query: TextDocument::default(),
            search: String::new(),
            clearing: false,
            last_wallpaper: None,
            arrived: false,
        }
    }

    pub(crate) fn start(&mut self) {
        self.live.start();
        self.behind.start(self.shared.seam());
        after(Duration::ZERO, |editor: &mut Editor| editor.arrived = true);
        stream(
            wallpaper::listings(self.shared.seam()),
            |editor: &mut Editor, listed: Vec<WallpaperEntry>| editor.wallpapers = Some(listed),
        );
        stream(
            self.shared.desk_scene.board.changes(),
            |editor: &mut Editor, _: u64| {
                editor.wear();
                skip_view();
            },
        );
        self.wear();
    }

    fn wallpapers(&self) -> &[WallpaperEntry] {
        self.wallpapers.as_deref().unwrap_or_default()
    }

    fn frame(&mut self) {
        self.wear();
        let settled = self.wallpaper_loader.settle();
        let last_wallpaper = Some(self.wallpaper_loader.readiness());
        self.watch_wallpaper();
        if !settled && self.last_wallpaper == last_wallpaper {
            skip_view();
        }
        self.last_wallpaper = last_wallpaper;
    }

    fn watch_wallpaper(&self) {
        if let Some(changes) = self.wallpaper_loader.unready() {
            stream_keyed(ARRIVING, changes, |editor: &mut Editor, ()| editor.frame());
        }
        if let Some(left) = self.wallpaper_loader.lingers_for() {
            after_keyed(LINGERING, left, Editor::frame);
        }
    }

    pub(crate) fn left(&mut self) {
        self.shared.left();
    }

    fn wear(&mut self) {
        if self.picking.is_some() && self.picking == self.target() {
            self.picking = None;
        }
        let showing = self.shared.desk_scene.board.showing();
        let Some(wallpaper) = self.wallpaper_loader.wanting(showing.as_deref()) else {
            return;
        };
        let cover = self.wallpaper_loader.cover();
        spawn_blocking(
            {
                let wallpaper = wallpaper.clone();
                move || {
                    let path = wallpaper.as_deref().and_then(library::file_of)?;
                    library::load(&path, cover)
                        .inspect_err(|reason| {
                            tracing::warn!("widget editor wallpaper unavailable: {reason}");
                        })
                        .ok()
                }
            },
            move |editor: &mut Editor, media: Option<Media>| {
                editor
                    .wallpaper_loader
                    .finish_loading(wallpaper.clone(), media);
                editor.frame();
            },
        );
    }

    pub(crate) fn select(&mut self, widget: Option<WidgetId>) {
        self.selected = widget;
        self.clearing = false;
    }

    pub(crate) fn pick(&mut self, index: usize) {
        let Some(id) = self.wallpapers().get(index).map(|listed| listed.id.clone()) else {
            return;
        };
        if self.target().as_deref() == Some(id.as_str()) {
            return;
        }
        self.picking = Some(id.clone());
        self.shared
            .seam()
            .notify(appearance::write(appearance::WALLPAPER, id));
    }

    pub(crate) fn close(&mut self) {
        self.shared.close();
    }

    pub(crate) fn add(&mut self, kind: Kind, size: Step) {
        let id = self.editing().add(kind, size, &self.area());
        self.select(Some(id));
    }

    pub(crate) fn resize(&mut self, widget: WidgetId, size: Step) {
        self.editing().resize(widget, size, &self.area());
    }

    pub(super) fn duplicate(&mut self, widget: WidgetId) {
        if let Some(copy) = self.editing().duplicate(widget, &self.area()) {
            self.select(Some(copy));
        }
    }

    pub(crate) fn remove(&mut self, widget: WidgetId) {
        self.editing().remove(widget);
        self.select(None);
    }

    pub(crate) fn undo(&mut self) {
        self.editing().undo();
    }

    pub(crate) fn redo(&mut self) {
        self.editing().redo();
    }

    pub(super) fn ask_clearing(&mut self, asking: bool) {
        self.clearing = asking;
    }

    pub(crate) fn clear(&mut self) {
        self.editing().clear();
        self.select(None);
    }

    pub(crate) fn typed(&mut self) {
        self.search = self.query.text().trim().to_lowercase();
    }

    #[must_use]
    pub(crate) fn search(&self) -> &str {
        &self.search
    }

    #[must_use]
    pub(crate) fn editing(&self) -> &Editing {
        &self.shared.editing
    }

    #[must_use]
    pub(crate) fn session(&self) -> &Session {
        &self.session
    }

    #[must_use]
    pub(crate) fn frost(&self) -> &DesktopFrost {
        &self.shared.frost
    }

    #[must_use]
    pub(crate) fn output_name(&self) -> &str {
        &self.place.name
    }

    #[must_use]
    pub(crate) fn live(&self) -> &Live {
        &self.live
    }

    #[must_use]
    pub(crate) fn taskbar(&self) -> Option<Shape> {
        self.shared.desk_scene.edges.taskbar()
    }

    #[must_use]
    pub(crate) fn query(&self) -> &TextDocument {
        &self.query
    }

    #[must_use]
    pub(super) fn has_arrived(&self) -> bool {
        self.arrived
    }

    #[must_use]
    pub(super) fn is_clearing(&self) -> bool {
        self.clearing
    }

    #[must_use]
    pub(crate) fn output(&self) -> Size {
        self.place.size
    }

    #[must_use]
    pub(crate) fn area(&self) -> Area {
        self.shared.desk_scene.area_of(self.place.size)
    }

    #[must_use]
    pub(crate) fn target(&self) -> Option<String> {
        self.shared.desk_scene.board.showing()
    }

    #[must_use]
    pub(crate) fn shown(&self) -> Option<String> {
        self.wallpaper_loader.shown(self.target().as_deref())
    }

    #[must_use]
    pub(crate) fn behind(&self) -> Vec<(Arc<str>, Rect)> {
        self.behind
            .on(&self.place)
            .filter(|window| window.on_screen && !window.minimized)
            .map(|window| {
                (
                    Arc::from(window.id.as_str()),
                    toplevels::local(window, &self.place),
                )
            })
            .collect()
    }

    #[must_use]
    pub(super) fn editable(&self) -> bool {
        self.shown() == self.target()
    }

    #[must_use]
    pub(crate) fn layout(&self) -> Layout {
        self.shared.desk_scene.board.shown_layout()
    }

    #[must_use]
    pub(super) fn layout_of(&self, wallpaper: Option<&str>) -> Layout {
        self.shared.desk_scene.board.layout(wallpaper)
    }

    #[must_use]
    pub(crate) fn selected(&self) -> Option<Widget> {
        let id = self.selected?;
        self.shared
            .desk_scene
            .board
            .with_shown(|layout| layout.get(id).cloned())
    }

    #[must_use]
    pub(crate) fn is_empty(&self) -> bool {
        self.shared.desk_scene.board.with_shown(Layout::is_empty)
    }

    pub(super) fn restyler(&self, widget: WidgetId) -> Rc<dyn Fn(Content)> {
        let editing = self.editing().clone();
        Rc::new(move |content: Content| editing.restyle(widget, content))
    }

    #[must_use]
    pub(crate) fn pictures(&self) -> Pictures {
        self.wallpaper_loader.pictures()
    }

    #[must_use]
    pub(crate) fn panel(&self) -> Rect {
        frame::panel(self.place.size, self.taskbar())
    }

    #[must_use]
    pub(crate) fn thumbs(&self) -> (Vec<Thumb>, Option<usize>) {
        let area = self.area();
        let target = self.picking.clone().or_else(|| self.target());
        let current = self
            .wallpapers()
            .iter()
            .position(|listed| Some(&listed.id) == target.as_ref());
        let thumbs =
            self.wallpapers()
                .iter()
                .map(|listed| Thumb {
                    preview: preview(listed),
                    motion: matches!(listed.preview, Preview::Motion { .. }),
                    silhouettes: self.shared.desk_scene.board.predicted(
                        Some(&listed.id),
                        |layout| {
                            layout
                                .widgets
                                .iter()
                                .map(|widget| area.place(widget.size, widget.spot))
                                .collect()
                        },
                    ),
                })
                .collect();
        (thumbs, current)
    }
}

fn preview(listed: &WallpaperEntry) -> Option<ImageSource> {
    match &listed.preview {
        Preview::Still {
            thumbnail: Thumbnail::Made { file },
        } => Some(ImageSource::file(file.as_path())),
        _ => None,
    }
}

impl std::fmt::Debug for Editor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Editor")
            .field("output", &self.place.name)
            .field("selected", &self.selected)
            .finish_non_exhaustive()
    }
}

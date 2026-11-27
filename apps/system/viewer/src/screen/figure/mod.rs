mod grip;
mod load;
mod orbit;
mod scene;

use rikka_sync::{Mutex, MutexGuard};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use rikka_app::prelude::*;
use rikka_dialog::{Chooser, ChooserResponse};
use rikka_model::Model;
use rikka_model_gpu::Prepared;
use rikka_renderer_3d::ViewportConfig;
use rikka_viewport::{Viewport3d, viewport};
use rikka_widgets::column;

use crate::screen::figure::grip::Grip;
use crate::screen::figure::load::{Load, Loaded, Purpose};
use crate::screen::figure::scene::Scene;
use crate::screen::player::{Player, Ticker};
use crate::screen::{Screen, status};
use crate::{bar, wording};

const TICK: Duration = Duration::from_millis(120);
const PICKER_WIDTH: f32 = 170.0;

#[derive(Debug)]
pub(crate) struct Figure {
    path: PathBuf,
    motion: Motion,
    awaiting: Option<PathBuf>,
    state: State,
    scene: scene::Shared,
    viewport: Viewport3d,
    cancel: CancelToken,
    ticker: Ticker,
}

#[derive(Debug)]
enum Motion {
    None,
    Pending,
    Loaded(Arc<Model>),
}

impl Motion {
    fn loaded(&self) -> Option<&Arc<Model>> {
        match self {
            Self::Loaded(motion) => Some(motion),
            Self::None | Self::Pending => None,
        }
    }
}

#[derive(Debug)]
enum State {
    Reading,
    Waiting(Label),
    Ready(ModelInfo),
    Failed(Label),
}

#[derive(Debug)]
struct ModelInfo {
    summary: Label,
    animations: Vec<Label>,
    chosen: Option<usize>,
    note: Option<Label>,
}

impl Figure {
    pub(crate) fn open(path: &Path, motion: bool) -> Self {
        let scene: scene::Shared = Arc::new(Mutex::new(Scene::new()));
        let viewport = Viewport3d::with_config(ViewportConfig::default().identity(false));
        let render_scene = Arc::clone(&scene);
        viewport.set_scene(move |ctx| render_scene.lock().build(ctx));

        Self {
            path: path.to_path_buf(),
            motion: match motion {
                true => Motion::Pending,
                false => Motion::None,
            },
            awaiting: None,
            state: State::Reading,
            scene,
            viewport,
            cancel: CancelToken::new(),
            ticker: Ticker::new(TICK),
        }
    }

    fn scene(&self) -> MutexGuard<'_, Scene> {
        self.scene.lock()
    }

    pub(crate) fn stage(&self) -> Element {
        match &self.state {
            State::Reading => container(spinner()).center().grow(1.0).build(),
            State::Waiting(summary) => waiting(summary),
            State::Ready(_) => self.ready_stage(),
            State::Failed(reason) => status_page(MaterialSymbol::ERROR, reason.clone())
                .grow(1.0)
                .build(),
        }
    }

    pub(crate) fn detail(&self) -> Option<Element> {
        let palette = palette();
        let State::Ready(info) = &self.state else {
            return None;
        };

        let error = self
            .scene()
            .error()
            .map(|error| Label::from(error.to_string()))
            .or_else(|| info.note.clone());
        Some(
            row! {
                error.map(|error| bar::detail(error, palette.danger));
                bar::detail(info.summary.clone(), palette.text_muted);
                if info.animations.len() > 1 {
                    picker(info);
                }
                bar::icon_button(
                    MaterialSymbol::FIT_SCREEN,
                    |figure: &mut Figure| figure.scene().recenter(),
                    true,
                );
            }
            .gap(Space::TIGHT)
            .align_items(Align::Center)
            .build(),
        )
    }

    pub(crate) fn animated(&self) -> bool {
        matches!(&self.state, State::Ready(info) if !info.animations.is_empty())
    }

    fn ready_stage(&self) -> Element {
        column! {
            container(viewport(&self.viewport).fill())
                .background(palette().background)
                .grow(1.0);
            Element::new(Grip::new(&self.scene))
                .position(0.0, 0.0)
                .fill();
        }
        .grow(1.0)
        .build()
    }

    fn read(&mut self, path: PathBuf) {
        self.cancel.cancel();
        self.cancel = CancelToken::new();
        self.awaiting = Some(path.clone());

        let purpose = match &self.motion {
            Motion::Pending => Purpose::Motion,
            Motion::None => Purpose::Figure(None),
            Motion::Loaded(motion) => Purpose::Figure(Some(Arc::clone(motion))),
        };
        let cancel = self.cancel.clone();
        let scene = Arc::clone(&self.scene);
        spawn_blocking(
            move || load::read(path, purpose, &scene, cancel),
            |screen: &mut Screen, load| {
                if let Some(figure) = screen.figure() {
                    figure.loaded(load);
                }
            },
        );
    }

    fn loaded(&mut self, load: Load) {
        if self.awaiting.as_deref() != Some(load.path.as_path()) {
            return;
        }
        self.awaiting = None;

        match load.read {
            Ok(Loaded::Motion(motion)) => {
                self.state = State::Waiting(wording::motion(&motion));
                self.motion = Motion::Loaded(motion);
            }
            Ok(Loaded::Figure(prepared)) => self.show(prepared),
            Err(error) => {
                let reason =
                    wording::model_refusal(&error).unwrap_or_else(|| status::unreadable(&error));
                self.state = State::Failed(reason);
            }
        }
    }

    fn show(&mut self, prepared: Prepared) {
        let model = prepared.model();
        let animations: Vec<Label> = model
            .animations
            .iter()
            .map(|animation| Label::from(animation.name.as_str()))
            .collect();
        let chosen = match self.motion.loaded().is_some() {
            true => animations.len().checked_sub(1),
            false => (!animations.is_empty()).then_some(0),
        };
        let state = ModelInfo {
            summary: wording::figure(model),
            note: self.unmoved(model),
            animations,
            chosen,
        };

        let mut scene = self.scene();
        let uploaded = scene.show(prepared);
        if let Some(at) = chosen {
            scene.play(at);
        }
        drop(scene);

        self.state = State::Ready(state);
        self.resume_ticker();
        spawn(
            async move {
                let _ = uploaded.await;
            },
            |screen: &mut Screen, ()| screen.on_tick(),
        );
    }

    fn unmoved(&self, model: &Model) -> Option<Label> {
        self.motion.loaded()?;
        let bound = model.animations.last()?;
        bound
            .channels
            .is_empty()
            .then(|| tr!("viewer.figure.unmoved"))
    }

    fn choose(&mut self, chosen: usize) {
        let State::Ready(info) = &mut self.state else {
            return;
        };
        if chosen >= info.animations.len() {
            return;
        }
        info.chosen = Some(chosen);
        self.scene().play(chosen);
        self.resume_ticker();
    }

    fn pick(&self) {
        let title = tr!("viewer.figure.chooser").to_string();
        let accept = tr!("viewer.figure.accept").to_string();
        let folder = self.path.parent().map(Path::to_path_buf);
        let parent = window_identifier();
        spawn_blocking(
            move || ask(title, accept, folder, parent),
            |screen: &mut Screen, picked| {
                if let Some(figure) = screen.figure() {
                    figure.picked(picked);
                }
            },
        );
    }

    fn picked(&mut self, picked: rikka_dialog::Result<Option<PathBuf>>) {
        match picked {
            Ok(Some(path)) => {
                self.state = State::Reading;
                self.read(path);
            }
            Ok(None) => {}
            Err(error) => self.state = State::Failed(Label::from(error.to_string())),
        }
    }

    fn resume_ticker(&mut self) {
        if self.scene().is_playing() {
            self.ticker.start();
        }
    }
}

impl Player for Figure {
    fn volume(&self) -> Option<f32> {
        None
    }

    fn set_volume(&self, _volume: f32) {}

    fn is_playing(&self) -> bool {
        self.scene().is_playing()
    }

    fn position(&self) -> Duration {
        Duration::from_secs_f32(self.scene().time().max(0.0))
    }

    fn duration(&self) -> Option<Duration> {
        self.scene().duration().map(Duration::from_secs_f32)
    }

    fn is_dead(&self) -> bool {
        self.duration().is_none()
    }

    fn start(&mut self) {
        let path = self.path.clone();
        self.read(path);
    }

    fn play_pause(&mut self) {
        let playing = {
            let mut scene = self.scene();
            let playing = !scene.is_playing();
            scene.set_playing(playing);
            playing
        };
        if !playing {
            self.ticker.stop();
            return;
        }
        self.resume_ticker();
    }

    fn seek(&mut self, position: Duration) {
        self.scene().seek(position.as_secs_f32());
    }

    fn on_tick(&mut self) {
        if !self.is_playing() {
            self.ticker.stop();
        }
    }
}

impl Drop for Figure {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

fn waiting(summary: &Label) -> Element {
    status_page(MaterialSymbol::ANIMATION, summary.clone())
        .action(button(tr!("viewer.figure.choose")).on_press(|figure: &mut Figure| figure.pick()))
        .grow(1.0)
        .build()
}

fn picker(info: &ModelInfo) -> impl View {
    combo(info.chosen)
        .options(info.animations.iter().cloned())
        .placeholder(tr!("viewer.figure.animation"))
        .on_select(|figure: &mut Figure, chosen| figure.choose(chosen))
        .control_size(ControlSize::Small)
        .width(PICKER_WIDTH)
}

fn ask(
    title: String,
    accept: String,
    folder: Option<PathBuf>,
    parent: Option<WindowIdentifier>,
) -> rikka_dialog::Result<Option<PathBuf>> {
    let mut chooser = Chooser::open(title).accept_label(accept).parent(parent);
    if let Some(folder) = folder {
        chooser = chooser.current_folder(folder);
    }
    match chooser.run()? {
        ChooserResponse::Files(picked) => Ok(picked.into_iter().next()),
        ChooserResponse::Cancelled => Ok(None),
    }
}

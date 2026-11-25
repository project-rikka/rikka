use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;

use rikka_seam::{Payload, Request, SeamClient, SeamError};
use rikka_session::gestures::{self, Swipe};
use rikka_session::toplevels::{Toplevel, ToplevelsClient};
use rikka_session::workspaces::{self, COUNT, State as Spaces, WorkspacesClient};
use rikka_ui::{
    DurationToken, Element, State, skip_view, spawn, spawn_blocking, stream, stream_keyed,
};
use shell_desk::Scene;
use shell_desk::widgets::{Live, Mode};
use shell_kit::taskbar::Shape;
use shell_kit::toplevels::{self, Toplevels};
use shell_kit::{CLOSE, Placement, framed};
use shell_wallpaper::{self as wallpaper, Media};

use crate::CAPABILITY;
use crate::deck::{Backdrop, Board, Cards, Pane, Space};
use crate::shared::Shared;
use crate::swipe::{SwipeState, Swipes};

pub(crate) const EXIT_DURATION: DurationToken = DurationToken::Medium1;

const ARRIVING: &str = "overview.wallpaper";

#[derive(State)]
pub(crate) struct Stage {
    shared: Shared,
    place: Placement,
    #[state]
    windows: Toplevels,
    workspaces: Spaces,
    selected: Option<Arc<str>>,
    passing: Swipes,
    last_backdrop: Option<BackdropSnapshot>,
    cards: Cards,
    #[state]
    live: Live,
}

#[derive(Debug, PartialEq)]
struct BackdropSnapshot {
    current_wallpaper: Option<(PathBuf, bool)>,
    taskbar: Option<Shape>,
}

impl Stage {
    #[must_use]
    pub(crate) fn new(shared: Shared, place: Placement) -> Stage {
        shared.desktop.load_shared(wallpaper::cover_of(&place));
        let live = Live::new(shared.desk_scene.clone());
        Stage {
            shared,
            place,
            windows: Toplevels::default(),
            workspaces: Spaces::default(),
            selected: None,
            passing: Swipes::default(),
            last_backdrop: None,
            cards: Cards::default(),
            live,
        }
    }

    pub(crate) fn start(&mut self) {
        self.live.start();
        stream(self.shared.desktop.changes(), |stage: &mut Stage, ()| {
            stage.frame();
        });
        let seam = self.seam().clone();
        self.windows.start(&seam);
        stream(
            workspaces::states(&seam),
            |stage: &mut Stage, state: Spaces| stage.workspaces = state,
        );
        stream(gestures::swipes(&seam), Stage::swiped);
        self.load_wallpaper();
    }

    pub(crate) fn select(&mut self, id: Arc<str>) {
        self.selected = Some(id);
    }

    pub(crate) fn activate(&mut self, id: Arc<str>) {
        send_request(ToplevelsClient::new(self.seam().clone()).focus(id.to_string()));
        self.shut();
    }

    pub(crate) fn close(&mut self, id: Arc<str>) {
        send_request(ToplevelsClient::new(self.seam().clone()).close(id.to_string()));
    }

    pub(crate) fn show(&mut self, number: usize) {
        if number == self.showing() {
            return;
        }
        send_request(
            WorkspacesClient::new(self.seam().clone()).show(Some(self.place.name.clone()), number),
        );
    }

    pub(crate) fn step(&mut self, step: i32) {
        let listed = self.listed();
        let at = listed
            .iter()
            .position(|number| *number == self.showing())
            .unwrap_or(0);
        let target = (at as i32 + step).clamp(0, listed.len() as i32 - 1) as usize;
        if let Some(number) = listed.get(target).copied() {
            self.show(number);
        }
    }

    pub(crate) fn send(&mut self, window: Arc<str>, workspace: usize) {
        send_request(
            ToplevelsClient::new(self.seam().clone())
                .move_to_workspace(window.to_string(), workspace),
        );
    }

    pub(super) fn shut(&mut self) {
        self.seam().notify(Request::new(CAPABILITY, CLOSE));
    }

    fn frame(&mut self) {
        self.load_wallpaper();
        let last_backdrop = BackdropSnapshot {
            current_wallpaper: self.shared.desktop.current_wallpaper(),
            taskbar: self.shared.desktop.taskbar(),
        };
        if let Some(changes) = self.shared.desktop.unready() {
            stream_keyed(ARRIVING, changes, |stage: &mut Stage, ()| stage.frame());
        }
        if self.last_backdrop.as_ref() == Some(&last_backdrop) {
            skip_view();
        }
        self.last_backdrop = Some(last_backdrop);
    }

    fn swiped(&mut self, swipe: Swipe) {
        if let SwipeState::Released(track) = self.passing.handle_swipe(swipe, true)
            && track.sideways_on(&self.place.name)
        {
            let step = track.step();
            if step != 0 {
                self.step(step);
            }
        }
    }

    fn load_wallpaper(&self) {
        let Some(path) = self.shared.desktop.wanting() else {
            return;
        };
        spawn_blocking(
            {
                let (path, cover) = (path.clone(), wallpaper::cover_of(&self.place));
                move || wallpaper::load(&path, cover)
            },
            move |stage: &mut Stage, read: Result<Media, String>| {
                stage.shared.desktop.finish_loading(path.clone(), read);
            },
        );
    }

    fn seam(&self) -> &SeamClient {
        &self.shared.seam
    }

    fn own_toplevels(&self) -> impl Iterator<Item = &Toplevel> + '_ {
        self.windows.on(&self.place)
    }

    fn showing(&self) -> usize {
        self.workspaces
            .display(&self.place.name)
            .map_or(1, |displayed| displayed.showing)
    }

    fn listed(&self) -> Vec<usize> {
        let occupied: BTreeSet<usize> = self
            .own_toplevels()
            .map(|window| window.workspace)
            .collect();
        listed(&occupied, self.showing())
    }

    pub(crate) fn board(&self) -> Board {
        let showing = self.showing();
        let listed = self.listed();
        let spaces: Vec<Space> = listed
            .iter()
            .map(|number| Space {
                number: *number,
                panes: self
                    .own_toplevels()
                    .filter(|window| window.workspace == *number)
                    .map(|window| Pane {
                        id: Arc::from(window.id.as_str()),
                        real: toplevels::local(window, &self.place),
                        minimized: window.minimized,
                    })
                    .collect(),
            })
            .collect();
        let showing_at = listed
            .iter()
            .position(|number| *number == showing)
            .unwrap_or(0);
        Board {
            selected: self.chosen(&spaces[showing_at]),
            spaces,
            showing: showing_at,
            output: Arc::from(self.place.name.as_str()),
            backdrop: Backdrop {
                wallpaper: self.shared.desktop.wallpaper(),
                taskbar: self.shared.desktop.taskbar(),
            },
            swipes: self.shared.swipes.clone(),
            cards: self.cards.clone(),
            frost: self.shared.frost.clone(),
        }
    }

    pub(crate) fn desks(&self, board: &Board) -> Vec<Element> {
        let desk_scene = &self.shared.desk_scene;
        let output = self.place.size;
        let wallpaper = desk_scene.board.showing();
        let layout = desk_scene.board.layout(wallpaper.as_deref());
        let area = desk_scene.area_of(output);
        board
            .spaces
            .iter()
            .map(|space| {
                let scene = Scene {
                    wallpaper: wallpaper.clone(),
                    layout: layout.clone(),
                    output,
                    area,
                    mode: Mode::Shown,
                    grabbed: None,
                };
                let (number, cards) = (space.number, self.cards.clone());
                framed(shell_desk::scene(scene), output, move |standing, ctx| {
                    cards.follow(number, standing, ctx)
                })
                .keyed(number)
            })
            .collect()
    }

    fn chosen(&self, space: &Space) -> Option<Arc<str>> {
        let has_pane = |id: &str| space.panes.iter().any(|pane| &*pane.id == id);
        if let Some(selected) = self.selected.as_ref().filter(|id| has_pane(id)) {
            return Some(Arc::clone(selected));
        }
        self.own_toplevels()
            .find(|window| window.focused && has_pane(&window.id))
            .map(|window| Arc::from(window.id.as_str()))
            .or_else(|| space.panes.last().map(|pane| Arc::clone(&pane.id)))
    }
}

fn listed(occupied: &BTreeSet<usize>, showing: usize) -> Vec<usize> {
    let mut numbers = occupied.clone();
    numbers.insert(showing);
    if let Some(last) = numbers.last().copied()
        && last < COUNT
    {
        numbers.insert(last + 1);
    }
    numbers.into_iter().collect()
}

fn send_request(
    request: impl std::future::Future<Output = Result<Payload, SeamError>> + Send + 'static,
) {
    spawn(
        request,
        |_: &mut Stage, reply: Result<Payload, SeamError>| {
            if let Err(error) = reply {
                tracing::warn!("overview request failed: {error}");
            }
        },
    );
}

impl std::fmt::Debug for Stage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Stage")
            .field("output", &self.place.name)
            .field("windows", &self.windows.count())
            .finish_non_exhaustive()
    }
}

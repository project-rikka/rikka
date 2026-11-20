use std::sync::Arc;
use std::time::Duration;

use rikka_seam::{Payload, SeamClient, SeamError};
use rikka_session::wallpaper::{self, CAPABILITY, SHOWING, WallpaperClient};
use rikka_ui::{
    Element, State, View, after, after_keyed, cancel, spawn, spawn_blocking, stream, stream_keyed,
};
use rikka_widgets::spacer;
use shell_kit::Placement;

use crate as loader;
use crate::Media;
use crate::backdrop::crossfade::{Crossfade, Phase};
use crate::backdrop::shown_on::{Decision, ShownOn};
use crate::backdrop::transition::{Circle, Transition};

const READY_LIMIT: Duration = Duration::from_secs(5);

const ARRIVING: &str = "wallpaper.arriving";

#[derive(Debug, State)]
pub(crate) struct Wallpaper {
    transition: Arc<dyn Transition>,
    screen: Placement,
    target: Option<wallpaper::DisplayedWallpaper>,
    seam: Option<SeamClient>,
    showing: Option<ShownWallpaper>,
    leaving: Option<ShownWallpaper>,
    arriving: Option<ShownWallpaper>,
    crossed: bool,
    awaiting: u64,
    reserved: f32,
    #[untracked]
    shown_on: ShownOn,
}

#[derive(Debug)]
struct ShownWallpaper {
    id: u64,
    wallpaper: String,
    media: Media,
}

impl Wallpaper {
    #[must_use]
    pub(crate) fn new() -> Wallpaper {
        Wallpaper {
            transition: Arc::new(Circle),
            screen: Placement::default(),
            target: None,
            seam: None,
            showing: None,
            leaving: None,
            arriving: None,
            crossed: true,
            awaiting: 0,
            reserved: 0.0,
            shown_on: ShownOn::default(),
        }
    }

    #[must_use]
    pub(crate) fn on(mut self, screen: Placement) -> Wallpaper {
        self.screen = screen;
        self
    }

    #[must_use]
    pub(crate) fn below(mut self, reserved: f32) -> Wallpaper {
        self.reserved = reserved;
        self
    }

    #[must_use]
    pub(crate) fn following(mut self, seam: SeamClient) -> Wallpaper {
        self.seam = Some(seam);
        self
    }

    #[must_use]
    pub(crate) fn shown_on(mut self, shown_on: ShownOn) -> Wallpaper {
        self.shown_on = shown_on;
        self
    }

    pub(crate) fn start(&mut self) {
        self.shown_on.decide(&self.screen.name, Decision::Pending);
        let Some(seam) = &self.seam else {
            return;
        };
        stream(
            wallpaper::displayed(seam),
            |wallpaper: &mut Wallpaper, shown: Option<wallpaper::DisplayedWallpaper>| {
                wallpaper.show(shown);
            },
        );
        stream(
            wallpaper::reserves(seam),
            |wallpaper: &mut Wallpaper, below: f32| wallpaper.reserved = below,
        );
        spawn(
            WallpaperClient::new(seam.clone()).showing(),
            |wallpaper: &mut Wallpaper, reply: Result<Payload, SeamError>| {
                wallpaper.showed(reply);
            },
        );
    }

    fn showed(&mut self, reply: Result<Payload, SeamError>) {
        match reply.and_then(|payload| payload.decode::<Option<wallpaper::DisplayedWallpaper>>()) {
            Ok(Some(shown)) => self.show(Some(shown)),
            Ok(None) => {}
            Err(error) => tracing::warn!("seam call {CAPABILITY}/{SHOWING} failed: {error}"),
        }
    }

    pub(crate) fn moved(&mut self, screen: Placement) {
        let before = loader::cover_of(&self.screen);
        self.screen = screen;
        let still = self
            .showing
            .as_ref()
            .is_some_and(|pictured| pictured.media.is_still());
        if still && loader::cover_of(&self.screen) != before {
            self.show(self.target.clone());
        }
    }

    fn show(&mut self, shown: Option<wallpaper::DisplayedWallpaper>) {
        self.target.clone_from(&shown);
        self.awaiting += 1;
        self.arriving = None;
        cancel(ARRIVING);
        let Some(shown) = shown else {
            return self.clear();
        };
        let id = self.awaiting;
        let cover = loader::cover_of(&self.screen);
        let wallpaper::DisplayedWallpaper { id: name, file } = shown;
        spawn_blocking(
            move || loader::load(&file, cover),
            move |wallpaper: &mut Wallpaper, loaded: Result<Media, String>| {
                wallpaper.loaded(id, name.clone(), loaded);
            },
        );
    }

    fn clear(&mut self) {
        self.showing = None;
        self.leaving = None;
        self.crossed = true;
        self.shown_on
            .decide(&self.screen.name, Decision::Made(None));
    }

    fn loaded(&mut self, id: u64, wallpaper: String, loaded: Result<Media, String>) {
        if id != self.awaiting {
            return;
        }
        match loaded {
            Ok(media) => {
                if !media.is_ready() {
                    stream_keyed(
                        ARRIVING,
                        media.changes(),
                        move |wallpaper: &mut Wallpaper, ()| {
                            wallpaper.reconsider(id);
                        },
                    );
                    after_keyed(
                        (ARRIVING, id),
                        READY_LIMIT,
                        move |wallpaper: &mut Wallpaper| {
                            wallpaper.give_up(id);
                        },
                    );
                }
                self.arriving = Some(ShownWallpaper {
                    id,
                    wallpaper,
                    media,
                });
                self.reconsider(id);
            }
            Err(error) => tracing::warn!("failed to change wallpaper: {error}"),
        }
    }

    fn reconsider(&mut self, id: u64) {
        let Some(arriving) = self.arriving.as_ref().filter(|_| id == self.awaiting) else {
            return;
        };
        if let Some(failure) = arriving.media.failure() {
            return self.abandon(format!("the wallpaper was not changed: {failure}"));
        }
        if !arriving.media.is_ready() {
            return;
        }
        let arriving = self.arriving.take().expect("checked just above");
        cancel(ARRIVING);
        cancel((ARRIVING, id));
        self.begin(arriving);
    }

    fn give_up(&mut self, id: u64) {
        if self
            .arriving
            .as_ref()
            .is_some_and(|arriving| arriving.id == id)
        {
            self.abandon(format!(
                "the wallpaper was not changed: still nothing to draw after {READY_LIMIT:?}"
            ));
        }
    }

    fn abandon(&mut self, why: String) {
        tracing::warn!("{why}");
        self.arriving = None;
        cancel(ARRIVING);
    }

    fn begin(&mut self, arriving: ShownWallpaper) {
        let id = arriving.id;
        self.shown_on.decide(
            &self.screen.name,
            Decision::Made(Some(arriving.wallpaper.clone())),
        );
        self.leaving = self.showing.replace(arriving);
        self.crossed = false;

        let over = self.transition.duration().duration();
        after(over, move |wallpaper: &mut Wallpaper| wallpaper.settled(id));
    }

    fn settled(&mut self, id: u64) {
        if self
            .showing
            .as_ref()
            .is_some_and(|pictured| pictured.id == id)
        {
            self.leaving = None;
            self.crossed = true;
        }
    }

    fn phase(&self) -> Phase {
        if self.arriving.is_some() {
            Phase::Holding
        } else if self.crossed {
            Phase::Settled
        } else {
            Phase::Crossfade
        }
    }
}

impl Default for Wallpaper {
    fn default() -> Wallpaper {
        Wallpaper::new()
    }
}

pub(crate) fn view(wallpaper: &Wallpaper) -> Element {
    let (incoming, outgoing, generation) = match (&wallpaper.arriving, &wallpaper.showing) {
        (Some(arriving), showing) => (
            arriving.media.picture(),
            showing.as_ref().map(|pictured| pictured.media.picture()),
            wallpaper.awaiting,
        ),
        (None, Some(showing)) => (
            showing.media.picture(),
            wallpaper
                .leaving
                .as_ref()
                .map(|pictured| pictured.media.picture()),
            showing.id,
        ),
        (None, None) => return spacer().build(),
    };
    Crossfade {
        transition: Arc::clone(&wallpaper.transition),
        outgoing,
        incoming,
        phase: wallpaper.phase(),
        generation,
        reserved: wallpaper.reserved,
    }
    .build()
}

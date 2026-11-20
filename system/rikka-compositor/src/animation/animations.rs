use std::collections::HashMap;
use std::time::Instant;

use rikka_animation::{Curve, DurationToken, Easing, Spring};
use rikka_compositor_caption::Button;
use rikka_compositor_desktop::{SnapshotId, WindowId};
use rikka_compositor_geometry::{Logical, Rect, Size};
use rikka_compositor_shell::LayerId;
use rikka_compositor_types::OutputId;

use super::closing::Closing;
use super::fades::{Fade, Fades};
use super::lock_transition::LockTransition;
use super::motion::WindowMotion;
use super::rounding::{Rounding, Roundings};
use super::scrim::{Fader, Scrim};
use super::slide::{Slide, SlidePosition};
use crate::render::{Leaving, Visual};

const OUTLINE_FADE: Curve = Curve::Tween {
    easing: Easing::Standard,
    duration: DurationToken::Medium1,
};

const SCRIM_FADE: Curve = Curve::Tween {
    easing: Easing::Standard,
    duration: DurationToken::Short3,
};

const CAPTION_HOVER: Curve = Curve::Spring(Spring::FAST_EFFECTS);

const CAPTION_FOCUS: Curve = Curve::Spring(Spring::DEFAULT_EFFECTS);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Finished {
    Closed {
        window: WindowId,
        snapshot: SnapshotId,
    },
    Left(SnapshotId),
}

#[derive(Debug)]
pub(crate) struct Animations {
    enabled: bool,
    motions: HashMap<WindowId, WindowMotion>,
    closing: Vec<Closing>,
    slides: Vec<Slide>,
    outlines: Fades<WindowId>,
    roundings: Roundings,
    scrims: Fades<LayerId>,
    caption_buttons: Fades<(WindowId, Button)>,
    caption_focus: Fades<WindowId>,
    minted: u64,
    done: Vec<Finished>,
    frames_needed: Vec<OutputId>,
    pub(crate) lock_transition: LockTransition,
}

impl Default for Animations {
    fn default() -> Self {
        Self {
            enabled: true,
            motions: HashMap::new(),
            closing: Vec::new(),
            outlines: Fades::new(OUTLINE_FADE),
            roundings: Roundings::default(),
            scrims: Fades::new(SCRIM_FADE),
            caption_buttons: Fades::new(CAPTION_HOVER),
            caption_focus: Fades::new(CAPTION_FOCUS),
            slides: Vec::new(),
            minted: 0,
            done: Vec::new(),
            frames_needed: Vec::new(),
            lock_transition: LockTransition::default(),
        }
    }
}

impl Animations {
    pub(crate) fn following(enabled: bool) -> Self {
        Self {
            enabled,
            ..Self::default()
        }
    }

    pub(crate) fn enabled(&self) -> bool {
        self.enabled
    }

    pub(crate) fn composed(&mut self, screen: OutputId, running: bool) {
        self.frames_needed
            .retain(|frames_needed| *frames_needed != screen);
        if running {
            self.frames_needed.push(screen);
        }
    }

    pub(crate) fn needs_frame(&self, screen: OutputId) -> bool {
        self.frames_needed.contains(&screen)
    }

    pub(crate) fn enable(&mut self, enabled: bool) {
        self.enabled = enabled;
        if enabled {
            return;
        }
        self.motions.clear();
        self.roundings = Roundings::default();
        let closing = std::mem::take(&mut self.closing);
        let slides = std::mem::take(&mut self.slides);
        self.done.extend(closing.into_iter().map(closed));
        self.done.extend(slides.into_iter().flat_map(left));
        self.lock_transition.drop_all();
    }

    pub(crate) fn mint(&mut self) -> SnapshotId {
        self.minted += 1;
        SnapshotId::new(self.minted)
    }

    pub(crate) fn open(&mut self, window: WindowId) {
        self.start(window, WindowMotion::open);
    }

    pub(crate) fn reshape(
        &mut self,
        window: WindowId,
        from: Rect<f64, Logical>,
        to: Rect<i32, Logical>,
        launch: Option<f64>,
    ) {
        if from == to.to_f64() {
            return;
        }
        self.start(window, |motion, now| motion.reshape(from, to, launch, now));
    }

    pub(crate) fn resize(
        &mut self,
        window: WindowId,
        from: Size<i32, Logical>,
        to: Size<i32, Logical>,
    ) {
        if from == to {
            return;
        }
        self.start(window, |motion, now| motion.resize(from, to, now));
    }

    pub(crate) fn stop_travel(&mut self, window: WindowId) {
        if let Some(motion) = self.motions.get_mut(&window) {
            motion.stop_travel();
        }
    }

    pub(crate) fn forget(&mut self, window: WindowId) {
        self.motions.remove(&window);
        self.outlines.forget(window);
        self.caption_focus.forget(window);
        self.roundings.forget(window);
    }

    fn start(&mut self, window: WindowId, begin: impl FnOnce(&mut WindowMotion, Instant)) {
        if !self.enabled {
            return;
        }
        let motion = self
            .motions
            .entry(window)
            .or_insert_with(WindowMotion::at_rest);
        begin(motion, Instant::now());
    }

    pub(crate) fn close(
        &mut self,
        window: WindowId,
        snapshot: SnapshotId,
        geometry: Rect<i32, Logical>,
    ) {
        self.motions.remove(&window);
        self.outlines.forget(window);
        self.caption_focus.forget(window);
        self.roundings.forget(window);
        self.closing.push(Closing::new(
            Some(window),
            snapshot,
            geometry,
            Instant::now(),
        ));
    }

    pub(crate) fn put_away(&mut self, snapshot: SnapshotId, geometry: Rect<i32, Logical>) {
        self.closing
            .push(Closing::new(None, snapshot, geometry, Instant::now()));
    }

    pub(crate) fn switch(
        &mut self,
        output: OutputId,
        apart: f32,
        leaving: Vec<(SnapshotId, Rect<i32, Logical>)>,
        arriving: Vec<WindowId>,
    ) {
        let (replaced, remaining): (Vec<Slide>, Vec<Slide>) = std::mem::take(&mut self.slides)
            .into_iter()
            .partition(|slide| slide.output == output);
        self.slides = remaining;
        self.done.extend(replaced.into_iter().flat_map(left));
        if !self.enabled || apart == 0.0 {
            self.done.extend(
                leaving
                    .into_iter()
                    .map(|(snapshot, _)| Finished::Left(snapshot)),
            );
            return;
        }
        self.slides
            .push(Slide::new(output, apart, leaving, arriving, Instant::now()));
    }

    pub(crate) fn advance(&mut self, now: Instant) {
        self.lock_transition.advance(now);
        self.outlines.advance(now);
        self.caption_buttons.advance(now);
        self.caption_focus.advance(now);
        self.roundings.advance(now);
        self.scrims.advance(now);
        let (finished, closing): (Vec<Closing>, Vec<Closing>) = std::mem::take(&mut self.closing)
            .into_iter()
            .map(|mut closing| {
                closing.advance(now);
                closing
            })
            .partition(Closing::is_finished);
        self.closing = closing;
        self.done.extend(finished.into_iter().map(closed));
        let (finished, slides): (Vec<Slide>, Vec<Slide>) = std::mem::take(&mut self.slides)
            .into_iter()
            .map(|mut slide| {
                slide.advance(now);
                slide
            })
            .partition(Slide::is_finished);
        self.slides = slides;
        self.done.extend(finished.into_iter().flat_map(left));
        self.motions.retain(|_, motion| {
            motion.advance(now);
            motion.is_animating()
        });
    }

    pub(crate) fn caption_buttons(
        &mut self,
        lit: Option<(WindowId, Button)>,
    ) -> impl Iterator<Item = Fade<(WindowId, Button)>> + '_ {
        self.caption_buttons
            .follow(lit.map(|key| (key, true)), self.enabled);
        self.caption_buttons.fades()
    }

    pub(crate) fn caption_focus(
        &mut self,
        focused: Option<WindowId>,
    ) -> impl Iterator<Item = Fade<WindowId>> + '_ {
        self.caption_focus
            .follow(focused.map(|window| (window, true)), self.enabled);
        self.caption_focus.fades()
    }

    pub(crate) fn outline(
        &mut self,
        outlined: Option<WindowId>,
    ) -> impl Iterator<Item = Fade<WindowId>> + '_ {
        self.outlines
            .follow(outlined.map(|window| (window, true)), self.enabled);
        self.outlines.fades()
    }

    pub(crate) fn round(&mut self, window: WindowId, target: f32) -> Rounding {
        self.roundings.follow(window, target, self.enabled)
    }

    pub(crate) fn rounding(&self, window: WindowId) -> Option<f32> {
        self.roundings.current(window)
    }

    pub(crate) fn scrims(&mut self, scrims: &[Scrim]) -> impl Iterator<Item = Fade<LayerId>> + '_ {
        self.scrims.follow(
            scrims
                .iter()
                .filter(|scrim| scrim.fader == Fader::Compositor)
                .map(|scrim| (scrim.layer, scrim.frosted)),
            self.enabled,
        );
        self.scrims.fades()
    }

    pub(crate) fn sample(
        &mut self,
        window: WindowId,
        geometry: Rect<i32, Logical>,
        now: Instant,
    ) -> Option<Visual> {
        self.motions.get_mut(&window)?.sample(now, geometry)
    }

    pub(crate) fn exiting(
        &mut self,
        now: Instant,
    ) -> impl Iterator<Item = (Leaving, Option<WindowId>)> + '_ {
        self.closing.iter_mut().map(move |closing| {
            let leaving = Leaving {
                snapshot: closing.snapshot,
                geometry: closing.geometry,
                shown: closing.shown(now),
            };
            (leaving, closing.window)
        })
    }

    pub(crate) fn slid(&self, window: WindowId, output: OutputId) -> SlidePosition {
        match self.slides.iter().find(|slide| slide.carries(window)) {
            Some(slide) if slide.output == output => SlidePosition::Along(slide.arriving()),
            Some(_) => SlidePosition::Elsewhere,
            None => SlidePosition::Still,
        }
    }

    pub(crate) fn leaving(&self, output: OutputId) -> impl Iterator<Item = Leaving> + '_ {
        self.slides
            .iter()
            .filter(move |slide| slide.output == output)
            .flat_map(Slide::leaving)
    }

    pub(crate) fn sliding(&self, output: OutputId) -> bool {
        self.slides.iter().any(|slide| slide.output == output)
    }

    pub(crate) fn take_done(&mut self) -> Vec<Finished> {
        self.done.extend(
            self.lock_transition
                .take_released()
                .into_iter()
                .map(Finished::Left),
        );
        std::mem::take(&mut self.done)
    }
}

fn closed(closing: Closing) -> Finished {
    match closing.window {
        Some(window) => Finished::Closed {
            window,
            snapshot: closing.snapshot,
        },
        None => Finished::Left(closing.snapshot),
    }
}

fn left(slide: Slide) -> impl Iterator<Item = Finished> {
    slide
        .leaving
        .into_iter()
        .map(|(snapshot, _)| Finished::Left(snapshot))
}

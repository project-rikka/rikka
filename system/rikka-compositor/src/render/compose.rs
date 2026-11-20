use std::time::{Duration, Instant};

use rikka_compositor_geometry::{Physical, Rect};
use rikka_compositor_output::blur_region;
use rikka_compositor_planes::Surface as Candidate;
use rikka_compositor_surface::{Presentation, Refresh, Surface, SurfaceId};
use rikka_compositor_types::OutputId;
use rikka_renderer::{Renderer, Scene};

use super::clock::monotonic;
use super::presenting::Presenting;
use super::scenes::Stage;
use super::staging::Staging;
use super::throttle::HIDDEN_FRAME_INTERVAL;
use crate::agent::Canvas;
use crate::animation::{Fader, Scrim, StageContext};
use crate::context::input_ctx;
use crate::state::{Compositor, Parts};
use crate::windows::{around, drawn_on, portrayed};

impl Parts<'_> {
    pub(crate) fn compose(
        &mut self,
        output: OutputId,
        renderer: &mut Renderer,
    ) -> Option<&mut Scene> {
        let published = Some(output).filter(|output| self.outputs.wayland().is_published(*output));
        let mut drawn = self.scenes.spare_drawn();
        if let Some(published) = published {
            drawn_on(self.windows, self.surfaces, published, &mut drawn);
        }
        self.input
            .popups
            .draw(&mut drawn, self.windows, self.surfaces);
        let (mut staging, running) = match published {
            Some(published) => {
                let scrims = self.scrims();
                StageContext {
                    animations: &mut *self.animations,
                    snap: &mut *self.snap,
                    windows: self.windows,
                    palette: self.policy.palette(),
                    caption: self.policy.caption(),
                    see_through: self
                        .selection
                        .drag_drop_target(self.input.seat)
                        .and_then(|toplevel| self.windows.desktop.window_for_toplevel(toplevel)),
                    scrims: &scrims,
                }
                .stage(published, &drawn, Instant::now())
            }
            None => (Staging::default(), false),
        };
        let animations = &*self.animations;
        staging.portrayed = portrayed(
            self.windows,
            self.surfaces,
            &drawn,
            self.policy.caption(),
            |window| animations.rounding(window),
        );
        self.animations.composed(output, running);
        let locked = self.windows.desktop.locked();
        let corners = self.core.profile.is_full()
            && (locked
                || published
                    .is_some_and(|published| around(self.windows, self.surfaces, published).top));
        let output = self.outputs.get(output)?;
        let stage = Stage {
            cursor: self.cursor,
            surfaces: self.surfaces,
            drawn,
            staging,
            icon: self.selection.icon(self.input.seat),
            corners,
            locked,
        };
        let scene = self.scenes.compose(output, stage, renderer);
        self.input
            .agent
            .draw(scene, Canvas::Output(output.id()), output);
        self.input.agent.frame_composed(output.id(), self.outputs);
        Some(scene)
    }

    fn scrims(&self) -> Vec<Scrim> {
        self.windows
            .scrims
            .iter()
            .filter_map(|&layer| {
                let surface = self.windows.desktop.layer(layer)?.surface();
                let frosted = blur_region(self.surfaces, surface).is_some();
                let fader = match self
                    .surfaces
                    .get(surface)
                    .is_some_and(Surface::has_alpha_modifier)
                {
                    true => Fader::Client,
                    false => Fader::Compositor,
                };
                Some(Scrim {
                    layer,
                    frosted,
                    fader,
                })
            })
            .collect()
    }

    pub(crate) fn frames_done(&mut self, output: OutputId, frame: u64) {
        for root in self.scenes.drawn(output, self.surfaces, monotonic(), frame) {
            self.throttle.wait(root, HIDDEN_FRAME_INTERVAL);
        }
        if self.animations.needs_frame(output) {
            self.frames.request(output);
        }
        self.frames.finished(output);
        self.resting.drew();
    }

    pub(crate) fn feedback(&mut self, output: OutputId, scanned: Option<SurfaceId>) -> Presenting {
        self.scenes.feedback(output, self.surfaces, scanned)
    }

    pub(crate) fn composited(
        &mut self,
        output: OutputId,
        frame: u64,
        rendered: bool,
        presentation: &Presentation,
    ) {
        self.frames_done(output, frame);
        if !rendered {
            return self.refreshed(output, next_presentation(presentation));
        }
        let presenting = self.feedback(output, None);
        self.presented(output, presenting, presentation);
    }

    pub(crate) fn presented(
        &mut self,
        output: OutputId,
        presenting: Presenting,
        presentation: &Presentation,
    ) {
        let wayland = self.outputs.wayland();
        for (feedback, presentation) in presenting.feedbacks(presentation) {
            feedback.presented(&presentation, |client| wayland.resources(output, client));
        }
        self.refreshed(output, next_presentation(presentation));
    }

    pub(crate) fn refreshed(&mut self, output: OutputId, next_presentation: Duration) {
        self.frames.refreshed(output, next_presentation);
        if self.input.agent.frame_finished(output, self.outputs) {
            self.core
                .reactor
                .idle(|compositor: &mut Compositor| input_ctx!(compositor).check_agent_idle());
        }
    }

    pub(crate) fn covering(
        &self,
        output: OutputId,
        whole: Rect<i32, Physical>,
    ) -> Option<Candidate<SurfaceId>> {
        self.scenes.covering(output, whole, self.surfaces)
    }
}

fn next_presentation(presentation: &Presentation) -> Duration {
    match presentation.refresh {
        Refresh::Fixed(period) => presentation.time + period,
        Refresh::Variable { fastest } => presentation.time + fastest,
        Refresh::Unknown => presentation.time,
    }
}

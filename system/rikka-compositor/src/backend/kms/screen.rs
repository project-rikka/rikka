use std::collections::VecDeque;
use std::fmt;
use std::sync::Arc;
use std::time::{Duration, Instant};

use rikka_colorimetry::BlendSpace;
use rikka_compositor_geometry::{Size, Transform};
use rikka_compositor_kms::{
    Buffers, Connector, CrtcId, Device, FrameOutcome, FramePacer, Framebuffer, Output,
    OutputConfig, Present, Presented, PresentedFlags, Rect, Refresh, Wake,
};
use rikka_compositor_planes::{
    Assignment, Blocked, Display, Layout, Promotion, Reason, Tearing, VrrPolicy, assign,
    presentation,
};
use rikka_compositor_reactor::{Reactor, Timestamp, Token};
use rikka_compositor_surface::{
    BufferContent, Presentation, PresentationKind, Refresh as SurfaceRefresh, Surface, SurfaceId,
};
use rikka_compositor_types::FormatSet;
use rikka_compositor_types::OutputId;
use rikka_compositor_types::{ContentType, Flip};
use rikka_renderer::{DamageSource, DmabufFence, DmabufOutput, RenderOutput, Renderer};

use crate::color::{image_color, scans_out_unchanged};
use crate::cursor::CursorImage;
use crate::render::Presenting;
use crate::state::{Compositor, Parts};
use crate::windows::fullscreen_surface;

use super::Kms;
use super::advice::Advice;
use super::card::{Card, CardId};
use super::link::{EnabledOutput, Link, Tier};
use super::rendered::Rendered;
use super::scanout::Scanout;

const CATCH_UP: Duration = Duration::from_millis(1);

const MAX_WATCHED_RENDERS: usize = 8;

pub(super) struct Screen {
    id: OutputId,
    card: CardId,
    tier: Tier,
    output: Output,
    link: Link,
    pacer: FramePacer,
    destination: DmabufOutput,
    space: BlendSpace,
    frame: u64,
    in_flight: usize,
    due: Option<Token>,
    queued: bool,
    presentations: VecDeque<(u64, Presenting)>,
    verdict: Option<Assignment<SurfaceId>>,
    promotion: Promotion<SurfaceId>,
    vrr_policy: VrrPolicy,
    scanout: Scanout,
    advised: Option<SurfaceId>,
    cursor: Option<Arc<CursorImage>>,
    cursor_refused: bool,
    transform: Transform,
    started: Instant,
    aimed: Timestamp,
    rendering: VecDeque<(u64, Instant, Timestamp)>,
    failing: bool,
}

pub(super) struct Drawing<'a> {
    pub(super) device: &'a Device,
    pub(super) renderer: &'a mut Renderer,
    pub(super) copier: Option<&'a mut Renderer>,
    pub(super) advice: &'a Advice,
}

enum Made {
    Shown(Rendered),
    Reading {
        framebuffer: Framebuffer,
        areas: Vec<Rect>,
        ready_at: u64,
    },
}

impl Screen {
    pub(super) fn new(
        id: OutputId,
        card: CardId,
        tier: Tier,
        enabled: EnabledOutput,
        renderer: &mut Renderer,
    ) -> Self {
        Self {
            id,
            card,
            tier,
            pacer: FramePacer::new(enabled.output.name()),
            output: enabled.output,
            link: enabled.link,
            destination: renderer.create_dmabuf_output(DamageSource::CompleteForBuffer),
            space: BlendSpace::Sdr,
            frame: 0,
            in_flight: 0,
            due: None,
            queued: false,
            presentations: VecDeque::new(),
            verdict: None,
            promotion: Promotion::default(),
            vrr_policy: VrrPolicy::default(),
            scanout: Scanout::default(),
            advised: None,
            cursor: None,
            cursor_refused: false,
            transform: Transform::Normal,
            started: Instant::now(),
            aimed: Timestamp::now(),
            rendering: VecDeque::new(),
            failing: false,
        }
    }

    pub(super) fn output(&self) -> &Output {
        &self.output
    }

    pub(super) fn card(&self) -> CardId {
        self.card
    }

    pub(super) fn tier(&self) -> Tier {
        self.tier
    }

    pub(super) fn failing(&self) -> bool {
        self.failing
    }

    pub(super) fn fail(&mut self) {
        self.failing = true;
    }

    pub(super) fn awaits(&self, frame: u64) -> bool {
        self.presentations
            .iter()
            .any(|(presented, _)| *presented == frame)
    }

    pub(super) fn release(self, card: &mut Card) -> Output {
        self.link.release(card);
        self.output
    }

    pub(super) fn set_vrr_policy(&mut self, policy: VrrPolicy) {
        self.vrr_policy = policy;
    }

    pub(super) fn modeset(
        &mut self,
        device: &mut Device,
        connector: &Connector,
        drm_config: OutputConfig,
        renderable: FormatSet,
        renderer: &mut Renderer,
    ) -> rikka_compositor_kms::Result<()> {
        let before = (self.output.crtc(), self.output.mode());
        device.reconfigure_output(
            &mut self.output,
            connector,
            drm_config,
            Buffers::Rendered(renderable),
        )?;
        if (self.output.crtc(), self.output.mode()) != before {
            self.link = Link::Direct(Default::default());
            self.destination = renderer.create_dmabuf_output(DamageSource::CompleteForBuffer);
            self.pacer.reset();
            self.rendering.clear();
            self.scanout.reset();
            self.promotion.reset();
        }
        Ok(())
    }

    pub(super) fn crtc(&self) -> CrtcId {
        self.output.crtc()
    }

    pub(super) fn set_blend_space(&mut self, space: BlendSpace) {
        self.destination.set_blend_space(space);
        if let Link::Staged(staged) = &mut self.link {
            staged.target().set_blend_space(space);
        }
        if self.space != space {
            self.space = space;
            self.cursor = None;
            self.promotion.reset();
        }
    }

    pub(super) fn blend_space(&self) -> BlendSpace {
        self.space
    }

    pub(super) fn turn(&mut self, transform: Transform) {
        self.transform = transform;
        self.destination.set_transform(transform);
        if let Link::Staged(staged) = &mut self.link {
            staged.target().set_transform(transform);
        }
    }

    pub(super) fn accepts_cursor_plane(&self) -> bool {
        self.output.has_cursor_plane()
            && !self.cursor_refused
            && self.transform == Transform::Normal
    }

    pub(super) fn refuse_cursor(&mut self) {
        self.cursor_refused = true;
        self.cursor = None;
    }

    pub(super) fn has_cursor_image(&self, image: &Arc<CursorImage>) -> bool {
        self.cursor
            .as_ref()
            .is_some_and(|worn| Arc::ptr_eq(worn, image))
    }

    pub(super) fn set_cursor_image(&mut self, image: Option<Arc<CursorImage>>) {
        self.cursor = image;
    }

    pub(super) fn has_any_cursor_image(&self) -> bool {
        self.cursor.is_some()
    }

    pub(super) fn schedule(&mut self, reactor: &Reactor<Compositor>, not_before: Duration) {
        if self.due.is_some() || self.in_flight >= self.pacer.frames_in_flight() {
            self.queued = true;
            return;
        }
        let delay = self
            .pacer
            .plan(&self.output.timeline().timing(), not_before);
        let scheduled = Instant::now() + delay;
        let id = self.id;
        self.due = Some(reactor.timer(scheduled, move |compositor| {
            Kms::frame(compositor, id, Wake::new(scheduled, Instant::now()));
        }));
    }

    pub(super) fn stop(&mut self, reactor: &Reactor<Compositor>) {
        if let Some(due) = self.due.take() {
            reactor.remove(due);
        }
        self.queued = false;
        self.pacer.reset();
        self.promotion.reset();
        self.rendering.clear();
        if let Link::Staged(staged) = &mut self.link
            && let Some(frame) = staged.abandon()
        {
            self.presentations
                .retain(|(presented, _)| *presented != frame);
            self.in_flight = self.in_flight.saturating_sub(1);
        }
    }

    pub(super) fn release_scanout(&mut self, parts: &mut Parts<'_>) {
        self.scanout.release(parts.scenes, parts.surfaces);
        self.promotion.reset();
        if let Some(surface) = self.advised.take() {
            parts.surfaces.set_surface_dmabuf_feedback(surface, None);
        }
    }

    pub(super) fn draw(&mut self, mut drawing: Drawing<'_>, parts: &mut Parts<'_>, wake: Wake) {
        self.due = None;
        let copier_busy = drawing
            .copier
            .as_ref()
            .is_some_and(|copier| !copier.has_free_frame());
        if !drawing.renderer.has_free_frame() || copier_busy {
            self.schedule(&parts.core.reactor, CATCH_UP);
            return;
        }
        let started = Instant::now();
        self.started = started;
        let aimed_at = self
            .pacer
            .begin(self.output.mode().refresh(), wake.reached(started));
        self.aimed = aimed_at;
        let turn = self.present(&mut drawing, parts);
        self.follow_presentation(drawing.device, parts);
        self.pacer.finished(turn);
        parts.frames_done(self.id, drawing.renderer.submitted_frame());
        if turn == FrameOutcome::Empty && self.in_flight == 0 {
            parts.refreshed(self.id, aimed_at.as_monotonic());
        }
        let stalled = drawing.renderer.take_stall_time();
        if !stalled.is_zero() {
            self.pacer.stalled(stalled);
        }
    }

    fn present(&mut self, drawing: &mut Drawing<'_>, parts: &mut Parts<'_>) -> FrameOutcome {
        if parts.compose(self.id, drawing.renderer).is_none() {
            return FrameOutcome::Empty;
        }
        if self.tier == Tier::Native {
            let assignment = self.plan(parts);
            self.advise(&assignment, drawing.advice, parts);
            let generation = assignment
                .layout()
                .and_then(|layout| parts.scenes.generation(layout.surface));
            let verdict =
                self.promotion
                    .update(assignment, Timestamp::now().as_monotonic(), generation);
            self.judge(verdict);
            if let Assignment::Primary(layout) = verdict
                && let Some(turn) = self.present_alone(drawing.device, parts, layout)
            {
                return turn;
            }
        }
        self.composite(drawing, parts)
    }

    fn follow_presentation(&mut self, device: &Device, parts: &Parts<'_>) {
        let flip = self
            .verdict
            .and_then(|verdict| verdict.layout())
            .map_or(Flip::Vsync, |layout| layout.flip);
        let chosen = presentation(self.vrr_policy, fullscreen_content(parts, self.id), flip);
        self.pacer.set_pacing(chosen.pacing);
        let vrr = chosen.vrr && self.output.vrr_capable();
        if vrr == self.output.vrr() {
            return;
        }
        tracing::debug!(output = self.output.name(), vrr, "variable refresh changed");
        if let Err(error) = device.set_vrr(&mut self.output, vrr) {
            tracing::warn!(
                "cannot change variable refresh of {}: {error}",
                self.output.name()
            );
        }
    }

    fn plan(&self, parts: &Parts<'_>) -> Assignment<SurfaceId> {
        let mode = self.output.mode();
        let tearing = match self.output.async_flips() {
            true => parts.policy.tearing(),
            false => Tearing::Forbidden,
        };
        let display = Display::new(
            Size::new(mode.width() as i32, mode.height() as i32),
            self.transform,
            self.output.scanout_formats(),
        )
        .with_tearing(tearing);
        let covering = parts
            .covering(self.id, display.whole())
            .filter(|candidate| {
                parts
                    .surfaces
                    .get(candidate.id)
                    .is_some_and(|surface| scans_out_unchanged(&image_color(surface), &self.space))
            });
        assign(&display, covering.as_ref(), |format| {
            self.scanout.refuses(format)
        })
    }

    fn judge(&mut self, verdict: Assignment<SurfaceId>) {
        if self.verdict == Some(verdict) {
            return;
        }
        let placed = |verdict: Option<Assignment<SurfaceId>>| {
            verdict
                .and_then(|verdict| verdict.layout())
                .map(|layout| layout.surface)
        };
        match placed(Some(verdict)) != placed(self.verdict) {
            true => tracing::debug!(output = self.output.name(), ?verdict, "scanout changed"),
            false => tracing::trace!(
                output = self.output.name(),
                ?verdict,
                "compositing reason changed"
            ),
        }
        self.verdict = Some(verdict);
    }

    fn present_alone(
        &mut self,
        device: &Device,
        parts: &mut Parts<'_>,
        layout: Layout<SurfaceId>,
    ) -> Option<FrameOutcome> {
        let surface = layout.surface;
        let generation = parts.scenes.generation(surface)?;
        if self.scanout.shows(&layout, generation) {
            return Some(FrameOutcome::Empty);
        }
        let Some(BufferContent::Dmabuf(dmabuf)) = parts.surfaces.content(surface) else {
            return None;
        };
        let framebuffer = self.scanout.framebuffer(device, dmabuf)?;
        let pin = parts.scenes.pin(surface)?;
        self.frame += 1;
        let present = Present::new(self.output.crtc(), self.frame, framebuffer).flip(layout.flip);
        match device.present(present) {
            Ok(()) => {
                let presenting = parts.feedback(self.id, Some(surface));
                self.landing(presenting);
                self.scanout.presented(self.frame, layout, generation, pin);
                Some(FrameOutcome::Committed)
            }
            Err(error) => {
                tracing::debug!(
                    "{}: cannot scan out client buffer on primary plane: {error}",
                    self.output.name()
                );
                parts.scenes.unpin(pin, parts.surfaces);
                None
            }
        }
    }

    fn composite(&mut self, drawing: &mut Drawing<'_>, parts: &mut Parts<'_>) -> FrameOutcome {
        let made = match self.render(drawing, parts) {
            Ok(Some(made)) => made,
            Ok(None) => return FrameOutcome::Empty,
            Err(turn) => return turn,
        };
        self.scanout.composited();
        self.frame += 1;
        match made {
            Made::Shown(rendered) => {
                let (present, submitted) = rendered.into_present(self.output.crtc(), self.frame);
                match drawing.device.present(present) {
                    Ok(()) => {
                        let presenting = parts.feedback(self.id, None);
                        self.landing(presenting);
                        self.watch(submitted, parts);
                        FrameOutcome::Committed
                    }
                    Err(error) => {
                        tracing::warn!("cannot commit {}: {error}", self.output.name());
                        FrameOutcome::Empty
                    }
                }
            }
            Made::Reading {
                framebuffer,
                areas,
                ready_at,
            } => {
                let Link::Staged(staged) = &mut self.link else {
                    return FrameOutcome::Empty;
                };
                staged.copying(framebuffer, self.frame, areas, ready_at);
                let presenting = parts.feedback(self.id, None);
                self.landing(presenting);
                self.watch(ready_at, parts);
                FrameOutcome::Committed
            }
        }
    }

    fn render(
        &mut self,
        drawing: &mut Drawing<'_>,
        parts: &mut Parts<'_>,
    ) -> Result<Option<Made>, FrameOutcome> {
        if let Link::Staged(staged) = &self.link
            && staged.busy()
        {
            self.queued = true;
            return Err(FrameOutcome::Empty);
        }
        let Some(slot) = self.output.swapchain_mut().acquire() else {
            self.queued = true;
            return Err(FrameOutcome::Empty);
        };
        let framebuffer = slot.framebuffer;
        if self.scanout.is_active() {
            parts.scenes.damage_whole(self.id);
        }
        let Some(scene) = parts.scenes.scene(self.id) else {
            return Err(FrameOutcome::Empty);
        };
        let drawn = match &mut self.link {
            Link::Direct(direct) => {
                let target = direct
                    .target(framebuffer.id(), &self.output, drawing.renderer)
                    .map_err(|error| {
                        tracing::warn!(
                            "cannot acquire render target for {}: {error}",
                            self.output.name()
                        );
                        FrameOutcome::Empty
                    })?;
                drawing
                    .renderer
                    .draw(scene, self.destination.with_target(target))
                    .map(|frame| (frame.output, frame.fence))
            }
            Link::Copied(copied) => {
                let Some(stage) = copied.stage(framebuffer.id()) else {
                    return Err(FrameOutcome::Empty);
                };
                drawing
                    .renderer
                    .draw(scene, self.destination.with_target(stage))
                    .map(|frame| (frame.output, frame.fence))
            }
            Link::Staged(staged) => staged
                .draw(drawing.renderer, scene, &framebuffer, slot.age)
                .inspect_err(|_| self.failing = true)
                .map(|output| (output, None)),
        };
        let (output, fence) = drawn.map_err(|error| {
            tracing::warn!("cannot render {}: {error}", self.output.name());
            FrameOutcome::Empty
        })?;
        if output.needs_full_redraw {
            parts.scenes.damage_whole(self.id);
            parts.frames.request(self.id);
        }
        if !output.rendered() {
            return Ok(None);
        }
        let damage = damage_of(&output);
        let submitted = drawing.renderer.submitted_frame();
        match &mut self.link {
            Link::Direct(_) => Ok(Some(Made::Shown(Rendered {
                framebuffer,
                damage,
                fence,
                submitted,
            }))),
            Link::Copied(copied) => {
                let (Some(copier), Some(DmabufFence::SyncFile(drawn))) =
                    (drawing.copier.as_deref_mut(), fence)
                else {
                    tracing::warn!("{}: cannot wait for copy", self.output.name());
                    self.failing = true;
                    return Err(FrameOutcome::Empty);
                };
                match copied.copy(copier, framebuffer.id(), &damage, drawn) {
                    Ok(Some(frame)) => Ok(Some(Made::Shown(Rendered {
                        framebuffer,
                        damage,
                        fence: frame.fence,
                        submitted,
                    }))),
                    Ok(None) => Err(FrameOutcome::Empty),
                    Err(error) => {
                        tracing::warn!("{}: cannot copy frame: {error}", self.output.name());
                        self.failing = true;
                        Err(FrameOutcome::Empty)
                    }
                }
            }
            Link::Staged(staged) => Ok(Some(Made::Reading {
                framebuffer,
                areas: staged.buffer_damage(damage, slot.age),
                ready_at: submitted,
            })),
        }
    }

    pub(super) fn copied(
        &mut self,
        device: &Device,
        renderer: &mut Renderer,
        finished: u64,
        reactor: &Reactor<Compositor>,
    ) {
        let Link::Staged(staged) = &mut self.link else {
            return;
        };
        let copying = match staged.copied(renderer, finished, &mut self.output) {
            None => return,
            Some(Ok(copying)) => copying,
            Some(Err(copying)) => {
                tracing::warn!("cannot fill {}", self.output.name());
                self.failing = true;
                self.lost(copying.frame, reactor);
                return self.schedule(reactor, Duration::ZERO);
            }
        };
        let frame = copying.frame;
        let present =
            Present::new(self.output.crtc(), frame, copying.framebuffer).damage(copying.areas);
        if let Err(error) = device.present(present) {
            tracing::warn!("cannot commit {}: {error}", self.output.name());
            self.lost(frame, reactor);
        }
    }

    fn lost(&mut self, frame: u64, reactor: &Reactor<Compositor>) {
        self.presentations
            .retain(|(presented, _)| *presented != frame);
        self.pacer.discarded();
        self.in_flight = self.in_flight.saturating_sub(1);
        if std::mem::take(&mut self.queued) {
            self.schedule(reactor, Duration::ZERO);
        }
    }

    fn landing(&mut self, presenting: Presenting) {
        self.in_flight += 1;
        self.presentations.push_back((self.frame, presenting));
    }

    fn watch(&mut self, submitted: u64, parts: &mut Parts<'_>) {
        if self.rendering.len() == MAX_WATCHED_RENDERS {
            self.rendering.pop_front();
        }
        self.rendering
            .push_back((submitted, self.started, self.aimed));
        parts.scenes.watch_frame(submitted);
    }

    fn advise(&mut self, verdict: &Assignment<SurfaceId>, advice: &Advice, parts: &mut Parts<'_>) {
        let candidate = match *verdict {
            Assignment::Composited(Some(Blocked {
                surface,
                reason: Reason::Format,
            })) => Some(surface),
            Assignment::Primary(layout) if self.advised == Some(layout.surface) => return,
            _ => None,
        };
        if candidate == self.advised {
            return;
        }
        if let Some(surface) = self.advised.take() {
            parts.surfaces.set_surface_dmabuf_feedback(surface, None);
        }
        let Some(surface) = candidate else {
            return;
        };
        if let Some(feedback) = advice.scanout(self.output.scanout_formats()) {
            tracing::debug!(
                output = self.output.name(),
                "sending scanout dmabuf feedback"
            );
            parts
                .surfaces
                .set_surface_dmabuf_feedback(surface, Some(feedback));
            self.advised = Some(surface);
        }
    }

    pub(super) fn rendered(&mut self, finished: u64, at: Instant) {
        while let Some(&(submitted, started, aimed)) = self.rendering.front()
            && submitted <= finished
        {
            self.rendering.pop_front();
            let done = Timestamp::now().saturating_sub(at.elapsed());
            self.pacer
                .rendered(at.saturating_duration_since(started), done, aimed);
        }
    }

    pub(super) fn rejected(&mut self, frame: u64, parts: &mut Parts<'_>) {
        if !self.scanout.rejected(frame) {
            return tracing::warn!("{} rejected frame {frame}", self.output.name());
        }
        self.promotion.reset();
        parts.scenes.damage_whole(self.id);
        parts.frames.request(self.id);
    }

    pub(super) fn presented(&mut self, presented: &Presented, parts: &mut Parts<'_>) {
        let refresh = match presented.refresh {
            Refresh::Fixed(refresh) => refresh,
            Refresh::Variable { shortest } => shortest,
        };
        self.pacer.presented(presented.at, refresh);
        let presentation = Presentation {
            time: presented.at.as_monotonic(),
            refresh: match presented.refresh {
                Refresh::Fixed(refresh) => SurfaceRefresh::Fixed(refresh),
                Refresh::Variable { shortest } => SurfaceRefresh::Variable { fastest: shortest },
            },
            sequence: presented.sequence,
            flags: kind(presented.flags),
        };
        let presenting = self
            .presentations
            .iter()
            .position(|(frame, _)| *frame == presented.frame)
            .and_then(|index| self.presentations.remove(index))
            .map(|(_, presenting)| presenting)
            .unwrap_or_default();
        parts.presented(self.id, presenting, &presentation);
        self.landed(parts);
    }

    pub(super) fn retired(&mut self, frame: u64, shown: bool, parts: &mut Parts<'_>) {
        self.presentations
            .retain(|(presented, _)| *presented != frame);
        self.scanout.retired(frame, parts.scenes, parts.surfaces);
        if !shown {
            self.pacer.discarded();
            self.landed(parts);
        }
    }

    fn landed(&mut self, parts: &mut Parts<'_>) {
        self.in_flight = self.in_flight.saturating_sub(1);
        if std::mem::take(&mut self.queued) {
            self.schedule(&parts.core.reactor, Duration::ZERO);
        }
    }
}

fn fullscreen_content(parts: &Parts<'_>, output: OutputId) -> Option<ContentType> {
    let root = fullscreen_surface(parts.windows, output)?;
    let surfaces = &*parts.surfaces;
    let content = std::iter::once(root)
        .chain(surfaces.tree(root).map(|placed| placed.surface))
        .filter_map(|surface| surfaces.get(surface))
        .map(Surface::content_type)
        .find(|content| *content != ContentType::None);
    Some(content.unwrap_or_default())
}

fn damage_of(output: &RenderOutput) -> Vec<Rect> {
    output
        .damage
        .iter()
        .map(|rect| {
            let x = rect.x().floor();
            let y = rect.y().floor();
            Rect {
                x: x as i32,
                y: y as i32,
                width: (rect.x() + rect.width() - x).ceil() as u32,
                height: (rect.y() + rect.height() - y).ceil() as u32,
            }
        })
        .collect()
}

fn kind(flags: PresentedFlags) -> PresentationKind {
    [
        (PresentedFlags::VSYNC, PresentationKind::VSYNC),
        (PresentedFlags::HW_CLOCK, PresentationKind::HW_CLOCK),
        (
            PresentedFlags::HW_COMPLETION,
            PresentationKind::HW_COMPLETION,
        ),
        (PresentedFlags::ZERO_COPY, PresentationKind::ZERO_COPY),
    ]
    .into_iter()
    .filter(|(presented, _)| flags.contains(*presented))
    .fold(PresentationKind::empty(), |kind, (_, presentation)| {
        kind | presentation
    })
}

impl fmt::Debug for Screen {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Screen")
            .field("id", &self.id)
            .field("card", &self.card)
            .field("tier", &self.tier)
            .field("output", &self.output.name())
            .field("frame", &self.frame)
            .field("in_flight", &self.in_flight)
            .field("scanout", &self.scanout)
            .finish_non_exhaustive()
    }
}

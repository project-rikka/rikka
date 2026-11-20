mod idle;
mod pointer;
mod ring;
mod service;

use std::time::Instant;

use rikka_compositor_geometry::{Logical, Point};
use rikka_compositor_seat::SeatId;
use rikka_compositor_types::OutputId;
use rikka_renderer::Scene;
use rikka_seam::{Call, Responder, SeamError};
use rikka_session::agent::{AgentPlacement, WaitIdleRequest};
use rikka_types::Color;

use crate::context::{InputCtx, InputRef, input_ctx, input_ref};
use crate::outputs::{Output, Outputs};
use idle::IdleWaits;
pub(crate) use ring::Canvas;
use ring::Ring;
pub(crate) use service::AgentService;

#[derive(Debug)]
pub(crate) struct Agent {
    seat: SeatId,
    at: Point<f64, Logical>,
    relayed: bool,
    shown: bool,
    colour: Option<Color>,
    rekeyed: Vec<u32>,
    idle: IdleWaits,
    ring: Ring,
}

impl Agent {
    pub(crate) fn seat(&self) -> SeatId {
        self.seat
    }

    pub(crate) fn new(seat: SeatId) -> Self {
        Self {
            seat,
            at: Point::default(),
            relayed: false,
            shown: false,
            colour: None,
            rekeyed: Vec::new(),
            idle: IdleWaits::default(),
            ring: Ring::default(),
        }
    }

    pub(crate) fn shown(&self) -> bool {
        self.shown
    }

    pub(crate) fn draw(&mut self, scene: &mut Scene, canvas: Canvas, output: &Output) {
        let at = self.shown.then_some(self.at);
        let colour = self.colour.unwrap_or(Color::WHITE);
        self.ring.draw(scene, canvas, output, at, colour);
    }

    pub(crate) fn forget(&mut self, canvas: Canvas) {
        self.ring.forget(canvas);
    }

    pub(crate) fn frame_composed(&mut self, output: OutputId, outputs: &Outputs) {
        if self.is_on(output, outputs) {
            self.idle.frame_composed(Instant::now());
        }
    }

    pub(crate) fn frame_finished(&mut self, output: OutputId, outputs: &Outputs) -> bool {
        if !self.is_on(output, outputs) {
            return false;
        }
        self.idle.frame_finished(Instant::now());
        !self.idle.is_empty()
    }

    fn is_on(&self, output: OutputId, outputs: &Outputs) -> bool {
        outputs
            .containing(self.at)
            .is_some_and(|containing| containing.id() == output)
    }
}

impl InputCtx<'_> {
    pub(crate) fn set_agent_visible(&mut self, visible: bool) {
        if self.input.agent.shown == visible {
            return;
        }
        self.input.agent.shown = visible;
        self.agent_damaged(self.input.agent.at);
    }

    pub(crate) fn wait_agent_idle(&mut self, responder: Responder, request: WaitIdleRequest) {
        self.input
            .agent
            .idle
            .add(responder, request, Instant::now());
        self.check_agent_idle();
    }

    pub(crate) fn check_agent_idle(&mut self) {
        let frame_requested = self
            .outputs
            .containing(self.input.agent.at)
            .is_some_and(|output| self.frames.is_requested(output.id()));
        let next_check = self
            .input
            .agent
            .idle
            .resolve(Instant::now(), frame_requested);
        let timer = next_check.map(|at| {
            self.core.reactor.timer(at, |compositor| {
                compositor.input.agent.idle.replace_timer(None);
                input_ctx!(compositor).check_agent_idle();
            })
        });
        if let Some(previous) = self.input.agent.idle.replace_timer(timer) {
            self.core.reactor.remove(previous);
        }
    }

    pub(crate) fn answer_agent_placement(&mut self, call: Call) {
        self.recenter_agent_pointer();
        match input_ref!(self).agent_placement() {
            Some(placement) => call.encode(&placement),
            None => call.fail(SeamError::Unavailable(
                "no output for the agent pointer".to_owned(),
            )),
        }
    }

    fn recenter_agent_pointer(&mut self) {
        if self.outputs.containing(self.input.agent.at).is_some() {
            return;
        }
        let Some(area) = self.outputs.enabled().find_map(Output::area) else {
            return;
        };
        let area = area.to_f64();
        self.input.agent.at = Point::new(
            area.loc.x + area.size.w / 2.0,
            area.loc.y + area.size.h / 2.0,
        );
    }

    fn agent_damaged(&mut self, at: Point<f64, Logical>) {
        if let Some(output) = self.outputs.containing(at) {
            self.frames.request(output.id());
        }
    }

    pub(crate) fn agent_turn(&mut self) {
        if self.input.agent.shown {
            self.draw_agent_ring();
        }
        self.recenter_agent_pointer();
    }

    fn draw_agent_ring(&mut self) {
        let colour = self
            .settings
            .is_some()
            .then(|| self.policy.palette().accent);
        if self.input.agent.colour != colour {
            self.input.agent.colour = colour;
            let at = self.input.agent.at;
            self.agent_damaged(at);
        }
    }
}

impl InputRef<'_> {
    fn agent_placement(&self) -> Option<AgentPlacement> {
        let agent = &self.input.agent;
        let output = self.outputs.containing(agent.at)?;
        let area = output.area()?;
        let at = agent.at - area.loc.to_f64();
        Some(AgentPlacement {
            output: output.name().to_owned(),
            width: area.size.w.max(0) as u32,
            height: area.size.h.max(0) as u32,
            x: at.x,
            y: at.y,
            visible: agent.shown,
        })
    }
}

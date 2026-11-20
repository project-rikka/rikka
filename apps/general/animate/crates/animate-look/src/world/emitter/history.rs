use rikka_mmd::FRAMES_PER_SECOND;
use rikka_renderer_3d::MAX_TRACK_KEYS;
use rikka_renderer_3d::glam::Vec3;
use rikka_shader_graph::{Constant, Env, Graph, NodeId, SocketRef, SocketValue, WorldPlan};

use crate::graph_ref::{GraphRef, SocketKey};
use crate::keys::{Keys, Target};
use crate::world::World;
use crate::world::emitter::Placement;
use crate::world::emitter::nodes::PLACEMENT;

const MARGIN: f32 = 1.0 / FRAMES_PER_SECOND;

#[derive(Debug)]
struct Keyed {
    socket: SocketRef,
    target: Target,
}

#[derive(Debug)]
struct Mover {
    node: Vec<NodeId>,
    steps: [Option<usize>; PLACEMENT.len()],
    end: usize,
    keyed: Vec<Keyed>,
}

#[derive(Debug, Default)]
pub(crate) struct Movers {
    movers: Vec<Mover>,
    values: Vec<Constant>,
}

pub(crate) struct Moment<'a> {
    pub(crate) plan: &'a WorldPlan,
    pub(crate) live: &'a Graph,
    pub(crate) keys: &'a Keys,
    pub(crate) frame: f32,
    pub(crate) time: f32,
}

impl Movers {
    pub(crate) fn of(plan: &WorldPlan, world: &World, keyed: &[&SocketRef]) -> Self {
        let program = plan.program();
        let movers = world
            .emitters
            .iter()
            .filter_map(|emitter| {
                let steps = PLACEMENT.map(|field| plan.step(&emitter.node, field));
                let reads = program.reads(steps.iter().flatten().copied());
                let keyed: Vec<Keyed> = keyed
                    .iter()
                    .filter(|socket| reads.sockets.contains(socket))
                    .map(|socket| Keyed {
                        socket: (*socket).clone(),
                        target: Target::Socket(SocketKey {
                            graph: GraphRef::World,
                            socket: (*socket).clone(),
                        }),
                    })
                    .collect();
                (reads.time || !keyed.is_empty()).then(|| Mover {
                    node: emitter.node.clone(),
                    end: steps.iter().flatten().max().map_or(0, |last| last + 1),
                    steps,
                    keyed,
                })
            })
            .collect();
        Self {
            movers,
            values: Vec::new(),
        }
    }

    pub(crate) fn trace(&mut self, moment: &Moment<'_>, world: &mut World) {
        if self.movers.is_empty() || !moment.time.is_finite() {
            return;
        }
        for emitter in &mut world.emitters {
            let Some(mover) = self.movers.iter().find(|mover| mover.node == emitter.node) else {
                continue;
            };
            let lifetime = emitter.lifetime();
            if !(lifetime.is_finite() && lifetime > 0.0) {
                continue;
            }
            let from = (moment.time - lifetime).max(emitter.start().min(moment.time)) - MARGIN;
            let span = moment.time - from;
            let count = ((span * FRAMES_PER_SECOND).ceil() as usize + 1).clamp(2, MAX_TRACK_KEYS);
            let step = span / (count - 1) as f32;
            emitter.track = (0..count)
                .map(|at| {
                    let time = match at + 1 == count {
                        true => moment.time,
                        false => from + step * at as f32,
                    };
                    (time, mover.placement(moment, time, &mut self.values))
                })
                .collect();
        }
    }
}

impl Mover {
    fn placement(&self, moment: &Moment<'_>, time: f32, values: &mut Vec<Constant>) -> Placement {
        let frame = moment.frame - (moment.time - time) * FRAMES_PER_SECOND;
        let sockets = |at: &SocketRef| -> Option<SocketValue> {
            self.keyed
                .iter()
                .find(|keyed| keyed.socket == *at)
                .and_then(|keyed| moment.keys.value_at(&keyed.target, frame))
                .and_then(|value| value.to_socket())
                .or_else(|| moment.live.socket(at).cloned())
        };
        moment.plan.program().run_until(
            &Env {
                time,
                sockets: &sockets,
            },
            values,
            self.end,
        );
        let base = Placement::default();
        let value = |at: usize| self.steps[at].map(|step| values[step]);
        let vector = |at: usize, fallback: Vec3| {
            value(at).map_or(fallback, |held| {
                let held = held.resized(3);
                Vec3::new(held.component(0), held.component(1), held.component(2))
            })
        };
        let scalar =
            |at: usize, fallback: f32| value(at).map_or(fallback, |held| held.component(0));
        Placement {
            position: vector(0, base.position),
            yaw: scalar(1, base.yaw),
            pitch: scalar(2, base.pitch),
            roll: scalar(3, base.roll),
            scale: vector(4, base.scale),
        }
    }
}

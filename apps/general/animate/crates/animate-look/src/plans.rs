use std::sync::Arc;

use rikka_shader_graph::{
    CompositePlan, Constant, Datum, Env, Graph, Library, Record, SocketRef, SocketValue, WorldPlan,
};

use crate::error::Error;
use crate::graph_ref::GraphRef;
use crate::keys::Sampled;
use crate::look::Look;
use crate::world::World;
use crate::world::emitter::history::{Moment, Movers};

const DEPTH_OF_FIELD: &str = "filter.depth_of_field";
const FOCUS: &str = "focus";
const NEAREST_FOCUS: f32 = 0.1;

#[derive(Debug)]
struct Planned<P> {
    source: Arc<Graph>,
    keyed: Vec<SocketRef>,
    live: Arc<Graph>,
    plan: Arc<P>,
}

#[derive(Debug, Default)]
pub struct Plans {
    world: Option<Planned<WorldPlan>>,
    composite: Option<Planned<CompositePlan>>,
    movers: Option<(Arc<WorldPlan>, Movers)>,
}

fn current<P>(
    planned: &mut Option<Planned<P>>,
    look: &Look,
    library: &Library,
    graph: &GraphRef,
    plan: impl FnOnce(&Graph) -> Result<P, rikka_shader_graph::Error>,
) -> Result<(Arc<Graph>, Arc<P>), Error> {
    let source = look
        .graph(graph)
        .ok_or_else(|| Error::Missing(graph.clone()))?;
    if let Some(held) = planned.as_ref()
        && Arc::ptr_eq(&held.source, source)
        && held
            .keyed
            .iter()
            .eq(look.keyed_sockets(graph).map(|(socket, _)| socket))
    {
        return Ok((Arc::clone(&held.live), Arc::clone(&held.plan)));
    }
    let live = look.live_graph(library, graph)?;
    let made = Arc::new(plan(&live).map_err(|source| Error::Graph {
        graph: graph.clone(),
        source,
    })?);
    *planned = Some(Planned {
        source: Arc::clone(source),
        keyed: look
            .keyed_sockets(graph)
            .map(|(socket, _)| socket.clone())
            .collect(),
        live: Arc::clone(&live),
        plan: Arc::clone(&made),
    });
    Ok((live, made))
}

impl Plans {
    pub fn evaluate<'a>(
        &mut self,
        look: &'a Look,
        library: &Library,
        frame: f32,
        time: f32,
    ) -> Result<Evaluated<'a>, Error> {
        let sampled = look.keys.sample(frame);
        let (world_graph, world_plan) =
            current(&mut self.world, look, library, &GraphRef::World, |graph| {
                graph.world(library)
            })?;
        let (composite_graph, composite) = current(
            &mut self.composite,
            look,
            library,
            &GraphRef::Composite,
            |graph| graph.composite(library),
        )?;
        let world_sockets = |at: &SocketRef| {
            sampled
                .socket_in(&GraphRef::World, at)
                .or_else(|| world_graph.socket(at))
                .cloned()
        };
        let mut world = World::from_record(&world_plan.evaluate(&Env {
            time,
            sockets: &world_sockets,
        }))?;
        let movers = match &mut self.movers {
            Some((plan, movers)) if Arc::ptr_eq(plan, &world_plan) => movers,
            held => {
                let keyed: Vec<&SocketRef> = look
                    .keyed_sockets(&GraphRef::World)
                    .map(|(socket, _)| socket)
                    .collect();
                &mut held
                    .insert((
                        Arc::clone(&world_plan),
                        Movers::of(&world_plan, &world, &keyed),
                    ))
                    .1
            }
        };
        movers.trace(
            &Moment {
                plan: &world_plan,
                live: &world_graph,
                keys: &look.keys,
                frame,
                time,
            },
            &mut world,
        );
        let composite_sockets = |at: &SocketRef| {
            sampled
                .socket_in(&GraphRef::Composite, at)
                .or_else(|| composite_graph.socket(at))
                .cloned()
        };
        let records = composite.evaluate(&Env {
            time,
            sockets: &composite_sockets,
        });
        Ok(Evaluated {
            look,
            sampled,
            world,
            composite,
            records,
        })
    }
}

#[derive(Debug)]
pub struct Evaluated<'a> {
    look: &'a Look,
    pub sampled: Sampled,
    pub world: World,
    pub composite: Arc<CompositePlan>,
    pub records: Vec<Option<Record>>,
}

impl Evaluated<'_> {
    #[must_use]
    pub fn look(&self) -> &Look {
        self.look
    }

    #[must_use]
    pub fn socket(&self, graph: &GraphRef, at: &SocketRef) -> Option<SocketValue> {
        self.sampled
            .socket_in(graph, at)
            .or_else(|| self.look.graph(graph)?.socket(at))
            .cloned()
    }

    pub fn autofocus(&mut self, distance: f32) {
        if !self.world.autofocus {
            return;
        }
        let focus = Datum::Number(Constant::scalar(distance.max(NEAREST_FOCUS)));
        for record in self.records.iter_mut().flatten() {
            if record.id != DEPTH_OF_FIELD {
                continue;
            }
            for (name, datum) in &mut record.fields {
                if name == FOCUS {
                    *datum = focus.clone();
                }
            }
        }
    }
}

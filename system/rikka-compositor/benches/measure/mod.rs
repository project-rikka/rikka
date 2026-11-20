mod app;
mod client;
mod clocks;
mod desk;
mod gpu_window;
mod moment;
mod overview;
mod probe;
mod report;
mod scenario;
mod session;
mod timeline;

use std::time::{Duration, Instant};

use rikka_compositor::Extent;
use tracing_subscriber::layer::SubscriberExt as _;
use tracing_subscriber::util::SubscriberInitExt as _;
use tracing_subscriber::{EnvFilter, Layer as _};

pub(crate) use app::PROGRAM as APP;
pub(crate) use report::{Measured, header};
pub(crate) use scenario::Scenario;

use clocks::Clocks;
use desk::Desk;
use probe::Probe;
use timeline::Timeline;

const FLUSH_FRAMES: usize = 8;

#[derive(Debug, Clone, Copy)]
pub(crate) struct Plan {
    pub(crate) extent: Extent,
    pub(crate) millihertz: u32,
    pub(crate) frames: usize,
    pub(crate) cycles: usize,
    pub(crate) rest: Duration,
    pub(crate) settle: Duration,
    pub(crate) clocks: bool,
    pub(crate) glass: bool,
    pub(crate) warm: bool,
}

impl Plan {
    fn interval(&self) -> Duration {
        Duration::from_secs_f64(1000.0 / f64::from(self.millihertz))
    }
}

const LOG: &str = "FRAME_COST_LOG";

pub(crate) fn measure(plan: Plan, scenarios: &[Scenario]) -> Vec<Measured> {
    let probe = Probe::default();
    let logged = EnvFilter::try_from_env(LOG).ok().map(|filter| {
        tracing_subscriber::fmt::layer()
            .with_writer(std::io::stderr)
            .with_filter(filter)
    });
    tracing_subscriber::registry()
        .with(probe.clone().with_filter(EnvFilter::new(probe::FILTER)))
        .with(logged)
        .init();
    let clocks = plan.clocks.then(Clocks::watch);
    let mut desk = Desk::set(plan.extent, plan.glass, plan.millihertz);
    let screen = f64::from(plan.extent.width) * f64::from(plan.extent.height);
    let mut spans = Vec::new();
    for &scenario in scenarios {
        scenario.prepare(&mut desk);
        std::thread::sleep(plan.rest);
        desk.client.roundtrip();
        let (turns, painted, imports) = {
            let seen = probe.seen();
            (seen.turns.len(), seen.painted.len(), seen.imported.len())
        };
        let started = Instant::now();
        let cpu_before = desk.session.compositor_cpu();
        let cycles = scenario::run(&mut desk, scenario, &plan);
        let busy = desk.session.compositor_cpu().saturating_sub(cpu_before);
        let ended = Instant::now();
        let client = desk
            .drawn
            .as_mut()
            .map(|(window, _)| std::mem::take(&mut window.presented))
            .unwrap_or_default();
        let seen = probe.seen();
        spans.push((
            scenario,
            turns..seen.turns.len(),
            painted..seen.painted.len(),
            started..ended,
            client,
            seen.imported.len() - imports,
            busy,
            cycles,
        ));
        scenario.finish(&mut desk);
    }
    scenario::run(
        &mut desk,
        Scenario::Spinner,
        &Plan {
            frames: FLUSH_FRAMES,
            ..plan
        },
    );
    let seen = probe.seen();
    let samples = clocks.as_ref().map(Clocks::taken).unwrap_or_default();
    spans
        .into_iter()
        .map(
            |(scenario, turns, painted, during, client, imports, busy, cycles)| Measured {
                name: scenario.name(),
                turns: seen.turns[turns].to_vec(),
                gpu: seen
                    .gpu
                    .get(painted.clone())
                    .map(<[_]>::to_vec)
                    .unwrap_or_default(),
                painted: seen.painted[painted].to_vec(),
                clocks: samples
                    .iter()
                    .filter(|sample| during.contains(&sample.at))
                    .copied()
                    .collect(),
                client,
                screen,
                imports,
                busy,
                timelines: cycles
                    .into_iter()
                    .enumerate()
                    .map(|(index, cycle)| {
                        Timeline::from_cycle(
                            scenario.name(),
                            index,
                            cycle,
                            &seen,
                            &samples,
                            plan.interval(),
                            screen,
                        )
                    })
                    .collect(),
            },
        )
        .collect()
}

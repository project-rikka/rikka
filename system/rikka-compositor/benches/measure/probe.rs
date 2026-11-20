use rikka_sync::{Mutex, MutexGuard};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tracing::field::{Field, Visit};
use tracing::{Event, Subscriber};
use tracing_subscriber::layer::{Context, Layer};

use super::session::COMPOSITOR_THREAD;

const PASSES: &str = "rikka_renderer::passes";
const PAINTER: &str = "rikka_renderer::renderer::painter";
const IMPORTS: &str = "rikka_renderer::imports";
const RELEASES: &str = "rikka_compositor::render::imports";
const HEADLESS: &str = "rikka_compositor::backend::headless";

const FRAME_PASS: &str = "rikka.frame";
const BACKDROP_PASS: &str = "rikka.scene.backdrop";
const FX_PASSES: &str = "rikka.fx.";

pub(crate) const FILTER: &str = "warn,rikka_renderer::passes=debug,rikka_renderer::renderer::painter=trace,rikka_renderer::imports=trace,rikka_compositor::render::imports=trace,rikka_compositor::backend::headless=trace";

#[derive(Debug, Clone, Default)]
pub(crate) struct GpuFrame {
    pub(crate) frame: u64,
    pub(crate) total: f64,
    pub(crate) backdrop: f64,
    pub(crate) fx: f64,
    pub(crate) fx_passes: usize,
    pub(crate) passes: Vec<(String, f64)>,
}

impl GpuFrame {
    pub(crate) fn spent(&self, pass: &str) -> f64 {
        self.passes
            .iter()
            .filter(|(name, _)| name == pass)
            .map(|(_, millis)| millis)
            .sum()
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Painted {
    pub(crate) at: Instant,
    pub(crate) area: f64,
    pub(crate) rects: u64,
    pub(crate) composes: bool,
    pub(crate) emitted: u64,
    pub(crate) restructured: u64,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Turn {
    pub(crate) at: Instant,
    pub(crate) work: Duration,
    pub(crate) composed: Duration,
    pub(crate) rendered: bool,
    pub(crate) frame: u64,
}

impl Turn {
    pub(crate) fn started(&self) -> Instant {
        self.at - self.work
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Import {
    pub(crate) at: Instant,
    pub(crate) width: u64,
    pub(crate) height: u64,
}

#[derive(Debug, Default)]
pub(crate) struct Seen {
    pub(crate) gpu: Vec<GpuFrame>,
    pub(crate) painted: Vec<Painted>,
    pub(crate) turns: Vec<Turn>,
    pub(crate) reached: HashMap<u64, Instant>,
    pub(crate) imported: Vec<Import>,
    pub(crate) released: Vec<Import>,
}

impl Seen {
    pub(crate) fn gpu_of(&self, frame: u64) -> Option<&GpuFrame> {
        self.gpu.iter().rev().find(|gpu| gpu.frame == frame)
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct Probe(Arc<Mutex<Seen>>);

impl Probe {
    pub(crate) fn seen(&self) -> MutexGuard<'_, Seen> {
        self.0.lock()
    }
}

#[derive(Default)]
struct Fields {
    pass: Option<String>,
    micros: f64,
    area: f64,
    painted: u64,
    frame: u64,
    width: u64,
    height: u64,
    work: Option<Duration>,
    heard: Option<Duration>,
    composed: Duration,
    rendered: bool,
    composes: bool,
    emitted: u64,
    restructured: u64,
}

impl Visit for Fields {
    fn record_f64(&mut self, field: &Field, value: f64) {
        match field.name() {
            "micros" => self.micros = value,
            "area" => self.area = value,
            _ => {}
        }
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        match field.name() {
            "painted" => self.painted = value,
            "emitted" => self.emitted = value,
            "restructured" => self.restructured = value,
            "frame" => self.frame = value,
            "width" => self.width = value,
            "height" => self.height = value,
            _ => {}
        }
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.record_u64(field, u64::try_from(value).unwrap_or_default());
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        match field.name() {
            "rendered" => self.rendered = value,
            "composes" => self.composes = value,
            _ => {}
        }
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "pass" {
            self.pass = Some(value.to_owned());
        }
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        let shown = || parse_duration(&format!("{value:?}"));
        match field.name() {
            "work" => self.work = shown(),
            "heard" => self.heard = shown(),
            "composed" => self.composed = shown().unwrap_or_default(),
            _ => {}
        }
    }
}

fn parse_duration(shown: &str) -> Option<Duration> {
    let units = [("ns", 1e-9), ("µs", 1e-6), ("ms", 1e-3), ("s", 1.0)];
    units.iter().find_map(|(unit, seconds)| {
        let number = shown.strip_suffix(unit)?;
        number
            .parse::<f64>()
            .ok()
            .map(|value| Duration::from_secs_f64(value * seconds))
    })
}

impl<S: Subscriber> Layer<S> for Probe {
    fn on_event(&self, event: &Event<'_>, _: Context<'_, S>) {
        let at = Instant::now();
        let target = event.metadata().target();
        if ![PASSES, PAINTER, IMPORTS, RELEASES, HEADLESS].contains(&target) {
            return;
        }
        if std::thread::current().name() != Some(COMPOSITOR_THREAD) {
            return;
        }
        let mut fields = Fields::default();
        event.record(&mut fields);
        let mut seen = self.seen();
        match target {
            IMPORTS | RELEASES => {
                let import = Import {
                    at,
                    width: fields.width,
                    height: fields.height,
                };
                match target {
                    IMPORTS => seen.imported.push(import),
                    _ => seen.released.push(import),
                }
            }
            PASSES => {
                let Some(pass) = fields.pass else {
                    return;
                };
                if pass == FRAME_PASS || seen.gpu.is_empty() {
                    seen.gpu.push(GpuFrame {
                        frame: fields.frame,
                        ..GpuFrame::default()
                    });
                }
                let frame = seen.gpu.last_mut().expect("frame in progress");
                let millis = fields.micros / 1000.0;
                if pass == FRAME_PASS {
                    frame.total = millis;
                }
                if pass == BACKDROP_PASS {
                    frame.backdrop += millis;
                }
                if pass.starts_with(FX_PASSES) {
                    frame.fx += millis;
                    frame.fx_passes += 1;
                }
                match frame.passes.iter_mut().find(|(name, _)| *name == pass) {
                    Some((_, spent)) => *spent += millis,
                    None => frame.passes.push((pass, millis)),
                }
            }
            PAINTER => seen.painted.push(Painted {
                at,
                area: fields.area,
                rects: fields.painted,
                composes: fields.composes,
                emitted: fields.emitted,
                restructured: fields.restructured,
            }),
            _ => match (fields.work, fields.heard) {
                (Some(work), _) => seen.turns.push(Turn {
                    at,
                    work,
                    composed: fields.composed,
                    rendered: fields.rendered,
                    frame: fields.frame,
                }),
                (None, Some(heard)) => {
                    seen.reached.insert(fields.frame, at - heard);
                }
                (None, None) => {}
            },
        }
    }
}

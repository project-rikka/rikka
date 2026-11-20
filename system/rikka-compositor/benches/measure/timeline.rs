use std::fmt;
use std::time::{Duration, Instant};

use super::clocks::Sample;
use super::moment::Cycle;
use super::probe::{Import, Painted, Seen, Turn};

const HEAVY_PASS: f64 = 0.5;

const FX_PASSES: &str = "rikka.fx.";

#[derive(Debug, Clone)]
pub(crate) struct Row {
    started: Instant,
    turn: Turn,
    ready: Option<Duration>,
    gpu: Option<(f64, f64, f64, f64, usize)>,
    painted: Option<Painted>,
    clock: Option<Sample>,
    notes: Vec<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct Timeline {
    name: &'static str,
    index: usize,
    cycle: Cycle,
    interval: Duration,
    screen: f64,
    rows: Vec<Row>,
}

impl Timeline {
    pub(crate) fn from_cycle(
        name: &'static str,
        index: usize,
        cycle: Cycle,
        seen: &Seen,
        clocks: &[Sample],
        interval: Duration,
        screen: f64,
    ) -> Self {
        let drawn: Vec<&Turn> = seen
            .turns
            .iter()
            .filter(|turn| turn.rendered && turn.at >= cycle.began && turn.started() <= cycle.ended)
            .collect();
        let mut previous = cycle.began;
        let rows = drawn
            .into_iter()
            .map(|turn| {
                let started = turn.started();
                let painted = seen
                    .painted
                    .iter()
                    .rev()
                    .find(|painted| painted.at >= started && painted.at <= turn.at)
                    .copied();
                let gpu = seen.gpu_of(turn.frame).map(|gpu| {
                    (
                        gpu.total,
                        gpu.spent("rikka.scene.draw"),
                        gpu.backdrop,
                        gpu.fx,
                        gpu.fx_passes,
                    )
                });
                let mut notes = Vec::new();
                let within = |at: Instant| at > previous && at <= turn.at;
                if let Some(answered) = cycle.answered {
                    if within(answered.configured) {
                        notes.push("configured".to_owned());
                    }
                    if within(answered.committed) {
                        notes.push(format!(
                            "client committed (drew {:.2})",
                            millis(answered.drawing)
                        ));
                    }
                }
                notes.extend(listed("+import", &seen.imported, within));
                notes.extend(listed("-release", &seen.released, within));
                if let Some(gpu) = seen.gpu_of(turn.frame) {
                    notes.extend(
                        gpu.passes
                            .iter()
                            .filter(|(name, millis)| {
                                *millis >= HEAVY_PASS && name.starts_with(FX_PASSES)
                            })
                            .map(|(name, millis)| format!("{name}={millis:.2}")),
                    );
                }
                previous = turn.at;
                Row {
                    started,
                    turn: *turn,
                    ready: seen
                        .reached
                        .get(&turn.frame)
                        .map(|reached| reached.saturating_duration_since(started)),
                    gpu,
                    painted,
                    clock: nearest(clocks, started),
                    notes,
                }
            })
            .collect();
        Self {
            name,
            index,
            cycle,
            interval,
            screen,
            rows,
        }
    }

    fn late(&self, row: &Row) -> bool {
        row.ready.is_some_and(|ready| ready > self.interval)
    }
}

fn listed<'a>(
    sign: &'a str,
    imports: &'a [Import],
    within: impl Fn(Instant) -> bool + 'a,
) -> impl Iterator<Item = String> + 'a {
    imports
        .iter()
        .filter(move |import| within(import.at))
        .map(move |import| format!("{sign} {}x{}", import.width, import.height))
}

fn nearest(clocks: &[Sample], at: Instant) -> Option<Sample> {
    clocks
        .iter()
        .copied()
        .min_by_key(|sample| match sample.at.checked_duration_since(at) {
            Some(after) => after,
            None => at - sample.at,
        })
}

fn millis(spent: Duration) -> f64 {
    spent.as_secs_f64() * 1000.0
}

fn since(at: Instant, began: Instant) -> f64 {
    match at.checked_duration_since(began) {
        Some(after) => millis(after),
        None => -millis(began - at),
    }
}

impl fmt::Display for Timeline {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let began = self.cycle.began;
        let late = self.rows.iter().filter(|row| self.late(row)).count();
        let mut run = 0;
        let mut longest = 0;
        for row in &self.rows {
            run = match self.late(row) {
                true => run + 1,
                false => 0,
            };
            longest = longest.max(run);
        }
        let gpu: f64 = self
            .rows
            .iter()
            .filter_map(|row| row.gpu)
            .map(|gpu| gpu.0)
            .sum();
        let worst_ready = self
            .rows
            .iter()
            .filter_map(|row| row.ready)
            .max()
            .unwrap_or_default();
        let last = self
            .rows
            .last()
            .map_or(0.0, |row| since(row.started, began));
        write!(formatter, "{} #{}:", self.name, self.index + 1)?;
        if let Some(reply) = self.cycle.answered {
            write!(
                formatter,
                " configure +{:.1} ms, commit +{:.1} ms,",
                since(reply.configured, began),
                since(reply.committed, began)
            )?;
        }
        writeln!(
            formatter,
            " {} frames until +{last:.0} ms, {late} with ready > {:.2} ms (longest run {longest}), worst ready {:.2} ms, gpu sum {gpu:.1} ms",
            self.rows.len(),
            millis(self.interval),
            millis(worst_ready),
        )?;
        writeln!(
            formatter,
            "  {:>7} {:>6} {:>6} {:>7} {:>6} {:>6} {:>6} {:>3} {:>6} {:>5} {:>6} {:>13}  notes",
            "t ms",
            "cpu",
            "comp",
            "ready",
            "gpu",
            "draw",
            "blur",
            "fxn",
            "area%",
            "rects",
            "emit",
            "P MHz gr/m",
        )?;
        for row in &self.rows {
            let (total, draw, blur, _, passes) = row.gpu.unwrap_or_default();
            let ready = row
                .ready
                .map_or_else(|| "-".to_owned(), |ready| format!("{:.2}", millis(ready)));
            let clock = row.clock.map_or_else(
                || "-".to_owned(),
                |clock| format!("P{} {}/{}", clock.pstate, clock.graphics, clock.memory),
            );
            let late = match self.late(row) {
                true => "LATE ",
                false => "",
            };
            let (area, rects, emitted, composes, restructured) =
                row.painted.map_or((0.0, 0, 0, false, false), |painted| {
                    (
                        painted.area,
                        painted.rects,
                        painted.emitted,
                        painted.composes,
                        painted.restructured > 0,
                    )
                });
            let composes = match composes {
                true => "composed ",
                false => "",
            };
            let restructured = match restructured {
                true => "relaid ",
                false => "",
            };
            writeln!(
                formatter,
                "  {:>7.1} {:>6.2} {:>6.2} {:>7} {:>6.2} {:>6.2} {:>6.2} {:>3} {:>6.1} {:>5} {:>6} {:>13}  {late}{composes}{restructured}{}",
                since(row.started, began),
                millis(row.turn.work),
                millis(row.turn.composed),
                ready,
                total,
                draw,
                blur,
                passes,
                area / self.screen * 100.0,
                rects,
                emitted,
                clock,
                row.notes.join(", "),
            )?;
        }
        Ok(())
    }
}

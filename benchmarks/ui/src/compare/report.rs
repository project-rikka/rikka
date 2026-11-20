use std::fmt;

use serde::Serializer;
use ui_scene::Scenario;

use super::framework::Framework;
use super::measurement::{Hover, Idle, Measurement, Startup, Steady};
use super::process::Memory;
use super::stats::{median, spread};

const MIB: f64 = 1_048_576.0;

#[derive(Clone, Debug, serde::Serialize)]
pub struct Record {
    pub framework: Framework,
    #[serde(serialize_with = "by_name")]
    pub scenario: Scenario,
    pub repetition: usize,
    pub measurement: Measurement,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct Failure {
    pub framework: Framework,
    #[serde(serialize_with = "by_name")]
    pub scenario: Scenario,
    pub repetition: usize,
    pub error: String,
}

#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct Report {
    pub machine: Vec<(String, String)>,
    pub records: Vec<Record>,
    pub failures: Vec<Failure>,
}

#[derive(Debug)]
pub struct Markdown<'a> {
    report: &'a Report,
    scenarios: &'a [Scenario],
    frameworks: &'a [Framework],
}

struct Row<'a> {
    framework: Framework,
    runs: Vec<&'a Measurement>,
}

impl Report {
    #[must_use]
    pub fn markdown<'a>(
        &'a self,
        scenarios: &'a [Scenario],
        frameworks: &'a [Framework],
    ) -> Markdown<'a> {
        Markdown {
            report: self,
            scenarios,
            frameworks,
        }
    }
}

impl fmt::Display for Markdown<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (name, value) in &self.report.machine {
            writeln!(out, "- {name}: {value}")?;
        }
        for scenario in self.scenarios {
            let rows: Vec<Row<'_>> = self
                .frameworks
                .iter()
                .map(|framework| Row {
                    framework: *framework,
                    runs: self
                        .report
                        .records
                        .iter()
                        .filter(|record| {
                            record.framework == *framework && record.scenario == *scenario
                        })
                        .map(|record| &record.measurement)
                        .collect(),
                })
                .filter(|row| !row.runs.is_empty())
                .collect();
            let Some(first) = rows.first() else {
                continue;
            };
            writeln!(out, "\n### {scenario}\n")?;
            match first.runs[0] {
                Measurement::Startup(_) => startup_table(out, &rows)?,
                Measurement::Steady(_) => steady_table(out, &rows)?,
                Measurement::Idle(_) => idle_table(out, &rows)?,
                Measurement::Hover(_) => hover_table(out, &rows)?,
            }
        }
        if !self.report.failures.is_empty() {
            writeln!(out, "\n### Failures\n")?;
            for failure in &self.report.failures {
                writeln!(
                    out,
                    "- {} {} (run {}): {}",
                    failure.framework, failure.scenario, failure.repetition, failure.error
                )?;
            }
        }
        Ok(())
    }
}

impl<'a> Row<'a> {
    fn measured<T>(&self, pick: fn(&'a Measurement) -> Option<&'a T>) -> Vec<&'a T> {
        self.runs.iter().filter_map(|run| pick(run)).collect()
    }

    fn memory_mb(&self, read: fn(Memory) -> u64) -> String {
        format!(
            "{:.1}",
            median(self.runs.iter().map(|run| read(run.memory()) as f64)) / MIB
        )
    }
}

fn startup_table(out: &mut fmt::Formatter<'_>, rows: &[Row<'_>]) -> fmt::Result {
    writeln!(
        out,
        "| Framework | First frame ms | Range | Window | RSS MB | PSS MB | Private MB | GPU MB |\n| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |"
    )?;
    for row in rows {
        let runs: Vec<&Startup> = row.measured(|run| match run {
            Measurement::Startup(startup) => Some(startup),
            _ => None,
        });
        let first = runs.iter().map(|run| run.first_frame_ms);
        let (low, high) = spread(first.clone());
        let (width, height) = runs[0].window;
        writeln!(
            out,
            "| {} | {:.0} | {low:.0}–{high:.0} | {width}×{height} | {} | {} | {} | {} |",
            row.framework,
            median(first),
            row.memory_mb(|memory| memory.rss),
            row.memory_mb(|memory| memory.pss),
            row.memory_mb(|memory| memory.private),
            gpu_mb(runs.iter().map(|run| run.gpu_memory)),
        )?;
    }
    Ok(())
}

fn steady_table(out: &mut fmt::Formatter<'_>, rows: &[Row<'_>]) -> fmt::Result {
    writeln!(
        out,
        "| Framework | FPS | CPU ms/frame | Range | Main thread ms/frame | CPU % | Frame p50 / p99 ms | Jank % | GPU MB | RSS MB | Server ms/frame |\n| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |"
    )?;
    for row in rows {
        let runs: Vec<&Steady> = row.measured(|run| match run {
            Measurement::Steady(steady) => Some(steady),
            _ => None,
        });
        let field = |read: fn(&Steady) -> f64| median(runs.iter().map(|run| read(run)));
        let (low, high) = spread(runs.iter().map(|run| run.cpu_ms_per_frame));
        writeln!(
            out,
            "| {} | {:.1} | {:.2} | {low:.2}–{high:.2} | {:.2} | {:.1} | {:.1} / {:.1} | {:.1} | {} | {} | {:.2} |",
            row.framework,
            field(|run| run.fps),
            field(|run| run.cpu_ms_per_frame),
            field(|run| run.main_thread_ms_per_frame),
            field(|run| run.cpu_percent),
            field(|run| run.frames.p50_ms),
            field(|run| run.frames.p99_ms),
            field(|run| run.frames.jank_percent),
            gpu_mb(runs.iter().map(|run| run.gpu_memory)),
            row.memory_mb(|memory| memory.rss),
            field(|run| run.server_ms_per_frame),
        )?;
    }
    Ok(())
}

fn idle_table(out: &mut fmt::Formatter<'_>, rows: &[Row<'_>]) -> fmt::Result {
    writeln!(
        out,
        "| Framework | CPU % | Wakeups/s | RSS MB | PSS MB | GPU MB | Server % |\n| --- | ---: | ---: | ---: | ---: | ---: | ---: |"
    )?;
    for row in rows {
        let runs: Vec<&Idle> = row.measured(|run| match run {
            Measurement::Idle(idle) => Some(idle),
            _ => None,
        });
        let field = |read: fn(&Idle) -> f64| median(runs.iter().map(|run| read(run)));
        writeln!(
            out,
            "| {} | {:.2} | {:.1} | {} | {} | {} | {:.2} |",
            row.framework,
            field(|run| run.cpu_percent),
            field(|run| run.wakeups_per_second),
            row.memory_mb(|memory| memory.rss),
            row.memory_mb(|memory| memory.pss),
            gpu_mb(runs.iter().map(|run| run.gpu_memory)),
            field(|run| run.server_percent),
        )?;
    }
    Ok(())
}

fn hover_table(out: &mut fmt::Formatter<'_>, rows: &[Row<'_>]) -> fmt::Result {
    writeln!(
        out,
        "| Framework | CPU % | CPU ms/move | Range | Main thread ms/move | RSS MB | Server % |\n| --- | ---: | ---: | ---: | ---: | ---: | ---: |"
    )?;
    for row in rows {
        let runs: Vec<&Hover> = row.measured(|run| match run {
            Measurement::Hover(hover) => Some(hover),
            _ => None,
        });
        let field = |read: fn(&Hover) -> f64| median(runs.iter().map(|run| read(run)));
        let (low, high) = spread(runs.iter().map(|run| run.cpu_ms_per_move));
        writeln!(
            out,
            "| {} | {:.1} | {:.2} | {low:.2}–{high:.2} | {:.2} | {} | {:.1} |",
            row.framework,
            field(|run| run.cpu_percent),
            field(|run| run.cpu_ms_per_move),
            field(|run| run.main_thread_ms_per_move),
            row.memory_mb(|memory| memory.rss),
            field(|run| run.server_percent),
        )?;
    }
    Ok(())
}

fn gpu_mb(values: impl Iterator<Item = Option<u64>>) -> String {
    let known: Vec<f64> = values.flatten().map(|bytes| bytes as f64 / MIB).collect();
    match known.is_empty() {
        true => "–".to_owned(),
        false => format!("{:.0}", median(known)),
    }
}

fn by_name<S: Serializer>(scenario: &Scenario, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.collect_str(scenario)
}

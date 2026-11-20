mod error;
mod framework;
mod gpu;
mod measurement;
mod process;
mod report;
mod runner;
mod session;
mod stats;
mod supervised;

use std::path::Path;

use ui_scene::{FRAME_INTERVAL, Scenario};

pub use error::Error;
pub use framework::Framework;
pub use measurement::{Hover, Idle, Measurement, Startup, Steady};
pub use process::Memory;
pub use report::{Failure, Markdown, Record, Report};
pub use runner::Timing;
pub use stats::FrameTimes;

use runner::Runner;
use session::{OUTPUT_HEIGHT, OUTPUT_WIDTH, Session};

const STARTUP_LAUNCHES: usize = 5;

const UNKNOWN: &str = "unknown";

#[derive(Clone, Debug)]
pub struct Plan {
    pub frameworks: Vec<Framework>,
    pub scenarios: Vec<Scenario>,
    pub repetitions: usize,
    pub timing: Timing,
}

pub fn run(compositor: &Path, plan: &Plan, progress: impl Fn(&str)) -> Result<Report, Error> {
    let session = Session::start(compositor)?;
    let mut runner = Runner::new(&session, plan.timing)?;
    let mut report = Report {
        machine: machine(&plan.frameworks),
        ..Report::default()
    };
    for repetition in 0..plan.repetitions {
        for scenario in &plan.scenarios {
            let launches = match scenario {
                Scenario::Startup => STARTUP_LAUNCHES,
                _ => 1,
            };
            for turn in 0..plan.frameworks.len() {
                let framework = plan.frameworks[(turn + repetition) % plan.frameworks.len()];
                for _ in 0..launches {
                    progress(&format!(
                        "measuring {framework} {scenario} (run {})",
                        repetition + 1
                    ));
                    match runner.measure(framework, *scenario) {
                        Ok(measurement) => report.records.push(Record {
                            framework,
                            scenario: *scenario,
                            repetition,
                            measurement,
                        }),
                        Err(error) => {
                            progress(&format!("{framework} {scenario} failed: {error}"));
                            report.failures.push(Failure {
                                framework,
                                scenario: *scenario,
                                repetition,
                                error: error.to_string(),
                            });
                        }
                    }
                }
            }
        }
    }
    Ok(report)
}

fn machine(frameworks: &[Framework]) -> Vec<(String, String)> {
    let cpu = std::fs::read_to_string("/proc/cpuinfo")
        .ok()
        .and_then(|cpuinfo| {
            cpuinfo
                .lines()
                .find_map(|line| line.strip_prefix("model name"))
                .and_then(|rest| rest.split_once(':'))
                .map(|(_, name)| name.trim().to_owned())
        })
        .unwrap_or_else(|| UNKNOWN.to_owned());
    let threads = std::thread::available_parallelism()
        .map_or_else(|_| UNKNOWN.to_owned(), |threads| threads.to_string());
    let kernel = std::fs::read_to_string("/proc/sys/kernel/osrelease")
        .map_or_else(|_| UNKNOWN.to_owned(), |kernel| kernel.trim().to_owned());
    let gpus = rikka_sysmon::gpu::Gpu::new()
        .adapters()
        .into_iter()
        .map(|adapter| format!("{} ({})", adapter.name, adapter.driver))
        .collect::<Vec<_>>()
        .join(", ");
    let mut machine = vec![
        ("CPU".to_owned(), format!("{cpu}, {threads} threads")),
        ("GPU".to_owned(), gpus),
        ("Kernel".to_owned(), kernel),
        (
            "Display".to_owned(),
            format!(
                "rikka-compositor --headless {OUTPUT_WIDTH}x{OUTPUT_HEIGHT} at {:.0} Hz",
                1.0 / FRAME_INTERVAL.as_secs_f64()
            ),
        ),
    ];
    machine.extend(
        frameworks
            .iter()
            .map(|framework| (framework.name().to_owned(), framework.version().to_owned())),
    );
    machine
}

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use ui_compare::{Framework, Plan, Timing};
use ui_scene::Scenario;

const COMPOSITOR: &str = "UI_BENCH_COMPOSITOR";

const REPETITIONS: usize = 3;

const WARM_UP: Duration = Duration::from_secs(3);

const WINDOW: Duration = Duration::from_secs(10);

const USAGE: &str =
    "usage: compare [--reps N] [--warmup SECONDS] [--seconds SECONDS] [--json PATH] [FILTER...]

FILTER picks frameworks (rikka gpui gpui-fast bui bui-gpu egui iced) and scenarios
(startup cells-<count>-<anim|one|idle> scroll hover) by name; with none of a kind, all run.";

struct Arguments {
    plan: Plan,
    json: Option<PathBuf>,
}

fn main() -> ExitCode {
    let arguments = match parse() {
        Ok(arguments) => arguments,
        Err(message) => {
            eprintln!("{message}\n\n{USAGE}");
            return ExitCode::FAILURE;
        }
    };
    let Some(compositor) = std::env::var_os(COMPOSITOR).map(PathBuf::from) else {
        eprintln!(
            "{COMPOSITOR} is not set; run through `bazel run --config=bench //benchmarks/ui:compare`"
        );
        return ExitCode::FAILURE;
    };
    let report = match ui_compare::run(&compositor, &arguments.plan, |step| eprintln!("{step}")) {
        Ok(report) => report,
        Err(error) => {
            eprintln!("failed to compare: {error}");
            return ExitCode::FAILURE;
        }
    };
    println!(
        "{}",
        report.markdown(&arguments.plan.scenarios, &arguments.plan.frameworks)
    );
    if let Some(path) = arguments.json {
        let written = serde_json::to_vec_pretty(&report)
            .map_err(|error| error.to_string())
            .and_then(|json| std::fs::write(&path, json).map_err(|error| error.to_string()));
        if let Err(error) = written {
            eprintln!("failed to write {}: {error}", path.display());
            return ExitCode::FAILURE;
        }
    }
    match report.failures.is_empty() {
        true => ExitCode::SUCCESS,
        false => ExitCode::FAILURE,
    }
}

fn parse() -> Result<Arguments, String> {
    let mut repetitions = REPETITIONS;
    let mut timing = Timing {
        warm_up: WARM_UP,
        window: WINDOW,
    };
    let mut json = None;
    let mut frameworks = Vec::new();
    let mut scenarios = Vec::new();
    let mut arguments = std::env::args()
        .skip(1)
        .filter(|argument| argument != "--bench");
    while let Some(argument) = arguments.next() {
        let mut value = |name: &str| arguments.next().ok_or(format!("{name} needs a value"));
        match argument.as_str() {
            "--reps" => {
                repetitions = value("--reps")?
                    .parse()
                    .map_err(|_| "--reps takes a count".to_owned())?;
            }
            "--warmup" => timing.warm_up = seconds(&value("--warmup")?)?,
            "--seconds" => timing.window = seconds(&value("--seconds")?)?,
            "--json" => json = Some(PathBuf::from(value("--json")?)),
            "--help" | "-h" => return Err(String::new()),
            filter => match Framework::from_name(filter) {
                Some(framework) => frameworks.push(framework),
                None => scenarios.push(
                    filter
                        .parse::<Scenario>()
                        .map_err(|error| error.to_string())?,
                ),
            },
        }
    }
    if frameworks.is_empty() {
        frameworks = Framework::ALL.to_vec();
    }
    if scenarios.is_empty() {
        scenarios = Scenario::all();
    }
    Ok(Arguments {
        plan: Plan {
            frameworks,
            scenarios,
            repetitions,
            timing,
        },
        json,
    })
}

fn seconds(value: &str) -> Result<Duration, String> {
    value
        .parse::<f64>()
        .ok()
        .filter(|seconds| *seconds > 0.0)
        .map(Duration::from_secs_f64)
        .ok_or(format!("{value} is not a number of seconds"))
}

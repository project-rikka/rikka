mod measure;

use std::time::Duration;

use measure::{APP, Plan, Scenario, header, measure};
use rikka_compositor::Extent;

const BENCH: Plan = Plan {
    extent: Extent {
        width: 2560,
        height: 1440,
    },
    millihertz: 180_000,
    frames: 240,
    cycles: 5,
    rest: Duration::from_secs(3),
    settle: Duration::from_millis(900),
    clocks: true,
    glass: true,
    warm: false,
};

const SMOKE: Plan = Plan {
    extent: Extent {
        width: 480,
        height: 300,
    },
    millihertz: 60_000,
    frames: 4,
    cycles: 1,
    rest: Duration::from_millis(100),
    settle: Duration::from_millis(100),
    clocks: false,
    glass: true,
    warm: false,
};

fn main() {
    let benching = std::env::args().any(|argument| argument == "--bench");
    let filters: Vec<String> = std::env::args()
        .skip(1)
        .filter(|argument| !argument.starts_with("--"))
        .collect();
    let app = std::env::var_os(APP).is_some();
    let offered = Scenario::ALL
        .into_iter()
        .filter(|scenario| app || !scenario.needs_app());
    let scenarios: Vec<Scenario> = match filters.is_empty() {
        true => offered.collect(),
        false => filters
            .iter()
            .flat_map(|name| {
                offered
                    .clone()
                    .filter(move |scenario| scenario.name().contains(name.as_str()))
            })
            .collect(),
    };
    let plan = Plan {
        glass: !std::env::args().any(|argument| argument == "--solid"),
        warm: std::env::args().any(|argument| argument == "--warm"),
        ..match benching {
            true => BENCH,
            false => SMOKE,
        }
    };
    let measured = measure(plan, &scenarios);
    println!("{}", header());
    for one in &measured {
        println!("{one}");
        assert!(
            one.name == Scenario::Idle.name() || one.painted() > 0,
            "{} drew nothing",
            one.name
        );
    }
    for one in &measured {
        for timeline in &one.timelines {
            print!("\n{timeline}");
        }
    }
}

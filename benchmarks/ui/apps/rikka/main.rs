mod clock;
mod ticker;

use std::process::ExitCode;
use std::rc::Rc;
use std::sync::LazyLock;

use rikka_app::{App, Window};
use rikka_ui::{Place, State, Styled, View};
use rikka_widgets::{column, live_observed};
use ui_scene::{Grid, LIST_ROWS, Probe, Row, Scenario, Table, WINDOW_HEIGHT, WINDOW_WIDTH};

use clock::Clock;
use ticker::ticker;

pub(crate) static PROBE: LazyLock<Probe> = LazyLock::new(Probe::from_env);

#[derive(State)]
struct Bench {
    #[untracked]
    scenario: Scenario,
    #[untracked]
    rows: Rc<[Row]>,
    #[untracked]
    clock: Clock,
    #[untracked]
    place: Place,
}

impl Bench {
    fn new(scenario: Scenario) -> Bench {
        let rows = match scenario.grid() {
            Some(_) => Rc::from(Vec::new()),
            None => Table::with_rows(LIST_ROWS).rows.into(),
        };
        Bench {
            scenario,
            rows,
            clock: Clock::default(),
            place: Place::new(),
        }
    }
}

fn bench(bench: &Bench) -> impl View {
    match bench.scenario.grid() {
        Some(grid) => {
            let clock = bench.clock.clone();
            let shown = live_observed(move || {
                ui_rikka_views::grid(&Grid {
                    frame: clock.frame(),
                    ..grid.clone()
                })
            });
            let tick = bench
                .scenario
                .animates()
                .then(|| ticker(bench.clock.clone(), None));
            column! {
                tick;
                shown.fill()
            }
            .fill()
            .any()
        }
        None => {
            let tick = bench
                .scenario
                .scrolls()
                .then(|| ticker(bench.clock.clone(), Some(bench.place.clone())));
            column! {
                tick;
                ui_rikka_views::list(Rc::clone(&bench.rows), &bench.place)
            }
            .fill()
            .any()
        }
    }
}

rikka_app::mimalloc!();

fn main() -> ExitCode {
    let scenario = match Scenario::from_args() {
        Ok(scenario) => scenario,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };
    LazyLock::force(&PROBE);
    let window = Window::new(move || Bench::new(scenario), bench)
        .title("ui-bench-rikka")
        .size(WINDOW_WIDTH, WINDOW_HEIGHT)
        .caption(|_| None);
    match App::new("ui-bench-rikka").window(window).run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("failed to run rikka: {error}");
            ExitCode::FAILURE
        }
    }
}

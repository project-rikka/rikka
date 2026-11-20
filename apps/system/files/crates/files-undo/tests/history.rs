use files_core::Location;
use files_undo::{Change, Direction, History, Kind, Step};
use files_vfs::{Outcome, Placement};

fn at(path: &str) -> Location {
    Location::from_path(path)
}

fn moved(names: &[&str]) -> Step {
    Step {
        kind: Kind::Move,
        change: Change::Moved(
            names
                .iter()
                .map(|name| {
                    Placement::new(at(&format!("/src/{name}")), at(&format!("/dst/{name}")))
                })
                .collect(),
        ),
    }
}

fn returned(names: &[&str], cancelled: bool) -> Outcome {
    Outcome {
        cancelled,
        placed: names
            .iter()
            .map(|name| Placement::new(at(&format!("/dst/{name}")), at(&format!("/src/{name}"))))
            .collect(),
        ..Outcome::default()
    }
}

fn top(history: &mut History, direction: Direction) -> Option<Step> {
    let reversal = history.begin(direction)?;
    let task = reversal.task();
    history.abandon(reversal);
    Some(Step {
        kind: Kind::Move,
        change: match task {
            files_vfs::Task::Relocate { placements } => {
                Change::Moved(placements.iter().rev().map(Placement::reversed).collect())
            }
            other => panic!("unexpected reversal {other:?}"),
        },
    })
}

#[test]
fn recording_clears_redo() {
    let mut history = History::default();
    history.record(moved(&["a"]));
    let reversal = history.begin(Direction::Undo).expect("undo");
    history.finish(reversal, &returned(&["a"], false));
    assert_eq!(history.next(Direction::Redo), Some(Kind::Move));

    history.record(moved(&["b"]));

    assert_eq!(history.next(Direction::Redo), None);
    assert_eq!(top(&mut history, Direction::Undo), Some(moved(&["b"])));
}

#[test]
fn reversal_in_flight_blocks_another() {
    let mut history = History::default();
    history.record(moved(&["a"]));
    history.record(moved(&["b"]));

    let reversal = history.begin(Direction::Undo).expect("undo");
    assert_eq!(history.next(Direction::Undo), None);
    assert!(history.begin(Direction::Undo).is_none());

    history.abandon(reversal);
    assert_eq!(top(&mut history, Direction::Undo), Some(moved(&["b"])));
}

#[test]
fn cancelled_reversal_keeps_untouched_items() {
    let mut history = History::default();
    history.record(moved(&["a", "b", "c"]));

    let reversal = history.begin(Direction::Undo).expect("undo");
    history.finish(reversal, &returned(&["c"], true));

    assert_eq!(top(&mut history, Direction::Undo), Some(moved(&["a", "b"])));
    let redo = top(&mut history, Direction::Redo).expect("redo");
    assert_eq!(
        redo.change,
        Change::Moved(vec![Placement::new(at("/dst/c"), at("/src/c"))])
    );
}

#[test]
fn finished_reversal_consumes_skipped_items() {
    let mut history = History::default();
    history.record(moved(&["a", "b"]));

    let reversal = history.begin(Direction::Undo).expect("undo");
    history.finish(reversal, &returned(&["b"], false));

    assert_eq!(history.next(Direction::Undo), None);
    assert_eq!(history.next(Direction::Redo), Some(Kind::Move));
}

#[test]
fn step_recorded_during_undo_stays_on_top() {
    let mut history = History::default();
    history.record(moved(&["a"]));
    history.record(moved(&["b"]));

    let reversal = history.begin(Direction::Undo).expect("undo");
    history.record(moved(&["c"]));
    history.abandon(reversal);

    assert_eq!(top(&mut history, Direction::Undo), Some(moved(&["c"])));
    let reversal = history.begin(Direction::Undo).expect("undo c");
    history.finish(reversal, &returned(&["c"], false));
    assert_eq!(top(&mut history, Direction::Undo), Some(moved(&["b"])));
}

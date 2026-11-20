use rikka_compositor_switcher::{Choosing, Finished, Holding, Step};
use rikka_session::switcher::Rows;
use rikka_types::Modifiers;

const ALT: Modifiers = Modifiers {
    alt: true,
    ..Modifiers::NONE
};

const SHIFT_ALT: Modifiers = Modifiers {
    alt: true,
    shift: true,
    ..Modifiers::NONE
};

fn begun(windows: &[u32], current: Option<u32>, back: bool) -> Choosing<u32> {
    Choosing::begin(windows.to_vec(), current, back, ALT).expect("candidates to choose from")
}

#[test]
fn begin_selects_the_window_after_the_current_one() {
    let cases = [
        (&[1, 2, 3][..], Some(1), false, 1),
        (&[1, 2, 3], Some(1), true, 2),
        (&[1, 2, 3], Some(9), false, 0),
        (&[1, 2, 3], None, false, 0),
        (&[1], Some(1), false, 0),
    ];
    for (windows, current, back, selected) in cases {
        let choosing = begun(windows, current, back);
        assert_eq!(
            choosing.selected(),
            selected,
            "{windows:?} from {current:?}, back {back}"
        );
    }
    assert!(Choosing::<u32>::begin(Vec::new(), None, false, ALT).is_none());
}

#[test]
fn steps_wrap_and_follow_rows() {
    let rows = Rows(vec![vec![10.0, 30.0], vec![20.0]]);
    let cases = [
        (0, Step::Next, Some(1)),
        (2, Step::Next, Some(0)),
        (0, Step::Previous, Some(2)),
        (1, Step::First, Some(0)),
        (0, Step::First, None),
        (0, Step::Last, Some(2)),
        (0, Step::Down, Some(2)),
        (2, Step::Up, Some(0)),
        (0, Step::Up, None),
    ];
    for (from, step, to) in cases {
        let mut choosing = begun(&[1, 2, 3], None, false);
        choosing.arrange(rows.clone());
        let picked = choosing.pick([1, 2, 3][from]);
        assert_eq!(picked, Some(from != 0));
        let moved = choosing.take(step);
        assert_eq!(moved, to.is_some(), "{step:?} from {from}");
        assert_eq!(
            choosing.selected(),
            to.unwrap_or(from),
            "{step:?} from {from}"
        );
    }
}

#[test]
fn losing_a_window_passes_the_selection_on() {
    let mut choosing = begun(&[1, 2, 3, 4], None, false);
    choosing.pick(3);
    assert!(choosing.lose(1));
    assert_eq!(choosing.chosen(), Some(3));
    assert!(choosing.lose(3));
    assert_eq!(choosing.chosen(), Some(4));
    assert!(!choosing.lose(9));
    assert!(choosing.lose(4));
    assert_eq!(choosing.chosen(), Some(2));
    assert!(choosing.lose(2));
    assert!(choosing.is_empty());
    assert_eq!(choosing.chosen(), None);
}

#[test]
fn releasing_the_held_modifier_commits_unless_sticky() {
    let held = Choosing::begin(vec![1, 2], Some(1), false, SHIFT_ALT).expect("candidates");
    assert!(!held.modifiers_released(ALT));
    assert!(held.modifiers_released(Modifiers::NONE));
    assert!(held.modifiers_released(Modifiers {
        shift: true,
        ..Modifiers::NONE
    }));
    let sticky = Choosing::begin(vec![1, 2], Some(1), false, Modifiers::NONE).expect("candidates");
    assert!(sticky.is_sticky());
    assert!(!sticky.modifiers_released(Modifiers::NONE));
}

#[test]
fn finishing_while_held_lingers() {
    let cases = [
        (ALT, true, ALT, Some(2), Some(Holding::of(ALT))),
        (ALT, false, ALT, None, Some(Holding::of(ALT))),
        (ALT, true, Modifiers::NONE, Some(2), None),
        (Modifiers::NONE, true, Modifiers::NONE, Some(2), None),
    ];
    for (pressed, choose, held, chosen, lingering) in cases {
        let choosing = Choosing::begin(vec![1, 2], Some(1), false, pressed).expect("candidates");
        assert_eq!(
            choosing.finish(choose, held),
            Finished { chosen, lingering },
            "begun with {pressed:?}, choose {choose}, held {held:?}"
        );
    }
}

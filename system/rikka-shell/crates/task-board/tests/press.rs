use rikka_session::toplevels::Toplevel;
use task_board::{Act, Board, Order};

use crate::{by_app, toplevel};

#[derive(Clone, Copy)]
enum Is {
    Plain,
    Focused,
    Minimized,
    FocusedTiled,
}

fn window(id: &str, is: Is) -> Toplevel {
    let mut window = toplevel(id, "files", 1);
    match is {
        Is::Plain => {}
        Is::Focused => window.focused = true,
        Is::Minimized => window.minimized = true,
        Is::FocusedTiled => {
            window.focused = true;
            window.tiled = true;
        }
    }
    window
}

fn focus(id: &str) -> Option<Act> {
    Some(Act::Focus(id.to_owned()))
}

#[test]
fn press_follows_taskbar_conventions() {
    let cases: [(&str, Vec<Toplevel>, Option<Act>); 7] = [
        (
            "unfocused window comes forward",
            vec![window("a", Is::Plain)],
            focus("a"),
        ),
        (
            "focused window is minimized",
            vec![window("a", Is::Focused)],
            Some(Act::Minimize("a".to_owned())),
        ),
        (
            "minimized window is restored",
            vec![window("a", Is::Minimized)],
            focus("a"),
        ),
        (
            "focused tiled window stays",
            vec![window("a", Is::FocusedTiled)],
            None,
        ),
        (
            "unfocused group raises its topmost window",
            vec![window("a", Is::Plain), window("b", Is::Plain)],
            focus("b"),
        ),
        (
            "focused group cycles to the next window",
            vec![
                window("a", Is::Plain),
                window("b", Is::Plain),
                window("c", Is::Focused),
            ],
            focus("a"),
        ),
        (
            "minimized windows come last",
            vec![window("a", Is::Plain), window("b", Is::Minimized)],
            focus("a"),
        ),
    ];
    for (what, toplevels, expected) in cases {
        let board = Board::arrange(&toplevels, "DP-1", 1, &mut Order::default(), by_app);
        let group = board.group(1, "files").expect("one group");
        assert_eq!(group.press(), expected, "{what}");
    }
}

#[test]
fn cycling_visits_every_window() {
    let mut order = Order::default();
    let mut toplevels = vec![
        window("a", Is::Focused),
        window("b", Is::Plain),
        window("c", Is::Plain),
    ];
    let mut visited = Vec::new();
    for _ in 0..3 {
        let board = Board::arrange(&toplevels, "DP-1", 1, &mut order, by_app);
        let Some(Act::Focus(next)) = board.group(1, "files").and_then(|group| group.press()) else {
            panic!("a group with a focused window focuses another");
        };
        let at = toplevels
            .iter()
            .position(|window| window.id == next)
            .expect("known window");
        let mut raised = toplevels.remove(at);
        for window in &mut toplevels {
            window.focused = false;
        }
        raised.focused = true;
        toplevels.push(raised);
        visited.push(next);
    }
    assert_eq!(visited, ["b", "c", "a"]);
}

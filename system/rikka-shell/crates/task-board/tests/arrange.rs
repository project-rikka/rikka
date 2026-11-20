use rikka_session::toplevels::Toplevel;
use rikka_session::workspaces::COUNT;
use task_board::{Board, Order};

use crate::{by_app, toplevel};

fn keys(board: &Board, space: usize) -> Vec<&str> {
    board.spaces[space - 1]
        .groups
        .iter()
        .map(|group| group.key.as_str())
        .collect()
}

fn ids(board: &Board, space: usize, key: &str) -> Vec<String> {
    board
        .group(space, key)
        .map(|group| {
            group
                .windows
                .iter()
                .map(|window| window.id.clone())
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn groups_windows_by_workspace_and_app() {
    let mut elsewhere = toplevel("x", "files", 1);
    elsewhere.output = "HDMI-A-1".to_owned();
    let toplevels = [
        toplevel("a", "files", 1),
        toplevel("b", "terminal", 1),
        toplevel("c", "files", 1),
        toplevel("d", "files", 3),
        toplevel("e", "", 3),
        toplevel("f", "", 3),
        elsewhere,
    ];
    let board = Board::arrange(&toplevels, "DP-1", 3, &mut Order::default(), by_app);

    assert_eq!(board.showing, 3);
    assert_eq!(board.spaces.len(), COUNT);
    assert_eq!(keys(&board, 1), ["files", "terminal"]);
    assert_eq!(ids(&board, 1, "files"), ["a", "c"]);
    assert_eq!(keys(&board, 3), ["files", "e", "f"]);
    assert!(keys(&board, 2).is_empty());
}

#[test]
fn order_survives_restacking() {
    let mut order = Order::default();
    let first = [
        toplevel("a", "files", 1),
        toplevel("b", "terminal", 1),
        toplevel("c", "files", 1),
    ];
    let _ = Board::arrange(&first, "DP-1", 1, &mut order, by_app);

    let restacked: Vec<Toplevel> = [&first[2], &first[1], &first[0]]
        .into_iter()
        .cloned()
        .chain([toplevel("d", "editor", 1)])
        .collect();
    let board = Board::arrange(&restacked, "DP-1", 1, &mut order, by_app);

    assert_eq!(keys(&board, 1), ["files", "terminal", "editor"]);
    assert_eq!(ids(&board, 1, "files"), ["a", "c"]);
}

#[test]
fn closed_group_gives_up_its_place() {
    let mut order = Order::default();
    let both = [toplevel("a", "files", 1), toplevel("b", "terminal", 1)];
    let _ = Board::arrange(&both, "DP-1", 1, &mut order, by_app);
    let _ = Board::arrange(&both[1..], "DP-1", 1, &mut order, by_app);

    let reopened = [toplevel("b", "terminal", 1), toplevel("c", "files", 1)];
    let board = Board::arrange(&reopened, "DP-1", 1, &mut order, by_app);

    assert_eq!(keys(&board, 1), ["terminal", "files"]);
}

#[test]
fn only_minimizing_changes_the_look_of_a_window() {
    let mut order = Order::default();
    let before = Board::arrange(&[toplevel("a", "files", 1)], "DP-1", 1, &mut order, by_app);
    let mut changed = toplevel("a", "files", 1);
    changed.title = "elsewhere".to_owned();
    changed.focused = true;
    changed.maximized = true;
    let after = Board::arrange(&[changed.clone()], "DP-1", 1, &mut order, by_app);
    assert!(after.looks_like(&before));

    changed.minimized = true;
    let minimized = Board::arrange(&[changed], "DP-1", 1, &mut order, by_app);
    assert!(!minimized.looks_like(&before));
}

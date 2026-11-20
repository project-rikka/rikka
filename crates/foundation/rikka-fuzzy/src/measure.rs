use std::cell::RefCell;

const GAP_LEADING: f32 = -0.005;
const GAP_TRAILING: f32 = -0.005;
const GAP_INNER: f32 = -0.01;
const MATCH_CONSECUTIVE: f32 = 1.0;

const MATCH_MAX_LEN: usize = 256;

#[derive(Default)]
struct Tables {
    ending: Vec<f32>,
    best: Vec<f32>,
}

thread_local! {
    static TABLES: RefCell<Tables> = RefCell::new(Tables::default());
}

pub(crate) fn is_subsequence(needle: &[char], haystack: &[char]) -> bool {
    let mut letters = haystack.iter();
    needle
        .iter()
        .all(|wanted| letters.any(|letter| letter == wanted))
}

pub(crate) fn measure(
    needle: &[char],
    haystack: &[char],
    bonus: &[f32],
    positions: Option<&mut Vec<usize>>,
) -> f32 {
    let (rows, columns) = (needle.len(), haystack.len());
    if columns > MATCH_MAX_LEN || rows > MATCH_MAX_LEN {
        if let Some(positions) = positions {
            leftmost(needle, haystack, positions);
        }
        return GAP_INNER * columns as f32;
    }
    TABLES.with_borrow_mut(|tables| {
        fill(tables, needle, haystack, bonus);
        if let Some(positions) = positions {
            trace(tables, rows, columns, positions);
        }
        tables.best[rows * columns - 1]
    })
}

fn fill(tables: &mut Tables, needle: &[char], haystack: &[char], bonus: &[f32]) {
    let (rows, columns) = (needle.len(), haystack.len());
    tables.ending.clear();
    tables.ending.resize(rows * columns, f32::NEG_INFINITY);
    tables.best.clear();
    tables.best.resize(rows * columns, f32::NEG_INFINITY);
    for (row, wanted) in needle.iter().enumerate() {
        let gap = match row == rows - 1 {
            true => GAP_TRAILING,
            false => GAP_INNER,
        };
        let (here, above) = (row * columns, row.wrapping_sub(1).wrapping_mul(columns));
        let mut running = f32::NEG_INFINITY;
        for column in 0..columns {
            if *wanted == haystack[column] {
                let score = match (row, column) {
                    (0, _) => column as f32 * GAP_LEADING + bonus[column],
                    (_, 0) => f32::NEG_INFINITY,
                    _ => (tables.best[above + column - 1] + bonus[column])
                        .max(tables.ending[above + column - 1] + MATCH_CONSECUTIVE),
                };
                tables.ending[here + column] = score;
                running = score.max(running + gap);
            } else {
                running += gap;
            }
            tables.best[here + column] = running;
        }
    }
}

fn trace(tables: &Tables, rows: usize, columns: usize, positions: &mut Vec<usize>) {
    let from = positions.len();
    let mut required = false;
    let mut column = columns;
    for row in (0..rows).rev() {
        while column > 0 {
            column -= 1;
            let at = row * columns + column;
            let ending = tables.ending[at];
            if ending == f32::NEG_INFINITY || !(required || ending == tables.best[at]) {
                continue;
            }
            required = row > 0
                && column > 0
                && tables.best[at] == tables.ending[at - columns - 1] + MATCH_CONSECUTIVE;
            positions.push(column);
            break;
        }
    }
    positions[from..].reverse();
}

fn leftmost(needle: &[char], haystack: &[char], positions: &mut Vec<usize>) {
    let mut letters = haystack.iter().enumerate();
    for wanted in needle {
        if let Some((at, _)) = letters.find(|(_, letter)| *letter == wanted) {
            positions.push(at);
        }
    }
}

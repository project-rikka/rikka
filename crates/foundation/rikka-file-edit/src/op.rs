use crate::text::{Text, lines_of, separated_lines};

use crate::error::{Error, Result};

#[derive(Clone, Debug)]
pub enum Op {
    Replace {
        start: usize,
        end: usize,
        text: String,
    },
    InsertBefore {
        line: usize,
        text: String,
    },
    Delete {
        start: usize,
        end: usize,
    },
}

#[derive(Debug)]
pub(crate) struct ResolvedOp<'a> {
    pub(crate) from: usize,
    pub(crate) to: usize,
    pub(crate) lines: Vec<&'a str>,
}

impl Op {
    pub(crate) fn resolved<'a>(&'a self, text: &Text, path: &str) -> Result<ResolvedOp<'a>> {
        let (from, to, lines) = match self {
            Op::Replace {
                start,
                end,
                text: put,
            } => {
                check_range(*start, *end, text, path)?;
                (*start, *end, separated_lines(put))
            }
            Op::Delete { start, end } => {
                check_range(*start, *end, text, path)?;
                (*start, *end, Vec::new())
            }
            Op::InsertBefore { line, text: put } => {
                check_insert_line(*line, text, path)?;
                (*line, *line - 1, lines_of(put))
            }
        };

        Ok(ResolvedOp { from, to, lines })
    }
}

fn check_range(start: usize, end: usize, text: &Text, path: &str) -> Result<()> {
    let count = text.line_count();
    if start == 0 || start > count || end > count {
        return Err(out_of_range(path, start.max(end), count));
    }
    if end < start {
        return Err(Error::ReversedRange {
            path: path.to_owned(),
            start,
            end,
        });
    }

    Ok(())
}

fn check_insert_line(line: usize, text: &Text, path: &str) -> Result<()> {
    let count = text.line_count();
    if line == 0 || line > count + 1 {
        return Err(out_of_range(path, line, count));
    }

    Ok(())
}

fn out_of_range(path: &str, line: usize, count: usize) -> Error {
    Error::OutOfRange {
        path: path.to_owned(),
        line,
        count,
    }
}

pub(crate) fn sort_disjoint(resolved: &[ResolvedOp<'_>]) -> Result<Vec<usize>> {
    let mut order: Vec<usize> = (0..resolved.len()).collect();
    order.sort_by_key(|&index| (resolved[index].from, resolved[index].to, index));

    let (mut reached, mut owner) = (0, 0);
    for &index in &order {
        if resolved[index].from <= reached {
            return Err(Error::Overlap {
                one: owner + 1,
                two: index + 1,
            });
        }
        if resolved[index].to > reached {
            (reached, owner) = (resolved[index].to, index);
        }
    }

    Ok(order)
}

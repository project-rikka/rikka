use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::sync::Arc;

use super::format::{BLOBS, JOURNAL, Line, StoredTurn, encode_line, stored, write_blob};
use super::writer::{Progress, settled_count};
use crate::journal::{Journal, Placement};
use crate::thread::Thread;
use crate::turn::{Brief, Turn};

pub(super) fn rewritten(
    at: &Path,
    thread: &Thread,
    progress: &mut Progress,
    journal: &Arc<Journal>,
) -> std::io::Result<()> {
    let blobs = at.join(BLOBS);
    let turns = thread.all_turns();
    let settled_turns = settled_count(thread);

    let mut body = Vec::new();
    let mut count = 0usize;
    let mut placements = Vec::with_capacity(settled_turns);
    for (index, turn) in turns[..settled_turns].iter().enumerate() {
        let line = line_of(turn, &blobs, journal)?;
        placements.push(Placement {
            index,
            at: body.len() as u64,
            len: line.len() as u32 - 1,
        });
        count += line.len();
        body.extend_from_slice(&line);
    }
    if let Some(running) = turns.get(settled_turns) {
        body.extend_from_slice(&encode_line(&Line::Running(stored(running, &blobs)?))?);
    }

    write_blob(&at.join(JOURNAL), &body)?;

    progress.committed = count;
    progress.journal = body.len();
    for (turn, placement) in turns[..settled_turns].iter().zip(&placements) {
        turn.settled_at(journal, *placement);
    }
    journal.note(placements);
    Ok(())
}

pub(super) fn appended(
    at: &Path,
    lines: &[(Option<&Arc<Turn>>, Vec<u8>)],
    progress: &mut Progress,
    journal: &Arc<Journal>,
) -> std::io::Result<()> {
    let mut body = Vec::new();
    let mut count = 0usize;
    let mut placed_turns = Vec::new();
    for (turn, line) in lines {
        if let Some(turn) = turn {
            count += line.len();
            placed_turns.push((
                *turn,
                Placement {
                    index: progress.turns + placed_turns.len(),
                    at: (progress.journal + body.len()) as u64,
                    len: line.len() as u32 - 1,
                },
            ));
        }
        body.extend_from_slice(line);
    }

    let mut journal_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(at.join(JOURNAL))?;
    journal_file.write_all(&body)?;
    journal_file.sync_data()?;

    progress.committed += count;
    progress.journal += body.len();
    for (turn, placement) in placed_turns {
        turn.settled_at(journal, placement);
    }
    Ok(())
}

pub(super) fn line_of(
    turn: &Turn,
    blobs: &Path,
    journal: &Arc<Journal>,
) -> std::io::Result<Vec<u8>> {
    if let Some((offset, placement)) = turn.written_at()
        && Arc::ptr_eq(&offset, journal)
        && let Some(mut line) = offset.line(placement, turn.speaker(), turn.at())
    {
        line.push(b'\n');
        return Ok(line);
    }
    encode_line(&Line::Turn(stored(turn, blobs)?))
}

pub(super) fn read_journal(at: &Path) -> std::io::Result<(Vec<Arc<Turn>>, Arc<Journal>)> {
    let journal = Journal::open(at);
    let file = match std::fs::File::open(at.join(JOURNAL)) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok((Vec::new(), journal));
        }
        Err(error) => return Err(error),
    };

    let mut turns: Vec<Arc<Turn>> = Vec::new();
    let mut running: Option<(StoredTurn, Placement)> = None;
    let mut placements: Vec<Placement> = Vec::new();
    let mut offset = 0_u64;
    for line in BufReader::new(file).split(b'\n') {
        let line = line?;
        let placement = Placement {
            index: turns.len(),
            at: offset,
            len: line.len() as u32,
        };
        offset += line.len() as u64 + 1;
        if line.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        match serde_json::from_slice::<Line>(&line) {
            Ok(Line::Turn(entry)) => {
                running = None;
                placements.push(placement);
                turns.push(Arc::new(restore_turn(entry, &journal, placement)));
            }
            Ok(Line::Running(entry)) => running = Some((entry, placement)),
            Ok(Line::Shed) => running = None,
            Err(error) => {
                tracing::warn!("{}: skipping truncated journal line: {error}", at.display());
            }
        }
    }
    if let Some((entry, mut placement)) = running {
        placement.index = turns.len();
        placements.push(placement);
        turns.push(Arc::new(restore_turn(entry, &journal, placement)));
    }
    journal.note(placements);
    Ok((turns, journal))
}

fn restore_turn(entry: StoredTurn, journal: &Arc<Journal>, placement: Placement) -> Turn {
    let brief = Brief::new(
        !entry.text.trim().is_empty(),
        &entry.model,
        &entry.rounds,
        entry.instructions.is_some(),
    );
    Turn::read(entry.speaker, entry.at, brief, journal, placement)
}

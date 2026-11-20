use crate::parse::cursor::Cursor;

const POINTS_ARE_WORDS: u8 = 0x80;
const POINT_RUN_COUNT_MASK: u8 = 0x7F;
const DELTAS_ARE_ZERO: u8 = 0x80;
const DELTAS_ARE_WORDS: u8 = 0x40;
const DELTA_RUN_COUNT_MASK: u8 = 0x3F;

#[derive(Clone, Copy, Debug)]
pub(crate) enum PointNumbers<'a> {
    All,
    Explicit { count: usize, data: Cursor<'a> },
}

impl<'a> PointNumbers<'a> {
    pub(crate) fn read(cursor: &mut Cursor<'a>) -> Option<Self> {
        let first = cursor.u8()?;
        let count = if first == 0 {
            return Some(Self::All);
        } else if first & POINTS_ARE_WORDS != 0 {
            usize::from(first & POINT_RUN_COUNT_MASK) << 8 | usize::from(cursor.u8()?)
        } else {
            usize::from(first)
        };
        let data = *cursor;
        let mut skipped = 0;
        while skipped < count {
            let control = cursor.u8()?;
            let run = usize::from(control & POINT_RUN_COUNT_MASK) + 1;
            let size = if control & POINTS_ARE_WORDS != 0 {
                2
            } else {
                1
            };
            cursor.skip(run * size)?;
            skipped += run;
        }
        Some(Self::Explicit { count, data })
    }

    pub(crate) fn count(&self, all: usize) -> usize {
        match self {
            Self::All => all,
            Self::Explicit { count, .. } => *count,
        }
    }

    pub(crate) fn iter(&self) -> PointIter<'a> {
        match *self {
            Self::All => PointIter {
                cursor: None,
                remaining: usize::MAX,
                run_left: 0,
                words: false,
                current: 0,
                all_next: 0,
            },
            Self::Explicit { count, data } => PointIter {
                cursor: Some(data),
                remaining: count,
                run_left: 0,
                words: false,
                current: 0,
                all_next: 0,
            },
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct PointIter<'a> {
    cursor: Option<Cursor<'a>>,
    remaining: usize,
    run_left: usize,
    words: bool,
    current: u32,
    all_next: u32,
}

impl Iterator for PointIter<'_> {
    type Item = u32;

    #[inline]
    fn next(&mut self) -> Option<u32> {
        let Some(cursor) = self.cursor.as_mut() else {
            let point = self.all_next;
            self.all_next = self.all_next.checked_add(1)?;
            return Some(point);
        };
        if self.remaining == 0 {
            return None;
        }
        if self.run_left == 0 {
            let control = cursor.u8()?;
            self.run_left = usize::from(control & POINT_RUN_COUNT_MASK) + 1;
            self.words = control & POINTS_ARE_WORDS != 0;
        }
        let step = if self.words {
            u32::from(cursor.u16()?)
        } else {
            u32::from(cursor.u8()?)
        };
        self.current = self.current.wrapping_add(step);
        self.run_left -= 1;
        self.remaining -= 1;
        Some(self.current)
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct PackedDeltas<'a> {
    cursor: Cursor<'a>,
    run_left: usize,
    kind: u8,
}

impl<'a> PackedDeltas<'a> {
    pub(crate) fn new(cursor: Cursor<'a>) -> Self {
        Self {
            cursor,
            run_left: 0,
            kind: 0,
        }
    }

    pub(crate) fn skip_deltas(&mut self, count: usize) -> Option<()> {
        let mut left = count;
        while left > 0 {
            if self.run_left == 0 {
                self.start_run()?;
            }
            let take = left.min(self.run_left);
            let size = match self.kind {
                DELTAS_ARE_ZERO => 0,
                DELTAS_ARE_WORDS => 2,
                0 => 1,
                _ => 4,
            };
            self.cursor.skip(take * size)?;
            self.run_left -= take;
            left -= take;
        }
        Some(())
    }

    pub(crate) fn each(&mut self, count: usize, mut visit: impl FnMut(usize, i32)) -> usize {
        let mut index = 0;
        while index < count {
            if self.run_left == 0 && self.start_run().is_none() {
                return index;
            }
            let take = (count - index).min(self.run_left);
            let size = match self.kind {
                DELTAS_ARE_ZERO => 0,
                DELTAS_ARE_WORDS => 2,
                0 => 1,
                _ => 4,
            };
            let Some(bytes) = self.cursor.take(take * size) else {
                for at in index..index + take {
                    match self.next() {
                        Some(value) => visit(at, value),
                        None => return at,
                    }
                }
                index += take;
                continue;
            };
            match size {
                0 => (index..index + take).for_each(|at| visit(at, 0)),
                1 => (index..)
                    .zip(bytes)
                    .for_each(|(at, &byte)| visit(at, i32::from(byte as i8))),
                2 => (index..)
                    .zip(bytes.as_chunks::<2>().0)
                    .for_each(|(at, &pair)| visit(at, i32::from(i16::from_be_bytes(pair)))),
                _ => (index..)
                    .zip(bytes.as_chunks::<4>().0)
                    .for_each(|(at, &quad)| visit(at, i32::from_be_bytes(quad))),
            }
            self.run_left -= take;
            index += take;
        }
        index
    }

    fn start_run(&mut self) -> Option<()> {
        let control = self.cursor.u8()?;
        self.run_left = usize::from(control & DELTA_RUN_COUNT_MASK) + 1;
        self.kind = control & (DELTAS_ARE_ZERO | DELTAS_ARE_WORDS);
        Some(())
    }
}

impl Iterator for PackedDeltas<'_> {
    type Item = i32;

    #[inline]
    fn next(&mut self) -> Option<i32> {
        if self.run_left == 0 {
            self.start_run()?;
        }
        self.run_left -= 1;
        match self.kind {
            DELTAS_ARE_ZERO => Some(0),
            DELTAS_ARE_WORDS => self.cursor.i16().map(i32::from),
            0 => self.cursor.i8().map(i32::from),
            _ => self.cursor.i32(),
        }
    }
}

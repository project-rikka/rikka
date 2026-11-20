mod bit_writer;
mod block;
mod matcher;
mod prefix_code;

use crate::tables::MIN_MATCH;
use bit_writer::BitWriter;
use block::Block;
use matcher::{Match, Matcher, Search};

const BLOCK_TOKENS: usize = 16384;
const SHORT_MATCH_REACH: usize = 4096;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Level {
    Fast,
    #[default]
    Balanced,
    Best,
}

#[derive(Clone, Copy, Debug)]
struct Effort {
    chain: usize,
    nice: usize,
    good: usize,
    lazy_below: usize,
}

impl Level {
    const fn effort(self) -> Effort {
        match self {
            Level::Fast => Effort {
                chain: 8,
                nice: 32,
                good: 8,
                lazy_below: 0,
            },
            Level::Balanced => Effort {
                chain: 128,
                nice: 128,
                good: 8,
                lazy_below: 16,
            },
            Level::Best => Effort {
                chain: 4096,
                nice: 258,
                good: 32,
                lazy_below: 258,
            },
        }
    }
}

#[derive(Debug)]
pub struct Deflater {
    level: Level,
    matcher: Matcher,
    block: Block,
}

impl Deflater {
    #[must_use]
    pub fn new(level: Level) -> Deflater {
        Deflater {
            level,
            matcher: Matcher::new(),
            block: Block::new(),
        }
    }

    #[must_use]
    pub const fn level(&self) -> Level {
        self.level
    }

    pub fn deflate(&mut self, data: &[u8], out: &mut Vec<u8>) {
        let effort = self.level.effort();
        let mut bits = BitWriter::new(out);
        self.matcher.reset();
        self.block.clear();
        let mut block_start = 0;
        let mut position = 0;
        let mut held: Option<Match> = None;
        while position < data.len() {
            if self.block.len() >= BLOCK_TOKENS && held.is_none() {
                self.block
                    .write(&mut bits, &data[block_start..position], false);
                block_start = position;
            }
            let search = Search {
                chain: match held {
                    Some(held) if held.length >= effort.good => effort.chain / 4,
                    _ => effort.chain,
                },
                nice: effort.nice,
                longer_than: held.map_or(MIN_MATCH - 1, |held| held.length),
            };
            let found = self
                .matcher
                .longest(data, position, search)
                .filter(|found| found.length > MIN_MATCH || found.distance <= SHORT_MATCH_REACH);
            self.matcher.insert(data, position);
            match (held.take(), found) {
                (Some(_), Some(longer)) => {
                    self.block.literal(data[position - 1]);
                    if longer.length < effort.lazy_below {
                        held = Some(longer);
                        position += 1;
                    } else {
                        position = self.repeat(data, position, longer, position + 1);
                    }
                }
                (Some(earlier), None) => {
                    position = self.repeat(data, position - 1, earlier, position + 1);
                }
                (None, Some(found)) if found.length < effort.lazy_below => {
                    held = Some(found);
                    position += 1;
                }
                (None, Some(found)) => {
                    position = self.repeat(data, position, found, position + 1);
                }
                (None, None) => {
                    self.block.literal(data[position]);
                    position += 1;
                }
            }
        }
        if let Some(held) = held {
            self.block.repeat(held.length, held.distance);
        }
        self.block.write(&mut bits, &data[block_start..], true);
        bits.align_to_byte();
    }

    fn repeat(&mut self, data: &[u8], start: usize, found: Match, uninserted: usize) -> usize {
        self.block.repeat(found.length, found.distance);
        let end = start + found.length;
        for position in uninserted..end {
            self.matcher.insert(data, position);
        }
        end
    }
}

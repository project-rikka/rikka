use crate::error::{Error, Result};

const MAGIC: u32 = 0x0723_0203;
const HEADER_WORDS: usize = 5;
const VERSION_WORD: usize = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Instruction<'a> {
    pub opcode: u32,
    pub operands: &'a [u32],
    pub end: usize,
}

#[derive(Clone, Debug)]
pub struct Instructions<'a> {
    words: &'a [u32],
    at: usize,
}

pub fn instructions(words: &[u32]) -> Result<Instructions<'_>> {
    if words.len() < HEADER_WORDS || words[0] != MAGIC {
        return Err(Error::Header);
    }
    Ok(Instructions {
        words,
        at: HEADER_WORDS,
    })
}

impl Instructions<'_> {
    #[must_use]
    pub fn version(&self) -> u32 {
        self.words[VERSION_WORD]
    }
}

impl<'a> Iterator for Instructions<'a> {
    type Item = Result<Instruction<'a>>;

    fn next(&mut self) -> Option<Self::Item> {
        let at = self.at;
        let head = *self.words.get(at)?;
        let count = (head >> 16) as usize;
        let end = at + count;
        if count == 0 || end > self.words.len() {
            self.at = self.words.len();
            return Some(Err(Error::Truncated { at }));
        }
        self.at = end;
        Some(Ok(Instruction {
            opcode: head & 0xffff,
            operands: &self.words[at + 1..end],
            end,
        }))
    }
}

#[must_use]
pub fn literal_string(operands: &[u32]) -> (String, usize) {
    let mut bytes = Vec::new();
    for (index, word) in operands.iter().enumerate() {
        for byte in word.to_le_bytes() {
            if byte == 0 {
                return (String::from_utf8_lossy(&bytes).into_owned(), index + 1);
            }
            bytes.push(byte);
        }
    }
    (String::from_utf8_lossy(&bytes).into_owned(), operands.len())
}

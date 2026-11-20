use std::ops::Range;

use rikka_text_document::Rope;

const STEPS: usize = 6000;

const CROWDED: usize = 48 * 1024;

const PIECES: [&str; 9] = [
    "a",
    "é",
    "日本語",
    "\n",
    "word ",
    "🙂",
    "line\nbreak\n",
    "\n\n\n",
    "    indented(text) { }",
];

struct Dice(u64);

impl Dice {
    fn roll(&mut self, sides: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 % sides.max(1) as u64) as usize
    }
}

struct Pair {
    rope: Rope,
    oracle: ropey::Rope,
}

impl Pair {
    fn boundary(&self, dice: &mut Dice) -> usize {
        let at = dice.roll(self.oracle.len_bytes() + 1);
        self.oracle.char_to_byte(self.oracle.byte_to_char(at))
    }

    fn span(&self, dice: &mut Dice, longest: usize) -> Range<usize> {
        let start = self.boundary(dice);
        let reach = (start + dice.roll(longest)).min(self.oracle.len_bytes());
        start..self.oracle.char_to_byte(self.oracle.byte_to_char(reach))
    }

    fn insert(&mut self, at: usize, text: &str) {
        self.rope.insert(at, text);
        self.oracle.insert(self.oracle.byte_to_char(at), text);
    }

    fn remove(&mut self, range: Range<usize>) {
        self.rope.remove(range.clone());
        let chars = self.oracle.byte_to_char(range.start)..self.oracle.byte_to_char(range.end);
        self.oracle.remove(chars);
    }

    fn agree_on_extent(&self) {
        assert_eq!(self.rope.len(), self.oracle.len_bytes());
        assert_eq!(self.rope.char_count(), self.oracle.len_chars());
        assert_eq!(self.rope.line_count(), self.oracle.len_lines());
        assert_eq!(self.rope.is_empty(), self.oracle.len_bytes() == 0);
    }

    fn agree_at(&self, at: usize) {
        let line = self.oracle.byte_to_line(at);
        let character = self.oracle.byte_to_char(at);
        let floor = self.oracle.char_to_byte(character);
        assert_eq!(self.rope.line_of(at), line);
        assert_eq!(self.rope.line_start(line), self.oracle.line_to_byte(line));
        assert_eq!(self.rope.byte_to_char(at), character);
        assert_eq!(self.rope.char_to_byte(character), floor);
        assert_eq!(self.rope.floor_char_boundary(at), floor);
        assert_eq!(self.rope.char_at(floor), self.oracle.get_char(character));
        assert_eq!(
            self.rope.char_before(floor),
            character
                .checked_sub(1)
                .and_then(|before| self.oracle.get_char(before))
        );
        let (chunk, start) = self.rope.chunk_at(at);
        assert_eq!(chunk.is_empty(), at >= self.oracle.len_bytes());
        assert!(start <= at && (chunk.is_empty() || at < start + chunk.len()));
    }

    fn agree_within(&self, range: Range<usize>) {
        assert_eq!(
            self.rope.slice(range.clone()),
            self.oracle.byte_slice(range.clone()).to_string()
        );
        let ahead: String = self.rope.chars_from(range.start).take(40).collect();
        let behind: String = self.rope.chars_before(range.end).take(40).collect();
        let at = self.oracle.byte_to_char(range.start);
        let until = self.oracle.byte_to_char(range.end);
        assert_eq!(ahead, self.oracle.chars_at(at).take(40).collect::<String>());
        assert_eq!(
            behind,
            self.oracle
                .chars_at(until)
                .reversed()
                .take(40)
                .collect::<String>()
        );
    }

    fn agree_on_lines(&self, dice: &mut Dice) {
        let line = dice.roll(self.oracle.len_lines());
        let whole = self.oracle.line(line).to_string();
        let shown = whole.strip_suffix('\n').unwrap_or(&whole);
        assert_eq!(self.rope.slice(self.rope.line_span(line)), shown);
    }
}

#[test]
fn random_edits_match_ropey() {
    let mut dice = Dice(0x2545_f491_4f6c_dd1d);
    let long = "long stretch of text, 長い文章。\n".repeat(120);
    let mut pair = Pair {
        rope: Rope::new(),
        oracle: ropey::Rope::new(),
    };
    let mut kept: Vec<(Rope, String)> = Vec::new();
    for step in 0..STEPS {
        let crowded = pair.oracle.len_bytes() > CROWDED;
        match dice.roll(10) {
            0..=5 if !crowded => {
                let at = pair.boundary(&mut dice);
                let text = match dice.roll(60) {
                    0 => long.as_str(),
                    piece => PIECES[piece % PIECES.len()],
                };
                pair.insert(at, text);
            }
            6 => {
                let range = pair.span(&mut dice, 4096);
                pair.remove(range);
            }
            _ => {
                let range = pair.span(&mut dice, 12);
                pair.remove(range);
            }
        }
        pair.agree_on_extent();
        let at = dice.roll(pair.oracle.len_bytes() + 1);
        pair.agree_at(at);
        let range = pair.span(&mut dice, 3000);
        pair.agree_within(range);
        pair.agree_on_lines(&mut dice);
        if step % 500 == 0 {
            assert_eq!(pair.rope.to_string(), pair.oracle.to_string());
            let whole = pair.oracle.to_string();
            assert!(pair.rope == *whole);
            kept.push((pair.rope.clone(), pair.oracle.to_string()));
        }
    }
    for (rope, text) in kept {
        assert_eq!(rope.to_string(), text);
        let replanted = Rope::from(text.as_str());
        assert!(rope == replanted);
    }
}

#[test]
fn typing_at_one_place_keeps_chunks_dense() {
    let mut rope = Rope::from("fn main() {\n}\n");
    for at in 12..20_012 {
        rope.insert(at, "x");
    }
    let mut chunks = 0;
    let mut start = 0;
    while start < rope.len() {
        let (chunk, begun) = rope.chunk_at(start);
        chunks += 1;
        start = begun + chunk.len();
    }
    assert!(chunks <= rope.len() / 256, "{chunks} chunks");
}

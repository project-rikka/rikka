const ESCAPE: u8 = 0x1b;
const BELL: u8 = 0x07;
const CANCEL: u8 = 0x18;
const SUBSTITUTE: u8 = 0x1a;
const TERMINATOR: u8 = 0x9c;
const STRING_END: &[u8] = b"\x1b\\";

const ID: &[u8] = b"52";
const ID_DIGITS: usize = 8;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Osc52 {
    Body { body: Vec<u8>, bell: bool },
    TooLarge,
}

#[derive(Debug)]
pub(crate) struct Scanner {
    state: State,
    pending: Vec<u8>,
    id: Vec<u8>,
    body: Vec<u8>,
    limit: Option<usize>,
    closing: bool,
    dropped: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    Ground,
    Escape,
    Intermediate,
    Csi,
    Dcs,
    Apc,
    Id,
    Body,
    BodyEscape,
    Other,
}

impl Scanner {
    pub(crate) fn new(limit: Option<usize>) -> Self {
        Self {
            state: State::Ground,
            pending: Vec::new(),
            id: Vec::new(),
            body: Vec::new(),
            limit,
            closing: false,
            dropped: false,
        }
    }

    pub(crate) fn feed(
        &mut self,
        mut input: &[u8],
        output: &mut Vec<u8>,
        sequences: &mut Vec<Osc52>,
    ) {
        while let Some((&byte, rest)) = input.split_first() {
            if self.state == State::Ground {
                let plain = input
                    .iter()
                    .position(|&byte| byte == ESCAPE)
                    .unwrap_or(input.len());
                if plain > 0 {
                    output.extend_from_slice(&input[..plain]);
                    input = &input[plain..];
                    continue;
                }
            }
            self.step(byte, output, sequences);
            input = rest;
        }
    }

    fn step(&mut self, byte: u8, output: &mut Vec<u8>, sequences: &mut Vec<Osc52>) {
        match self.state {
            State::Ground => self.ground(byte, output),
            State::Escape => self.escape(byte, output),
            State::Intermediate => self.intermediate(byte, output),
            State::Csi => self.csi(byte, output),
            State::Dcs => self.dcs(byte, output),
            State::Apc => self.apc(byte, output),
            State::Id => self.id(byte, output),
            State::Body => self.body(byte, output, sequences),
            State::BodyEscape => self.body_escape(byte, output, sequences),
            State::Other => self.other(byte, output),
        }
    }

    fn ground(&mut self, byte: u8, output: &mut Vec<u8>) {
        if byte == ESCAPE {
            self.pending.push(byte);
            self.state = State::Escape;
            return;
        }
        output.push(byte);
    }

    fn escape(&mut self, byte: u8, output: &mut Vec<u8>) {
        match byte {
            b']' => {
                self.pending.push(byte);
                self.id.clear();
                self.state = State::Id;
            }
            ESCAPE => {}
            0x00..=0x17 | 0x19 | 0x1c..=0x1f => output.push(byte),
            0x7f..=0xff => {}
            b'[' => self.pass(byte, output, State::Csi),
            b'P' => self.pass(byte, output, State::Dcs),
            b'X' | b'^' | b'_' => self.pass(byte, output, State::Apc),
            0x20..=0x2f => self.pass(byte, output, State::Intermediate),
            _ => self.pass(byte, output, State::Ground),
        }
    }

    fn intermediate(&mut self, byte: u8, output: &mut Vec<u8>) {
        match byte {
            ESCAPE => self.interrupt(),
            0x20..=0x2f => output.push(byte),
            0x30..=0x7e | CANCEL | SUBSTITUTE => {
                output.push(byte);
                self.state = State::Ground;
            }
            _ => output.push(byte),
        }
    }

    fn csi(&mut self, byte: u8, output: &mut Vec<u8>) {
        match byte {
            ESCAPE => self.interrupt(),
            0x40..=0x7e | CANCEL | SUBSTITUTE => {
                output.push(byte);
                self.state = State::Ground;
            }
            _ => output.push(byte),
        }
    }

    fn dcs(&mut self, byte: u8, output: &mut Vec<u8>) {
        match byte {
            ESCAPE => self.interrupt(),
            TERMINATOR | CANCEL | SUBSTITUTE => {
                output.push(byte);
                self.state = State::Ground;
            }
            _ => output.push(byte),
        }
    }

    fn apc(&mut self, byte: u8, output: &mut Vec<u8>) {
        match byte {
            ESCAPE => self.interrupt(),
            CANCEL | SUBSTITUTE => {
                output.push(byte);
                self.state = State::Ground;
            }
            _ => output.push(byte),
        }
    }

    fn id(&mut self, byte: u8, output: &mut Vec<u8>) {
        let is_osc52 = self.id == ID;
        match byte {
            b'0'..=b'9' if self.id.len() < ID_DIGITS => {
                self.id.push(byte);
                self.pending.push(byte);
            }
            b';' if is_osc52 => {
                self.begin_body(output);
                self.body.clear();
                self.dropped = false;
                self.state = State::Body;
            }
            ESCAPE if is_osc52 => {
                self.begin_body(output);
                self.body.clear();
                self.dropped = false;
                self.state = State::BodyEscape;
            }
            BELL if is_osc52 => {
                self.begin_body(output);
                self.state = State::Ground;
            }
            CANCEL | SUBSTITUTE if is_osc52 => {
                self.begin_body(output);
                output.push(byte);
                self.state = State::Ground;
            }
            ESCAPE => {
                self.flush(output);
                self.interrupt();
            }
            BELL | CANCEL | SUBSTITUTE => {
                self.flush(output);
                output.push(byte);
                self.state = State::Ground;
            }
            _ => {
                self.flush(output);
                output.push(byte);
                self.state = State::Other;
            }
        }
    }

    fn body(&mut self, byte: u8, output: &mut Vec<u8>, sequences: &mut Vec<Osc52>) {
        match byte {
            BELL => {
                self.emit(sequences, true);
                self.state = State::Ground;
            }
            ESCAPE => self.state = State::BodyEscape,
            CANCEL | SUBSTITUTE => {
                self.emit(sequences, false);
                output.push(byte);
                self.state = State::Ground;
            }
            0x00..=0x06 | 0x08..=0x17 | 0x19 | 0x1c..=0x1f => {}
            _ => self.gather(byte),
        }
    }

    fn body_escape(&mut self, byte: u8, output: &mut Vec<u8>, sequences: &mut Vec<Osc52>) {
        self.emit(sequences, false);
        if byte == b'\\' {
            self.state = State::Ground;
            return;
        }
        self.pending.clear();
        self.pending.push(ESCAPE);
        self.closing = false;
        self.state = State::Escape;
        self.step(byte, output, sequences);
    }

    fn other(&mut self, byte: u8, output: &mut Vec<u8>) {
        match byte {
            ESCAPE => self.interrupt(),
            BELL | CANCEL | SUBSTITUTE => {
                output.push(byte);
                self.state = State::Ground;
            }
            _ => output.push(byte),
        }
    }

    fn gather(&mut self, byte: u8) {
        if self.dropped {
            return;
        }
        if self.limit.is_some_and(|limit| self.body.len() >= limit) {
            self.dropped = true;
            self.body = Vec::new();
            return;
        }
        self.body.push(byte);
    }

    fn emit(&mut self, sequences: &mut Vec<Osc52>, bell: bool) {
        let body = std::mem::take(&mut self.body);
        sequences.push(match self.dropped {
            true => Osc52::TooLarge,
            false => Osc52::Body { body, bell },
        });
        self.dropped = false;
    }

    fn pass(&mut self, byte: u8, output: &mut Vec<u8>, state: State) {
        self.flush(output);
        output.push(byte);
        self.state = state;
    }

    fn flush(&mut self, output: &mut Vec<u8>) {
        output.extend_from_slice(&self.pending);
        self.pending.clear();
        self.id.clear();
        self.closing = false;
    }

    fn begin_body(&mut self, output: &mut Vec<u8>) {
        if self.closing {
            output.extend_from_slice(STRING_END);
        }
        self.pending.clear();
        self.id.clear();
        self.closing = false;
    }

    fn interrupt(&mut self) {
        self.closing = true;
        self.pending.push(ESCAPE);
        self.state = State::Escape;
    }
}

use std::fmt;

const MAX_LINE_OCTETS: usize = 75;
const LINE_BREAK: &str = "\r\n";
const CONTINUATION: &str = "\r\n ";

pub(crate) struct FoldingWriter<W> {
    out: W,
    line_octets: usize,
}

impl<W: fmt::Write> FoldingWriter<W> {
    pub(crate) fn new(out: W) -> FoldingWriter<W> {
        FoldingWriter {
            out,
            line_octets: 0,
        }
    }

    pub(crate) fn finish(mut self) -> fmt::Result {
        self.out.write_str(LINE_BREAK)
    }
}

impl<W: fmt::Write> fmt::Write for FoldingWriter<W> {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        for character in text.chars() {
            let octets = character.len_utf8();
            if self.line_octets + octets > MAX_LINE_OCTETS {
                self.out.write_str(CONTINUATION)?;
                self.line_octets = 1;
            }
            self.out.write_char(character)?;
            self.line_octets += octets;
        }
        Ok(())
    }
}

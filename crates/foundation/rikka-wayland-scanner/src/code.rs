const INDENT: &str = "    ";

#[derive(Debug, Default)]
pub(crate) struct Code {
    text: String,
    depth: usize,
}

impl Code {
    pub(crate) fn line(&mut self, line: impl AsRef<str>) -> &mut Self {
        let line = line.as_ref();
        if !line.is_empty() {
            for _ in 0..self.depth {
                self.text.push_str(INDENT);
            }
            self.text.push_str(line);
        }
        self.text.push('\n');
        self
    }

    pub(crate) fn blank(&mut self) -> &mut Self {
        self.line("")
    }

    pub(crate) fn open(&mut self, line: impl AsRef<str>) -> &mut Self {
        self.line(line);
        self.depth += 1;
        self
    }

    pub(crate) fn close(&mut self, line: impl AsRef<str>) -> &mut Self {
        self.depth -= 1;
        self.line(line)
    }

    pub(crate) fn reopen(&mut self, line: impl AsRef<str>) -> &mut Self {
        self.depth -= 1;
        self.line(line);
        self.depth += 1;
        self
    }

    pub(crate) fn finish(self) -> String {
        self.text
    }
}

pub(crate) fn quoted(text: &str) -> String {
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    format!("{text:?}")
}

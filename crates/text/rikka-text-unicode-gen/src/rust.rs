const MAX_WIDTH: usize = 100;
const INDENT: &str = "    ";
const SHORT_ELEMENT_WIDTH: usize = 10;

#[derive(Debug, Default)]
pub(crate) struct RustSource {
    text: String,
}

impl RustSource {
    pub(crate) fn line(&mut self, line: impl AsRef<str>) {
        self.text.push_str(line.as_ref());
        self.text.push('\n');
    }

    pub(crate) fn blank(&mut self) {
        if !self.text.is_empty() && !self.text.ends_with("\n\n") {
            self.text.push('\n');
        }
    }

    pub(crate) fn assignment(&mut self, left: &str, right: &str) {
        let single = format!("{left} = {right};");
        if single.len() <= MAX_WIDTH {
            self.line(single);
        } else {
            self.line(format!("{left} ="));
            self.line(format!("{INDENT}{right};"));
        }
    }

    pub(crate) fn constant(&mut self, name: &str, kind: &str, value: impl std::fmt::Display) {
        self.line(format!("pub(crate) const {name}: {kind} = {value};"));
    }

    pub(crate) fn array<I: std::fmt::Display>(
        &mut self,
        declaration: &str,
        element: &str,
        items: impl IntoIterator<Item = I>,
    ) {
        let items: Vec<String> = items.into_iter().map(|item| item.to_string()).collect();
        let head = format!("{declaration}: [{element}; {}] = [", items.len());
        let single = format!("{head}{}];", items.join(", "));
        self.blank();
        if single.len() <= MAX_WIDTH && !single.contains('\n') {
            self.line(single);
        } else if items.iter().all(|item| item.len() <= SHORT_ELEMENT_WIDTH) {
            self.line(&head);
            let mut line = String::from(INDENT);
            for item in &items {
                if line.len() > INDENT.len() && line.len() + item.len() + 1 >= MAX_WIDTH {
                    self.line(line.trim_end());
                    line.truncate(INDENT.len());
                }
                line.push_str(item);
                line.push_str(", ");
            }
            self.line(line.trim_end());
            self.line("];");
        } else {
            self.line(&head);
            for item in &items {
                self.line(format!("{INDENT}{item},"));
            }
            self.line("];");
        }
        self.blank();
    }

    pub(crate) fn finish(self) -> String {
        let mut text = self.text;
        while text.ends_with("\n\n") {
            text.pop();
        }
        text
    }
}

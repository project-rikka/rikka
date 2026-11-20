#[derive(Debug, Default)]
pub(crate) struct Expression {
    tokens: Vec<String>,
    operand: Option<String>,
}

impl Expression {
    pub(crate) fn text(&self) -> String {
        self.tokens
            .iter()
            .chain(self.operand.as_ref())
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join(" ")
    }

    pub(crate) fn clear(&mut self) {
        self.tokens.clear();
        self.operand = None;
    }

    pub(crate) fn discard_operand(&mut self) {
        self.operand = None;
    }

    pub(crate) fn set_operand(&mut self, text: String) {
        self.operand = Some(text);
    }

    pub(crate) fn wrap(&mut self, function: &str, display: String) {
        let inner = self.take_operand(display);
        self.operand = Some(format!("{function}({inner})"));
    }

    pub(crate) fn push_operator(&mut self, display: String, symbol: &str) {
        let operand = self.take_operand(display);
        self.tokens.push(operand);
        self.tokens.push(String::from(symbol));
    }

    pub(crate) fn replace_operator(&mut self, symbol: &str) {
        match self.tokens.last_mut() {
            Some(last) => *last = String::from(symbol),
            None => self.tokens.push(String::from(symbol)),
        }
    }

    pub(crate) fn equals(&mut self, display: String) {
        self.push_operator(display, "=");
    }

    pub(crate) fn set_repeated(&mut self, left: String, symbol: &str, right: String) {
        self.clear();
        self.push_operator(left, symbol);
        self.equals(right);
    }

    fn take_operand(&mut self, display: String) -> String {
        self.operand.take().unwrap_or(display)
    }
}

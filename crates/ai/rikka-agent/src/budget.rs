use rikka_llm::Usage;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Budget {
    turns: u32,
    tokens: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Limit {
    Turns,
    Tokens,
    Output,
}

const DEFAULT_TURNS: u32 = 16;

impl Limit {
    #[must_use]
    pub fn reason(self) -> &'static str {
        match self {
            Limit::Turns => "ran out of turns",
            Limit::Tokens => "ran out of tokens",
            Limit::Output => "kept running into the model's output limit",
        }
    }
}

impl Budget {
    #[must_use]
    pub fn new() -> Budget {
        Budget::default()
    }

    #[must_use]
    pub fn turns(mut self, turns: u32) -> Self {
        self.turns = turns.max(1);
        self
    }

    #[must_use]
    pub fn tokens(mut self, tokens: u64) -> Self {
        self.tokens = Some(tokens);
        self
    }

    #[must_use]
    pub fn reached(self, turns: u32, usage: Usage) -> Option<Limit> {
        if turns >= self.turns {
            return Some(Limit::Turns);
        }
        if self.tokens.is_some_and(|ceiling| usage.total() >= ceiling) {
            return Some(Limit::Tokens);
        }
        None
    }
}

impl Default for Budget {
    fn default() -> Budget {
        Budget {
            turns: DEFAULT_TURNS,
            tokens: None,
        }
    }
}

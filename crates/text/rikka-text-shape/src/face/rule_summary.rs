use rikka_text_font::ChainedSequenceRule;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RuleSummary {
    pub(crate) inputs: u16,
    pub(crate) lookaheads: u16,
    pub(crate) input: [u16; 2],
    pub(crate) lookahead: [u16; 2],
}

impl RuleSummary {
    pub(crate) fn new(rule: &ChainedSequenceRule<'_>) -> Self {
        let input = rule.input();
        let lookahead = rule.lookahead();
        Self {
            inputs: input.len() as u16,
            lookaheads: lookahead.len() as u16,
            input: [input.get(0).unwrap_or(0), input.get(1).unwrap_or(0)],
            lookahead: [lookahead.get(0).unwrap_or(0), lookahead.get(1).unwrap_or(0)],
        }
    }

    #[inline]
    pub(crate) fn first_key(&self) -> u32 {
        if self.inputs > 0 {
            u32::from(self.input[0])
        } else if self.lookaheads > 0 {
            LOOKAHEAD_KEY | u32::from(self.lookahead[0])
        } else {
            WILDCARD_KEY
        }
    }
}

pub(crate) const LOOKAHEAD_KEY: u32 = 1 << 16;
pub(crate) const WILDCARD_KEY: u32 = u32::MAX;

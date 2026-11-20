use rikka_tool::Usage;

#[derive(Debug, Clone, Copy, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub struct Price {
    pub input: f64,
    pub output: f64,
    pub cache_read: f64,
    pub cache_write: f64,
}

impl Price {
    #[must_use]
    pub fn new(input: f64, output: f64) -> Price {
        Price {
            input,
            output,
            cache_read: input * 0.1,
            cache_write: input * 1.25,
        }
    }

    #[must_use]
    pub fn cache_read(mut self, price: f64) -> Self {
        self.cache_read = price;
        self
    }

    #[must_use]
    pub fn cache_write(mut self, price: f64) -> Self {
        self.cache_write = price;
        self
    }

    #[must_use]
    pub fn cost(self, usage: Usage) -> f64 {
        let per_token = |tokens: u64, price: f64| tokens as f64 * price / 1_000_000.0;

        per_token(usage.input, self.input)
            + per_token(usage.output, self.output)
            + per_token(usage.cache_read, self.cache_read)
            + per_token(usage.cache_write, self.cache_write)
    }
}

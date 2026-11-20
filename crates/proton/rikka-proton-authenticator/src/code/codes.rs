use crate::error::Result;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Codes {
    pub current: String,
    pub next: String,
}

impl Codes {
    pub(crate) fn for_counter(
        counter: u64,
        code_for: impl Fn(u64) -> Result<String>,
    ) -> Result<Codes> {
        Ok(Codes {
            current: code_for(counter)?,
            next: code_for(counter + 1)?,
        })
    }
}

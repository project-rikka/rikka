use rikka_prompt::Prompt;

use crate::error::Result;

pub(crate) fn most(prompt: &Prompt, name: &str) -> Result<usize> {
    Ok(usize::try_from(prompt.limit(name)?).unwrap_or(usize::MAX))
}

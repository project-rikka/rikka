use crate::aliases::ValueSet;
use crate::error::{Error, Result};
use crate::ucd::CODEPOINT_COUNT;

#[derive(Debug)]
pub(crate) struct Variant {
    pub(crate) short: String,
    pub(crate) long: String,
}

impl Variant {
    pub(crate) fn rust_name(&self) -> String {
        self.long
            .split('_')
            .map(|word| {
                let mut characters = word.chars();
                let first = characters.next().map(|first| first.to_ascii_uppercase());
                let rest: String = if word.chars().any(|c| c.is_ascii_lowercase()) {
                    characters.collect()
                } else {
                    characters.map(|c| c.to_ascii_lowercase()).collect()
                };
                first.into_iter().chain(rest.chars()).collect::<String>()
            })
            .collect()
    }
}

#[derive(Debug)]
pub(crate) struct Enumeration {
    pub(crate) name: &'static str,
    pub(crate) variants: Vec<Variant>,
}

impl Enumeration {
    pub(crate) fn bits(&self) -> u32 {
        usize::BITS - (self.variants.len() - 1).leading_zeros()
    }

    pub(crate) fn position(&self, short: &str) -> Result<u8> {
        self.variants
            .iter()
            .position(|variant| variant.short == short)
            .map(|position| position as u8)
            .ok_or_else(|| Error::Data(format!("{} has no value `{short}`", self.name)))
    }
}

#[derive(Debug)]
pub(crate) struct Column {
    pub(crate) enumeration: Enumeration,
    pub(crate) values: Vec<u8>,
}

impl Column {
    pub(crate) fn compact(name: &'static str, set: &ValueSet, raw: &[u16]) -> Result<Self> {
        let mut used = vec![false; set.len()];
        for &value in raw {
            used[value as usize] = true;
        }
        let mut remap = vec![u8::MAX; set.len()];
        let mut variants = Vec::new();
        for (value, _) in used.iter().enumerate().filter(|(_, used)| **used) {
            remap[value] = u8::try_from(variants.len())
                .map_err(|_| Error::Data(format!("{name} has more than 256 values")))?;
            variants.push(Variant {
                short: set.short(value).to_owned(),
                long: set.long(value).to_owned(),
            });
        }
        debug_assert_eq!(raw.len(), CODEPOINT_COUNT);
        Ok(Self {
            enumeration: Enumeration { name, variants },
            values: raw.iter().map(|&value| remap[value as usize]).collect(),
        })
    }
}

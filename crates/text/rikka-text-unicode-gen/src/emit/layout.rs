use crate::rust::RustSource;

#[derive(Debug, Default)]
pub(crate) struct Layout {
    shift: u32,
    fields: Vec<(String, u32, u32)>,
}

impl Layout {
    pub(crate) fn field(&mut self, name: &str, bits: u32) -> u32 {
        let shift = self.shift;
        self.fields.push((name.to_owned(), shift, bits));
        self.shift += bits;
        shift
    }

    pub(crate) fn bits(&self) -> u32 {
        self.shift
    }

    pub(crate) fn emit(&self, source: &mut RustSource, word: &str) {
        for (name, shift, bits) in &self.fields {
            if *bits == 1 {
                source.constant(name, word, format!("1 << {shift}"));
            } else {
                source.constant(&format!("{name}_SHIFT"), "u32", shift);
                source.constant(
                    &format!("{name}_MASK"),
                    word,
                    format!("0x{:x}", (1u64 << bits) - 1),
                );
            }
        }
    }
}

pub(crate) fn bits_for(count: usize) -> u32 {
    usize::BITS - count.saturating_sub(1).leading_zeros()
}

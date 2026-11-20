pub(crate) mod cj;
mod dawg;
mod packing;
mod words;

use crate::error::Result;
use crate::source::Directory;

pub(crate) use cj::CjDictionary;
pub(crate) use packing::{ACCEPTING, LABEL_BITS, TARGET_SHIFT};
pub(crate) use words::WINDOW;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Language {
    pub(crate) name: &'static str,
    pub(crate) label: &'static str,
    pub(crate) word_list: &'static str,
    pub(crate) window: u32,
}

pub(crate) const LANGUAGES: [Language; 4] = [
    Language {
        name: "thai",
        label: "thai dictionary",
        word_list: "thaidict.txt",
        window: 0x0E00,
    },
    Language {
        name: "lao",
        label: "lao dictionary",
        word_list: "laodict.txt",
        window: 0x0E80,
    },
    Language {
        name: "khmer",
        label: "khmer dictionary",
        word_list: "khmerdict.txt",
        window: 0x1780,
    },
    Language {
        name: "burmese",
        label: "burmese dictionary",
        word_list: "burmesedict.txt",
        window: 0x1000,
    },
];

#[derive(Debug)]
pub(crate) struct Dictionary {
    pub(crate) language: Language,
    pub(crate) symbols: [u8; WINDOW],
    pub(crate) root: u32,
    pub(crate) slots: Vec<u32>,
}

#[derive(Debug)]
pub(crate) struct Dictionaries {
    pub(crate) complex: Vec<Dictionary>,
    pub(crate) cj: CjDictionary,
}

pub(crate) fn compile(source: &Directory) -> Result<Dictionaries> {
    Ok(Dictionaries {
        complex: compile_complex(source)?,
        cj: cj::compile(&source.read(cj::WORD_LIST)?)?,
    })
}

fn compile_complex(source: &Directory) -> Result<Vec<Dictionary>> {
    LANGUAGES
        .iter()
        .map(|&language| {
            let list = words::parse(
                language.word_list,
                &source.read(language.word_list)?,
                language.window,
            )?;
            let words = list.words.iter().map(|word| (word.as_slice(), true));
            let packed = packing::pack(&dawg::build(words))?;
            Ok(Dictionary {
                language,
                symbols: list.symbols,
                root: packed.root,
                slots: packed.slots,
            })
        })
        .collect()
}

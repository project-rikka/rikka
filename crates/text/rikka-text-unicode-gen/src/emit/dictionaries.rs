use crate::dictionary::{
    ACCEPTING, Dictionaries, Dictionary, LABEL_BITS, TARGET_SHIFT, WINDOW, cj,
};
use crate::rust::RustSource;

pub(crate) fn file_name(dictionary: &Dictionary) -> String {
    format!("{}.dict", dictionary.language.name)
}

pub(crate) fn blob(dictionary: &Dictionary) -> Vec<u8> {
    dictionary
        .slots
        .iter()
        .flat_map(|slot| slot.to_le_bytes())
        .collect()
}

pub(crate) fn cj_file_name() -> String {
    format!("{}.dict", cj::NAME)
}

pub(crate) fn emit(all: &Dictionaries, sizes: &mut Vec<(&'static str, usize)>) -> String {
    let dictionaries = &all.complex;
    let mut source = RustSource::default();
    source.constant("DICTIONARY_WINDOW", "u32", WINDOW);
    source.constant("DICTIONARY_LABEL_MASK", "u32", (1u32 << LABEL_BITS) - 1);
    source.constant("DICTIONARY_ACCEPTING", "u32", ACCEPTING);
    source.constant("DICTIONARY_TARGET_SHIFT", "u32", TARGET_SHIFT);
    source.array(
        "pub(crate) static DICTIONARY_WINDOW_STARTS",
        "u32",
        dictionaries
            .iter()
            .map(|dictionary| format!("0x{:x}", dictionary.language.window)),
    );
    source.array(
        "pub(crate) static DICTIONARY_ROOTS",
        "u32",
        dictionaries.iter().map(|dictionary| dictionary.root),
    );
    source.array(
        "pub(crate) static DICTIONARY_SYMBOLS",
        "u8",
        dictionaries
            .iter()
            .flat_map(|dictionary| dictionary.symbols),
    );
    source.array(
        "pub(crate) static DICTIONARY_SLOTS",
        "&[u8]",
        dictionaries
            .iter()
            .map(|dictionary| format!("include_bytes!(\"{}\")", file_name(dictionary))),
    );
    for dictionary in dictionaries {
        sizes.push((
            dictionary.language.label,
            dictionary.slots.len() * 4 + dictionary.symbols.len(),
        ));
    }
    let cj = &all.cj;
    source.constant("CJ_SYMBOLS", "usize", cj.symbols.len());
    source.constant("CJ_STATES", "usize", cj.states());
    source.constant("CJ_TARGET_BITS", "u32", cj.target_bits);
    source.line(format!(
        "pub(crate) static CJ_DICTIONARY: &[u8] = include_bytes!(\"{}\");",
        cj_file_name()
    ));
    sizes.push((cj::LABEL, cj.blob().len()));
    source.finish()
}

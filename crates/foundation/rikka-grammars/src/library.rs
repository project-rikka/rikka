use std::env::consts::{DLL_PREFIX, DLL_SUFFIX};
use std::path::Path;

use libloading::Library;
use rikka_shipped::Shipped;
use tree_sitter as ts;
use tree_sitter_language::LanguageFn;

const GRAMMARS: Shipped = Shipped {
    variable: "RIKKA_GRAMMAR_PATH",
    installed: "lib/rikka/grammars",
};

type Entry = unsafe extern "C" fn() -> *const ();

pub(crate) struct Loaded {
    pub(crate) language: ts::Language,
    _library: Library,
}

pub(crate) fn load(name: &str, symbol: &str) -> Option<Loaded> {
    let file = format!("{DLL_PREFIX}tree-sitter-{name}{DLL_SUFFIX}");
    let Some(path) = GRAMMARS.find(&file) else {
        tracing::warn!(%file, grammar = name, "grammar library not installed");
        return None;
    };
    match opened(&path, symbol) {
        Ok(loaded) => Some(loaded),
        Err(error) => {
            tracing::warn!(%error, path = %path.display(), "failed to load grammar library");
            None
        }
    }
}

fn opened(path: &Path, symbol: &str) -> Result<Loaded, libloading::Error> {
    let library = unsafe { Library::new(path) }?;
    let entry = *unsafe { library.get::<Entry>(symbol.as_bytes()) }?;
    let language = ts::Language::new(unsafe { LanguageFn::from_raw(entry) });
    Ok(Loaded {
        language,
        _library: library,
    })
}

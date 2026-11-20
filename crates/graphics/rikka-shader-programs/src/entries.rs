const ENTRIES: &str = include_str!("entries.slang");

pub(crate) const BLOCK: &str = "RikkaMaterialBlock";

pub(crate) const PROBE: &str = "rikka_entry_probe";

pub fn source(module: &str) -> String {
    format!("{module}\n\n{ENTRIES}")
}

pub fn builtin_source(module: &str) -> String {
    source(&format!("import rikka;\nimport {module};\n"))
}

use crate::model::Model;
use crate::rust::RustSource;

pub(crate) fn emit(model: &Model, sizes: &mut Vec<(&'static str, usize)>) -> String {
    let scripts = &model.script.enumeration;
    let name = |index: u8| format!("Script::{}", scripts.variants[index as usize].rust_name());
    let mut source = RustSource::default();
    source.line("use super::enums::Script;");
    source.array(
        "pub(crate) static SCRIPT_CODES",
        "&str",
        scripts
            .variants
            .iter()
            .map(|variant| format!("\"{}\"", variant.short)),
    );
    source.array(
        "pub(crate) static ALL_SCRIPTS",
        "Script",
        (0..scripts.variants.len() as u8).map(name),
    );
    let mut starts = vec![0u16];
    let mut members = Vec::new();
    for set in &model.script_extension_sets {
        members.extend(set.iter().copied().map(name));
        starts.push(members.len() as u16);
    }
    source.array("pub(crate) static SCRIPT_EXTENSION_STARTS", "u16", &starts);
    source.array("pub(crate) static SCRIPT_EXTENSIONS", "Script", &members);
    sizes.push((
        "script codes and extension sets",
        scripts.variants.len() * 5 + starts.len() * 2 + members.len(),
    ));
    source.finish()
}

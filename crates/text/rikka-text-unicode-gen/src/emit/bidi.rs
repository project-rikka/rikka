use crate::model::Model;
use crate::rust::RustSource;

pub(crate) fn emit(model: &Model, sizes: &mut Vec<(&'static str, usize)>) -> String {
    let mut source = RustSource::default();
    for (name, label, pairs) in [
        ("MIRROR", "mirroring pairs", &model.mirrors),
        ("BRACKET", "bracket pairs", &model.brackets),
    ] {
        let mut pairs = pairs.clone();
        pairs.sort_unstable();
        source.array(
            &format!("pub(crate) static {name}_SOURCES"),
            "u32",
            pairs.iter().map(|(source, _)| format!("0x{source:x}")),
        );
        source.array(
            &format!("pub(crate) static {name}_TARGETS"),
            "u32",
            pairs.iter().map(|(_, target)| format!("0x{target:x}")),
        );
        sizes.push((label, pairs.len() * 8));
    }
    source.finish()
}

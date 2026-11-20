use crate::enumeration::Enumeration;
use crate::model::Model;
use crate::rust::RustSource;

pub(crate) fn emit(model: &Model) -> String {
    let mut source = RustSource::default();
    for column in [
        &model.general_category,
        &model.script,
        &model.bidi_class,
        &model.line_break,
        &model.grapheme_cluster_break,
        &model.word_break,
        &model.east_asian_width,
        &model.joining_type,
        &model.joining_group,
        &model.indic_syllabic_category,
        &model.indic_positional_category,
        &model.indic_conjunct_break,
    ] {
        enumeration(&mut source, &column.enumeration);
    }
    source.finish()
}

fn enumeration(source: &mut RustSource, enumeration: &Enumeration) {
    let name = enumeration.name;
    let variants: Vec<String> = enumeration
        .variants
        .iter()
        .map(|variant| variant.rust_name())
        .collect();
    source.blank();
    source.line("#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]");
    source.line("#[repr(u8)]");
    source.line(format!("pub enum {name} {{"));
    for variant in &variants {
        source.line(format!("    {variant},"));
    }
    source.line("}");
    source.blank();
    source.line(format!("impl {name} {{"));
    source.line("    #[inline]");
    source.line("    pub const fn from_bits(bits: u8) -> Self {");
    source.line("        match bits {");
    let (last, rest) = variants.split_last().expect("empty enumeration");
    for (bits, variant) in rest.iter().enumerate() {
        source.line(format!("            {bits} => Self::{variant},"));
    }
    source.line(format!("            _ => Self::{last},"));
    source.line("        }");
    source.line("    }");
    source.line("}");
    source.blank();
}

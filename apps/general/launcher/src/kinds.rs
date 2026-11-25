use launcher_instance::ContentKind;
use rikka_app::prelude::*;

pub(crate) const KINDS: [ContentKind; 4] = [
    ContentKind::Mod,
    ContentKind::ResourcePack,
    ContentKind::ShaderPack,
    ContentKind::DataPack,
];

pub(crate) fn kind_label(kind: ContentKind) -> Label {
    match kind {
        ContentKind::Mod => tr!("launcher.kind.mods"),
        ContentKind::ResourcePack => tr!("launcher.kind.resource-packs"),
        ContentKind::ShaderPack => tr!("launcher.kind.shaders"),
        ContentKind::DataPack => tr!("launcher.kind.data-packs"),
    }
}

pub(crate) fn kind_symbol(kind: ContentKind) -> MaterialSymbol {
    match kind {
        ContentKind::Mod => MaterialSymbol::EXTENSION,
        ContentKind::ResourcePack => MaterialSymbol::PALETTE,
        ContentKind::ShaderPack => MaterialSymbol::WB_SUNNY,
        ContentKind::DataPack => MaterialSymbol::DATABASE,
    }
}

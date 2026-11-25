use launcher_instance::ContentKind;
use launcher_modrinth::ProjectType;

#[must_use]
pub fn loaders(kind: ContentKind) -> Vec<String> {
    let loaders: &[&str] = match kind {
        ContentKind::Mod => &["fabric"],
        ContentKind::ResourcePack => &["minecraft"],
        ContentKind::ShaderPack => &["iris"],
        ContentKind::DataPack => &["datapack"],
    };
    loaders.iter().map(|loader| (*loader).to_owned()).collect()
}

#[must_use]
pub fn content_kind(project: ProjectType) -> Option<ContentKind> {
    match project {
        ProjectType::Mod => Some(ContentKind::Mod),
        ProjectType::ResourcePack => Some(ContentKind::ResourcePack),
        ProjectType::Shader => Some(ContentKind::ShaderPack),
        ProjectType::DataPack => Some(ContentKind::DataPack),
        ProjectType::Modpack | ProjectType::Plugin | ProjectType::Other => None,
    }
}

#[must_use]
pub fn project_type(kind: ContentKind) -> ProjectType {
    match kind {
        ContentKind::Mod => ProjectType::Mod,
        ContentKind::ResourcePack => ProjectType::ResourcePack,
        ContentKind::ShaderPack => ProjectType::Shader,
        ContentKind::DataPack => ProjectType::DataPack,
    }
}

use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProjectType {
    Mod,
    #[serde(rename = "resourcepack")]
    ResourcePack,
    #[serde(rename = "shader")]
    Shader,
    #[serde(rename = "datapack")]
    DataPack,
    Modpack,
    Plugin,
    #[serde(other)]
    Other,
}

impl ProjectType {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            ProjectType::Mod => "mod",
            ProjectType::ResourcePack => "resourcepack",
            ProjectType::Shader => "shader",
            ProjectType::DataPack => "datapack",
            ProjectType::Modpack => "modpack",
            ProjectType::Plugin => "plugin",
            ProjectType::Other => "other",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SortIndex {
    #[default]
    Relevance,
    Downloads,
    Follows,
    Newest,
    Updated,
}

impl SortIndex {
    pub const ALL: [SortIndex; 5] = [
        SortIndex::Relevance,
        SortIndex::Downloads,
        SortIndex::Follows,
        SortIndex::Newest,
        SortIndex::Updated,
    ];

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            SortIndex::Relevance => "relevance",
            SortIndex::Downloads => "downloads",
            SortIndex::Follows => "follows",
            SortIndex::Newest => "newest",
            SortIndex::Updated => "updated",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct SearchHit {
    pub project_id: String,
    pub slug: Option<String>,
    pub project_type: ProjectType,
    pub title: String,
    pub description: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub display_categories: Vec<String>,
    #[serde(default)]
    pub downloads: u64,
    #[serde(default)]
    pub follows: u64,
    #[serde(default)]
    pub icon_url: Option<String>,
    #[serde(default)]
    pub date_modified: String,
    #[serde(default)]
    pub featured_gallery: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct SearchResults {
    pub hits: Vec<SearchHit>,
    pub offset: u32,
    pub limit: u32,
    pub total_hits: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct License {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub url: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct GalleryImage {
    pub url: String,
    #[serde(default)]
    pub featured: bool,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Project {
    pub id: String,
    pub slug: String,
    pub project_type: ProjectType,
    pub title: String,
    pub description: String,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub published: String,
    #[serde(default)]
    pub updated: String,
    #[serde(default)]
    pub license: Option<License>,
    #[serde(default)]
    pub downloads: u64,
    #[serde(default)]
    pub followers: u64,
    #[serde(default)]
    pub categories: Vec<String>,
    #[serde(default)]
    pub game_versions: Vec<String>,
    #[serde(default)]
    pub loaders: Vec<String>,
    #[serde(default)]
    pub icon_url: Option<String>,
    #[serde(default)]
    pub issues_url: Option<String>,
    #[serde(default)]
    pub source_url: Option<String>,
    #[serde(default)]
    pub wiki_url: Option<String>,
    #[serde(default)]
    pub discord_url: Option<String>,
    #[serde(default)]
    pub gallery: Vec<GalleryImage>,
    #[serde(default)]
    pub team: String,
}

impl Project {
    #[must_use]
    pub fn page_url(&self) -> String {
        format!(
            "https://modrinth.com/{}/{}",
            self.project_type.name(),
            self.slug
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DependencyType {
    Required,
    Optional,
    Incompatible,
    Embedded,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Dependency {
    #[serde(default)]
    pub version_id: Option<String>,
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub file_name: Option<String>,
    pub dependency_type: DependencyType,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VersionType {
    Release,
    Beta,
    Alpha,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Hashes {
    pub sha1: String,
    pub sha512: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct VersionFile {
    pub hashes: Hashes,
    pub url: String,
    pub filename: String,
    #[serde(default)]
    pub primary: bool,
    pub size: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Version {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub version_number: String,
    #[serde(default)]
    pub changelog: Option<String>,
    #[serde(default)]
    pub date_published: String,
    #[serde(default)]
    pub downloads: u64,
    pub version_type: VersionType,
    #[serde(default)]
    pub featured: bool,
    pub files: Vec<VersionFile>,
    #[serde(default)]
    pub dependencies: Vec<Dependency>,
    #[serde(default)]
    pub game_versions: Vec<String>,
    #[serde(default)]
    pub loaders: Vec<String>,
}

impl Version {
    #[must_use]
    pub fn primary_file(&self) -> Option<&VersionFile> {
        self.files
            .iter()
            .find(|file| file.primary)
            .or_else(|| self.files.first())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct User {
    pub id: String,
    pub username: String,
    #[serde(default)]
    pub avatar_url: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct TeamMember {
    pub user: User,
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub ordering: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Category {
    pub name: String,
    pub project_type: ProjectType,
    #[serde(default)]
    pub header: String,
    #[serde(default)]
    pub icon: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HashAlgorithm {
    Sha1,
    Sha512,
}

impl HashAlgorithm {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            HashAlgorithm::Sha1 => "sha1",
            HashAlgorithm::Sha512 => "sha512",
        }
    }
}

impl fmt::Display for ProjectType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.name())
    }
}

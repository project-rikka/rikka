use crate::model::{ProjectType, SortIndex};

pub const PAGE: u32 = 20;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SearchQuery {
    pub text: String,
    pub project_type: ProjectType,
    pub categories: Vec<String>,
    pub game_version: Option<String>,
    pub loaders: Vec<String>,
    pub sort: SortIndex,
    pub offset: u32,
    pub limit: u32,
}

impl SearchQuery {
    #[must_use]
    pub fn new(project_type: ProjectType) -> SearchQuery {
        SearchQuery {
            text: String::new(),
            project_type,
            categories: Vec::new(),
            game_version: None,
            loaders: Vec::new(),
            sort: SortIndex::Relevance,
            offset: 0,
            limit: PAGE,
        }
    }

    #[must_use]
    pub fn facets(&self) -> Vec<Vec<String>> {
        let mut facets = vec![vec![format!("project_type:{}", self.project_type.name())]];
        if let Some(version) = &self.game_version {
            facets.push(vec![format!("versions:{version}")]);
        }
        if !self.loaders.is_empty() {
            facets.push(
                self.loaders
                    .iter()
                    .map(|loader| format!("categories:{loader}"))
                    .collect(),
            );
        }
        if self.project_type == ProjectType::Mod {
            facets.push(vec![
                "client_side:optional".to_owned(),
                "client_side:required".to_owned(),
            ]);
        }
        facets.extend(
            self.categories
                .iter()
                .map(|category| vec![format!("categories:{category}")]),
        );
        facets
    }
}

use std::collections::HashMap;
use std::time::Duration;

use rikka_net::{Client, StatusCode};
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::error::{Error, Result};
use crate::model::{Category, HashAlgorithm, Project, SearchResults, TeamMember, Version};
use crate::search::SearchQuery;

pub const API: &str = "https://api.modrinth.com/v2";

const TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct VersionFilter {
    pub loaders: Vec<String>,
    pub game_versions: Vec<String>,
}

#[derive(Serialize)]
struct HashLookup<'a> {
    hashes: &'a [String],
    algorithm: &'static str,
}

#[derive(Serialize)]
struct UpdateLookup<'a> {
    hashes: &'a [String],
    algorithm: &'static str,
    loaders: &'a [String],
    game_versions: &'a [String],
}

#[derive(Clone, Debug)]
pub struct Modrinth {
    http: Client,
}

impl Modrinth {
    pub fn new(user_agent: &str) -> Result<Modrinth> {
        let http = Client::builder()
            .user_agent(user_agent)
            .timeout(TIMEOUT)
            .build()?;
        Ok(Modrinth { http })
    }

    async fn get<T: DeserializeOwned>(&self, path: &str, query: &[(&str, String)]) -> Result<T> {
        Ok(self
            .http
            .get(format!("{API}{path}"))
            .query(query)
            .json_response()
            .await?)
    }

    async fn post<B: Serialize, T: DeserializeOwned>(&self, path: &str, body: &B) -> Result<T> {
        Ok(self
            .http
            .post(format!("{API}{path}"))
            .json(body)
            .json_response()
            .await?)
    }

    pub async fn search(&self, query: &SearchQuery) -> Result<SearchResults> {
        let facets = serde_json::to_string(&query.facets())?;
        let mut parameters = vec![
            ("facets", facets),
            ("index", query.sort.name().to_owned()),
            ("offset", query.offset.to_string()),
            ("limit", query.limit.to_string()),
        ];
        if !query.text.trim().is_empty() {
            parameters.push(("query", query.text.trim().to_owned()));
        }
        self.get("/search", &parameters).await
    }

    pub async fn project(&self, id: &str) -> Result<Project> {
        match self.get(&format!("/project/{}", segment(id)), &[]).await {
            Err(Error::Network(error)) if error.status() == Some(StatusCode::NOT_FOUND) => {
                Err(Error::NoProject(id.to_owned()))
            }
            found => found,
        }
    }

    pub async fn projects(&self, ids: &[String]) -> Result<Vec<Project>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        self.get("/projects", &[("ids", serde_json::to_string(ids)?)])
            .await
    }

    pub async fn members(&self, project: &str) -> Result<Vec<TeamMember>> {
        self.get(&format!("/project/{}/members", segment(project)), &[])
            .await
    }

    pub async fn project_versions(
        &self,
        project: &str,
        filter: &VersionFilter,
    ) -> Result<Vec<Version>> {
        let mut parameters = Vec::new();
        if !filter.loaders.is_empty() {
            parameters.push(("loaders", serde_json::to_string(&filter.loaders)?));
        }
        if !filter.game_versions.is_empty() {
            parameters.push((
                "game_versions",
                serde_json::to_string(&filter.game_versions)?,
            ));
        }
        self.get(
            &format!("/project/{}/version", segment(project)),
            &parameters,
        )
        .await
    }

    pub async fn version(&self, id: &str) -> Result<Version> {
        self.get(&format!("/version/{}", segment(id)), &[]).await
    }

    pub async fn versions(&self, ids: &[String]) -> Result<Vec<Version>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        self.get("/versions", &[("ids", serde_json::to_string(ids)?)])
            .await
    }

    pub async fn versions_from_hashes(
        &self,
        hashes: &[String],
        algorithm: HashAlgorithm,
    ) -> Result<HashMap<String, Version>> {
        if hashes.is_empty() {
            return Ok(HashMap::new());
        }
        self.post(
            "/version_files",
            &HashLookup {
                hashes,
                algorithm: algorithm.name(),
            },
        )
        .await
    }

    pub async fn latest_from_hashes(
        &self,
        hashes: &[String],
        algorithm: HashAlgorithm,
        filter: &VersionFilter,
    ) -> Result<HashMap<String, Version>> {
        if hashes.is_empty() {
            return Ok(HashMap::new());
        }
        self.post(
            "/version_files/update",
            &UpdateLookup {
                hashes,
                algorithm: algorithm.name(),
                loaders: &filter.loaders,
                game_versions: &filter.game_versions,
            },
        )
        .await
    }

    pub async fn categories(&self) -> Result<Vec<Category>> {
        self.get("/tag/category", &[]).await
    }
}

fn segment(id: &str) -> String {
    id.chars()
        .filter(|character| character.is_ascii_alphanumeric() || "-_.".contains(*character))
        .collect()
}

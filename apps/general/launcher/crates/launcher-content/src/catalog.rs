use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};

use launcher_instance::{ContentFile, ContentKind, ContentRecord, Instance, Place};
use launcher_modrinth::{
    DependencyType, HashAlgorithm, Modrinth, Project, Version, VersionFile, VersionFilter,
    VersionType,
};
use rikka_net::{Checksum, Client};

use crate::error::{Error, Result};
use crate::kinds::{content_kind, loaders};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Identified {
    pub file: ContentFile,
    pub project: Option<Project>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Planned {
    pub project: Project,
    pub kind: ContentKind,
    pub place: Place,
    pub version: Version,
    pub replaces: Option<ContentFile>,
}

impl Planned {
    pub fn file(&self) -> Result<&VersionFile> {
        self.version
            .primary_file()
            .ok_or_else(|| Error::NoFile(self.version.id.clone()))
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Plan {
    pub items: Vec<Planned>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Migration {
    pub updated: Vec<String>,
    pub incompatible: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct Catalog {
    modrinth: Modrinth,
    downloads: Client,
}

fn preferred(versions: Vec<Version>) -> Option<Version> {
    let release = versions
        .iter()
        .position(|version| version.version_type == VersionType::Release);
    match release {
        Some(index) => versions.into_iter().nth(index),
        None => versions.into_iter().next(),
    }
}

fn filter(kind: ContentKind, game: &str) -> VersionFilter {
    VersionFilter {
        loaders: loaders(kind),
        game_versions: vec![game.to_owned()],
    }
}

impl Catalog {
    pub fn new(user_agent: &str) -> Result<Catalog> {
        let downloads = Client::builder()
            .user_agent(user_agent)
            .build()
            .map_err(|source| Error::Download {
                file: String::new(),
                source,
            })?;
        Ok(Catalog {
            modrinth: Modrinth::new(user_agent)?,
            downloads,
        })
    }

    #[must_use]
    pub fn modrinth(&self) -> &Modrinth {
        &self.modrinth
    }

    pub async fn identify(
        &self,
        instance: &mut Instance,
        files: Vec<ContentFile>,
    ) -> Result<Vec<Identified>> {
        let unknown: Vec<String> = files
            .iter()
            .filter(|file| !file.record.identified)
            .map(|file| file.record.sha512.clone())
            .collect();
        let found = self
            .modrinth
            .versions_from_hashes(&unknown, HashAlgorithm::Sha512)
            .await?;
        let mut files = files;
        for file in files.iter_mut().filter(|file| !file.record.identified) {
            let version = found.get(&file.record.sha512);
            file.record.identified = true;
            file.record.project_id = version.map(|version| version.project_id.clone());
            file.record.version_id = version.map(|version| version.id.clone());
            instance.record(file.key.clone(), file.record.clone());
        }
        let ids: Vec<String> = files
            .iter()
            .filter_map(|file| file.record.project_id.clone())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        let projects: HashMap<String, Project> = self
            .modrinth
            .projects(&ids)
            .await?
            .into_iter()
            .map(|project| (project.id.clone(), project))
            .collect();
        Ok(files
            .into_iter()
            .map(|file| Identified {
                project: file
                    .record
                    .project_id
                    .as_ref()
                    .and_then(|id| projects.get(id).cloned()),
                file,
            })
            .collect())
    }

    async fn choose(
        &self,
        project: &Project,
        kind: ContentKind,
        game: &str,
        version: Option<&str>,
    ) -> Result<Version> {
        if let Some(version) = version {
            return Ok(self.modrinth.version(version).await?);
        }
        let versions = self
            .modrinth
            .project_versions(&project.id, &filter(kind, game))
            .await?;
        preferred(versions).ok_or_else(|| Error::NoCompatibleVersion(project.title.clone()))
    }

    pub async fn plan(
        &self,
        installed: &[ContentFile],
        project: &str,
        place: Place,
        game: &str,
        version: Option<&str>,
    ) -> Result<Plan> {
        let installed_projects: HashMap<&str, &ContentFile> = installed
            .iter()
            .filter_map(|file| Some((file.record.project_id.as_deref()?, file)))
            .collect();
        let mut queue = VecDeque::from([(project.to_owned(), version.map(str::to_owned), true)]);
        let mut seen = HashSet::new();
        let mut plan = Plan::default();
        while let Some((id, version, requested)) = queue.pop_front() {
            if !seen.insert(id.clone()) {
                continue;
            }
            let existing = installed_projects.get(id.as_str()).copied();
            if existing.is_some() && !requested {
                continue;
            }
            let project = self.modrinth.project(&id).await?;
            if project.id != id && !seen.insert(project.id.clone()) {
                continue;
            }
            let existing =
                existing.or_else(|| installed_projects.get(project.id.as_str()).copied());
            let kind = content_kind(project.project_type)
                .ok_or_else(|| Error::Unsupported(project.title.clone()))?;
            let place = match kind {
                ContentKind::DataPack => place.clone(),
                ContentKind::Mod | ContentKind::ResourcePack | ContentKind::ShaderPack => {
                    Place::Instance
                }
            };
            let chosen = self
                .choose(&project, kind, game, version.as_deref())
                .await?;
            if existing.is_some_and(|file| file.record.version_id.as_deref() == Some(&chosen.id)) {
                continue;
            }
            for dependency in &chosen.dependencies {
                let dependency_project = match (&dependency.project_id, &dependency.version_id) {
                    (Some(project), _) => project.clone(),
                    (None, Some(version)) => self.modrinth.version(version).await?.project_id,
                    (None, None) => continue,
                };
                match dependency.dependency_type {
                    DependencyType::Required => {
                        queue.push_back((dependency_project, dependency.version_id.clone(), false));
                    }
                    DependencyType::Incompatible => {
                        if let Some(file) = installed_projects.get(dependency_project.as_str()) {
                            return Err(Error::Incompatible {
                                project: project.title.clone(),
                                installed: file.title().to_owned(),
                            });
                        }
                    }
                    DependencyType::Optional | DependencyType::Embedded => {}
                }
            }
            plan.items.push(Planned {
                kind,
                place,
                version: chosen,
                replaces: existing.cloned(),
                project,
            });
        }
        Ok(plan)
    }

    async fn download(&self, file: &VersionFile, target: &std::path::Path) -> Result<()> {
        let failed = |source| Error::Download {
            file: file.filename.clone(),
            source,
        };
        let checksum = Checksum::sha512(&file.hashes.sha512).map_err(failed)?;
        self.downloads
            .download(&file.url, target)
            .checksum(checksum)
            .resume(false)
            .start()
            .await
            .map_err(failed)?;
        Ok(())
    }

    pub async fn apply(&self, instance: &mut Instance, plan: &Plan) -> Result<()> {
        for item in &plan.items {
            let file = item.file()?;
            let target = instance.target(item.kind, &item.place, &file.filename)?;
            self.download(file, &target).await?;
            if let Some(previous) = &item.replaces
                && previous.path != target
            {
                instance.remove_content(previous)?;
            }
            let key = instance.key(item.kind, &item.place, &file.filename);
            let mut record = instance.measure(&target)?;
            record.identified = true;
            record.project_id = Some(item.project.id.clone());
            record.version_id = Some(item.version.id.clone());
            instance.record(key, record);
            let keep_disabled = item
                .replaces
                .as_ref()
                .is_some_and(|previous| !previous.enabled && !previous.record.incompatible);
            if keep_disabled {
                instance.disable(item.kind, &item.place, &file.filename)?;
            }
        }
        Ok(())
    }

    async fn latest(&self, files: &[ContentFile], game: &str) -> Result<HashMap<String, Version>> {
        let mut by_kind: BTreeMap<ContentKind, Vec<String>> = BTreeMap::new();
        for file in files.iter().filter(|file| file.record.project_id.is_some()) {
            by_kind
                .entry(file.kind)
                .or_default()
                .push(file.record.sha512.clone());
        }
        let mut latest = HashMap::new();
        for (kind, hashes) in by_kind {
            latest.extend(
                self.modrinth
                    .latest_from_hashes(&hashes, HashAlgorithm::Sha512, &filter(kind, game))
                    .await?,
            );
        }
        Ok(latest)
    }

    async fn planned_updates(
        &self,
        files: &[ContentFile],
        latest: &HashMap<String, Version>,
        game: &str,
    ) -> Result<Vec<Planned>> {
        let changed: Vec<(&ContentFile, &Version)> = files
            .iter()
            .filter_map(|file| {
                let version = latest.get(&file.record.sha512)?;
                (file.record.version_id.as_deref() != Some(&version.id)).then_some((file, version))
            })
            .collect();
        let current_ids: Vec<String> = changed
            .iter()
            .filter_map(|(file, _)| file.record.version_id.clone())
            .collect();
        let current: HashMap<String, VersionType> = self
            .modrinth
            .versions(&current_ids)
            .await?
            .into_iter()
            .map(|version| (version.id, version.version_type))
            .collect();
        let mut settled = Vec::with_capacity(changed.len());
        for (file, version) in changed {
            let on_release = file
                .record
                .version_id
                .as_ref()
                .and_then(|id| current.get(id))
                == Some(&VersionType::Release);
            if version.version_type == VersionType::Release || !on_release {
                settled.push((file, version.clone()));
                continue;
            }
            let preferred = preferred(
                self.modrinth
                    .project_versions(&version.project_id, &filter(file.kind, game))
                    .await?,
            );
            if let Some(preferred) = preferred
                && file.record.version_id.as_deref() != Some(&preferred.id)
            {
                settled.push((file, preferred));
            }
        }
        let ids: Vec<String> = settled
            .iter()
            .map(|(_, version)| version.project_id.clone())
            .collect();
        let projects: HashMap<String, Project> = self
            .modrinth
            .projects(&ids)
            .await?
            .into_iter()
            .map(|project| (project.id.clone(), project))
            .collect();
        Ok(settled
            .into_iter()
            .filter_map(|(file, version)| {
                Some(Planned {
                    project: projects.get(&version.project_id)?.clone(),
                    kind: file.kind,
                    place: file.place.clone(),
                    version,
                    replaces: Some(file.clone()),
                })
            })
            .collect())
    }

    pub async fn updates(&self, files: &[ContentFile], game: &str) -> Result<Vec<Planned>> {
        let latest = self.latest(files, game).await?;
        self.planned_updates(files, &latest, game).await
    }

    pub async fn migrate(
        &self,
        instance: &mut Instance,
        files: &[ContentFile],
        game: &str,
    ) -> Result<Migration> {
        let latest = self.latest(files, game).await?;
        let items = self.planned_updates(files, &latest, game).await?;
        let updated: Vec<String> = items
            .iter()
            .filter_map(|planned| planned.replaces.as_ref().map(|file| file.key.clone()))
            .collect();
        self.apply(instance, &Plan { items }).await?;
        let mut incompatible = Vec::new();
        for file in files.iter().filter(|file| {
            file.record.project_id.is_some() && !latest.contains_key(&file.record.sha512)
        }) {
            if file.enabled {
                instance.set_enabled(file, false)?;
            }
            let record = ContentRecord {
                incompatible: true,
                ..file.record.clone()
            };
            instance.record(file.key.clone(), record);
            incompatible.push(file.key.clone());
        }
        Ok(Migration {
            updated,
            incompatible,
        })
    }
}

use std::path::Path;

use anyhow::{Context, Result, bail, ensure};
use rikka_net::Bytes;
use serde::{Deserialize, Serialize};
use sha1::Sha1;
use sha2::{Digest, Sha256};

use crate::{host, web};

const LOCK: &str = "minecraft/minecraft.lock.json";

const VERSION_MANIFEST: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
const FABRIC_LOADERS: &str = "https://meta.fabricmc.net/v2/versions/loader";
const FABRIC_MAVEN: &str = "https://maven.fabricmc.net/";
const MAVEN_CENTRAL: &str = "https://repo1.maven.org/maven2/";

const FABRIC_LOADER: &str = "net.fabricmc:fabric-loader";
const VINEFLOWER: &str = "org.vineflower:vineflower";
const CLIENT: &str = "com.mojang:minecraft";
const CLIENT_CLASSIFIER: &str = "client";

const OS_NAME: &str = "linux";
const OS_ARCH: &str = "x86_64";

const CLASSPATH_ARGUMENTS: [&str; 2] = ["-cp", "${classpath}"];
const PLACEHOLDERS: [&str; 3] = [
    "${natives_directory}",
    "${launcher_name}",
    "${launcher_version}",
];

pub(crate) struct Request {
    pub(crate) minecraft: Option<String>,
    pub(crate) loader: Option<String>,
}

pub(crate) fn pin(workspace: &Path, request: Request) -> Result<()> {
    let minecraft = minecraft(request.minecraft.as_deref())?;
    let fabric_loader = fabric_loader(request.loader.as_deref())?;
    let vineflower = vineflower()?;
    println!(
        "pinned Minecraft {}, Fabric Loader {} and Vineflower {}",
        minecraft.version, fabric_loader.version, vineflower.version
    );
    let lock = Lock {
        minecraft,
        fabric_loader,
        vineflower,
    };
    let path = workspace.join(LOCK);
    host::write(&path, format!("{}\n", serde_json::to_string_pretty(&lock)?))?;
    println!("wrote {}", path.display());
    Ok(())
}

#[derive(Serialize)]
struct Lock {
    minecraft: Minecraft,
    fabric_loader: FabricLoader,
    vineflower: Vineflower,
}

#[derive(Serialize)]
struct Minecraft {
    version: String,
    java_release: u32,
    jvm_arguments: Vec<String>,
    client: Artifact,
    libraries: Vec<Artifact>,
    asset_index: AssetIndex,
}

#[derive(Serialize)]
struct FabricLoader {
    version: String,
    main_class: String,
    jar: Artifact,
    libraries: Vec<Artifact>,
    development_libraries: Vec<Artifact>,
}

#[derive(Serialize)]
struct Vineflower {
    version: String,
    jar: Artifact,
}

#[derive(Serialize)]
struct Artifact {
    name: String,
    path: String,
    url: String,
    sha256: String,
}

#[derive(Serialize)]
struct AssetIndex {
    id: String,
    url: String,
    sha256: String,
}

enum Checksum<'a> {
    Sha1(&'a str),
    Sha256(&'a str),
}

impl Checksum<'_> {
    fn verify(&self, url: &str, bytes: &[u8]) -> Result<()> {
        let (expected, actual) = match self {
            Checksum::Sha1(expected) => (expected, rikka_types::hex::encode(Sha1::digest(bytes))),
            Checksum::Sha256(expected) => {
                (expected, rikka_types::hex::encode(Sha256::digest(bytes)))
            }
        };
        ensure!(
            expected.eq_ignore_ascii_case(&actual),
            "checksum mismatch for {url}: expected {expected}, got {actual}"
        );
        Ok(())
    }
}

struct Coordinate<'a> {
    group: &'a str,
    artifact: &'a str,
    version: &'a str,
    classifier: Option<&'a str>,
}

impl<'a> Coordinate<'a> {
    fn parse(name: &'a str) -> Result<Coordinate<'a>> {
        let mut parts = name.split(':');
        match (
            parts.next(),
            parts.next(),
            parts.next(),
            parts.next(),
            parts.next(),
        ) {
            (Some(group), Some(artifact), Some(version), classifier, None) => Ok(Coordinate {
                group,
                artifact,
                version,
                classifier,
            }),
            _ => bail!("invalid Maven coordinate {name}"),
        }
    }

    fn path(&self) -> String {
        let file = match self.classifier {
            Some(classifier) => format!("{}-{}-{classifier}.jar", self.artifact, self.version),
            None => format!("{}-{}.jar", self.artifact, self.version),
        };
        format!(
            "{}/{}/{}/{file}",
            self.group.replace('.', "/"),
            self.artifact,
            self.version
        )
    }
}

#[derive(Deserialize)]
struct VersionManifest {
    latest: Latest,
    versions: Vec<VersionEntry>,
}

#[derive(Deserialize)]
struct Latest {
    release: String,
}

#[derive(Deserialize)]
struct VersionEntry {
    id: String,
    url: String,
    sha1: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Version {
    id: String,
    arguments: Arguments,
    asset_index: VersionAssetIndex,
    downloads: Downloads,
    java_version: JavaVersion,
    libraries: Vec<Library>,
}

#[derive(Deserialize)]
struct Arguments {
    jvm: Vec<Argument>,
    #[serde(rename = "default-user-jvm", default)]
    default_user_jvm: Vec<Argument>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Argument {
    Plain(String),
    Conditional {
        #[serde(default)]
        rules: Vec<Rule>,
        value: Values,
    },
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Values {
    One(String),
    Many(Vec<String>),
}

#[derive(Deserialize)]
struct Rule {
    action: Action,
    os: Option<Os>,
    features: Option<serde_json::Map<String, serde_json::Value>>,
}

#[derive(Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
enum Action {
    Allow,
    Disallow,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Os {
    name: Option<String>,
    arch: Option<String>,
    version_range: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct VersionAssetIndex {
    id: String,
    url: String,
    sha1: String,
}

#[derive(Deserialize)]
struct Downloads {
    client: Download,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct JavaVersion {
    major_version: u32,
}

#[derive(Deserialize)]
struct Library {
    name: String,
    downloads: LibraryDownloads,
    #[serde(default)]
    rules: Vec<Rule>,
}

#[derive(Deserialize)]
struct LibraryDownloads {
    artifact: Option<Download>,
}

#[derive(Deserialize)]
struct Download {
    path: Option<String>,
    url: String,
    sha1: String,
}

#[derive(Deserialize)]
struct LoaderVersion {
    version: String,
    stable: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LoaderInstaller {
    libraries: LoaderLibraries,
    main_class: LoaderMainClass,
}

#[derive(Deserialize)]
struct LoaderLibraries {
    common: Vec<LoaderLibrary>,
    client: Vec<LoaderLibrary>,
    development: Vec<LoaderLibrary>,
}

#[derive(Deserialize)]
struct LoaderLibrary {
    name: String,
    url: String,
    sha256: String,
}

#[derive(Deserialize)]
struct LoaderMainClass {
    client: String,
}

fn minecraft(requested: Option<&str>) -> Result<Minecraft> {
    let manifest: VersionManifest = web::json(VERSION_MANIFEST)?;
    let id = requested.unwrap_or(&manifest.latest.release);
    let entry = manifest
        .versions
        .iter()
        .find(|entry| entry.id == id)
        .with_context(|| format!("Minecraft {id} is not in Mojang's version manifest"))?;
    let document = fetched(&entry.url, Checksum::Sha1(&entry.sha1))?;
    let version: Version = serde_json::from_slice(&document)
        .with_context(|| format!("unexpected version document at {}", entry.url))?;

    let client = &version.downloads.client;
    let client_name = format!("{CLIENT}:{}:{CLIENT_CLASSIFIER}", version.id);
    let client = artifact(
        &client_name,
        Coordinate::parse(&client_name)?.path(),
        &client.url,
        Checksum::Sha1(&client.sha1),
    )?;

    let mut libraries = Vec::new();
    for library in &version.libraries {
        if !allowed(&library.rules)? {
            continue;
        }
        let download = library
            .downloads
            .artifact
            .as_ref()
            .with_context(|| format!("library {} has no artifact", library.name))?;
        let path = match &download.path {
            Some(path) => path.clone(),
            None => Coordinate::parse(&library.name)?.path(),
        };
        libraries.push(artifact(
            &library.name,
            path,
            &download.url,
            Checksum::Sha1(&download.sha1),
        )?);
    }

    let index = &version.asset_index;
    let index_bytes = fetched(&index.url, Checksum::Sha1(&index.sha1))?;

    Ok(Minecraft {
        java_release: version.java_version.major_version,
        jvm_arguments: jvm_arguments(&version.arguments)?,
        client,
        libraries,
        asset_index: AssetIndex {
            id: index.id.clone(),
            url: index.url.clone(),
            sha256: rikka_types::hex::encode(Sha256::digest(&index_bytes)),
        },
        version: version.id,
    })
}

fn jvm_arguments(arguments: &Arguments) -> Result<Vec<String>> {
    let mut flattened = Vec::new();
    for argument in arguments.jvm.iter().chain(&arguments.default_user_jvm) {
        match argument {
            Argument::Plain(value) => flattened.push(value.clone()),
            Argument::Conditional { rules, value } => {
                if allowed(rules)? {
                    match value {
                        Values::One(value) => flattened.push(value.clone()),
                        Values::Many(values) => flattened.extend(values.iter().cloned()),
                    }
                }
            }
        }
    }
    let classpath = flattened
        .windows(CLASSPATH_ARGUMENTS.len())
        .position(|window| window == CLASSPATH_ARGUMENTS)
        .context("the JVM arguments name no classpath")?;
    flattened.drain(classpath..classpath + CLASSPATH_ARGUMENTS.len());
    for argument in &flattened {
        let mut rest = argument.as_str();
        while let Some(start) = rest.find("${") {
            let placeholder = &rest[start..];
            let end = placeholder
                .find('}')
                .with_context(|| format!("unterminated placeholder in {argument}"))?;
            ensure!(
                PLACEHOLDERS.contains(&&placeholder[..=end]),
                "unsupported placeholder {} in JVM argument {argument}",
                &placeholder[..=end]
            );
            rest = &placeholder[end + 1..];
        }
    }
    Ok(flattened)
}

fn allowed(rules: &[Rule]) -> Result<bool> {
    if rules.is_empty() {
        return Ok(true);
    }
    let mut allowed = false;
    for rule in rules {
        if matches(rule)? {
            allowed = rule.action == Action::Allow;
        }
    }
    Ok(allowed)
}

fn matches(rule: &Rule) -> Result<bool> {
    if rule
        .features
        .as_ref()
        .is_some_and(|features| !features.is_empty())
    {
        return Ok(false);
    }
    let Some(os) = &rule.os else {
        return Ok(true);
    };
    if os.name.as_deref().is_some_and(|name| name != OS_NAME) {
        return Ok(false);
    }
    if os.arch.as_deref().is_some_and(|arch| arch != OS_ARCH) {
        return Ok(false);
    }
    ensure!(
        os.version_range.is_none(),
        "unsupported OS version range in a rule for {OS_NAME}"
    );
    Ok(true)
}

fn fabric_loader(requested: Option<&str>) -> Result<FabricLoader> {
    let versions: Vec<LoaderVersion> = web::json(FABRIC_LOADERS)?;
    let version = match requested {
        Some(requested) => versions
            .iter()
            .find(|version| version.version == requested)
            .with_context(|| format!("Fabric Loader {requested} is not published"))?,
        None => versions
            .iter()
            .find(|version| version.stable)
            .context("Fabric publishes no stable loader")?,
    };
    let name = format!("{FABRIC_LOADER}:{}", version.version);
    let jar_path = Coordinate::parse(&name)?.path();
    let installer_path = jar_path
        .strip_suffix(".jar")
        .with_context(|| format!("{jar_path} is not a jar"))?;
    let installer_url = format!("{FABRIC_MAVEN}{installer_path}.json");
    let installer: LoaderInstaller = web::json(&installer_url)?;
    let jar = maven_artifact(FABRIC_MAVEN, &name)?;
    let libraries = installer
        .libraries
        .common
        .iter()
        .chain(&installer.libraries.client)
        .map(loader_library)
        .collect::<Result<_>>()?;
    let development_libraries = installer
        .libraries
        .development
        .iter()
        .map(loader_library)
        .collect::<Result<_>>()?;
    Ok(FabricLoader {
        version: version.version.clone(),
        main_class: installer.main_class.client,
        jar,
        libraries,
        development_libraries,
    })
}

fn loader_library(library: &LoaderLibrary) -> Result<Artifact> {
    let path = Coordinate::parse(&library.name)?.path();
    let url = format!("{}{path}", library.url);
    artifact(&library.name, path, &url, Checksum::Sha256(&library.sha256))
}

fn vineflower() -> Result<Vineflower> {
    let metadata_url = format!(
        "{MAVEN_CENTRAL}{}/maven-metadata.xml",
        VINEFLOWER.replace(['.', ':'], "/")
    );
    let metadata = web::text(&metadata_url)?;
    let document = rikka_xml::parse(&metadata)
        .with_context(|| format!("unexpected metadata at {metadata_url}"))?;
    let version = document
        .descendants()
        .find(|element| element.name() == "release")
        .map(|element| element.text().into_owned())
        .filter(|release| !release.is_empty())
        .with_context(|| format!("no release in {metadata_url}"))?;
    let jar = maven_artifact(MAVEN_CENTRAL, &format!("{VINEFLOWER}:{version}"))?;
    Ok(Vineflower { version, jar })
}

fn maven_artifact(repository: &str, name: &str) -> Result<Artifact> {
    let path = Coordinate::parse(name)?.path();
    let url = format!("{repository}{path}");
    let published = web::text(&format!("{url}.sha256"))?;
    let sha256 = published
        .split_whitespace()
        .next()
        .with_context(|| format!("empty checksum for {url}"))?;
    artifact(name, path, &url, Checksum::Sha256(sha256))
}

fn artifact(name: &str, path: String, url: &str, checksum: Checksum) -> Result<Artifact> {
    let bytes = fetched(url, checksum)?;
    println!("pinned {name}");
    Ok(Artifact {
        name: name.to_owned(),
        path,
        url: url.to_owned(),
        sha256: rikka_types::hex::encode(Sha256::digest(&bytes)),
    })
}

fn fetched(url: &str, checksum: Checksum) -> Result<Bytes> {
    let bytes = web::bytes(url)?;
    checksum.verify(url, &bytes)?;
    Ok(bytes)
}

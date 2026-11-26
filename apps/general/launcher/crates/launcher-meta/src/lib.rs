mod arguments;
mod assets;
mod checked;
mod coordinate;
mod error;
mod fabric;
mod java;
mod library;
mod platform;
mod rules;
mod target;
mod version;

pub use arguments::{Argument, Arguments, Substitutions, Values, expand};
pub use assets::{AssetIndex, AssetObject, RESOURCES};
pub use checked::{RelativePath, Sha1};
pub use coordinate::Coordinate;
pub use error::{Error, Result};
pub use fabric::{FABRIC_META, LoaderRelease, loaders_from_json, loaders_url, profile_url};
pub use java::{
    RUNTIME_INDEX, RuntimeDownloads, RuntimeFile, RuntimeIndex, RuntimeManifest, RuntimeName,
    RuntimeRelease,
};
pub use library::{Artifact, Download, Library, MOJANG_LIBRARIES};
pub use platform::{Arch, Os, Platform};
pub use rules::{Action, Environment, Feature, Features, OsRule, Rule, allowed};
pub use target::GameTarget;
pub use version::{
    AssetIndexReference, ClientVersion, JavaVersion, Latest, ManifestEntry, VERSION_MANIFEST,
    Version, VersionManifest,
};

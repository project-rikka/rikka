use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use crate::dist::{Release, SYSTEM};
use crate::gecko::{ENGINE, PACKAGE};
use crate::host;
use crate::release::github::gh;

const REPOSITORY: &str = "project-rikka/rikka";

const WORKFLOW: &str = "gecko.yml";

const BRANCH: &str = "main";

const SEARCHED_RUNS: &str = "100";

const PROTOCOL: &str = "crates/web/rikka-gecko-protocol";

const GECKO_LINK: &str = "gecko";

#[derive(Deserialize)]
struct Run {
    #[serde(rename = "databaseId")]
    id: u64,
    #[serde(rename = "headSha")]
    head: String,
}

pub(super) fn download(workspace: &Path, into: &Path) -> Result<PathBuf> {
    let engine =
        tree_of(workspace, "HEAD", ENGINE)?.with_context(|| format!("HEAD has no {ENGINE}"))?;
    let listed = host::output(gh()?.args([
        "run",
        "list",
        "--repo",
        REPOSITORY,
        "--workflow",
        WORKFLOW,
        "--branch",
        BRANCH,
        "--status",
        "success",
        "--limit",
        SEARCHED_RUNS,
        "--json",
        "databaseId,headSha",
    ]))?;
    let runs: Vec<Run> =
        serde_json::from_str(&listed).context("gh run list printed unexpected JSON")?;
    let mut built = None;
    for run in runs {
        if tree_of(workspace, &run.head, ENGINE)?.as_deref() == Some(engine.as_str()) {
            built = Some(run);
            break;
        }
    }
    let Some(run) = built else {
        bail!(
            "no successful {WORKFLOW} run on {BRANCH} built {ENGINE} as HEAD holds it; push it \
             and wait for the build, or start one with `gh workflow run {WORKFLOW} --repo \
             {REPOSITORY}`"
        );
    };
    println!("downloading the engine {WORKFLOW} built at {}", run.head);
    host::run(
        gh()?
            .args(["run", "download", &run.id.to_string()])
            .args(["--repo", REPOSITORY, "--name", PACKAGE, "--dir"])
            .arg(into),
    )
    .with_context(|| {
        format!(
            "the artifact of run {} may have expired; start a new build with `gh workflow run \
             {WORKFLOW} --repo {REPOSITORY}`",
            run.id
        )
    })?;
    let archive = single_archive(into)?;
    host::run(
        gh()?
            .args(["attestation", "verify"])
            .arg(&archive)
            .args(["--repo", REPOSITORY])
            .args([
                "--signer-workflow",
                &format!("{REPOSITORY}/.github/workflows/{WORKFLOW}"),
            ])
            .args(["--source-ref", &format!("refs/heads/{BRANCH}")]),
    )?;
    Ok(archive)
}

pub(super) fn version_of(file: &str) -> Result<&str> {
    file.strip_prefix(&format!("{PACKAGE}-"))
        .and_then(|rest| rest.split_once(&format!("-{SYSTEM}-")))
        .map(|(version, _)| version)
        .with_context(|| format!("{file} is not a packed {PACKAGE}"))
}

pub(super) fn stale_components(workspace: &Path, published: &Release) -> Result<Vec<String>> {
    let protocol =
        tree_of(workspace, "HEAD", PROTOCOL)?.with_context(|| format!("HEAD has no {PROTOCOL}"))?;
    let mut stale = Vec::new();
    for (id, entry) in &published.components {
        let links_gecko = entry.links.values().any(|link| link == GECKO_LINK);
        if links_gecko
            && tree_of(workspace, &entry.revision, PROTOCOL)?.as_deref() != Some(protocol.as_str())
        {
            stale.push(id.clone());
        }
    }
    Ok(stale)
}

fn single_archive(folder: &Path) -> Result<PathBuf> {
    let mut archives = Vec::new();
    for entry in
        std::fs::read_dir(folder).with_context(|| format!("failed to list {}", folder.display()))?
    {
        let path = entry
            .with_context(|| format!("failed to list {}", folder.display()))?
            .path();
        let packed = path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| version_of(name).is_ok());
        if packed {
            archives.push(path);
        }
    }
    match archives.as_slice() {
        [archive] => Ok(archive.clone()),
        _ => bail!(
            "expected one packed {PACKAGE} in {}, found {}",
            folder.display(),
            archives.len()
        ),
    }
}

fn tree_of(workspace: &Path, revision: &str, path: &str) -> Result<Option<String>> {
    let mut command = host::clean_command(&host::require("git")?);
    let output = command
        .current_dir(workspace)
        .args([
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("{revision}:{path}"),
        ])
        .output()
        .context("failed to run git rev-parse")?;
    Ok(output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned()))
}

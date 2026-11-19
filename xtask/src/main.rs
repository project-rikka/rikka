use std::path::PathBuf;

use anyhow::Result;
use clap::{ArgGroup, Args, Parser, Subcommand};

use crate::bazel::Profile;
use crate::release::{Bump, Selection, Version};

mod bazel;
mod components;
mod dist;
mod ffmpeg_layout;
mod gecko;
mod host;
mod ltc;
mod minecraft;
mod public_suffix;
mod release;
mod run;
mod sdk;
mod shipped;
mod tree;
mod unicode;
mod web;

#[derive(Parser)]
#[command(
    bin_name = "bazel run //xtask --",
    about = "Rikka workspace tasks",
    arg_required_else_help = true
)]
struct Xtask {
    #[command(subcommand)]
    task: Task,
}

#[derive(Subcommand)]
enum Task {
    #[command(
        subcommand,
        about = "build portable components for a local distribution"
    )]
    Dist(DistTask),
    #[command(
        subcommand,
        about = "release new versions to project-rikka/packages (needs write access there)"
    )]
    Release(ReleaseTask),
    #[command(about = "assemble, build and package the Gecko engine and follow Firefox releases")]
    Gecko(GeckoArgs),
    #[command(
        about = "build the session into a tree of its own and start its compositor in a window \
                 of this session"
    )]
    Compositor {
        #[command(flatten)]
        build: DevBuild,
        #[arg(last = true, help = "arguments for the compositor")]
        arguments: Vec<String>,
    },
    #[command(about = "build one application and run it as a separate dev instance")]
    App {
        application: String,
        #[command(flatten)]
        build: DevBuild,
        #[arg(last = true, help = "arguments for the application")]
        arguments: Vec<String>,
    },
    #[command(about = "regenerate the emoji list and its localized names")]
    EmojiData,
    #[command(about = "regenerate the CLDR Windows time-zone table")]
    WindowsZones,
    #[command(about = "regenerate rikka-video's FFmpeg ABI layout assertions")]
    FfmpegLayout,
    #[command(about = "refit rikka-ltc's GGX area-light tables")]
    Ltc,
    #[command(about = "regenerate the SDK's committed language bindings from its IDL")]
    Sdk,
    #[command(about = "refresh the committed public suffix list and its test vectors")]
    PublicSuffix,
    #[command(
        about = "pin Minecraft, Fabric Loader and Vineflower for //minecraft, newest stable by \
                 default"
    )]
    Minecraft {
        #[arg(
            long,
            help = "pin this Minecraft version instead of the latest release"
        )]
        version: Option<String>,
        #[arg(
            long,
            help = "pin this Fabric Loader version instead of the latest stable one"
        )]
        loader: Option<String>,
    },
}

#[derive(Subcommand)]
enum DistTask {
    #[command(about = "build components portably and pack them with their release entries")]
    Build {
        #[arg(required = true)]
        components: Vec<String>,
        #[arg(long, help = "the components' version")]
        version: String,
        #[arg(long, default_value_t = 1, help = "the build of this version")]
        build: u32,
        #[arg(long, help = "folder that receives the archives and the entries")]
        out: PathBuf,
    },
    #[command(about = "merge packed components into a clone of the distribution flake")]
    Publish {
        #[arg(long, help = "folder holding the packed components")]
        entries: PathBuf,
        #[arg(long, help = "clone of the distribution flake to update")]
        into: PathBuf,
    },
    #[command(about = "build the portable Windows ZIP of the general applications")]
    Windows {
        #[arg(
            long,
            help = "the version written into the programs, MAJOR.MINOR.PATCH"
        )]
        version: String,
        #[arg(long, help = "folder that receives the archive")]
        out: PathBuf,
    },
}

#[derive(Subcommand)]
enum ReleaseTask {
    #[command(about = "build, verify and publish new versions of components")]
    Components {
        #[arg(required_unless_present = "all", conflicts_with = "all")]
        components: Vec<String>,
        #[arg(long, help = "every component")]
        all: bool,
        #[command(flatten)]
        bump: BumpArgs,
    },
    #[command(about = "publish every component again as its next build, after nixpkgs moved")]
    Rebuild,
    #[command(about = "build and publish the static installer")]
    Installer {
        #[command(flatten)]
        bump: BumpArgs,
    },
    #[command(about = "publish the Gecko engine that CI built for HEAD and pin it in nix/dist")]
    Gecko,
}

#[derive(Args)]
struct GeckoArgs {
    #[arg(
        long,
        global = true,
        value_name = "DIR",
        help = "folder holding the Firefox tree and its builds (default .bazel/gecko)"
    )]
    work: Option<PathBuf>,
    #[command(subcommand)]
    task: GeckoTask,
}

#[derive(Subcommand)]
enum GeckoTask {
    #[command(about = "check out the pinned Firefox, apply the patches and add the Rikka project")]
    Assemble,
    #[command(about = "assemble and build the engine")]
    Build {
        #[command(flatten)]
        build: GeckoBuild,
    },
    #[command(about = "build the engine and pack it as a content-addressed archive")]
    Package {
        #[command(flatten)]
        build: GeckoBuild,
        #[arg(long, help = "folder that receives the archive")]
        out: PathBuf,
    },
    #[command(about = "turn edits in the Firefox tree into engine/gecko/patches")]
    ExportPatches,
    #[command(about = "carry the patches onto the newest Firefox release and pin it")]
    Follow,
}

#[derive(Args)]
struct GeckoBuild {
    #[arg(long, help = "build with cross-language LTO, as published engines are")]
    release: bool,
}

impl GeckoBuild {
    fn profile(&self) -> gecko::Profile {
        match self.release {
            true => gecko::Profile::Release,
            false => gecko::Profile::Dev,
        }
    }
}

#[derive(Args)]
#[command(group(ArgGroup::new("bump").required(true).multiple(false)))]
struct BumpArgs {
    #[arg(long, group = "bump", help = "raise the patch version")]
    patch: bool,
    #[arg(long, group = "bump", help = "raise the minor version")]
    minor: bool,
    #[arg(long, group = "bump", help = "raise the major version")]
    major: bool,
    #[arg(long, group = "bump", help = "publish exactly this MAJOR.MINOR.PATCH")]
    version: Option<String>,
}

impl BumpArgs {
    fn bump(&self) -> Result<Bump> {
        Ok(match &self.version {
            Some(version) => Bump::Exact(Version::parse(version)?),
            None if self.major => Bump::Major,
            None if self.minor => Bump::Minor,
            None => Bump::Patch,
        })
    }
}

#[derive(Args)]
struct DevBuild {
    #[arg(
        long,
        help = "build with `--config=release` instead of the development configuration"
    )]
    release: bool,
}

impl DevBuild {
    fn profile(&self) -> Profile {
        match self.release {
            true => Profile::Release,
            false => Profile::Dev,
        }
    }
}

fn main() -> Result<()> {
    let task = Xtask::parse().task;
    let workspace = bazel::workspace()?;
    match task {
        Task::Dist(DistTask::Build {
            components,
            version,
            build,
            out,
        }) => {
            let orders = components
                .iter()
                .map(|id| {
                    Ok(dist::Order {
                        component: dist::find(id)?,
                        version: version.clone(),
                        build,
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            dist::build(&workspace, &orders, &bazel::invoked_at(&out)?).map(drop)
        }
        Task::Dist(DistTask::Publish { entries, into }) => dist::publish(
            &workspace,
            &bazel::invoked_at(&entries)?,
            &bazel::invoked_at(&into)?,
        ),
        Task::Dist(DistTask::Windows { version, out }) => dist::windows(
            &workspace,
            Version::parse(&version)?,
            &bazel::invoked_at(&out)?,
        ),
        Task::Release(ReleaseTask::Components {
            components,
            all,
            bump,
        }) => {
            let selection = match all {
                true => Selection::All,
                false => Selection::Named(components),
            };
            release::components(&workspace, &selection, &bump.bump()?)
        }
        Task::Release(ReleaseTask::Rebuild) => release::rebuild(&workspace),
        Task::Release(ReleaseTask::Installer { bump }) => {
            release::installer(&workspace, &bump.bump()?)
        }
        Task::Release(ReleaseTask::Gecko) => release::gecko(&workspace),
        Task::Gecko(GeckoArgs { work, task }) => {
            let work = work.map(|work| bazel::invoked_at(&work)).transpose()?;
            let engine = gecko::Engine::new(&workspace, work);
            match task {
                GeckoTask::Assemble => engine.assemble(),
                GeckoTask::Build { build } => engine.build(build.profile()),
                GeckoTask::Package { build, out } => engine
                    .package(build.profile(), &bazel::invoked_at(&out)?)
                    .map(drop),
                GeckoTask::ExportPatches => engine.export_patches(),
                GeckoTask::Follow => engine.follow(),
            }
        }
        Task::Compositor { build, arguments } => {
            run::compositor(&workspace, build.profile(), &arguments)
        }
        Task::App {
            application,
            build,
            arguments,
        } => run::app(&workspace, &application, build.profile(), &arguments),
        Task::EmojiData => unicode::emoji(&workspace),
        Task::WindowsZones => unicode::windows_zones(&workspace),
        Task::FfmpegLayout => ffmpeg_layout::run(&workspace),
        Task::Ltc => ltc::run(&workspace),
        Task::Sdk => sdk::regenerate(&workspace),
        Task::PublicSuffix => public_suffix::refresh(&workspace),
        Task::Minecraft { version, loader } => minecraft::pin(
            &workspace,
            minecraft::Request {
                minecraft: version,
                loader,
            },
        ),
    }
}

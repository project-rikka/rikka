use std::path::{Path, PathBuf};

use rikka_open::is_runnable;

use crate::exec::{Exec, FieldCodes, Target};
use crate::keyfile::{Group, KeyFile};
use crate::terminal::Terminals;

const GROUP: &str = "Desktop Entry";

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TerminalArgs {
    pub exec: Option<String>,
    pub dir: Option<String>,
    pub title: Option<String>,
    pub app_id: Option<String>,
    pub hold: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct DesktopEntry {
    pub id: String,
    pub path: PathBuf,
    pub name: String,
    pub generic_name: Option<String>,
    pub comment: Option<String>,
    pub keywords: Vec<String>,
    pub categories: Vec<String>,
    pub mime_types: Vec<String>,
    pub exec: String,
    pub icon: Option<String>,
    pub startup_wm_class: Option<String>,
    pub working_dir: Option<PathBuf>,
    pub terminal: bool,
    pub no_display: bool,
    pub only_show_in: Vec<String>,
    pub not_show_in: Vec<String>,
    pub try_exec: Option<String>,
    pub terminal_args: TerminalArgs,
}

impl DesktopEntry {
    #[must_use]
    pub fn parse(id: &str, path: &Path, text: &str, locales: &[String]) -> Option<Self> {
        Self::from_group(id, path, KeyFile::parse(text).group(GROUP)?, locales)
    }

    fn from_group(id: &str, path: &Path, fields: &Group, locales: &[String]) -> Option<Self> {
        if fields.raw("Type") != Some("Application") || fields.boolean("Hidden") {
            return None;
        }
        let exec = fields.string("Exec").filter(|exec| !exec.is_empty())?;
        let name = fields
            .localized("Name", locales)
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| id.trim_end_matches(".desktop").to_owned());

        Some(Self {
            id: id.to_owned(),
            path: path.to_path_buf(),
            name,
            generic_name: fields.localized("GenericName", locales),
            comment: fields.localized("Comment", locales),
            keywords: fields.list("Keywords", locales),
            categories: fields.list("Categories", &[]),
            mime_types: fields.list("MimeType", &[]),
            exec,
            icon: fields.string("Icon").filter(|icon| !icon.is_empty()),
            startup_wm_class: fields.string("StartupWMClass"),
            working_dir: fields
                .string("Path")
                .filter(|path| !path.is_empty())
                .map(PathBuf::from),
            terminal: fields.boolean("Terminal"),
            no_display: fields.boolean("NoDisplay"),
            only_show_in: fields.list("OnlyShowIn", &[]),
            not_show_in: fields.list("NotShowIn", &[]),
            try_exec: fields
                .string("TryExec")
                .filter(|program| !program.is_empty()),
            terminal_args: TerminalArgs::from_group(fields),
        })
    }

    #[must_use]
    pub fn shows_in(&self, desktops: &[String]) -> bool {
        let names_one = |listed: &[String]| {
            listed
                .iter()
                .any(|desktop| desktops.iter().any(|here| here == desktop))
        };
        (self.only_show_in.is_empty() || names_one(&self.only_show_in))
            && !names_one(&self.not_show_in)
    }

    #[must_use]
    pub fn is_installed(&self) -> bool {
        self.try_exec.as_deref().is_none_or(is_runnable)
    }

    #[must_use]
    pub fn handles(&self, mime: &str) -> bool {
        self.mime_types.iter().any(|listed| listed == mime)
    }

    #[must_use]
    pub fn description(&self) -> Option<&str> {
        self.comment
            .as_deref()
            .or(self.generic_name.as_deref())
            .filter(|text| !text.is_empty())
    }

    #[must_use]
    pub fn program(&self) -> Option<String> {
        Exec::program(&self.exec)
    }

    #[must_use]
    pub fn command(&self) -> Option<Vec<String>> {
        self.command_lines(&[]).into_iter().next()
    }

    #[must_use]
    pub fn command_lines(&self, targets: &[Target]) -> Vec<Vec<String>> {
        let batches: Vec<&[Target]> = match Exec::takes_many(&self.exec) || targets.is_empty() {
            true => vec![targets],
            false => targets.iter().map(std::slice::from_ref).collect(),
        };
        batches
            .into_iter()
            .filter_map(|batch| self.one_command(batch))
            .collect()
    }

    fn one_command(&self, targets: &[Target]) -> Option<Vec<String>> {
        let paths: Vec<&str> = targets.iter().map(|target| target.path.as_str()).collect();
        let uris: Vec<&str> = targets.iter().map(|target| target.uri.as_str()).collect();
        let argv = Exec::expand(
            &self.exec,
            &FieldCodes {
                paths: &paths,
                uris: &uris,
                name: &self.name,
                icon: self.icon.as_deref(),
                location: self.path.to_str(),
            },
        );
        if argv.is_empty() {
            return None;
        }
        if !self.terminal {
            return Some(argv);
        }
        let terminal = Terminals::session()?.preferred()?;
        let app_id = self.id.strip_suffix(".desktop").unwrap_or(&self.id);
        terminal
            .launch()
            .app_id(app_id)
            .run(argv)
            .resolve()
            .map(|resolved| resolved.argv)
    }
}

impl TerminalArgs {
    fn from_group(fields: &Group) -> TerminalArgs {
        let key = |name: &str| {
            fields
                .string(&format!("X-TerminalArg{name}"))
                .or_else(|| fields.string(&format!("TerminalArg{name}")))
        };
        TerminalArgs {
            exec: key("Exec")
                .or_else(|| fields.string("X-ExecArg"))
                .or_else(|| fields.string("ExecArg")),
            dir: key("Dir").filter(|flag| !flag.is_empty()),
            title: key("Title").filter(|flag| !flag.is_empty()),
            app_id: key("AppId").filter(|flag| !flag.is_empty()),
            hold: key("Hold").filter(|flag| !flag.is_empty()),
        }
    }
}

#[must_use]
pub(crate) fn desktops() -> Vec<String> {
    std::env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .split(':')
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .collect()
}

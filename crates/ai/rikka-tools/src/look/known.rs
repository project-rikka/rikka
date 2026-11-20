use super::{Inspection, Look};

struct Known {
    name: &'static str,
    inspection: Inspection,
    subject: Subject,
    valued: &'static [&'static str],
    forbidden: &'static [&'static str],
    running: &'static [&'static str],
    commands: &'static [&'static str],
}

impl Known {
    #[must_use]
    const fn new(name: &'static str, inspection: Inspection, subject: Subject) -> Known {
        Known {
            name,
            inspection,
            subject,
            valued: &[],
            forbidden: &[],
            running: &[],
            commands: &[],
        }
    }

    #[must_use]
    const fn valued(mut self, valued: &'static [&'static str]) -> Known {
        self.valued = valued;
        self
    }

    #[must_use]
    const fn forbidding(mut self, forbidden: &'static [&'static str]) -> Known {
        self.forbidden = forbidden;
        self
    }

    #[must_use]
    const fn running(mut self, running: &'static [&'static str]) -> Known {
        self.running = running;
        self
    }

    #[must_use]
    const fn commanding(mut self, commands: &'static [&'static str]) -> Known {
        self.commands = commands;
        self
    }

    fn forbids(&self, flag: &str) -> bool {
        among(self.forbidden, flag)
    }

    fn hands_on(&self, flag: &str) -> bool {
        among(self.running, flag)
    }

    fn takes_value(&self, flag: &str) -> bool {
        self.valued.contains(&flag) || self.names(flag)
    }

    fn names(&self, flag: &str) -> bool {
        matches!(self.subject, Subject::Named(flags) if flags.contains(&flag))
    }
}

enum Subject {
    Operands,
    First,
    Named(&'static [&'static str]),
    Script,
}

impl Subject {
    fn describe(&self, operands: &[&str], named: Option<&str>) -> Option<String> {
        Some(match self {
            Subject::Operands => operands.join(", "),
            Subject::First => operands.first().copied().unwrap_or_default().to_owned(),
            Subject::Named(_) => named
                .or_else(|| operands.first().copied())
                .unwrap_or_default()
                .to_owned(),
            Subject::Script => {
                let (script, files) = operands.split_first()?;
                is_range(script).then(|| files.join(", "))?
            }
        })
    }
}

const GREP_VALUED: &[&str] = &[
    "-f",
    "--file",
    "-m",
    "--max-count",
    "-A",
    "--after-context",
    "-B",
    "--before-context",
    "-C",
    "--context",
    "-d",
    "--directories",
    "-D",
    "--devices",
    "--include",
    "--exclude",
    "--exclude-dir",
    "--exclude-from",
    "--color",
    "--colour",
    "--label",
    "--binary-files",
];

const GREP_PATTERN: &[&str] = &["-e", "--regexp"];

const FIND_NAMES: &[&str] = &[
    "-name",
    "-iname",
    "-path",
    "-ipath",
    "-wholename",
    "-regex",
    "-iregex",
];

const KNOWN: &[Known] = &[
    Known::new("cat", Inspection::Read, Subject::Operands),
    Known::new("head", Inspection::Read, Subject::Operands)
        .valued(&["-n", "--lines", "-c", "--bytes"]),
    Known::new("tail", Inspection::Read, Subject::Operands)
        .valued(&["-n", "--lines", "-c", "--bytes", "--pid"]),
    Known::new("nl", Inspection::Read, Subject::Operands).valued(&[
        "-b",
        "--body-numbering",
        "-d",
        "--section-delimiter",
        "-f",
        "--footer-numbering",
        "-h",
        "--header-numbering",
        "-i",
        "--line-increment",
        "-l",
        "--join-blank-lines",
        "-n",
        "--number-format",
        "-s",
        "--number-separator",
        "-v",
        "--starting-line-number",
        "-w",
        "--number-width",
    ]),
    Known::new("bat", Inspection::Read, Subject::Operands)
        .valued(&[
            "-l",
            "--language",
            "-r",
            "--line-range",
            "-H",
            "--highlight-line",
            "-m",
            "--map-syntax",
            "--style",
            "--theme",
            "--paging",
            "--color",
            "--wrap",
            "--terminal-width",
            "--tabs",
        ])
        .forbidding(&["--pager", "--generate-config-file"])
        .commanding(&["cache"]),
    Known::new("sed", Inspection::Read, Subject::Script).forbidding(&[
        "-i",
        "--in-place",
        "-e",
        "--expression",
        "-f",
        "--file",
    ]),
    Known::new("grep", Inspection::Search, Subject::Named(GREP_PATTERN)).valued(GREP_VALUED),
    Known::new("egrep", Inspection::Search, Subject::Named(GREP_PATTERN)).valued(GREP_VALUED),
    Known::new("fgrep", Inspection::Search, Subject::Named(GREP_PATTERN)).valued(GREP_VALUED),
    Known::new("rg", Inspection::Search, Subject::Named(GREP_PATTERN))
        .valued(&[
            "-f",
            "--file",
            "-g",
            "--glob",
            "--iglob",
            "-t",
            "--type",
            "-T",
            "--type-not",
            "-m",
            "--max-count",
            "-A",
            "--after-context",
            "-B",
            "--before-context",
            "-C",
            "--context",
            "-r",
            "--replace",
            "-M",
            "--max-columns",
            "-j",
            "--threads",
            "-E",
            "--encoding",
            "--color",
            "--colors",
            "--sort",
            "--sortr",
            "--engine",
            "--max-depth",
            "--max-filesize",
        ])
        .forbidding(&["--pre"]),
    Known::new("ag", Inspection::Search, Subject::First)
        .valued(&[
            "-A",
            "--after",
            "-B",
            "--before",
            "-C",
            "--context",
            "-G",
            "--file-search-regex",
            "-m",
            "--max-count",
            "--ignore",
            "--workers",
            "--color-match",
        ])
        .forbidding(&["--pager"]),
    Known::new("ack", Inspection::Search, Subject::Named(&["--match"]))
        .valued(&[
            "--type",
            "-A",
            "--after-context",
            "-B",
            "--before-context",
            "-C",
            "--context",
            "-m",
            "--max-count",
            "--ignore-dir",
            "--ignore-file",
        ])
        .forbidding(&["--pager"]),
    Known::new("find", Inspection::Search, Subject::Named(FIND_NAMES))
        .valued(&[
            "-regextype",
            "-lname",
            "-ilname",
            "-type",
            "-xtype",
            "-maxdepth",
            "-mindepth",
            "-size",
            "-mtime",
            "-mmin",
            "-ctime",
            "-cmin",
            "-atime",
            "-amin",
            "-newer",
            "-anewer",
            "-cnewer",
            "-samefile",
            "-user",
            "-group",
            "-uid",
            "-gid",
            "-inum",
            "-links",
            "-perm",
            "-printf",
        ])
        .forbidding(&["-delete", "-fprint", "-fprint0", "-fprintf", "-fls"])
        .running(&["-exec", "-execdir", "-ok", "-okdir"]),
    Known::new("fd", Inspection::Search, Subject::First)
        .valued(&[
            "-e",
            "--extension",
            "-t",
            "--type",
            "-d",
            "--max-depth",
            "--min-depth",
            "--exact-depth",
            "-E",
            "--exclude",
            "--ignore-file",
            "-c",
            "--color",
            "-j",
            "--threads",
            "-S",
            "--size",
            "--changed-within",
            "--changed-before",
            "--path-separator",
            "--base-directory",
            "--format",
        ])
        .running(&["-x", "--exec", "-X", "--exec-batch"]),
    Known::new("ls", Inspection::List, Subject::Operands).valued(&[
        "-I",
        "--ignore",
        "--hide",
        "-w",
        "--width",
        "-T",
        "--tabsize",
        "--block-size",
        "--color",
        "--format",
        "--sort",
        "--time",
        "--time-style",
        "--indicator-style",
        "--quoting-style",
    ]),
    Known::new("tree", Inspection::List, Subject::Operands)
        .valued(&[
            "-L",
            "-P",
            "-I",
            "-o",
            "--filelimit",
            "--charset",
            "--timefmt",
            "--sort",
        ])
        .forbidding(&["-o", "-R"]),
];

pub(super) fn read(name: &str, arguments: &[String]) -> Option<Look> {
    reading(name, arguments, true)
}

fn reading(name: &str, args: &[String], handing: bool) -> Option<Look> {
    let known = KNOWN.iter().find(|known| known.name == name)?;

    let mut operands: Vec<&str> = Vec::new();
    let mut named: Option<&str> = None;
    let mut flagged = true;
    let mut rest = args.iter();

    while let Some(word) = rest.next() {
        if !flagged || !word.starts_with('-') || word == "-" {
            operands.push(word);
            continue;
        }
        if word == "--" {
            flagged = false;
            continue;
        }

        let (flag, flag_value) = match word.split_once('=') {
            Some((flag, flag_value)) => (flag, Some(flag_value)),
            None => (word.as_str(), None),
        };
        if known.forbids(flag) {
            return None;
        }
        if known.hands_on(flag) {
            if !handing || flag_value.is_some() || !known.running.contains(&flag) {
                return None;
            }
            let handed = rest.as_slice();
            let ended = handed
                .iter()
                .position(|word| word == ";" || word == "+")
                .unwrap_or(handed.len());
            let (name, arguments) = handed[..ended].split_first()?;
            reading(name, arguments, false)?;
            let _ = rest.nth(ended);
            continue;
        }

        let value = match flag_value {
            Some(flag_value) => Some(flag_value),
            None if known.takes_value(flag) => Some(rest.next()?.as_str()),
            None => None,
        };
        if known.names(flag) {
            named = named.or(value);
        }
    }

    if operands
        .first()
        .is_some_and(|first| known.commands.contains(first))
    {
        return None;
    }

    Some(Look {
        inspection: known.inspection,
        subject: known.subject.describe(&operands, named)?,
    })
}

fn among(flags: &[&str], flag: &str) -> bool {
    flags
        .iter()
        .any(|known| flag == *known || clustered(flag, known))
}

fn clustered(flag: &str, known: &str) -> bool {
    if known.len() != 2 || known.starts_with("--") || flag.starts_with("--") {
        return false;
    }
    flag.contains(&known[1..])
}

fn is_range(script: &str) -> bool {
    let Some(range) = script.strip_suffix('p') else {
        return false;
    };
    !range.is_empty()
        && range
            .chars()
            .all(|character| character.is_ascii_digit() || matches!(character, ',' | '$'))
}

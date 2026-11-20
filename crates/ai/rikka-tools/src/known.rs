use crate::words::segments;

const DEEPEST: usize = 4;

const NAMED: usize = 48;

const ASKING: &[&str] = &["--version", "-V", "--help", "-h", "help", "version"];

const SHELLS: &[&str] = &["bash", "sh", "zsh", "dash", "ksh", "fish"];

enum When {
    Always,
    Among(&'static [&'static str]),
}

struct Known {
    program: &'static str,
    when: When,
}

impl Known {
    const fn always(program: &'static str) -> Known {
        Known {
            program,
            when: When::Always,
        }
    }

    const fn among(program: &'static str, words: &'static [&'static str]) -> Known {
        Known {
            program,
            when: When::Among(words),
        }
    }

    fn holds(&self, arguments: &[String]) -> bool {
        match self.when {
            When::Always => true,
            When::Among(words) => arguments
                .iter()
                .any(|argument| words.contains(&argument.as_str())),
        }
    }
}

struct Wrapper {
    program: &'static str,
    words: &'static [&'static str],
    after: &'static [&'static str],
}

impl Wrapper {
    const fn new(program: &'static str, words: &'static [&'static str]) -> Wrapper {
        Wrapper {
            program,
            words,
            after: &[],
        }
    }

    const fn after(self, after: &'static [&'static str]) -> Wrapper {
        Wrapper {
            program: self.program,
            words: self.words,
            after,
        }
    }

    fn strip_wrapper<'a>(&self, arguments: &'a [String]) -> Option<&'a [String]> {
        let mut left = arguments;
        for word in self.words {
            left = onwards(left, Some(word));
            let (first, rest) = left.split_first()?;
            if first != word {
                return None;
            }
            left = rest;
        }

        if self.after.is_empty() {
            return Some(onwards(left, None));
        }

        let index = left
            .iter()
            .position(|word| self.after.contains(&word.as_str()))?;
        Some(&left[index + 1..])
    }
}

const CARGO: &[&str] = &[
    "build",
    "b",
    "check",
    "c",
    "test",
    "t",
    "bench",
    "run",
    "r",
    "clippy",
    "doc",
    "rustdoc",
    "nextest",
    "miri",
    "tarpaulin",
    "llvm-cov",
    "install",
    "fix",
    "expand",
    "package",
    "publish",
];

const NODE: &[&str] = &[
    "run",
    "run-script",
    "install",
    "i",
    "ci",
    "add",
    "remove",
    "update",
    "test",
    "build",
    "start",
    "rebuild",
    "pack",
    "publish",
    "lint",
    "typecheck",
];

const GO: &[&str] = &[
    "build", "test", "run", "install", "vet", "generate", "get", "mod", "work", "tool", "clean",
];

const DOTNET: &[&str] = &[
    "build", "test", "run", "publish", "restore", "pack", "clean", "msbuild", "watch",
];

const ZIG: &[&str] = &[
    "build",
    "test",
    "run",
    "build-exe",
    "build-lib",
    "build-obj",
    "cc",
    "c++",
    "translate-c",
];

const SWIFT: &[&str] = &["build", "test", "run", "package"];

const FLUTTER: &[&str] = &[
    "build", "test", "run", "pub", "analyze", "compile", "assemble",
];

const NIX: &[&str] = &["build", "flake", "run", "profile", "repl", "store", "copy"];

const CONTAINER: &[&str] = &["build", "buildx", "compose", "pull", "push"];

const PYTHON: &[&str] = &[
    "install", "download", "wheel", "build", "sync", "lock", "compile",
];

const RUBY: &[&str] = &["install", "update", "exec", "check"];

const PHP: &[&str] = &[
    "install",
    "update",
    "require",
    "remove",
    "dump-autoload",
    "run-script",
];

const HASKELL: &[&str] = &[
    "build",
    "test",
    "run",
    "install",
    "configure",
    "setup",
    "bench",
];

const ANGULAR: &[&str] = &["build", "test", "serve", "lint", "e2e"];

const KNOWN: &[Known] = &[
    Known::among("cargo", CARGO),
    Known::always("rustc"),
    Known::always("rustdoc"),
    Known::always("cc"),
    Known::always("c++"),
    Known::always("gcc"),
    Known::always("g++"),
    Known::always("clang"),
    Known::always("clang++"),
    Known::always("cl"),
    Known::always("nvcc"),
    Known::always("emcc"),
    Known::always("ld"),
    Known::always("make"),
    Known::always("gmake"),
    Known::always("bmake"),
    Known::always("ninja"),
    Known::always("samu"),
    Known::always("cmake"),
    Known::always("ctest"),
    Known::always("cpack"),
    Known::always("meson"),
    Known::always("xmake"),
    Known::always("scons"),
    Known::always("configure"),
    Known::always("bazel"),
    Known::always("bazelisk"),
    Known::always("buck2"),
    Known::always("pants"),
    Known::always("msbuild"),
    Known::always("xcodebuild"),
    Known::always("gradle"),
    Known::always("gradlew"),
    Known::always("mvn"),
    Known::always("mvnw"),
    Known::always("ant"),
    Known::always("sbt"),
    Known::always("mill"),
    Known::always("lein"),
    Known::always("javac"),
    Known::always("kotlinc"),
    Known::always("scalac"),
    Known::always("swiftc"),
    Known::among("swift", SWIFT),
    Known::among("go", GO),
    Known::among("zig", ZIG),
    Known::among("dotnet", DOTNET),
    Known::among("npm", NODE),
    Known::among("pnpm", NODE),
    Known::among("yarn", NODE),
    Known::among("bun", NODE),
    Known::among(
        "deno",
        &[
            "run", "test", "task", "compile", "bundle", "check", "lint", "cache",
        ],
    ),
    Known::always("tsc"),
    Known::always("esbuild"),
    Known::always("rollup"),
    Known::always("webpack"),
    Known::always("vite"),
    Known::always("parcel"),
    Known::always("turbo"),
    Known::always("nx"),
    Known::always("next"),
    Known::always("nuxt"),
    Known::always("expo"),
    Known::among("ng", ANGULAR),
    Known::always("jest"),
    Known::always("vitest"),
    Known::always("mocha"),
    Known::always("karma"),
    Known::always("cypress"),
    Known::always("playwright"),
    Known::always("pytest"),
    Known::always("tox"),
    Known::always("nox"),
    Known::always("mypy"),
    Known::always("pyright"),
    Known::always("pylint"),
    Known::among("pip", PYTHON),
    Known::among("pip3", PYTHON),
    Known::among("uv", PYTHON),
    Known::among("poetry", PYTHON),
    Known::among("pdm", PYTHON),
    Known::among("hatch", PYTHON),
    Known::always("setup.py"),
    Known::among("python", &["setup.py"]),
    Known::among("python3", &["setup.py"]),
    Known::always("rake"),
    Known::always("rspec"),
    Known::among("bundle", RUBY),
    Known::among("composer", PHP),
    Known::always("phpunit"),
    Known::always("pest"),
    Known::always("mix"),
    Known::always("rebar3"),
    Known::among("cabal", HASKELL),
    Known::among("stack", HASKELL),
    Known::always("ghc"),
    Known::among("flutter", FLUTTER),
    Known::among("dart", FLUTTER),
    Known::among("nix", NIX),
    Known::always("nix-build"),
    Known::always("nixos-rebuild"),
    Known::among("docker", CONTAINER),
    Known::among("podman", CONTAINER),
    Known::always("docker-compose"),
    Known::always("podman-compose"),
];

const WRAPPERS: &[Wrapper] = &[
    Wrapper::new("env", &[]),
    Wrapper::new("time", &[]),
    Wrapper::new("nice", &[]),
    Wrapper::new("ionice", &[]),
    Wrapper::new("nohup", &[]),
    Wrapper::new("setsid", &[]),
    Wrapper::new("stdbuf", &[]),
    Wrapper::new("timeout", &[]),
    Wrapper::new("xvfb-run", &[]),
    Wrapper::new("sudo", &[]),
    Wrapper::new("doas", &[]),
    Wrapper::new("command", &[]),
    Wrapper::new("exec", &[]),
    Wrapper::new("npx", &[]),
    Wrapper::new("bunx", &[]),
    Wrapper::new("uvx", &[]),
    Wrapper::new("uv", &["run"]),
    Wrapper::new("poetry", &["run"]),
    Wrapper::new("pipenv", &["run"]),
    Wrapper::new("pdm", &["run"]),
    Wrapper::new("rye", &["run"]),
    Wrapper::new("hatch", &["run"]),
    Wrapper::new("bundle", &["exec"]),
    Wrapper::new("npm", &["exec"]),
    Wrapper::new("pnpm", &["exec"]),
    Wrapper::new("pnpm", &["dlx"]),
    Wrapper::new("yarn", &["dlx"]),
    Wrapper::new("nix", &["develop"]).after(&["--command", "-c"]),
    Wrapper::new("nix", &["shell"]).after(&["--command", "-c"]),
    Wrapper::new("nix-shell", &[]).after(&["--run"]),
    Wrapper::new("python", &["-m"]),
    Wrapper::new("python3", &["-m"]),
];

#[derive(Debug)]
pub(crate) struct Reading {
    builds: bool,
    named: String,
}

impl Reading {
    pub(crate) fn builds(&self) -> bool {
        self.builds
    }

    pub(crate) fn named(&self) -> &str {
        &self.named
    }
}

pub(crate) fn read(command: &str) -> Reading {
    let segments = segments(command);
    let built = segments.iter().find(|words| is_build(words, DEEPEST));
    let rendered = built
        .or_else(|| segments.iter().find(|words| !moves(words)))
        .or_else(|| segments.first());

    Reading {
        builds: built.is_some(),
        named: match rendered.map(|words| gist(words)).unwrap_or_default() {
            gist if gist.is_empty() => "command".to_owned(),
            gist => gist,
        },
    }
}

fn built(command: &str, depth: usize) -> bool {
    depth > 0
        && segments(command)
            .iter()
            .any(|words| is_build(words, depth - 1))
}

fn is_build(words: &[String], depth: usize) -> bool {
    let Some((program, arguments)) = peeled(words) else {
        return false;
    };

    if let Some(script) = script(program, arguments) {
        return built(script, depth);
    }

    !asks_only(arguments)
        && KNOWN
            .iter()
            .filter(|known| known.program == program)
            .any(|known| known.holds(arguments))
}

fn peeled(words: &[String]) -> Option<(&str, &[String])> {
    let mut words = plainly(words);

    for _ in 0..DEEPEST {
        let (first, arguments) = words.split_first()?;
        let program = program_of(first);
        match wrapped(program, arguments) {
            Some(inner) => words = inner,
            None => return Some((program, arguments)),
        }
    }

    None
}

fn wrapped<'a>(program: &str, arguments: &'a [String]) -> Option<&'a [String]> {
    WRAPPERS
        .iter()
        .filter(|wrapper| wrapper.program == program)
        .find_map(|wrapper| wrapper.strip_wrapper(arguments))
}

fn onwards<'a>(words: &'a [String], target_word: Option<&str>) -> &'a [String] {
    let skipped = words
        .iter()
        .take_while(|word| target_word != Some(word.as_str()) && passed_over(word))
        .count();
    &words[skipped..]
}

fn plainly(words: &[String]) -> &[String] {
    let skipped = words.iter().take_while(|word| assigns(word)).count();
    &words[skipped..]
}

fn passed_over(word: &str) -> bool {
    word.starts_with('-') || assigns(word) || waits(word)
}

fn assigns(word: &str) -> bool {
    word.split_once('=')
        .is_some_and(|(name, _)| !name.is_empty() && name.chars().all(is_namely))
}

fn is_namely(letter: char) -> bool {
    letter.is_ascii_alphanumeric() || letter == '_'
}

fn waits(word: &str) -> bool {
    let digits = word.trim_end_matches(['s', 'm', 'h', 'd']);
    !digits.is_empty() && digits.chars().all(|letter| letter.is_ascii_digit())
}

fn script<'a>(program: &'a str, arguments: &'a [String]) -> Option<&'a str> {
    if program.contains(char::is_whitespace) {
        return Some(program);
    }
    if !SHELLS.contains(&program) {
        return None;
    }

    let index = arguments.iter().position(|word| word == "-c")?;
    arguments.get(index + 1).map(String::as_str)
}

fn asks_only(arguments: &[String]) -> bool {
    arguments
        .iter()
        .any(|argument| ASKING.contains(&argument.as_str()))
}

fn program_of(word: &str) -> &str {
    let name = word.rsplit('/').next().unwrap_or(word);
    match name.starts_with("python3.") {
        true => "python3",
        false => name,
    }
}

fn moves(words: &[String]) -> bool {
    words
        .first()
        .is_some_and(|program| program_of(program) == "cd")
}

fn gist(words: &[String]) -> String {
    let Some((program, arguments)) = peeled(words) else {
        return String::new();
    };

    let mut line = String::from(program);
    for argument in arguments
        .iter()
        .filter(|argument| !argument.starts_with('-') && !assigns(argument))
        .take(2)
    {
        line.push('-');
        line.push_str(argument);
    }

    tidy(&line)
}

fn tidy(text: &str) -> String {
    let mut tidied = String::new();
    for letter in text.chars().take(NAMED * 2) {
        match letter {
            letter if letter.is_ascii_alphanumeric() => {
                tidied.extend(letter.to_lowercase());
            }
            '_' => tidied.push(letter),
            _ if tidied.ends_with('-') => {}
            _ => tidied.push('-'),
        }
    }

    tidied.truncate(NAMED);
    tidied.trim_matches('-').to_owned()
}

use std::fmt::Write as _;
use std::process::ExitCode;

const USAGE: &str = "usage: rikka-syntax-queries <output> (<name> <path suffix> <files>)…";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(trouble) => {
            eprintln!("rikka-syntax-queries: {trouble}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut arguments = std::env::args().skip(1);
    let output = arguments.next().ok_or(USAGE)?;
    let queries: Vec<String> = arguments.collect();
    let (queries, []) = queries.as_chunks::<3>() else {
        return Err(USAGE.to_owned());
    };
    if queries.is_empty() {
        return Err(USAGE.to_owned());
    }
    let mut module = String::new();
    for [name, suffix, files] in queries {
        let file = files
            .split_whitespace()
            .find(|file| {
                file.strip_suffix(suffix.as_str())
                    .is_some_and(|folder| folder.ends_with('/'))
            })
            .ok_or_else(|| format!("no {suffix} for {name}"))?;
        let text = std::fs::read_to_string(file)
            .map_err(|error| format!("failed to read {file}: {error}"))?;
        if text.contains("\"#####") {
            return Err(format!("{file} ends the raw string it is embedded in"));
        }
        writeln!(
            module,
            "pub(crate) const {name}: &str = r#####\"{text}\"#####;\n"
        )
        .map_err(|error| error.to_string())?;
    }
    std::fs::write(&output, module).map_err(|error| format!("failed to write {output}: {error}"))
}

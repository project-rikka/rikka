#![feature(proc_macro_tracked_env)]

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use proc_macro::TokenStream;

const CATALOGS: &str = "locales";

const LISTED: &str = "RIKKA_LOCALES";

const MANIFEST_DIR: &str = "CARGO_MANIFEST_DIR";

#[proc_macro]
pub fn catalogs(input: TokenStream) -> TokenStream {
    if !input.is_empty() {
        return compile_error("`catalogs!` takes no arguments");
    }
    let Ok(listed) = proc_macro::tracked::env_var(LISTED) else {
        return compile_error(&format!(
            "`{LISTED}` is unset; build the crate with the `rikka_*` rules of `//bazel/rust:defs.bzl`, which list `{CATALOGS}/`"
        ));
    };
    let directory = match std::env::var(MANIFEST_DIR) {
        Ok(manifest) => PathBuf::from(manifest).join(CATALOGS),
        Err(_) => {
            return compile_error(&format!("`{MANIFEST_DIR}` is not set"));
        }
    };
    let names: Vec<String> = listed.split_whitespace().map(str::to_owned).collect();
    if let Err(reason) = verify(&directory, &names) {
        return compile_error(&reason);
    }
    let entries: String = names
        .iter()
        .map(|name| {
            format!(
                "({name:?}, ::core::include_str!(::core::concat!(::core::env!(\"CARGO_MANIFEST_DIR\"), \"/{CATALOGS}/\", {name:?}))),"
            )
        })
        .collect();
    format!("&[{entries}]")
        .parse()
        .unwrap_or_else(|_| compile_error("generated catalog list failed to parse"))
}

fn verify(directory: &Path, names: &[String]) -> Result<(), String> {
    let sources = names
        .iter()
        .map(|name| {
            let path = directory.join(name);
            fs::read_to_string(&path)
                .map(|source| (name.as_str(), source))
                .map_err(|error| format!("failed to read `{}`: {error}", path.display()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let borrowed: Vec<(&str, &str)> = sources
        .iter()
        .map(|(name, source)| (*name, source.as_str()))
        .collect();
    rikka_i18n_catalog::verify_catalogs(&borrowed).map_err(|errors| {
        errors.iter().fold(
            String::from("locale catalogs disagree:"),
            |mut report, error| {
                report.push_str("\n- ");
                report.push_str(&describe(error));
                report
            },
        )
    })
}

fn describe(error: &dyn Error) -> String {
    let mut chain = vec![error.to_string()];
    let mut cause = error.source();
    while let Some(source) = cause {
        chain.push(source.to_string());
        cause = source.source();
    }
    chain.join(": ")
}

fn compile_error(reason: &str) -> TokenStream {
    format!("::core::compile_error!({reason:?})")
        .parse()
        .expect("compile_error! invocation parses")
}

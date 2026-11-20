use std::io::ErrorKind;
use std::path::Path;
use std::sync::Arc;

use rikka_agent::{Function, Reply};
use rikka_file_edit::Files;
use rikka_llm::{Image, Media, Tool, Value};
use rikka_prompt::{Facts, Prompt};
use serde::Deserialize;

use crate::error::{Error, Result};

#[derive(Debug, Deserialize)]
struct Args {
    path: String,
}

pub(crate) fn declare(files: &Arc<Files>, prompt: &Prompt, facts: &Facts) -> Result<Function> {
    let tool = Tool::new(prompt.name())
        .description(prompt.body(facts)?)
        .string("path", prompt.param("path", facts)?)
        .build()
        .map_err(|error| Error::Undeclarable {
            name: prompt.name().to_owned(),
            reason: error.to_string(),
        })?;

    let largest = prompt.limit("max_bytes")?;

    let files = Arc::clone(files);
    Ok(Function::new(tool, move |input: Value| {
        let files = Arc::clone(&files);
        async move {
            tokio::task::spawn_blocking(move || run(&files, largest, input))
                .await
                .map_err(|error| format!("the picture was not looked at: {error}"))?
        }
    }))
}

fn run(files: &Files, largest: u64, input: Value) -> std::result::Result<Reply, String> {
    let args: Args =
        serde_json::from_value(input).map_err(|error| format!("invalid arguments: {error}"))?;

    let path = files.resolve(&args.path);
    let name = files.relative_path(&path);

    let about = std::fs::metadata(&path).map_err(|error| match error.kind() {
        ErrorKind::NotFound => format!("there is no file at {name}"),
        _ => format!("{name} could not be looked at: {error}"),
    })?;

    if about.is_dir() {
        return Err(format!("{name} is a folder rather than a picture"));
    }

    let Some(media) = picture(&path) else {
        return Err(format!(
            "{name} is not a picture; this looks at PNG, JPEG, GIF and WebP, \
             and everything else is read as text with `cat`"
        ));
    };

    if about.len() > largest {
        return Err(format!(
            "{name} is {} bytes, which is more than the {largest} a picture may be",
            about.len()
        ));
    }

    let bytes =
        std::fs::read(&path).map_err(|error| format!("{name} could not be read: {error}"))?;

    Ok(Reply::new(format!("{name}, {} bytes", bytes.len())).display(Image::inline(media, bytes)))
}

fn picture(path: &Path) -> Option<Media> {
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    match extension.as_str() {
        "png" => Some(Media::Png),
        "jpg" | "jpeg" => Some(Media::Jpeg),
        "gif" => Some(Media::Gif),
        "webp" => Some(Media::WebP),
        _ => None,
    }
}

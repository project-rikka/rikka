use std::fmt;
use std::sync::Arc;

use rikka_prompt::{Facts, Prompt};
use rikka_tool::{Function, Reply};
use rikka_tool::{Tool, Value};
use serde::Deserialize;

use crate::catalog::Skills;
use crate::error::{Error, Result};
use crate::skill::Skill;

#[derive(Debug, Deserialize)]
struct Args {
    skill: String,
}

pub fn declare(skills: &Skills, prompt: &Prompt, facts: &Facts) -> Result<Function> {
    let tool = Tool::new(prompt.name())
        .description(prompt.body(facts)?)
        .string("skill", prompt.param("skill", facts)?)
        .build()
        .map_err(|source| Error::Declare {
            name: prompt.name().to_owned(),
            source,
        })?;

    let skills = Arc::new(skills.clone());
    Ok(Function::new(tool, move |input: Value| {
        let skills = Arc::clone(&skills);
        async move { run(&skills, input) }
    })
    .survives_compaction())
}

fn run(skills: &Skills, input: Value) -> std::result::Result<Reply, String> {
    let args: Args =
        serde_json::from_value(input).map_err(|error| format!("invalid arguments: {error}"))?;

    let name = args.skill.trim();
    let Some(skill) = skills.get(name) else {
        return Err(format!(
            "no skill named \"{name}\"; available skills: {}",
            skills.names().collect::<Vec<&str>>().join(", ")
        ));
    };

    let mut content = String::new();
    write_content(&mut content, skill).expect("writing to a String cannot fail");
    Ok(Reply::new(content))
}

fn write_content(out: &mut impl fmt::Write, skill: &Skill) -> fmt::Result {
    writeln!(out, "<skill_content name=\"{}\">", skill.name())?;

    if let Some(compatibility) = skill.compatibility() {
        writeln!(out, "This skill was written for: {compatibility}\n")?;
    }

    out.write_str(skill.body())?;

    write!(
        out,
        "\n\nSkill directory: {}\nEvery relative path in this skill is relative to it.",
        skill.folder().display()
    )?;

    let resources = skill.resources();
    if !resources.is_empty() {
        out.write_str("\n\n<skill_resources>")?;
        for file in resources.files() {
            write!(out, "\n  <file>{file}</file>")?;
        }
        if resources.is_partial() {
            out.write_str("\n  <more>the folder holds more than is listed here</more>")?;
        }
        out.write_str("\n</skill_resources>")?;
    }

    out.write_str("\n</skill_content>")
}

use std::fmt::{self, Write as _};

use crate::memory::Memory;

const DERIVED: &str = "<!-- Written by the Rikka ADE from the notes beside it. Anything typed here is overwritten. -->";

const NONE: &str = "_Nothing has been noted about this project yet._";

pub(crate) fn render(memories: &[Memory]) -> String {
    if memories.is_empty() {
        return NONE.to_owned();
    }

    let mut index = String::new();
    write_index(&mut index, memories).expect("writing to a String cannot fail");
    index.trim_end().to_owned()
}

fn write_index(out: &mut String, memories: &[Memory]) -> fmt::Result {
    let mut heading: Option<&str> = None;
    for memory in memories {
        if memory.about() != heading {
            heading = memory.about();
            if let Some(about) = heading {
                if !out.is_empty() {
                    out.push('\n');
                }
                writeln!(out, "## {about}\n")?;
            }
        }
        writeln!(
            out,
            "- [{}]({}) — {}",
            titled(memory.title()),
            target(memory.name()),
            memory.description()
        )?;
    }
    Ok(())
}

pub(crate) fn file(memories: &[Memory]) -> String {
    format!("{DERIVED}\n\n# Project memory\n\n{}\n", render(memories))
}

fn titled(title: &str) -> String {
    title.replace('[', "\\[").replace(']', "\\]")
}

fn target(name: &str) -> String {
    let file = format!("{name}.md");
    match file.contains([' ', '(', ')', '<', '>']) {
        false => file,
        true => format!("<{}>", file.replace(['<', '>'], "")),
    }
}

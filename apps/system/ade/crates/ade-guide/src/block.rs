use std::path::Path;

pub(crate) fn block(path: &Path, text: &str) -> String {
    format!(
        "<project_instructions path=\"{}\">\n{}\n</project_instructions>",
        path.display(),
        text.trim_end()
    )
}

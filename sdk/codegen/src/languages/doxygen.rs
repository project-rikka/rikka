use crate::languages::wrap;

const WIDTH: usize = 96;

pub(crate) fn block<S: AsRef<str>>(indent: &str, paragraphs: &[S]) -> String {
    let paragraphs: Vec<&str> = paragraphs
        .iter()
        .map(AsRef::as_ref)
        .filter(|text| !text.is_empty())
        .collect();
    if paragraphs.is_empty() {
        return String::new();
    }
    let mut out = format!("{indent}/**\n");
    for (index, paragraph) in paragraphs.iter().enumerate() {
        let tagged_run =
            index > 0 && paragraph.starts_with('@') && paragraphs[index - 1].starts_with('@');
        if index > 0 && !tagged_run {
            out.push_str(&format!("{indent} *\n"));
        }
        for line in wrap(paragraph, WIDTH - indent.len() - 3) {
            out.push_str(format!("{indent} * {line}").trim_end());
            out.push('\n');
        }
    }
    out.push_str(&format!("{indent} */\n"));
    out
}
